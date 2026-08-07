// =============================================================================
// LoadingSystem - Self-Learning Algorithm (Level 5)
// =============================================================================
// Thuật toán tự học khép kín (Closed-Loop Self-Learning) cho đĩa xoay.
//
// Nguyên lý hoạt động:
// Khi chuyên gia nhấn nút Fine Tune trên HMI để can thiệp thủ công (Level 4),
// hệ thống ghi nhận độ lệch Δθ (delta xung) và sử dụng thuật toán
// Exponential Moving Average (EMA) để cập nhật hệ số bù offset.
//
// Công thức:
//   new_offset = current_offset + round(delta_pulses × learning_coefficient)
//
// Với learning_coefficient = 0.1:
//   - Mỗi lần hiệu chỉnh, 10% giá trị sai lệch được tích lũy vào offset
//   - Sau nhiều lần hiệu chỉnh, offset hội tụ dần về giá trị bù tối ưu
//   - Tránh overshoot khi sai số đo lường có nhiễu
// =============================================================================

/// Tính toán giá trị offset mới sau khi nhận được tín hiệu hiệu chuẩn.
///
/// # Arguments
/// * `current_offset` - Giá trị offset hiện tại (xung)
/// * `delta_pulses` - Số xung sai lệch cần bù (dương = lệch phải, âm = lệch trái)
/// * `coefficient` - Hệ số học (0.0 - 1.0), thường = 0.1
///
/// # Returns
/// Giá trị offset mới đã được cập nhật
///
/// # Examples
/// ```
/// use loading_system::calibration::learning::compute_offset_update;
///
/// // Offset ban đầu = 0, sai lệch 100 xung, hệ số 0.1
/// let new_offset = compute_offset_update(0, 100, 0.1);
/// assert_eq!(new_offset, 10); // 0 + round(100 × 0.1) = 10
///
/// // Tiếp tục học: offset = 10, sai lệch 100 xung
/// let newer_offset = compute_offset_update(10, 100, 0.1);
/// assert_eq!(newer_offset, 20); // 10 + round(100 × 0.1) = 20
/// ```
pub fn compute_offset_update(
    current_offset: i32,
    delta_pulses: i32,
    coefficient: f64,
) -> i32 {
    let adjustment = (delta_pulses as f64 * coefficient).round() as i32;
    current_offset + adjustment
}

/// Tính hệ số suy giảm (decay) cho learning coefficient theo số lần hiệu chuẩn.
/// Khi số lần hiệu chuẩn tăng, hệ số học giảm dần để hệ thống ổn định.
///
/// # Arguments
/// * `base_coefficient` - Hệ số học ban đầu
/// * `calibration_count` - Số lần hiệu chuẩn đã thực hiện
/// * `decay_rate` - Tốc độ suy giảm (mặc định 0.95)
///
/// # Returns
/// Hệ số học đã suy giảm (không nhỏ hơn 0.01)
pub fn compute_decayed_coefficient(
    base_coefficient: f64,
    calibration_count: u32,
    decay_rate: f64,
) -> f64 {
    let decayed = base_coefficient * decay_rate.powi(calibration_count as i32);
    decayed.max(0.01) // Sàn tối thiểu 1% để luôn có khả năng học
}

/// Ước tính số lần hiệu chuẩn cần thiết để offset hội tụ về giá trị mục tiêu.
///
/// # Arguments
/// * `target_offset` - Giá trị offset mục tiêu (xung)
/// * `current_offset` - Giá trị offset hiện tại
/// * `delta_per_adjustment` - Sai lệch trung bình mỗi lần (xung)
/// * `coefficient` - Hệ số học
///
/// # Returns
/// Số lần hiệu chuẩn ước tính
pub fn estimate_convergence_steps(
    target_offset: i32,
    current_offset: i32,
    delta_per_adjustment: i32,
    coefficient: f64,
) -> u32 {
    if delta_per_adjustment == 0 || coefficient <= 0.0 {
        return u32::MAX;
    }

    let remaining = (target_offset - current_offset).abs() as f64;
    let step_size = (delta_per_adjustment.abs() as f64 * coefficient).max(1.0);
    (remaining / step_size).ceil() as u32
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_offset_update() {
        // Từ 0, sai lệch 100 xung, hệ số 0.1 → offset mới = 10
        assert_eq!(compute_offset_update(0, 100, 0.1), 10);
    }

    #[test]
    fn test_cumulative_offset_update() {
        // Tích lũy nhiều lần: 0 → 10 → 20
        let step1 = compute_offset_update(0, 100, 0.1);
        let step2 = compute_offset_update(step1, 100, 0.1);
        assert_eq!(step1, 10);
        assert_eq!(step2, 20);
    }

    #[test]
    fn test_negative_delta() {
        // Sai lệch âm (lệch trái)
        assert_eq!(compute_offset_update(0, -100, 0.1), -10);
    }

    #[test]
    fn test_high_coefficient() {
        // Hệ số cao = học nhanh
        assert_eq!(compute_offset_update(0, 100, 1.0), 100);
        assert_eq!(compute_offset_update(0, 100, 0.5), 50);
    }

    #[test]
    fn test_zero_delta() {
        // Không có sai lệch → offset không đổi
        assert_eq!(compute_offset_update(42, 0, 0.1), 42);
    }

    #[test]
    fn test_coefficient_decay() {
        let base = 0.1;
        let decayed = compute_decayed_coefficient(base, 10, 0.95);
        assert!(decayed < base);
        assert!(decayed > 0.01);
    }

    #[test]
    fn test_coefficient_decay_floor() {
        // Sau rất nhiều lần, coefficient phải >= 0.01
        let decayed = compute_decayed_coefficient(0.1, 1000, 0.95);
        assert!(decayed >= 0.01);
    }

    #[test]
    fn test_convergence_estimation() {
        let steps = estimate_convergence_steps(100, 0, 100, 0.1);
        assert_eq!(steps, 10); // 100 / (100 × 0.1) = 10
    }
}
