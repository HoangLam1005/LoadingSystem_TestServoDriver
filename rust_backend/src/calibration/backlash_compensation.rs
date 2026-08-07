// =============================================================================
// LoadingSystem - Backlash Compensation Algorithm
// =============================================================================
// Thuật toán tự động bù độ rơ và khe hở cơ khí (backlash) khi đảo chiều quay.
//
// Vấn đề giải quyết:
//   Hệ truyền động bánh răng luôn tồn tại khe hở (backlash gap) giữa các
//   răng ăn khớp. Khi motor đảo chiều, trục motor phải quay qua khe hở này
//   trước khi tiếp xúc mặt răng đối diện. Trong khoảng khe hở đó, motor
//   đã phát xung nhưng mâm đĩa KHÔNG di chuyển (dead travel).
//
// Giải pháp:
//   Module theo dõi hướng quay lần trước (last_direction). Khi phát hiện
//   lệnh quay mới có hướng KHÁC với lần trước, tự động cộng thêm xung bù
//   (backlash_pulses) vào lệnh để triệt tiêu khe hở.
//
// Mô hình toán học:
//   Gọi B là giá trị backlash (xung), D_prev là hướng quay trước, D_new là hướng mới:
//     P_comp = B    nếu D_new ≠ D_prev (đảo chiều)
//     P_comp = 0    nếu D_new == D_prev (cùng chiều)
//
//   Số xung cuối cùng: P_final = P_raw + P_comp
//
// Thuật toán tự học (Adaptive Backlash):
//   Khi nhận feedback sai lệch từ HMI Override, hệ thống cập nhật giá trị
//   backlash bằng Weighted Moving Average (WMA):
//     B_new = B_old × (1 - α) + |feedback| × α
//   Với α = learning_rate (mặc định 0.15)
// =============================================================================

/// Giá trị backlash mặc định (xung).
/// 109 xung ≈ 0.3° — giá trị tiêu biểu cho hệ truyền động bánh răng công nghiệp nhẹ.
const DEFAULT_BACKLASH_PULSES: u32 = 109;

/// Giới hạn backlash tối đa (xung) để tránh bù quá mức.
/// 364 xung ≈ 1.0° — nếu backlash thực tế lớn hơn, hệ cơ khí cần bảo trì.
const MAX_BACKLASH_PULSES: u32 = 364;

/// Giới hạn backlash tối thiểu (xung).
/// 10 xung ≈ 0.027° — sàn tối thiểu để tránh backlash bị học về 0.
const MIN_BACKLASH_PULSES: u32 = 10;

/// Hệ số học mặc định cho adaptive backlash.
/// 0.15 = mỗi lần feedback, 15% giá trị sai lệch được tích lũy.
const DEFAULT_LEARNING_RATE: f64 = 0.15;

// ---------------------------------------------------------------------------
// Direction Tracking
// ---------------------------------------------------------------------------

/// Hướng quay đã ghi nhận (dùng nội bộ cho backlash tracking).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackedDirection {
    /// Chưa có lệnh quay nào (trạng thái khởi tạo)
    Unknown,
    /// Quay sang trái (xung âm)
    Left,
    /// Quay sang phải (xung dương)
    Right,
}

impl Default for TrackedDirection {
    fn default() -> Self {
        TrackedDirection::Unknown
    }
}

impl std::fmt::Display for TrackedDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackedDirection::Unknown => write!(f, "Unknown"),
            TrackedDirection::Left => write!(f, "Left"),
            TrackedDirection::Right => write!(f, "Right"),
        }
    }
}

// ---------------------------------------------------------------------------
// Backlash Compensator
// ---------------------------------------------------------------------------

/// Bộ bù độ rơ cơ khí (backlash) khi đảo chiều quay.
///
/// Theo dõi hướng quay lần trước và tự động cộng thêm xung bù
/// khi phát hiện lệnh đảo chiều. Hỗ trợ tự học giá trị backlash
/// từ phản hồi thực tế.
#[derive(Debug, Clone)]
pub struct BacklashCompensator {
    /// Giá trị backlash hiện tại (xung) — được tự học từ feedback
    backlash_pulses: u32,
    /// Hướng quay của lệnh gần nhất
    last_direction: TrackedDirection,
    /// Hệ số học cho adaptive backlash (0.0 - 1.0)
    learning_rate: f64,
    /// Số lần đảo chiều đã ghi nhận (thống kê)
    reversal_count: u32,
}

