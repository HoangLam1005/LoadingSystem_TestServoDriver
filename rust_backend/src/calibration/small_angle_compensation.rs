// =============================================================================
// LoadingSystem - Small Angle Compensation Algorithm
// =============================================================================
// Thuật toán tính bù góc quay nhỏ cho đĩa xoay (Rotating Disc).
//
// Vấn đề thực tế:
//   Khi đĩa xoay quay các góc nhỏ (< 15°), sai lệch vị trí thực tế lớn hơn
//   đáng kể so với góc lớn do tổ hợp 3 hiện tượng cơ-điện:
//     1. Dead-zone encoder: Vùng chết khởi động motor ≈ 50-80 xung
//     2. Ma sát tĩnh (Static Friction): Lực cản ban đầu của mâm đĩa + ổ bi
//     3. Backlash cơ khí: Khe hở giữa bánh răng truyền động
//
// Mô hình toán học:
//   Gọi θ là góc quay mục tiêu (độ), P_raw là số xung lý thuyết:
//     P_raw = round(θ × 131,072 / 360)
//
//   Số xung bù bổ sung cho góc nhỏ:
//     P_comp = round(K_base × e^(-α × θ))     khi θ < θ_threshold
//     P_comp = 0                                khi θ ≥ θ_threshold
//
//   Với:
//     K_base: Hệ số bù cơ sở (xung), phụ thuộc ma sát hệ cơ khí (mặc định 60)
//     α: Tốc độ suy giảm hàm mũ (mặc định 0.15)
//     θ_threshold: Ngưỡng góc nhỏ (mặc định 15°)
//
//   Số xung cuối cùng sau bù:
//     P_final = P_raw + sign(direction) × P_comp
// =============================================================================

/// Ngưỡng mặc định phân biệt "góc nhỏ" (độ).
/// Các góc quay < 15° được coi là góc nhỏ cần bù bổ sung.
const DEFAULT_SMALL_ANGLE_THRESHOLD: f64 = 15.0;

/// Hệ số bù cơ sở mặc định (xung).
/// Biểu thị tổng dead-zone + ma sát tĩnh ở trạng thái nghỉ.
const DEFAULT_BASE_COMPENSATION: f64 = 60.0;

/// Hệ số suy giảm mũ mặc định.
/// Quyết định tốc độ giảm lượng bù khi góc tăng dần.
const DEFAULT_DECAY_RATE: f64 = 0.15;

/// Bộ tính bù góc quay nhỏ cho đĩa xoay.
/// Sử dụng mô hình suy giảm hàm mũ (Exponential Decay) để tính toán
/// lượng xung bù bổ sung, triệt tiêu sai lệch dead-zone và ma sát tĩnh.
#[derive(Debug, Clone)]
pub struct SmallAngleCompensator {
    /// Ngưỡng phân biệt góc nhỏ (độ). Góc < threshold được bù bổ sung.
    threshold_degrees: f64,
    /// Hệ số bù cơ sở (xung) — lượng bù tối đa khi θ → 0
    base_compensation_pulses: f64,
    /// Hệ số suy giảm mũ α — tốc độ giảm bù khi θ tăng
    decay_rate: f64,
    /// Độ phân giải encoder (131,072 xung/vòng cho 17-bit)
    encoder_resolution: u32,
}

impl SmallAngleCompensator {
    /// Tạo bộ bù góc nhỏ với tham số mặc định đã hiệu chỉnh cho Servo CSD7.
    pub fn new(encoder_resolution: u32) -> Self {
        SmallAngleCompensator {
            threshold_degrees: DEFAULT_SMALL_ANGLE_THRESHOLD,
            base_compensation_pulses: DEFAULT_BASE_COMPENSATION,
            decay_rate: DEFAULT_DECAY_RATE,
            encoder_resolution,
        }
    }

