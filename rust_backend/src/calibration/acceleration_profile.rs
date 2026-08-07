// =============================================================================
// LoadingSystem - S-Curve Acceleration Profile Generator
// =============================================================================
// Module sinh đường cong gia tốc S-curve (Smoothstep) cho đĩa xoay,
// triệt tiêu rung lắc cơ khí khi khởi động và dừng motor.
//
// Vấn đề giải quyết:
//   Khi PLC nhận lệnh DDRVI, motor khởi động đột ngột từ 0 Hz → f_max
//   trong 1 scan cycle (~5ms), tạo bước nhảy gia tốc gây rung lắc mâm đĩa,
//   giật cơ khí ở khớp nối, và tiếng ồn cộng hưởng tần số thấp.
//
// Giải pháp:
//   Chia quãng đường quay thành N phân đoạn (segments) với 3 pha:
//     Phase 1 — Tăng tốc (Acceleration): f tăng dần từ f_min → f_max
//     Phase 2 — Tốc độ ổn định (Cruise): f = f_max = const
//     Phase 3 — Giảm tốc (Deceleration): f giảm dần từ f_max → f_min
//
// Mô hình toán học:
//   Sử dụng hàm Smoothstep (Hermite interpolation) cho đường cong mượt:
//     S(t) = 3t² - 2t³       (t ∈ [0, 1])
//
//   Tần số tại segment thứ i trong pha tăng tốc:
//     f_i = f_min + (f_max - f_min) × S(i / N_accel)
//
//   Tần số tại segment thứ j trong pha giảm tốc:
//     f_j = f_max - (f_max - f_min) × S(j / N_decel)
//
//   Mỗi segment được phân bổ xung tỷ lệ theo tần số:
//     P_segment_i = round(P_total × f_i / Σf)
// =============================================================================

/// Tần số khởi động tối thiểu mặc định (Hz).
/// Motor Servo CSD7 cần tối thiểu ~200 Hz để bắt đầu quay êm ái.
const DEFAULT_MIN_FREQUENCY: u32 = 200;

/// Số phân đoạn mặc định cho toàn bộ profile.
/// 10 segments cân bằng giữa độ mượt và số lần ghi Modbus.
const DEFAULT_TOTAL_SEGMENTS: u32 = 10;

/// Tỷ lệ phân bổ quãng đường cho pha tăng tốc (20%).
const DEFAULT_ACCEL_RATIO: f64 = 0.20;

/// Tỷ lệ phân bổ quãng đường cho pha giảm tốc (20%).
const DEFAULT_DECEL_RATIO: f64 = 0.20;

/// Ngưỡng xung tối thiểu để sử dụng profile S-curve.
/// Dưới ngưỡng này, hệ thống fallback về single-shot command.
const MIN_PULSES_FOR_PROFILE: u32 = 500;

// ---------------------------------------------------------------------------
// Smoothstep Function
// ---------------------------------------------------------------------------

/// Hàm Smoothstep (Hermite cubic interpolation).
///
/// Công thức: S(t) = 3t² - 2t³
///
/// Đặc tính:
///   - S(0) = 0, S(1) = 1
///   - S'(0) = 0, S'(1) = 0 (đạo hàm bằng 0 tại 2 đầu → không có bước nhảy)
///   - Đường cong chữ S mượt, gia tốc liên tục
///
/// # Arguments
/// * `t` - Tham số nội suy, giá trị trong [0, 1]. Clamp tự động nếu ngoài khoảng.
///
/// # Returns
/// Giá trị nội suy trong [0, 1]
pub fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// Motion Segment
// ---------------------------------------------------------------------------

/// Một phân đoạn chuyển động trong chuỗi S-curve.
/// Mỗi segment tương ứng với 1 lệnh ghi Modbus (D100-D103 + SET M0).
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSegment {
    /// Số xung phát trong segment này (có dấu: dương = phải, âm = trái)
    pub pulses: i32,
    /// Tần số phát xung (Hz)
    pub frequency: u32,
    /// Thời gian thực thi ước tính (ms)
    pub duration_ms: u64,
    /// Pha chuyển động: "accel", "cruise", hoặc "decel"
    pub phase: &'static str,
}

