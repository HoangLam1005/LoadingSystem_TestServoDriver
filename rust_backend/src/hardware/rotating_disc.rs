// =============================================================================
// LoadingSystem - Rotating Disc Controller (Core: Level 2-5)
// =============================================================================
// Bộ điều khiển chính đĩa xoay, thực hiện toàn bộ logic quy đổi góc-xung,
// phát lệnh Modbus tới PLC và tích hợp cơ chế tự học Level 5.
//
// Mô hình toán học:
//   Encoder 17-bit: 131,072 xung/vòng (360°)
//   P = round(θ × 131,072 / 360)
//   F = round(ω × 131,072 / 360)
//
// Level 2: rotate_degrees(), return_to_origin()
// Level 4: inject_manual_override()
// Level 5: Tự động cập nhật offset_pulses qua learning algorithm
// =============================================================================

use crate::calibration::acceleration_profile::AccelerationProfile;
use crate::calibration::backlash_compensation::{BacklashCompensator, TrackedDirection};
use crate::calibration::learning;
use crate::calibration::profile::CalibrationProfile;
use crate::error::LoadingError;
use crate::hardware::modbus_client::{ModbusInterface, COIL_TRIGGER_M0, REG_PULSE_POSITION};

/// Giới hạn biên tối đa cho offset bù sai số (xung).
/// 200 xung ≈ 0.55° — đủ bù sai số cơ khí mà không gây quay lố.
const MAX_OFFSET_PULSES: i32 = 200;

/// Hướng quay đĩa xoay
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    /// Quay sang trái (xung âm)
    Left,
    /// Quay sang phải (xung dương)
    Right,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Left => write!(f, "Left"),
            Direction::Right => write!(f, "Right"),
        }
    }
}

/// Bộ điều khiển đĩa xoay - generic over Modbus interface.
/// Sử dụng generic `M: ModbusInterface` để cho phép swap giữa
/// RealModbusClient (production) và MockModbusClient (testing).
pub struct RotatingDiscController<M: ModbusInterface> {
    /// Client Modbus (thật hoặc mock)
    modbus: M,
    /// Độ phân giải encoder (131,072 xung/vòng cho 17-bit)
    encoder_resolution: u32,
    /// Tần số phát xung mặc định (Hz)
    default_frequency: u32,
    /// Profile hiệu chuẩn (Level 5)
    calibration: CalibrationProfile,
    /// Bộ bù độ rơ cơ khí khi đảo chiều quay
    backlash: BacklashCompensator,
    /// Góc quay tích lũy hiện tại (tracking phần mềm)
    current_angle: f64,
}

impl<M: ModbusInterface> RotatingDiscController<M> {
    /// Tạo controller mới
    ///
    /// # Arguments
    /// * `modbus` - Client Modbus (thật hoặc mock)
    /// * `encoder_resolution` - Độ phân giải encoder (131072)
    /// * `default_frequency` - Tần số xung mặc định (Hz)
    /// * `calibration` - Profile hiệu chuẩn đã load từ đĩa
    pub fn new(
        modbus: M,
        encoder_resolution: u32,
        default_frequency: u32,
        calibration: CalibrationProfile,
    ) -> Self {
        RotatingDiscController {
            modbus,
            encoder_resolution,
            default_frequency,
            calibration,
            backlash: BacklashCompensator::new(),
            current_angle: 0.0,
        }
    }

    // =========================================================================
    // Level 2: Hàm nguyên thủy (Primitives)
    // =========================================================================

    /// Quy đổi góc (độ) sang số xung.
    /// Công thức: P = round(θ × encoder_resolution / 360)
    ///
    /// # Examples
    /// ```text
    /// 90°  → 32,768 xung
    /// 45°  → 16,384 xung
    /// 180° → 65,536 xung
    /// 360° → 131,072 xung
    /// ```
    pub fn degrees_to_pulses(&self, degrees: f64) -> i32 {
        ((degrees * self.encoder_resolution as f64) / 360.0).round() as i32
    }