    /// Tạo bộ bù góc nhỏ với tham số tùy chỉnh.
    ///
    /// # Arguments
    /// * `encoder_resolution` - Độ phân giải encoder (xung/vòng)
    /// * `threshold_degrees` - Ngưỡng góc nhỏ (độ)
    /// * `base_compensation_pulses` - Lượng bù cơ sở tối đa (xung)
    /// * `decay_rate` - Hệ số suy giảm hàm mũ
    pub fn with_params(
        encoder_resolution: u32,
        threshold_degrees: f64,
        base_compensation_pulses: f64,
        decay_rate: f64,
    ) -> Self {
        SmallAngleCompensator {
            threshold_degrees: threshold_degrees.abs(),
            base_compensation_pulses: base_compensation_pulses.abs(),
            decay_rate: decay_rate.abs(),
            encoder_resolution,
        }
    }

    /// Tính số xung bù bổ sung cho một góc quay cho trước.
    ///
    /// Công thức: P_comp = round(K_base × e^(-α × |θ|))
    ///
    /// # Arguments
    /// * `degrees` - Góc quay mục tiêu (giá trị tuyệt đối, đơn vị: độ)
    ///
    /// # Returns
    /// Số xung bù bổ sung (luôn ≥ 0). Trả về 0 nếu góc ≥ ngưỡng.
    ///
    /// # Examples
    /// ```text
    ///  1° → P_comp ≈ 51 xung (bù lớn nhất — dead-zone chiếm ưu thế)
    ///  5° → P_comp ≈ 28 xung (bù trung bình)
    /// 10° → P_comp ≈ 13 xung (bù nhỏ — gần ngưỡng)
    /// 15° → P_comp =  0 xung (vượt ngưỡng, không bù)
    /// ```
    pub fn compute_compensation_pulses(&self, degrees: f64) -> i32 {
        let abs_degrees = degrees.abs();

        // Góc lớn hơn hoặc bằng ngưỡng → không cần bù
        if abs_degrees >= self.threshold_degrees {
            return 0;
        }

        // Mô hình suy giảm hàm mũ: K_base × e^(-α × θ)
        let compensation = self.base_compensation_pulses
            * (-self.decay_rate * abs_degrees).exp();

        compensation.round() as i32
    }

    /// Tính tổng số xung cuối cùng sau khi áp dụng bù góc nhỏ.
    ///
    /// # Arguments
    /// * `degrees` - Góc quay mục tiêu (độ, giá trị dương)
    /// * `is_positive_direction` - true = quay phải (xung dương), false = quay trái
    ///
    /// # Returns
    /// Số xung cuối cùng (có dấu) đã bao gồm bù góc nhỏ.
    pub fn apply_compensation(&self, degrees: f64, is_positive_direction: bool) -> i32 {
        let raw_pulses = ((degrees.abs() * self.encoder_resolution as f64) / 360.0).round() as i32;
        let comp_pulses = self.compute_compensation_pulses(degrees);

        let total = raw_pulses + comp_pulses;

        if is_positive_direction {
            total
        } else {
            -total
        }
    }

    /// Quy đổi lượng xung bù sang góc bù tương đương (độ).
    pub fn compensation_to_degrees(&self, degrees: f64) -> f64 {
        let comp_pulses = self.compute_compensation_pulses(degrees);
        (comp_pulses as f64 * 360.0) / self.encoder_resolution as f64
    }

    /// Phân loại góc quay vào vùng hoạt động.
    ///
    /// # Returns
    /// * `"micro"` — Góc cực nhỏ (< 5°): bù tối đa, rủi ro dead-zone cao
    /// * `"small"` — Góc nhỏ (5° - 15°): bù suy giảm dần
    /// * `"normal"` — Góc bình thường (≥ 15°): không cần bù
    pub fn classify_angle(degrees: f64) -> &'static str {
        let abs_deg = degrees.abs();
        if abs_deg < 5.0 {
            "micro"
        } else if abs_deg < DEFAULT_SMALL_ANGLE_THRESHOLD {
            "small"
        } else {
            "normal"
        }
    }

    // --- Accessors ---

    pub fn get_threshold(&self) -> f64 {
        self.threshold_degrees
    }

    pub fn get_base_compensation(&self) -> f64 {
        self.base_compensation_pulses
    }

    pub fn get_decay_rate(&self) -> f64 {
        self.decay_rate
    }
}

