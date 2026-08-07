// =============================================================================
// LoadingSystem - 17-bit Serial Encoder Reader Module
// =============================================================================
// Module chuyên biệt đọc, giải mã và xác thực dữ liệu từ bộ mã hóa vòng quay
// (Rotary Encoder) 17-bit tích hợp trên Servo Driver RS Automation CSD7.
//
// Thông số kỹ thuật encoder:
//   - Độ phân giải: 17-bit → 2^17 = 131,072 xung/vòng (360°)
//   - Loại: Absolute Single-Turn Serial Encoder
//   - Độ chính xác góc tối thiểu: 360° / 131,072 ≈ 0.00275°
//
// Module này thực hiện:
//   1. Struct Encoder17Bit quản lý toàn bộ trạng thái encoder
//   2. Giải mã raw position từ dữ liệu 17-bit (2 thanh ghi 16-bit)
//   3. Multi-turn tracking: đếm số vòng quay tích lũy
//   4. Quy đổi qua lại giữa raw_count ↔ góc (độ) ↔ radian
//   5. Validate dữ liệu encoder chống tràn và nhiễu
// =============================================================================

/// Độ phân giải tối đa của encoder 17-bit: 2^17 = 131,072 xung/vòng
const ENCODER_17BIT_RESOLUTION: u32 = 131_072;

/// Giá trị raw tối đa hợp lệ (0 đến 131,071)
const ENCODER_17BIT_MAX_RAW: u32 = ENCODER_17BIT_RESOLUTION - 1;

/// Ngưỡng phát hiện bước nhảy vòng (wrap-around detection).
/// Nếu delta giữa 2 lần đọc liên tiếp vượt quá nửa vòng encoder,
/// hệ thống xác định đã xảy ra chuyển vòng (multi-turn transition).
const WRAP_THRESHOLD: u32 = ENCODER_17BIT_RESOLUTION / 2;

/// Bộ đọc và giải mã encoder 17-bit trên Servo CSD7.
/// Quản lý trạng thái vị trí tuyệt đối đơn vòng, đếm vòng tích lũy
/// và cung cấp các hàm quy đổi xung ↔ góc.
#[derive(Debug, Clone)]
pub struct Encoder17Bit {
    /// Giá trị raw hiện tại trong vòng đơn (0 → 131,071)
    raw_position: u32,
    /// Bộ đếm số vòng quay tích lũy (dương = CW, âm = CCW)
    turn_count: i32,
    /// Vị trí raw của lần đọc trước (phục vụ phát hiện wrap-around)
    previous_raw: u32,
    /// Cờ đánh dấu đã khởi tạo lần đọc đầu tiên
    initialized: bool,
}

impl Encoder17Bit {
    /// Khởi tạo bộ đọc encoder 17-bit ở trạng thái chưa có dữ liệu.
    pub fn new() -> Self {
        Encoder17Bit {
            raw_position: 0,
            turn_count: 0,
            previous_raw: 0,
            initialized: false,
        }
    }

    /// Cập nhật vị trí encoder từ dữ liệu raw 17-bit đọc được.
    /// Tự động phát hiện chuyển vòng (wrap-around) và cập nhật turn_count.
    ///
    /// # Arguments
    /// * `raw_value` - Giá trị raw 17-bit từ encoder (0 → 131,071)
    ///
    /// # Returns
    /// * `Ok(())` nếu dữ liệu hợp lệ
    /// * `Err(String)` nếu raw_value vượt quá dải 17-bit
    pub fn update_position(&mut self, raw_value: u32) -> Result<(), String> {
        // Kiểm tra dải hợp lệ: raw phải nằm trong [0, 131071]
        if raw_value > ENCODER_17BIT_MAX_RAW {
            return Err(format!(
                "Encoder raw value {} exceeds 17-bit range [0, {}]",
                raw_value, ENCODER_17BIT_MAX_RAW
            ));
        }

        if self.initialized {
            // Phát hiện chuyển vòng bằng so sánh delta với ngưỡng nửa vòng.
            // So sánh raw_value mới với raw_position hiện tại (lần đọc gần nhất).
            let delta = raw_value as i64 - self.raw_position as i64;
            if delta > WRAP_THRESHOLD as i64 {
                // Chuyển vòng ngược (CCW): raw nhảy từ thấp lên cao
                self.turn_count -= 1;
            } else if delta < -(WRAP_THRESHOLD as i64) {
                // Chuyển vòng thuận (CW): raw nhảy từ cao xuống thấp
                self.turn_count += 1;
            }
        }

        self.previous_raw = self.raw_position;
        self.raw_position = raw_value;
        self.initialized = true;

        Ok(())
    }