    /// Quy đổi số xung ngược lại thành góc (độ)
    pub fn pulses_to_degrees(&self, pulses: i32) -> f64 {
        (pulses as f64 * 360.0) / self.encoder_resolution as f64
    }

    /// Tính toán tần số băm xung động (Dynamic Frequency Auto-Tune) dựa trên góc quay và thời gian mục tiêu.
    /// Ép tần số trong dải an toàn [min_freq, max_freq] (mặc định 1000 Hz - 9000 Hz) để triệt tiêu lực giật quán tính.
    pub fn calculate_dynamic_frequency(
        &self,
        degrees: f64,
        target_time_secs: Option<f64>,
        min_freq: Option<u32>,
        max_freq: Option<u32>,
    ) -> u32 {
        let pulses = self.degrees_to_pulses(degrees.abs()) as f64;
        let t_target = target_time_secs.unwrap_or(1.0).max(0.2);
        let f_calc = (pulses / t_target).round() as u32;

        let f_min = min_freq.unwrap_or(1000);
        let f_max = max_freq.unwrap_or(9000);

        f_calc.clamp(f_min, f_max)
    }

    /// Kiểm tra xem đĩa xoay có đang phát xung (đang di chuyển) hay không.
    /// LƯU Ý: PLC RST M0 ngay trong scan cycle, nên hàm này chỉ đáng tin cậy
    /// nếu gọi rất nhanh sau khi SET M0. Ưu tiên dùng wait_for_completion() thay thế.
    pub async fn is_moving(&mut self) -> Result<bool, LoadingError> {
        let coils = self.modbus.read_coils(COIL_TRIGGER_M0, 1).await?;
        Ok(coils.first().copied().unwrap_or(false))
    }

    /// Quay đĩa xoay một góc θ theo hướng chỉ định kèm tần số phát xung tùy chỉnh.
    pub async fn rotate_degrees_with_freq(
        &mut self,
        degrees: f64,
        direction: Direction,
        custom_freq: Option<u32>,
    ) -> Result<(), LoadingError> {
        if degrees < 0.0 {
            return Err(LoadingError::Command(
                "Degrees must be positive. Use Direction to specify rotation direction.".into(),
            ));
        }

        let raw_pulses = self.degrees_to_pulses(degrees);

        // Chuyển đổi Direction → TrackedDirection để BacklashCompensator sử dụng
        let tracked_dir = match direction {
            Direction::Right => TrackedDirection::Right,
            Direction::Left => TrackedDirection::Left,
        };

        // Tính xung bù backlash: > 0 nếu đảo chiều, = 0 nếu cùng chiều
        let backlash_comp = self.backlash.compute_compensation(tracked_dir) as i32;

        // Áp dụng offset bù từ calibration (Level 5)
        // Offset dương = bù thêm xung theo hướng dương (phải)
        // Quay phải: xung dương + offset + backlash
        // Quay trái: xung âm + offset - backlash
        let final_pulses = match direction {
            Direction::Right => raw_pulses + self.calibration.offset_pulses + backlash_comp,
            Direction::Left => -(raw_pulses) + self.calibration.offset_pulses - backlash_comp,
        };

        let freq = custom_freq.unwrap_or_else(|| self.calculate_dynamic_frequency(degrees, Some(1.0), Some(1000), Some(9000)));

        tracing::info!(
            "Rotate {:.2}° {} @ {}Hz → raw={}p, offset={}p, backlash={}p, final={}p",
            degrees,
            direction,
            freq,
            raw_pulses,
            self.calibration.offset_pulses,
            backlash_comp,
            final_pulses
        );

        self.execute_pulse_command(final_pulses, freq)
            .await?;

        // Chờ motor hoàn thành trước khi cho phép lệnh tiếp theo
        wait_for_motor_completion(final_pulses, freq).await;

        // Ghi nhận hướng quay sau khi thực thi thành công
        self.backlash.record_direction(tracked_dir);

        // Cập nhật góc tracking nội bộ
        match direction {
            Direction::Right => self.current_angle += degrees,
            Direction::Left => self.current_angle -= degrees,
        }

        Ok(())
    }