// ---------------------------------------------------------------------------
// Acceleration Profile
// ---------------------------------------------------------------------------

/// Bộ sinh đường cong gia tốc S-curve cho đĩa xoay.
///
/// Cấu hình các tham số gia tốc và sinh chuỗi phân đoạn chuyển động
/// (MotionSegment) sẵn sàng ghi xuống PLC qua Modbus.
#[derive(Debug, Clone)]
pub struct AccelerationProfile {
    /// Tần số khởi động tối thiểu (Hz) — tốc độ bắt đầu/kết thúc
    min_frequency: u32,
    /// Tổng số phân đoạn cho toàn bộ profile
    total_segments: u32,
    /// Tỷ lệ quãng đường dành cho pha tăng tốc (0.0 - 0.5)
    accel_ratio: f64,
    /// Tỷ lệ quãng đường dành cho pha giảm tốc (0.0 - 0.5)
    decel_ratio: f64,
}

impl AccelerationProfile {
    /// Tạo profile gia tốc mặc định.
    ///
    /// Tham số mặc định:
    ///   - f_min = 200 Hz
    ///   - 10 segments
    ///   - Tỷ lệ Accel:Cruise:Decel = 20%:60%:20%
    pub fn new() -> Self {
        AccelerationProfile {
            min_frequency: DEFAULT_MIN_FREQUENCY,
            total_segments: DEFAULT_TOTAL_SEGMENTS,
            accel_ratio: DEFAULT_ACCEL_RATIO,
            decel_ratio: DEFAULT_DECEL_RATIO,
        }
    }

    /// Tạo profile gia tốc với tham số tùy chỉnh.
    ///
    /// # Arguments
    /// * `min_frequency` - Tần số khởi động/kết thúc tối thiểu (Hz)
    /// * `total_segments` - Tổng số phân đoạn (≥ 3)
    /// * `accel_ratio` - Tỷ lệ pha tăng tốc (0.0 - 0.5)
    /// * `decel_ratio` - Tỷ lệ pha giảm tốc (0.0 - 0.5)
    pub fn with_params(
        min_frequency: u32,
        total_segments: u32,
        accel_ratio: f64,
        decel_ratio: f64,
    ) -> Self {
        AccelerationProfile {
            min_frequency: min_frequency.max(50),
            total_segments: total_segments.max(3),
            accel_ratio: accel_ratio.clamp(0.05, 0.50),
            decel_ratio: decel_ratio.clamp(0.05, 0.50),
        }
    }

