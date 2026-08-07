// =============================================================================
// Test: Calibration Profile & Self-Learning Algorithm (Level 5)
// =============================================================================

use loading_system::calibration::learning::{
    compute_decayed_coefficient, compute_offset_update, estimate_convergence_steps,
};
use loading_system::calibration::profile::CalibrationProfile;

// ---------------------------------------------------------------------------
// Test: compute_offset_update
// ---------------------------------------------------------------------------

#[test]
fn test_offset_update_basic() {
    // Offset = 0, delta = 100 xung, coefficient = 0.1
    // Expected: 0 + round(100 × 0.1) = 10
    let result = compute_offset_update(0, 100, 0.1);
    assert_eq!(result, 10);
}

#[test]
fn test_offset_update_cumulative() {
    // Mô phỏng 5 lần hiệu chuẩn liên tiếp với cùng sai lệch
    let mut offset = 0;
    for _ in 0..5 {
        offset = compute_offset_update(offset, 100, 0.1);
    }
    // 0 → 10 → 20 → 30 → 40 → 50
    assert_eq!(offset, 50);
}

#[test]
fn test_offset_update_negative_delta() {
    let result = compute_offset_update(50, -100, 0.1);
    // 50 + round(-100 × 0.1) = 50 - 10 = 40
    assert_eq!(result, 40);
}

#[test]
fn test_offset_update_full_coefficient() {
    // coefficient = 1.0 → học 100%
    let result = compute_offset_update(0, 100, 1.0);
    assert_eq!(result, 100);
}

#[test]
fn test_offset_update_zero_delta() {
    let result = compute_offset_update(42, 0, 0.1);
    assert_eq!(result, 42); // Không thay đổi
}

#[test]
fn test_offset_update_zero_coefficient() {
    let result = compute_offset_update(42, 100, 0.0);
    assert_eq!(result, 42); // Không học
}

#[test]
fn test_offset_update_small_delta() {
    // Delta nhỏ: round(3 × 0.1) = round(0.3) = 0
    let result = compute_offset_update(0, 3, 0.1);
    assert_eq!(result, 0);
}

#[test]
fn test_offset_update_rounding() {
    // round(5 × 0.1) = round(0.5) = 1 (Rust rounds to nearest even, but 0.5 rounds to 0)
    // Actually: (5.0 * 0.1).round() = 0.5.round() = 0 in Rust (banker's rounding)
    // Wait, Rust's f64::round() rounds half away from zero: 0.5.round() = 1.0
    let result = compute_offset_update(0, 5, 0.1);
    assert_eq!(result, 1); // round(0.5) = 1
}

// ---------------------------------------------------------------------------
// Test: coefficient decay
// ---------------------------------------------------------------------------

#[test]
fn test_coefficient_no_decay() {
    let result = compute_decayed_coefficient(0.1, 0, 0.95);
    assert!((result - 0.1).abs() < f64::EPSILON);
}

#[test]
fn test_coefficient_decay_one_step() {
    let result = compute_decayed_coefficient(0.1, 1, 0.95);
    assert!((result - 0.095).abs() < 1e-10);
}

#[test]
fn test_coefficient_decay_minimum_floor() {
    // Sau rất nhiều lần, coefficient phải >= 0.01
    let result = compute_decayed_coefficient(0.1, 10000, 0.95);
    assert!(result >= 0.01);
}

// ---------------------------------------------------------------------------
// Test: convergence estimation
// ---------------------------------------------------------------------------

#[test]
fn test_convergence_basic() {
    let steps = estimate_convergence_steps(100, 0, 100, 0.1);
    assert_eq!(steps, 10);
}

#[test]
fn test_convergence_zero_delta() {
    let steps = estimate_convergence_steps(100, 0, 0, 0.1);
    assert_eq!(steps, u32::MAX);
}

#[test]
fn test_convergence_zero_coefficient() {
    let steps = estimate_convergence_steps(100, 0, 100, 0.0);
    assert_eq!(steps, u32::MAX);
}

// ---------------------------------------------------------------------------
// Test: CalibrationProfile persistence
// ---------------------------------------------------------------------------

#[test]
fn test_profile_creation() {
    let profile = CalibrationProfile::new(0.1);
    assert_eq!(profile.offset_pulses, 0);
    assert!((profile.learning_coefficient - 0.1).abs() < f64::EPSILON);
    assert!(profile.history.is_empty());
}

#[test]
fn test_profile_add_history() {
    let mut profile = CalibrationProfile::new(0.1);
    profile.offset_pulses = 10;
    profile.add_history_entry(0.5, 182);
    assert_eq!(profile.history.len(), 1);
    assert!((profile.history[0].adjustment_degrees - 0.5).abs() < f64::EPSILON);
    assert_eq!(profile.history[0].delta_pulses, 182);
}

#[test]
fn test_profile_save_and_load() {
    let temp_file = std::env::temp_dir().join("test_calibration.json");
    let temp_path = temp_file.to_str().unwrap();

    // Tạo và lưu profile
    let mut profile = CalibrationProfile::new(0.15);
    profile.offset_pulses = 42;
    profile.add_history_entry(1.0, 364);
    profile.save_to_file(temp_path).unwrap();

    // Load lại và kiểm tra
    let loaded = CalibrationProfile::load_from_file(temp_path, 0.1).unwrap();
    assert_eq!(loaded.offset_pulses, 42);
    assert!((loaded.learning_coefficient - 0.15).abs() < f64::EPSILON);
    assert_eq!(loaded.history.len(), 1);

    // Cleanup
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn test_profile_load_nonexistent() {
    let profile = CalibrationProfile::load_from_file("nonexistent_file.json", 0.2).unwrap();
    assert_eq!(profile.offset_pulses, 0);
    assert!((profile.learning_coefficient - 0.2).abs() < f64::EPSILON);
}

#[test]
fn test_profile_recent_history() {
    let mut profile = CalibrationProfile::new(0.1);
    for i in 0..10 {
        profile.add_history_entry(i as f64 * 0.5, i * 182);
    }
    let recent = profile.get_recent_history(3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].delta_pulses, 7 * 182);
}