    /// Quay đĩa xoay một góc θ theo hướng chỉ định (tự động tính tần số động).
    pub async fn rotate_degrees(
        &mut self,
        degrees: f64,
        direction: Direction,
    ) -> Result<(), LoadingError> {
        self.rotate_degrees_with_freq(degrees, direction, None).await
    }

    /// Quay đĩa xoay một góc θ với gia tốc S-curve mượt (triệt tiêu rung lắc).
    ///
    /// Thay vì gửi 1 lệnh duy nhất với tần số cố định, hàm này chia quãng đường
    /// thành N phân đoạn với 3 pha: Tăng tốc → Cruise → Giảm tốc.
    /// Motor tăng dần tần số theo đường cong Smoothstep S(t) = 3t² - 2t³,
    /// loại bỏ hoàn toàn hiện tượng giật cơ khí và rung lắc mâm đĩa.
    ///
    /// Nếu tổng xung quá nhỏ (< 500 xung ≈ 1.37°), tự động fallback về
    /// single-shot command (không cần S-curve cho góc cực nhỏ).
    ///
    /// # Arguments
    /// * `degrees` - Góc quay mục tiêu (dương, đơn vị: độ)
    /// * `direction` - Hướng quay (Left/Right)
    /// * `accel_profile` - Profile gia tốc tùy chỉnh (None = dùng mặc định)
    ///
    /// # Examples
    /// ```text
    /// // Quay 90° phải với profile mặc định (10 segments, 20%/60%/20%)
    /// controller.rotate_degrees_smooth(90.0, Direction::Right, None).await?;
    ///
    /// // Quay 30° trái với profile tùy chỉnh (15 segments, 30%/40%/30%)
    /// let profile = AccelerationProfile::with_params(200, 15, 0.30, 0.30);
    /// controller.rotate_degrees_smooth(30.0, Direction::Left, Some(profile)).await?;
    /// ```
    pub async fn rotate_degrees_smooth(
        &mut self,
        degrees: f64,
        direction: Direction,
        accel_profile: Option<AccelerationProfile>,
    ) -> Result<(), LoadingError> {
        if degrees < 0.0 {
            return Err(LoadingError::Command(
                "Degrees must be positive. Use Direction to specify rotation direction.".into(),
            ));
        }

        let raw_pulses = self.degrees_to_pulses(degrees);

        // Chuyển đổi Direction → TrackedDirection để BacklashCompensator sử dụng
        let tracked_dir = match direction {
            Direction::Right => TrackedDirection::Right,
            Direction::Left => TrackedDirection::Left,
        };

        // Tính xung bù backlash: > 0 nếu đảo chiều, = 0 nếu cùng chiều
        let backlash_comp = self.backlash.compute_compensation(tracked_dir) as i32;

        // Áp dụng offset bù từ calibration (Level 5) + backlash compensation
        let final_pulses = match direction {
            Direction::Right => raw_pulses + self.calibration.offset_pulses + backlash_comp,
            Direction::Left => -(raw_pulses) + self.calibration.offset_pulses - backlash_comp,
        };

        let max_freq = self.calculate_dynamic_frequency(degrees, Some(1.0), Some(1000), Some(9000));
        let profile = accel_profile.unwrap_or_default();

        // Sinh chuỗi phân đoạn S-curve hoặc fallback single-shot
        let segments = if AccelerationProfile::should_use_profile(final_pulses) {
            match profile.generate_segments(final_pulses, max_freq) {
                Some(segs) => segs,
                None => AccelerationProfile::single_shot(final_pulses, max_freq),
            }
        } else {
            AccelerationProfile::single_shot(final_pulses, max_freq)
        };

        let total_segments = segments.len();

        tracing::info!(
            "Smooth rotate {:.2}° {} → {} segments, total_pulses={}, backlash={}p, f_max={}Hz",
            degrees,
            direction,
            total_segments,
            final_pulses,
            backlash_comp,
            max_freq
        );

        // Thực thi từng segment tuần tự
        for (idx, segment) in segments.iter().enumerate() {
            tracing::debug!(
                "  Segment [{}/{}] phase={}, pulses={}, freq={}Hz, duration={}ms",
                idx + 1,
                total_segments,
                segment.phase,
                segment.pulses,
                segment.frequency,
                segment.duration_ms
            );

            self.execute_pulse_command(segment.pulses, segment.frequency)
                .await?;

            // Chờ segment hoàn thành trước khi gửi segment tiếp theo
            wait_for_motor_completion(segment.pulses, segment.frequency).await;
        }

        // Ghi nhận hướng quay sau khi thực thi thành công
        self.backlash.record_direction(tracked_dir);

        // Cập nhật góc tracking nội bộ
        match direction {
            Direction::Right => self.current_angle += degrees,
            Direction::Left => self.current_angle -= degrees,
        }

        Ok(())
    }