impl BacklashCompensator {
    /// Tạo bộ bù backlash với tham số mặc định.
    ///
    /// Tham số mặc định:
    ///   - backlash_pulses = 109 (≈ 0.3°)
    ///   - learning_rate = 0.15
    pub fn new() -> Self {
        BacklashCompensator {
            backlash_pulses: DEFAULT_BACKLASH_PULSES,
            last_direction: TrackedDirection::Unknown,
            learning_rate: DEFAULT_LEARNING_RATE,
            reversal_count: 0,
        }
    }

    /// Tạo bộ bù backlash với giá trị backlash tùy chỉnh.
    ///
    /// # Arguments
    /// * `backlash_pulses` - Giá trị backlash ban đầu (xung)
    /// * `learning_rate` - Hệ số học (0.0 - 1.0)
    pub fn with_params(backlash_pulses: u32, learning_rate: f64) -> Self {
        BacklashCompensator {
            backlash_pulses: backlash_pulses.clamp(MIN_BACKLASH_PULSES, MAX_BACKLASH_PULSES),
            last_direction: TrackedDirection::Unknown,
            learning_rate: learning_rate.clamp(0.01, 1.0),
            reversal_count: 0,
        }
    }

    /// Tính số xung bù backlash cho lệnh quay mới.
    ///
    /// Logic:
    ///   - Nếu last_direction == Unknown (lần đầu): không bù (trả về 0)
    ///   - Nếu new_direction == last_direction (cùng chiều): không bù (trả về 0)
    ///   - Nếu new_direction ≠ last_direction (đảo chiều): trả về backlash_pulses
    ///
    /// LƯU Ý: Hàm này KHÔNG cập nhật last_direction. Gọi `record_direction()`
    /// sau khi lệnh thực thi thành công.
    ///
    /// # Arguments
    /// * `new_direction` - Hướng quay của lệnh mới
    ///
    /// # Returns
    /// Số xung cần bù thêm (luôn ≥ 0)
    pub fn compute_compensation(&self, new_direction: TrackedDirection) -> u32 {
        match self.last_direction {
            // Lần đầu tiên quay: chưa có chiều trước → không cần bù
            TrackedDirection::Unknown => 0,
            // Cùng chiều với lần trước → không có backlash
            prev if prev == new_direction => 0,
            // Đảo chiều → bù backlash
            _ => self.backlash_pulses,
        }
    }

    /// Ghi nhận hướng quay sau khi lệnh thực thi thành công.
    /// Đồng thời đếm số lần đảo chiều để phục vụ thống kê.
    ///
    /// # Arguments
    /// * `direction` - Hướng quay vừa thực thi
    pub fn record_direction(&mut self, direction: TrackedDirection) {
        if self.last_direction != TrackedDirection::Unknown
            && self.last_direction != direction
        {
            self.reversal_count += 1;
        }
        self.last_direction = direction;
    }

    /// Cập nhật giá trị backlash từ phản hồi sai lệch thực tế (Adaptive Learning).
    ///
    /// Khi chuyên gia nhận thấy sai lệch sau đảo chiều và nhấn Fine Tune trên HMI,
    /// hàm này nhận feedback (số xung sai lệch đo được) và cập nhật backlash_pulses
    /// bằng Weighted Moving Average:
    ///   B_new = B_old × (1 - α) + |feedback| × α
    ///
    /// # Arguments
    /// * `feedback_pulses` - Số xung sai lệch đo được từ phản hồi (có dấu)
    pub fn update_from_feedback(&mut self, feedback_pulses: i32) {
        let abs_feedback = feedback_pulses.unsigned_abs();

        // Weighted Moving Average: trộn giá trị cũ với feedback mới
        let new_backlash = (self.backlash_pulses as f64 * (1.0 - self.learning_rate))
            + (abs_feedback as f64 * self.learning_rate);

        // Clamp trong giới hạn an toàn
        self.backlash_pulses =
            (new_backlash.round() as u32).clamp(MIN_BACKLASH_PULSES, MAX_BACKLASH_PULSES);
    }