    /// Sinh chuỗi phân đoạn chuyển động S-curve.
    ///
    /// Thuật toán:
    /// 1. Phân bổ số segment cho mỗi pha (accel, cruise, decel)
    /// 2. Tính tần số từng segment dùng hàm Smoothstep
    /// 3. Phân bổ xung tỷ lệ theo tần số (bảo toàn tổng xung)
    /// 4. Tính thời gian thực thi từng segment
    ///
    /// # Arguments
    /// * `total_pulses` - Tổng số xung cần phát (có dấu: dương = phải, âm = trái)
    /// * `max_frequency` - Tần số đỉnh (Hz), thường từ calculate_dynamic_frequency()
    ///
    /// # Returns
    /// * `Some(Vec<MotionSegment>)` - Chuỗi phân đoạn nếu tổng xung đủ lớn
    /// * `None` - Nếu tổng xung < ngưỡng MIN_PULSES_FOR_PROFILE, nên dùng single-shot
    pub fn generate_segments(
        &self,
        total_pulses: i32,
        max_frequency: u32,
    ) -> Option<Vec<MotionSegment>> {
        let abs_pulses = total_pulses.unsigned_abs();

        // Xung quá nhỏ → fallback về single-shot (không cần profile)
        if abs_pulses < MIN_PULSES_FOR_PROFILE {
            return None;
        }

        let sign = if total_pulses >= 0 { 1i32 } else { -1i32 };
        let f_min = self.min_frequency as f64;
        let f_max = (max_frequency.max(self.min_frequency + 100)) as f64;

        // --- Phân bổ số segment cho từng pha ---
        let n_accel = ((self.total_segments as f64 * self.accel_ratio).round() as u32).max(1);
        let n_decel = ((self.total_segments as f64 * self.decel_ratio).round() as u32).max(1);
        let n_cruise = self.total_segments.saturating_sub(n_accel + n_decel).max(1);
        let n_total = n_accel + n_cruise + n_decel;

        // --- Tính tần số cho từng segment ---
        let mut frequencies: Vec<f64> = Vec::with_capacity(n_total as usize);

        // Phase 1: Tăng tốc — Smoothstep từ f_min → f_max
        for i in 0..n_accel {
            let t = (i as f64 + 1.0) / n_accel as f64;
            let f = f_min + (f_max - f_min) * smoothstep(t);
            frequencies.push(f);
        }

        // Phase 2: Cruise — f_max ổn định
        for _ in 0..n_cruise {
            frequencies.push(f_max);
        }

        // Phase 3: Giảm tốc — Smoothstep từ f_max → f_min
        for i in 0..n_decel {
            let t = (i as f64 + 1.0) / n_decel as f64;
            let f = f_max - (f_max - f_min) * smoothstep(t);
            frequencies.push(f);
        }

        // --- Phân bổ xung tỷ lệ theo tần số (bảo toàn tổng xung) ---
        let freq_sum: f64 = frequencies.iter().sum();

        let mut segments: Vec<MotionSegment> = Vec::with_capacity(n_total as usize);
        let mut allocated_pulses: u32 = 0;

        for (idx, &freq) in frequencies.iter().enumerate() {
            let is_last = idx == frequencies.len() - 1;

            // Xung segment = tỷ lệ tần số × tổng xung
            let seg_pulses = if is_last {
                // Segment cuối nhận hết xung còn lại để bảo toàn tổng
                abs_pulses - allocated_pulses
            } else {
                let p = ((freq / freq_sum) * abs_pulses as f64).round() as u32;
                // Đảm bảo mỗi segment có ít nhất 1 xung
                p.max(1).min(abs_pulses - allocated_pulses)
            };

            allocated_pulses += seg_pulses;

            let freq_u32 = (freq.round() as u32).max(self.min_frequency);

            // Thời gian = |pulses| / frequency × 1000 + margin 20ms/segment
            let duration_ms = if freq_u32 > 0 {
                ((seg_pulses as f64 / freq_u32 as f64) * 1000.0) as u64 + 20
            } else {
                50
            };

            // Xác định pha
            let phase = if idx < n_accel as usize {
                "accel"
            } else if idx < (n_accel + n_cruise) as usize {
                "cruise"
            } else {
                "decel"
            };

            segments.push(MotionSegment {
                pulses: sign * (seg_pulses as i32),
                frequency: freq_u32,
                duration_ms,
                phase,
            });
        }

        Some(segments)
    }

    /// Tính tổng thời gian thực thi của chuỗi phân đoạn (ms).
    ///
    /// # Arguments
    /// * `segments` - Chuỗi phân đoạn từ `generate_segments()`
    ///
    /// # Returns
    /// Tổng thời gian (ms) bao gồm cả margin giữa các segment
    pub fn total_duration_ms(segments: &[MotionSegment]) -> u64 {
        segments.iter().map(|s| s.duration_ms).sum()
    }

    /// Kiểm tra xem tổng xung có đủ lớn để sử dụng profile S-curve hay không.
    ///
    /// # Arguments
    /// * `total_pulses` - Tổng số xung (có dấu)
    ///
    /// # Returns
    /// `true` nếu nên dùng S-curve, `false` nếu nên dùng single-shot
    pub fn should_use_profile(total_pulses: i32) -> bool {
        total_pulses.unsigned_abs() >= MIN_PULSES_FOR_PROFILE
    }

    /// Tạo single-shot segment (fallback khi tổng xung quá nhỏ).
    ///
    /// # Arguments
    /// * `pulses` - Tổng số xung (có dấu)
    /// * `frequency` - Tần số phát xung (Hz)
    ///
    /// # Returns
    /// Vec chứa 1 MotionSegment duy nhất
    pub fn single_shot(pulses: i32, frequency: u32) -> Vec<MotionSegment> {
        let duration_ms = if frequency > 0 {
            ((pulses.unsigned_abs() as f64 / frequency as f64) * 1000.0) as u64 + 150
        } else {
            200
        };

        vec![MotionSegment {
            pulses,
            frequency,
            duration_ms,
            phase: "single",
        }]
    }

