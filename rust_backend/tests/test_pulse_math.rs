// =============================================================================
// Test: Công thức quy đổi Xung ↔ Góc (Pulse-to-Degree Physics Model)
// =============================================================================
// Kiểm chứng công thức: P = round(θ × 131,072 / 360)
//
// Bảng giá trị tham chiếu:
//   90°   → 32,768 xung
//   45°   → 16,384 xung
//   180°  → 65,536 xung
//   360°  → 131,072 xung
//   1°    → 364 xung (≈ 363.89)
//   0.5°  → 182 xung (≈ 181.94)
// =============================================================================

use loading_system::calibration::profile::CalibrationProfile;
use loading_system::hardware::modbus_client::MockModbusClient;
use loading_system::hardware::rotating_disc::RotatingDiscController;

/// Helper: Tạo controller mock cho testing
fn create_test_controller() -> RotatingDiscController<MockModbusClient> {
    let mock = MockModbusClient::new();
    let calibration = CalibrationProfile::new(0.1);
    RotatingDiscController::new(mock, 131_072, 4_000, calibration)
}

// ---------------------------------------------------------------------------
// Test Case: Quy đổi góc → xung
// ---------------------------------------------------------------------------

#[test]
fn test_90_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(90.0), 32_768);
}

#[test]
fn test_45_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(45.0), 16_384);
}

#[test]
fn test_180_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(180.0), 65_536);
}

#[test]
fn test_360_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(360.0), 131_072);
}

#[test]
fn test_1_degree_to_pulses() {
    let ctrl = create_test_controller();
    // 1° × 131072 / 360 = 363.89 → round = 364
    assert_eq!(ctrl.degrees_to_pulses(1.0), 364);
}

#[test]
fn test_0_5_degrees_to_pulses() {
    let ctrl = create_test_controller();
    // 0.5° × 131072 / 360 = 181.94 → round = 182
    assert_eq!(ctrl.degrees_to_pulses(0.5), 182);
}

#[test]
fn test_0_degrees_to_pulses() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.degrees_to_pulses(0.0), 0);
}

#[test]
fn test_negative_degrees_to_pulses() {
    let ctrl = create_test_controller();
    // Giá trị âm cũng phải quy đổi đúng
    assert_eq!(ctrl.degrees_to_pulses(-90.0), -32_768);
}

#[test]
fn test_small_angle_to_pulses() {
    let ctrl = create_test_controller();
    // 0.01° × 131072 / 360 ≈ 3.64 → round = 4
    assert_eq!(ctrl.degrees_to_pulses(0.01), 4);
}

// ---------------------------------------------------------------------------
// Test Case: Quy đổi xung → góc
// ---------------------------------------------------------------------------

#[test]
fn test_pulses_to_90_degrees() {
    let ctrl = create_test_controller();
    let degrees = ctrl.pulses_to_degrees(32_768);
    assert!((degrees - 90.0).abs() < 0.01);
}

#[test]
fn test_pulses_to_45_degrees() {
    let ctrl = create_test_controller();
    let degrees = ctrl.pulses_to_degrees(16_384);
    assert!((degrees - 45.0).abs() < 0.01);
}

#[test]
fn test_pulses_to_360_degrees() {
    let ctrl = create_test_controller();
    let degrees = ctrl.pulses_to_degrees(131_072);
    assert!((degrees - 360.0).abs() < 0.01);
}

#[test]
fn test_zero_pulses_to_degrees() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.pulses_to_degrees(0), 0.0);
}

// ---------------------------------------------------------------------------
// Test Case: Tính đối xứng (symmetry)
// ---------------------------------------------------------------------------

#[test]
fn test_roundtrip_conversion() {
    let ctrl = create_test_controller();
    for angle in [0.0, 1.0, 15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 180.0, 270.0, 360.0] {
        let pulses = ctrl.degrees_to_pulses(angle);
        let back = ctrl.pulses_to_degrees(pulses);
        assert!(
            (back - angle).abs() < 0.003,
            "Roundtrip failed for {:.1}°: got {:.4}°",
            angle,
            back
        );
    }
}

#[test]
fn test_encoder_resolution() {
    let ctrl = create_test_controller();
    assert_eq!(ctrl.get_encoder_resolution(), 131_072);
}