    /// Reset hướng quay về Unknown (ví dụ sau khi return_to_origin).
    pub fn reset_direction(&mut self) {
        self.last_direction = TrackedDirection::Unknown;
    }

    // --- Accessors ---

    /// Lấy giá trị backlash hiện tại (xung)
    pub fn get_backlash_pulses(&self) -> u32 {
        self.backlash_pulses
    }

    /// Lấy giá trị backlash dạng góc (độ), dựa trên encoder 17-bit (131,072 xung/vòng)
    pub fn get_backlash_degrees(&self) -> f64 {
        (self.backlash_pulses as f64 * 360.0) / 131_072.0
    }

    /// Lấy hướng quay lần trước
    pub fn get_last_direction(&self) -> TrackedDirection {
        self.last_direction
    }

    /// Lấy số lần đảo chiều đã ghi nhận
    pub fn get_reversal_count(&self) -> u32 {
        self.reversal_count
    }

    /// Lấy hệ số học hiện tại
    pub fn get_learning_rate(&self) -> f64 {
        self.learning_rate
    }
}

impl Default for BacklashCompensator {
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
    // Nhóm F: Kiểm thử tính toán bù backlash
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_compensation_on_first_move() {
        // Lần đầu tiên quay: last_direction = Unknown → không bù
        let comp = BacklashCompensator::new();
        assert_eq!(comp.compute_compensation(TrackedDirection::Right), 0);
    }

    #[test]
    fn test_no_compensation_same_direction() {
        // Quay cùng chiều liên tục → không bù
        let mut comp = BacklashCompensator::new();
        comp.record_direction(TrackedDirection::Right);
        assert_eq!(comp.compute_compensation(TrackedDirection::Right), 0);
    }

    #[test]
    fn test_compensation_on_reversal_right_to_left() {
        // Đảo chiều Right → Left → bù backlash
        let mut comp = BacklashCompensator::new();
        comp.record_direction(TrackedDirection::Right);
        let compensation = comp.compute_compensation(TrackedDirection::Left);
        assert_eq!(compensation, DEFAULT_BACKLASH_PULSES);
    }

    #[test]
    fn test_compensation_on_reversal_left_to_right() {
        // Đảo chiều Left → Right → bù backlash
        let mut comp = BacklashCompensator::new();
        comp.record_direction(TrackedDirection::Left);
        let compensation = comp.compute_compensation(TrackedDirection::Right);
        assert_eq!(compensation, DEFAULT_BACKLASH_PULSES);
    }

    #[test]
    fn test_no_compensation_after_same_direction_recorded() {
        // Right → Right → Right: luôn cùng chiều, không bù
        let mut comp = BacklashCompensator::new();
        comp.record_direction(TrackedDirection::Right);
        assert_eq!(comp.compute_compensation(TrackedDirection::Right), 0);
        comp.record_direction(TrackedDirection::Right);
        assert_eq!(comp.compute_compensation(TrackedDirection::Right), 0);
    }

    #[test]
    fn test_multiple_reversals() {
        // Right → Left → Right: 2 lần đảo chiều, mỗi lần đều bù
        let mut comp = BacklashCompensator::new();

        comp.record_direction(TrackedDirection::Right);
        assert_eq!(comp.compute_compensation(TrackedDirection::Left), DEFAULT_BACKLASH_PULSES);

        comp.record_direction(TrackedDirection::Left);
        assert_eq!(comp.compute_compensation(TrackedDirection::Right), DEFAULT_BACKLASH_PULSES);
    }

    // -----------------------------------------------------------------------
    // Nhóm G: Kiểm thử tự học adaptive
    // -----------------------------------------------------------------------

