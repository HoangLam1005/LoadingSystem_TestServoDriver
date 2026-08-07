// =============================================================================
// Test: Mock Modbus Integration (Test Case 2 từ kientruc.md)
// =============================================================================
// Kiểm chứng: "Gọi rotate_degrees(45.0, Direction::Right), PLC phải nhận
// được giá trị 16384 trong cụm thanh ghi D100-D101"
//
// Sử dụng MockModbusClient để kiểm tra giá trị chính xác được ghi vào
// các thanh ghi Modbus mà không cần phần cứng PLC thật.
// =============================================================================

use loading_system::calibration::profile::CalibrationProfile;
use loading_system::hardware::modbus_client::{
    MockModbusClient, COIL_TRIGGER_M0, REG_PULSE_POSITION,
};
use loading_system::hardware::rotating_disc::{Direction, RotatingDiscController};

/// Helper: Tạo controller mock với calibration sạch (offset = 0)
fn create_mock_controller() -> (RotatingDiscController<MockModbusClient>, MockModbusClient) {
    let mock = MockModbusClient::new();
    let mock_clone = mock.clone(); // Clone để giữ reference cho assertion
    let calibration = CalibrationProfile::new(0.1);
    let controller = RotatingDiscController::new(mock, 131_072, 4_000, calibration);
    (controller, mock_clone)
}

// ---------------------------------------------------------------------------
// Test Case 2 từ kientruc.md: rotate_degrees(45.0, Direction::Right)
// PLC phải nhận giá trị 16384 tại D100-D101
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_case2_rotate_45_right_register_values() {
    let (mut ctrl, mock) = create_mock_controller();

    ctrl.rotate_degrees(45.0, Direction::Right).await.unwrap();

    // Kiểm tra giá trị trong thanh ghi D100 (vị trí xung)
    let regs = mock.get_registers(REG_PULSE_POSITION).unwrap();
    // regs = [p_low, p_high, f_low, f_high]
    assert_eq!(regs.len(), 4);

    // Tái tạo giá trị 32-bit: 16384 (0x4000)
    let pulse_value = (regs[1] as i32) << 16 | (regs[0] as i32);
    assert_eq!(pulse_value, 16_384, "45° right should produce 16384 pulses");

    // Tần số phải là 4000 Hz (giá trị mặc định)
    let freq_value = (regs[3] as u32) << 16 | (regs[2] as u32);
    assert_eq!(freq_value, 4_000);

    // Coil M0 phải được SET
    assert_eq!(mock.get_coil(COIL_TRIGGER_M0), Some(true));
}