impl Default for SmallAngleCompensator {
    fn default() -> Self {
        Self::new(131_072)
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const ENCODER_RES: u32 = 131_072;

    #[test]
    fn test_no_compensation_above_threshold() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        // Góc 15° = ngưỡng → không bù
        assert_eq!(comp.compute_compensation_pulses(15.0), 0);
        // Góc 90° → không bù
        assert_eq!(comp.compute_compensation_pulses(90.0), 0);
    }

    #[test]
    fn test_compensation_decreases_with_angle() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let comp_1 = comp.compute_compensation_pulses(1.0);
        let comp_5 = comp.compute_compensation_pulses(5.0);
        let comp_10 = comp.compute_compensation_pulses(10.0);
        // Bù phải giảm dần khi góc tăng
        assert!(comp_1 > comp_5, "comp_1={} should > comp_5={}", comp_1, comp_5);
        assert!(comp_5 > comp_10, "comp_5={} should > comp_10={}", comp_5, comp_10);
        assert!(comp_10 > 0, "comp_10={} should > 0", comp_10);
    }

    #[test]
    fn test_compensation_at_zero_degrees() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let c = comp.compute_compensation_pulses(0.0);
        // Tại 0°: P_comp = K_base × e^0 = K_base = 60
        assert_eq!(c, DEFAULT_BASE_COMPENSATION.round() as i32);
    }

    #[test]
    fn test_apply_compensation_positive_direction() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let result = comp.apply_compensation(5.0, true);
        let raw = ((5.0 * ENCODER_RES as f64) / 360.0).round() as i32;
        // Kết quả phải lớn hơn raw do có bù bổ sung
        assert!(result > raw, "result={} should > raw={}", result, raw);
    }

    #[test]
    fn test_apply_compensation_negative_direction() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let result = comp.apply_compensation(5.0, false);
        // Quay trái → xung âm
        assert!(result < 0, "result={} should be negative", result);
    }

    #[test]
    fn test_apply_compensation_large_angle_no_extra() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let result = comp.apply_compensation(90.0, true);
        let raw = ((90.0 * ENCODER_RES as f64) / 360.0).round() as i32;
        // Góc lớn: kết quả bằng đúng raw (không bù)
        assert_eq!(result, raw);
    }

    #[test]
    fn test_custom_params() {
        let comp = SmallAngleCompensator::with_params(ENCODER_RES, 20.0, 100.0, 0.2);
        assert_eq!(comp.get_threshold(), 20.0);
        assert_eq!(comp.get_base_compensation(), 100.0);
        assert_eq!(comp.get_decay_rate(), 0.2);
        // Ngưỡng 20° → góc 19° vẫn được bù
        assert!(comp.compute_compensation_pulses(19.0) > 0);
    }

    #[test]
    fn test_classify_angle() {
        assert_eq!(SmallAngleCompensator::classify_angle(2.0), "micro");
        assert_eq!(SmallAngleCompensator::classify_angle(10.0), "small");
        assert_eq!(SmallAngleCompensator::classify_angle(45.0), "normal");
    }

    #[test]
    fn test_negative_degrees_treated_as_positive() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let c_pos = comp.compute_compensation_pulses(5.0);
        let c_neg = comp.compute_compensation_pulses(-5.0);
        assert_eq!(c_pos, c_neg);
    }

    #[test]
    fn test_compensation_to_degrees() {
        let comp = SmallAngleCompensator::new(ENCODER_RES);
        let deg_comp = comp.compensation_to_degrees(5.0);
        // Bù phải là góc rất nhỏ (< 1° cho thiết lập mặc định)
        assert!(deg_comp > 0.0 && deg_comp < 1.0,
            "compensation_degrees={} should be in (0, 1)", deg_comp);
    }
}