    /// Đưa đĩa xoay về vị trí gốc (0°).
    /// Tính toán delta xung dựa trên góc tích lũy hiện tại, kèm offset bù.
    pub async fn return_to_origin(&mut self) -> Result<(), LoadingError> {
        // Tính xung cần quay ngược, có tính offset tích lũy
        let raw_return = -self.degrees_to_pulses(self.current_angle);
        // Trừ offset để bù lại sai số đã tích lũy qua các lần quay trước
        let return_pulses = raw_return - self.calibration.offset_pulses;

        let freq = self.default_frequency;

        tracing::info!(
            "Return to origin from {:.2}° → raw={}p, offset={}p, final={}p",
            self.current_angle,
            raw_return,
            self.calibration.offset_pulses,
            return_pulses
        );

        self.execute_pulse_command(return_pulses, freq)
            .await?;

        // Chờ motor hoàn thành
        wait_for_motor_completion(return_pulses, freq).await;

        // Reset backlash tracking vì đĩa đã về gốc (hướng tiếp theo là lần đầu)
        self.backlash.reset_direction();
        self.current_angle = 0.0;

        Ok(())
    }

    // =========================================================================
    // Level 4 & 5: Can thiệp thủ công và tự học
    // =========================================================================

    /// Can thiệp thủ công (Level 4) với tự học (Level 5).
    /// Khi chuyên gia nhấn Fine Tune trên HMI, hàm này:
    /// 1. Quy đổi góc điều chỉnh sang xung
    /// 2. Cập nhật offset thông qua thuật toán EMA
    /// 3. Ghi lại lịch sử hiệu chuẩn
    ///
    /// # Arguments
    /// * `adjustment_degrees` - Góc hiệu chỉnh (dương = phải, âm = trái)
    pub fn inject_manual_override(&mut self, adjustment_degrees: f64) {
        let delta_pulses = self.degrees_to_pulses(adjustment_degrees);

        let old_offset = self.calibration.offset_pulses;
        self.calibration.offset_pulses = learning::compute_offset_update(
            self.calibration.offset_pulses,
            delta_pulses,
            self.calibration.learning_coefficient,
        );

        // Saturation limit: giới hạn offset trong phạm vi an toàn
        self.calibration.offset_pulses = self
            .calibration
            .offset_pulses
            .clamp(-MAX_OFFSET_PULSES, MAX_OFFSET_PULSES);

        self.calibration
            .add_history_entry(adjustment_degrees, delta_pulses);

        tracing::info!(
            "Manual override: adj={:.2}°, delta={}p, offset: {} → {} (limit=±{}p)",
            adjustment_degrees,
            delta_pulses,
            old_offset,
            self.calibration.offset_pulses,
            MAX_OFFSET_PULSES
        );
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Lấy góc quay tích lũy hiện tại
    pub fn get_current_angle(&self) -> f64 {
        self.current_angle
    }

    /// Lấy reference tới calibration profile
    pub fn get_calibration(&self) -> &CalibrationProfile {
        &self.calibration
    }

    /// Lấy mutable reference tới calibration profile
    pub fn get_calibration_mut(&mut self) -> &mut CalibrationProfile {
        &mut self.calibration
    }

    /// Lấy reference tới bộ bù backlash
    pub fn get_backlash(&self) -> &BacklashCompensator {
        &self.backlash
    }

    /// Lấy mutable reference tới bộ bù backlash
    pub fn get_backlash_mut(&mut self) -> &mut BacklashCompensator {
        &mut self.backlash
    }

    /// Lấy reference tới Modbus client (hữu ích cho testing)
    pub fn get_modbus(&self) -> &M {
        &self.modbus
    }

    /// Lấy encoder resolution
    pub fn get_encoder_resolution(&self) -> u32 {
        self.encoder_resolution
    }

    // =========================================================================
    // Internal: Ghi lệnh xung xuống PLC qua Modbus
    // =========================================================================

    /// Ghi lệnh phát xung xuống PLC qua Modbus RTU.
    /// Quy trình:
    /// 1. Phân tách giá trị 32-bit thành hai thanh ghi 16-bit
    /// 2. Ghi đồng thời 4 thanh ghi: D100, D101 (vị trí), D102, D103 (tần số)
    /// 3. SET coil M0 để PLC thực thi lệnh DDRVI
    async fn execute_pulse_command(
        &mut self,
        pulses: i32,
        frequency: u32,
    ) -> Result<(), LoadingError> {
        // Phân tách 32-bit signed thành cặp 16-bit (little-endian cho PLC Delta)
        let p_low = (pulses & 0xFFFF) as u16;
        let p_high = ((pulses >> 16) & 0xFFFF) as u16;
        let f_low = (frequency & 0xFFFF) as u16;
        let f_high = ((frequency >> 16) & 0xFFFF) as u16;

        // Ghi cụm 4 thanh ghi liên tiếp: D100, D101, D102, D103
        // Địa chỉ Modbus gốc: 0x1064
        self.modbus
            .write_multiple_registers(REG_PULSE_POSITION, &[p_low, p_high, f_low, f_high])
            .await?;

        // Kích hoạt coil M0 (0x0800) → PLC thực thi DDRVI
        self.modbus
            .write_single_coil(COIL_TRIGGER_M0, true)
            .await?;

        tracing::debug!(
            "Pulse command sent: pulses={} [0x{:04X}, 0x{:04X}], freq={} [0x{:04X}, 0x{:04X}]",
            pulses, p_low, p_high, frequency, f_low, f_high
        );

        Ok(())
    }
}

/// Chờ motor hoàn thành lệnh phát xung dựa trên thời gian tính toán.
/// PLC RST M0 ngay trong scan cycle nên không thể dùng M0 để kiểm tra.
/// Thay vào đó, tính thời gian quay = |pulses| / frequency + margin.
///
/// Hàm này là standalone (không phải method) để tránh vấn đề Send/Sync
/// khi sử dụng trong tokio::spawn context.
///
/// # Arguments
/// * `pulses` - Số xung đã phát (dùng để tính thời gian)
/// * `frequency` - Tần số phát xung (Hz)
async fn wait_for_motor_completion(pulses: i32, frequency: u32) {
    if frequency == 0 {
        return;
    }
    // Thời gian quay (ms) = |pulses| / frequency * 1000 + margin 150ms
    let duration_ms = ((pulses.unsigned_abs() as f64 / frequency as f64) * 1000.0) as u64 + 150;
    tracing::debug!("Waiting {}ms for motor completion", duration_ms);
    tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;
}