// ---------------------------------------------------------------------------
// Test: Quay phải 90°
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rotate_90_right() {
    let (mut ctrl, mock) = create_mock_controller();

    ctrl.rotate_degrees(90.0, Direction::Right).await.unwrap();

    let regs = mock.get_registers(REG_PULSE_POSITION).unwrap();
    let pulse_value = (regs[1] as i32) << 16 | (regs[0] as i32);
    assert_eq!(pulse_value, 32_768, "90° right should produce 32768 pulses");

    // Góc hiện tại phải cập nhật
    assert!((ctrl.get_current_angle() - 90.0).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Test: Quay trái 90°
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_rotate_90_left() {
    let (mut ctrl, mock) = create_mock_controller();

    ctrl.rotate_degrees(90.0, Direction::Left).await.unwrap();

    let regs = mock.get_registers(REG_PULSE_POSITION).unwrap();
    let pulse_value = (regs[1] as i32) << 16 | (regs[0] as i32);
    assert_eq!(pulse_value, -32_768, "90° left should produce -32768 pulses");

    assert!((ctrl.get_current_angle() - (-90.0)).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Test: Return to origin
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_return_to_origin_from_right() {
    let (mut ctrl, mock) = create_mock_controller();

    // Quay phải 90° rồi về gốc
    ctrl.rotate_degrees(90.0, Direction::Right).await.unwrap();
    ctrl.return_to_origin().await.unwrap();

    let regs = mock.get_registers(REG_PULSE_POSITION).unwrap();
    let pulse_value = (regs[1] as i32) << 16 | (regs[0] as i32);
    // Phải gửi -32768 xung để quay ngược lại
    assert_eq!(pulse_value, -32_768);

    assert!((ctrl.get_current_angle()).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_return_to_origin_from_left() {
    let (mut ctrl, _mock) = create_mock_controller();

    ctrl.rotate_degrees(45.0, Direction::Left).await.unwrap();
    ctrl.return_to_origin().await.unwrap();

    assert!((ctrl.get_current_angle()).abs() < f64::EPSILON);
}

// ---------------------------------------------------------------------------
// Test: Modbus write counts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_write_counts() {
    let (mut ctrl, mock) = create_mock_controller();

    ctrl.rotate_degrees(90.0, Direction::Right).await.unwrap();

    // Mỗi lệnh ghi 1 lần registers + 1 lần coil
    assert_eq!(mock.get_write_register_count(), 1);
    assert_eq!(mock.get_write_coil_count(), 1);

    ctrl.rotate_degrees(45.0, Direction::Left).await.unwrap();

    assert_eq!(mock.get_write_register_count(), 2);
    assert_eq!(mock.get_write_coil_count(), 2);
}

// ---------------------------------------------------------------------------
// Test: Manual override (Level 4 & 5)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_manual_override_affects_rotation() {
    let (mut ctrl, mock) = create_mock_controller();

    // Inject manual override: +0.5° → delta = 182 xung → offset += round(182 × 0.1) = 18
    ctrl.inject_manual_override(0.5);
    assert_eq!(ctrl.get_calibration().offset_pulses, 18);

    // Quay 90° phải với offset bù
    ctrl.rotate_degrees(90.0, Direction::Right).await.unwrap();

    let regs = mock.get_registers(REG_PULSE_POSITION).unwrap();
    let pulse_value = (regs[1] as i32) << 16 | (regs[0] as i32);
    // Expected: 32768 + 18 (offset) = 32786
    assert_eq!(pulse_value, 32_786);
}

#[tokio::test]
async fn test_multiple_overrides_accumulate() {
    let (mut ctrl, _mock) = create_mock_controller();

    // 5 lần override +0.5°
    for _ in 0..5 {
        ctrl.inject_manual_override(0.5);
    }
    // Mỗi lần: offset += round(182 × 0.1) = 18
    // Sau 5 lần: 18 × 5 = 90
    assert_eq!(ctrl.get_calibration().offset_pulses, 90);
    assert_eq!(ctrl.get_calibration().history.len(), 5);
}

// ---------------------------------------------------------------------------
// Test: Negative degrees validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_negative_degrees_error() {
    let (mut ctrl, _mock) = create_mock_controller();

    let result = ctrl.rotate_degrees(-90.0, Direction::Right).await;
    assert!(result.is_err(), "Negative degrees should return error");
}

// ---------------------------------------------------------------------------
// Test: Chuỗi thao tác phức tạp (Complex sequence)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_complex_sequence() {
    let (mut ctrl, _mock) = create_mock_controller();

    // Quay phải 90° → trái 45° → phải 30° → về gốc
    ctrl.rotate_degrees(90.0, Direction::Right).await.unwrap();
    assert!((ctrl.get_current_angle() - 90.0).abs() < f64::EPSILON);

    ctrl.rotate_degrees(45.0, Direction::Left).await.unwrap();
    assert!((ctrl.get_current_angle() - 45.0).abs() < f64::EPSILON);

    ctrl.rotate_degrees(30.0, Direction::Right).await.unwrap();
    assert!((ctrl.get_current_angle() - 75.0).abs() < f64::EPSILON);

    ctrl.return_to_origin().await.unwrap();
    assert!((ctrl.get_current_angle()).abs() < f64::EPSILON);
}