    // --- Accessors ---

    pub fn get_min_frequency(&self) -> u32 {
        self.min_frequency
    }

    pub fn get_total_segments(&self) -> u32 {
        self.total_segments
    }

    pub fn get_accel_ratio(&self) -> f64 {
        self.accel_ratio
    }

    pub fn get_decel_ratio(&self) -> f64 {
        self.decel_ratio
    }
}

impl Default for AccelerationProfile {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Nhóm D: Kiểm thử hàm Smoothstep
    // -----------------------------------------------------------------------

    #[test]
    fn test_smoothstep_boundaries() {
        // S(0) = 0, S(1) = 1
        assert!((smoothstep(0.0) - 0.0).abs() < 1e-10);
        assert!((smoothstep(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_smoothstep_midpoint() {
        // S(0.5) = 3×0.25 - 2×0.125 = 0.75 - 0.25 = 0.5
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_smoothstep_monotonic() {
        // Smoothstep phải tăng đơn điệu trong [0, 1]
        let mut prev = 0.0;
        for i in 1..=100 {
            let t = i as f64 / 100.0;
            let s = smoothstep(t);
            assert!(s >= prev, "smoothstep({}) = {} < prev {}", t, s, prev);
            prev = s;
        }
    }

    #[test]
    fn test_smoothstep_clamping() {
        // Giá trị ngoài [0, 1] phải được clamp
        assert!((smoothstep(-0.5) - 0.0).abs() < 1e-10);
        assert!((smoothstep(1.5) - 1.0).abs() < 1e-10);
    }

    // -----------------------------------------------------------------------
    // Nhóm E: Kiểm thử sinh profile S-curve
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_segments_returns_correct_count() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(10_000, 5000).unwrap();
        // Mặc định 10 segments
        assert_eq!(segments.len(), 10);
    }

    #[test]
    fn test_pulse_conservation() {
        // Bảo toàn tổng xung: Σ|P_segment| = |P_total|
        let profile = AccelerationProfile::new();
        let total = 32_768; // 90°
        let segments = profile.generate_segments(total, 5000).unwrap();

        let sum_pulses: i32 = segments.iter().map(|s| s.pulses).sum();
        assert_eq!(
            sum_pulses, total,
            "Tổng xung segments {} ≠ tổng ban đầu {}",
            sum_pulses, total
        );
    }

    #[test]
    fn test_pulse_conservation_negative_direction() {
        // Bảo toàn tổng xung cho hướng âm (quay trái)
        let profile = AccelerationProfile::new();
        let total = -32_768;
        let segments = profile.generate_segments(total, 5000).unwrap();

        let sum_pulses: i32 = segments.iter().map(|s| s.pulses).sum();
        assert_eq!(sum_pulses, total);

        // Mỗi segment phải có xung âm
        for seg in &segments {
            assert!(seg.pulses < 0, "Segment xung phải âm: {}", seg.pulses);
        }
    }

    #[test]
    fn test_accel_phase_frequency_increases() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(50_000, 5000).unwrap();

        // Lọc segments pha tăng tốc
        let accel_segs: Vec<&MotionSegment> =
            segments.iter().filter(|s| s.phase == "accel").collect();

        assert!(!accel_segs.is_empty(), "Phải có ít nhất 1 segment tăng tốc");

        // Tần số phải tăng dần
        for i in 1..accel_segs.len() {
            assert!(
                accel_segs[i].frequency >= accel_segs[i - 1].frequency,
                "Tần số accel phải tăng: [{}]={} < [{}]={}",
                i - 1,
                accel_segs[i - 1].frequency,
                i,
                accel_segs[i].frequency
            );
        }
    }

    #[test]
    fn test_decel_phase_frequency_decreases() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(50_000, 5000).unwrap();

        // Lọc segments pha giảm tốc
        let decel_segs: Vec<&MotionSegment> =
            segments.iter().filter(|s| s.phase == "decel").collect();

        assert!(!decel_segs.is_empty(), "Phải có ít nhất 1 segment giảm tốc");

        // Tần số phải giảm dần
        for i in 1..decel_segs.len() {
            assert!(
                decel_segs[i].frequency <= decel_segs[i - 1].frequency,
                "Tần số decel phải giảm: [{}]={} > [{}]={}",
                i - 1,
                decel_segs[i - 1].frequency,
                i,
                decel_segs[i].frequency
            );
        }
    }

    #[test]
    fn test_cruise_phase_frequency_constant() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(50_000, 5000).unwrap();

        // Lọc segments pha cruise
        let cruise_segs: Vec<&MotionSegment> =
            segments.iter().filter(|s| s.phase == "cruise").collect();

        assert!(!cruise_segs.is_empty(), "Phải có ít nhất 1 segment cruise");

        // Tất cả cruise segments phải cùng tần số = f_max
        let first_freq = cruise_segs[0].frequency;
        for seg in &cruise_segs {
            assert_eq!(
                seg.frequency, first_freq,
                "Tần số cruise phải đồng nhất: {} ≠ {}",
                seg.frequency, first_freq
            );
        }
    }

    #[test]
    fn test_small_pulses_returns_none() {
        let profile = AccelerationProfile::new();
        // Xung quá nhỏ → fallback single-shot
        let result = profile.generate_segments(100, 5000);
        assert!(result.is_none());
    }

    #[test]
    fn test_should_use_profile() {
        assert!(!AccelerationProfile::should_use_profile(400));
        assert!(AccelerationProfile::should_use_profile(500));
        assert!(AccelerationProfile::should_use_profile(32_768));
    }

    #[test]
    fn test_single_shot_fallback() {
        let segments = AccelerationProfile::single_shot(1000, 3000);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].pulses, 1000);
        assert_eq!(segments[0].frequency, 3000);
        assert_eq!(segments[0].phase, "single");
    }

    #[test]
    fn test_total_duration_positive() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(32_768, 5000).unwrap();
        let total_ms = AccelerationProfile::total_duration_ms(&segments);
        assert!(total_ms > 0, "Tổng thời gian phải > 0");
    }

    #[test]
    fn test_custom_params() {
        let profile = AccelerationProfile::with_params(300, 15, 0.30, 0.25);
        assert_eq!(profile.get_min_frequency(), 300);
        assert_eq!(profile.get_total_segments(), 15);
        assert!((profile.get_accel_ratio() - 0.30).abs() < 1e-10);
        assert!((profile.get_decel_ratio() - 0.25).abs() < 1e-10);

        let segments = profile.generate_segments(50_000, 6000).unwrap();
        assert_eq!(segments.len(), 15);
    }

    #[test]
    fn test_all_segments_have_positive_frequency() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(32_768, 5000).unwrap();

        for (i, seg) in segments.iter().enumerate() {
            assert!(
                seg.frequency >= DEFAULT_MIN_FREQUENCY,
                "Segment [{}] tần số {} < f_min {}",
                i,
                seg.frequency,
                DEFAULT_MIN_FREQUENCY
            );
        }
    }

    #[test]
    fn test_each_segment_has_nonzero_pulses() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(32_768, 5000).unwrap();

        for (i, seg) in segments.iter().enumerate() {
            assert!(
                seg.pulses != 0,
                "Segment [{}] có xung = 0, không hợp lệ",
                i
            );
        }
    }

    #[test]
    fn test_phase_ordering() {
        let profile = AccelerationProfile::new();
        let segments = profile.generate_segments(50_000, 5000).unwrap();

        // Thứ tự pha phải là: accel* → cruise* → decel*
        let phases: Vec<&str> = segments.iter().map(|s| s.phase).collect();

        let mut seen_cruise = false;
        let mut seen_decel = false;

        for phase in &phases {
            match *phase {
                "accel" => {
                    assert!(!seen_cruise && !seen_decel, "accel sau cruise/decel");
                }
                "cruise" => {
                    assert!(!seen_decel, "cruise sau decel");
                    seen_cruise = true;
                }
                "decel" => {
                    seen_decel = true;
                }
                _ => panic!("Pha không hợp lệ: {}", phase),
            }
        }
    }
}