    /// Giải mã vị trí encoder từ cặp thanh ghi Modbus 16-bit.
    /// Servo CSD7 trả dữ liệu encoder qua 2 thanh ghi: word thấp và word cao.
    /// Giá trị 17-bit nằm trong 17 bit thấp nhất của cặp thanh ghi ghép.
    ///
    /// # Arguments
    /// * `reg_low` - Thanh ghi 16-bit thấp (bits 0-15)
    /// * `reg_high` - Thanh ghi 16-bit cao (bit 16 chứa MSB encoder)
    pub fn decode_from_registers(&mut self, reg_low: u16, reg_high: u16) -> Result<(), String> {
        let combined = ((reg_high as u32) << 16) | (reg_low as u32);
        // Mask 17 bit thấp nhất: 0x1FFFF = 2^17 - 1 = 131,071
        let raw_17bit = combined & 0x0001_FFFF;
        self.update_position(raw_17bit)
    }

    /// Lấy vị trí raw hiện tại trong vòng đơn (0 → 131,071)
    pub fn get_raw_position(&self) -> u32 {
        self.raw_position
    }

    /// Lấy số vòng quay tích lũy (dương = CW, âm = CCW)
    pub fn get_turn_count(&self) -> i32 {
        self.turn_count
    }

    /// Quy đổi vị trí raw đơn vòng sang góc (độ).
    /// Công thức: angle = raw_position × 360 / 131,072
    pub fn get_angle_degrees(&self) -> f64 {
        (self.raw_position as f64 * 360.0) / ENCODER_17BIT_RESOLUTION as f64
    }

    /// Quy đổi vị trí raw đơn vòng sang góc (radian).
    /// Công thức: angle_rad = raw_position × 2π / 131,072
    pub fn get_angle_radians(&self) -> f64 {
        (self.raw_position as f64 * 2.0 * std::f64::consts::PI) / ENCODER_17BIT_RESOLUTION as f64
    }

    /// Tính vị trí tuyệt đối tích lũy (tính cả multi-turn) ra góc (độ).
    /// Công thức: absolute_angle = turn_count × 360 + single_turn_angle
    pub fn get_absolute_angle_degrees(&self) -> f64 {
        (self.turn_count as f64 * 360.0) + self.get_angle_degrees()
    }

    /// Quy đổi ngược từ góc mục tiêu (độ) sang số xung raw encoder.
    /// Công thức: pulses = round(degrees × 131,072 / 360)
    pub fn degrees_to_raw(degrees: f64) -> u32 {
        let raw = ((degrees.abs() * ENCODER_17BIT_RESOLUTION as f64) / 360.0).round() as u32;
        raw % ENCODER_17BIT_RESOLUTION
    }

    /// Trả về độ phân giải góc tối thiểu của encoder (độ/xung).
    /// Giá trị = 360 / 131,072 ≈ 0.00275°
    pub fn get_angular_resolution() -> f64 {
        360.0 / ENCODER_17BIT_RESOLUTION as f64
    }

    /// Reset bộ đọc encoder về trạng thái gốc (vị trí 0, vòng 0).
    pub fn reset(&mut self) {
        self.raw_position = 0;
        self.turn_count = 0;
        self.previous_raw = 0;
        self.initialized = false;
    }
}