    #[test]
    fn test_adaptive_learning_increases() {
        // Feedback lớn hơn backlash hiện tại → backlash tăng lên
        let mut comp = BacklashCompensator::new();
        let old = comp.get_backlash_pulses();
        comp.update_from_feedback(200); // feedback = 200 > default 109
        assert!(
            comp.get_backlash_pulses() > old,
            "backlash should increase: {} > {}",
            comp.get_backlash_pulses(),
            old
        );
    }

    #[test]
    fn test_adaptive_learning_decreases() {
        // Feedback nhỏ hơn backlash hiện tại → backlash giảm dần
        let mut comp = BacklashCompensator::new();
        let old = comp.get_backlash_pulses();
        comp.update_from_feedback(20); // feedback = 20 < default 109
        assert!(
            comp.get_backlash_pulses() < old,
            "backlash should decrease: {} < {}",
            comp.get_backlash_pulses(),
            old
        );
    }

    #[test]
    fn test_adaptive_learning_negative_feedback() {
        // Feedback âm: giá trị tuyệt đối được sử dụng
        let mut comp = BacklashCompensator::new();
        let old = comp.get_backlash_pulses();
        comp.update_from_feedback(-200);
        assert!(
            comp.get_backlash_pulses() > old,
            "negative feedback should still increase backlash"
        );
    }

    #[test]
    fn test_backlash_clamped_max() {
        // Feedback rất lớn → backlash không vượt MAX_BACKLASH_PULSES
        let mut comp = BacklashCompensator::new();
        comp.update_from_feedback(100_000);
        assert!(comp.get_backlash_pulses() <= MAX_BACKLASH_PULSES);
    }

    #[test]
    fn test_backlash_clamped_min() {
        // Feedback = 0 nhiều lần → backlash không dưới MIN_BACKLASH_PULSES
        let mut comp = BacklashCompensator::new();
        for _ in 0..100 {
            comp.update_from_feedback(0);
        }
        assert!(comp.get_backlash_pulses() >= MIN_BACKLASH_PULSES);
    }

    // -----------------------------------------------------------------------
    // Nhóm H: Kiểm thử tracking và thống kê
    // -----------------------------------------------------------------------

    #[test]
    fn test_reversal_count() {
        let mut comp = BacklashCompensator::new();
        assert_eq!(comp.get_reversal_count(), 0);

        comp.record_direction(TrackedDirection::Right);
        assert_eq!(comp.get_reversal_count(), 0); // Lần đầu từ Unknown → không đếm

        comp.record_direction(TrackedDirection::Left); // Đảo chiều lần 1
        assert_eq!(comp.get_reversal_count(), 1);

        comp.record_direction(TrackedDirection::Right); // Đảo chiều lần 2
        assert_eq!(comp.get_reversal_count(), 2);

        comp.record_direction(TrackedDirection::Right); // Cùng chiều
        assert_eq!(comp.get_reversal_count(), 2);
    }

    #[test]
    fn test_reset_direction() {
        let mut comp = BacklashCompensator::new();
        comp.record_direction(TrackedDirection::Right);
        comp.reset_direction();
        assert_eq!(comp.get_last_direction(), TrackedDirection::Unknown);
        // Sau reset, lệnh mới sẽ không bù (giống lần đầu)
        assert_eq!(comp.compute_compensation(TrackedDirection::Left), 0);
    }

    #[test]
    fn test_custom_params() {
        let comp = BacklashCompensator::with_params(200, 0.25);
        assert_eq!(comp.get_backlash_pulses(), 200);
        assert!((comp.get_learning_rate() - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_backlash_degrees_conversion() {
        let comp = BacklashCompensator::new();
        let degrees = comp.get_backlash_degrees();
        // 109 xung × 360° / 131072 ≈ 0.2993°
        assert!(degrees > 0.29 && degrees < 0.31, "degrees={}", degrees);
    }

    #[test]
    fn test_custom_params_clamped() {
        // Giá trị ngoài giới hạn phải được clamp
        let comp = BacklashCompensator::with_params(0, 2.0);
        assert!(comp.get_backlash_pulses() >= MIN_BACKLASH_PULSES);
        assert!((comp.get_learning_rate() - 1.0).abs() < 1e-10);
    }
}