impl Default for Encoder17Bit {
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

    #[test]
    fn test_new_encoder_defaults() {
        let enc = Encoder17Bit::new();
        assert_eq!(enc.get_raw_position(), 0);
        assert_eq!(enc.get_turn_count(), 0);
        assert_eq!(enc.get_angle_degrees(), 0.0);
    }

    #[test]
    fn test_update_position_valid() {
        let mut enc = Encoder17Bit::new();
        assert!(enc.update_position(32_768).is_ok()); // 90°
        assert_eq!(enc.get_raw_position(), 32_768);
        let angle = enc.get_angle_degrees();
        assert!((angle - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_update_position_overflow_rejected() {
        let mut enc = Encoder17Bit::new();
        // Giá trị 131,072 vượt quá dải 17-bit hợp lệ
        assert!(enc.update_position(131_072).is_err());
        assert!(enc.update_position(200_000).is_err());
    }

    #[test]
    fn test_wrap_around_clockwise() {
        let mut enc = Encoder17Bit::new();
        enc.update_position(130_000).unwrap(); // Gần cuối vòng
        enc.update_position(500).unwrap();     // Nhảy qua 0 → CW
        assert_eq!(enc.get_turn_count(), 1);
    }

    #[test]
    fn test_wrap_around_counter_clockwise() {
        let mut enc = Encoder17Bit::new();
        enc.update_position(500).unwrap();     // Gần đầu vòng
        enc.update_position(130_000).unwrap(); // Nhảy ngược → CCW
        assert_eq!(enc.get_turn_count(), -1);
    }

    #[test]
    fn test_decode_from_registers() {
        let mut enc = Encoder17Bit::new();
        // 32,768 = 0x8000 → reg_low = 0x8000, reg_high = 0x0000
        enc.decode_from_registers(0x8000, 0x0000).unwrap();
        assert_eq!(enc.get_raw_position(), 32_768);
    }

    #[test]
    fn test_decode_from_registers_17th_bit() {
        let mut enc = Encoder17Bit::new();
        // 65,536 = 0x10000 → reg_low = 0x0000, reg_high = 0x0001
        enc.decode_from_registers(0x0000, 0x0001).unwrap();
        assert_eq!(enc.get_raw_position(), 65_536);
    }

    #[test]
    fn test_absolute_angle_multi_turn() {
        let mut enc = Encoder17Bit::new();
        enc.update_position(130_000).unwrap();
        enc.update_position(100).unwrap(); // CW wrap → turn_count = 1
        // Absolute = 1 × 360 + (100/131072 × 360)
        let abs_angle = enc.get_absolute_angle_degrees();
        assert!(abs_angle > 360.0);
    }

    #[test]
    fn test_degrees_to_raw_90() {
        assert_eq!(Encoder17Bit::degrees_to_raw(90.0), 32_768);
    }

    #[test]
    fn test_degrees_to_raw_360_wraps() {
        // 360° → 131,072 xung → mod 131,072 = 0
        assert_eq!(Encoder17Bit::degrees_to_raw(360.0), 0);
    }

    #[test]
    fn test_angular_resolution() {
        let res = Encoder17Bit::get_angular_resolution();
        assert!((res - 0.00274658).abs() < 0.0001);
    }

    #[test]
    fn test_radians_conversion() {
        let mut enc = Encoder17Bit::new();
        enc.update_position(32_768).unwrap(); // 90°
        let rad = enc.get_angle_radians();
        assert!((rad - std::f64::consts::FRAC_PI_2).abs() < 0.01);
    }

    #[test]
    fn test_reset() {
        let mut enc = Encoder17Bit::new();
        enc.update_position(50_000).unwrap();
        enc.update_position(130_000).unwrap();
        enc.reset();
        assert_eq!(enc.get_raw_position(), 0);
        assert_eq!(enc.get_turn_count(), 0);
    }
}
