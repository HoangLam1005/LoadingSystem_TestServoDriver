// =============================================================================
// Test: JSON Command/Response Serialization
// =============================================================================
// Kiểm chứng format JSON khớp với giao thức đặc tả trong kiến trúc.
// =============================================================================

use loading_system::api::commands::{CommandMessage, ResponseMessage, StatusData};

// ---------------------------------------------------------------------------
// Test: Command serialization
// ---------------------------------------------------------------------------

#[test]
fn test_rotate_right_command_json() {
    let cmd = CommandMessage::rotate_right(90.0);
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"ROTATE_RIGHT\""));
    assert!(json.contains("\"value\":90.0"));
}

#[test]
fn test_rotate_left_command_json() {
    let cmd = CommandMessage::rotate_left(45.0);
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"ROTATE_LEFT\""));
    assert!(json.contains("\"value\":45.0"));
}

#[test]
fn test_return_home_command_json() {
    let cmd = CommandMessage::return_home();
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"RETURN_HOME\""));
    // value should be null
    assert!(json.contains("\"value\":null"));
}

#[test]
fn test_override_adjust_command_json() {
    // Phải khớp với format đặc tả: {"command": "OVERRIDE_ADJUST", "value": 0.5}
    let cmd = CommandMessage::override_adjust(0.5);
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"OVERRIDE_ADJUST\""));
    assert!(json.contains("\"value\":0.5"));
}

#[test]
fn test_get_status_command_json() {
    let cmd = CommandMessage::get_status();
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"GET_STATUS\""));
}

#[test]
fn test_shutdown_command_json() {
    let cmd = CommandMessage::shutdown();
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"command\":\"SHUTDOWN\""));
}

// ---------------------------------------------------------------------------
// Test: Command deserialization
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_rotate_right() {
    let json = r#"{"command": "ROTATE_RIGHT", "value": 90.0}"#;
    let cmd: CommandMessage = serde_json::from_str(json).unwrap();
    assert_eq!(cmd.command, "ROTATE_RIGHT");
    assert_eq!(cmd.value, Some(90.0));
}

#[test]
fn test_deserialize_override_adjust() {
    // Đúng format từ kiến trúc
    let json = r#"{"command": "OVERRIDE_ADJUST", "value": 0.5}"#;
    let cmd: CommandMessage = serde_json::from_str(json).unwrap();
    assert_eq!(cmd.command, "OVERRIDE_ADJUST");
    assert_eq!(cmd.value, Some(0.5));
}

#[test]
fn test_deserialize_without_value() {
    let json = r#"{"command": "GET_STATUS"}"#;
    let cmd: CommandMessage = serde_json::from_str(json).unwrap();
    assert_eq!(cmd.command, "GET_STATUS");
    assert_eq!(cmd.value, None);
}

#[test]
fn test_deserialize_negative_value() {
    let json = r#"{"command": "OVERRIDE_ADJUST", "value": -0.5}"#;
    let cmd: CommandMessage = serde_json::from_str(json).unwrap();
    assert_eq!(cmd.value, Some(-0.5));
}

// ---------------------------------------------------------------------------
// Test: Response serialization
// ---------------------------------------------------------------------------

#[test]
fn test_success_response_json() {
    let resp = ResponseMessage::success("Rotated 90°".into());
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"message\":\"Rotated 90°\""));
    // data field should be absent (skip_serializing_if = None)
    assert!(!json.contains("\"data\""));
}

#[test]
fn test_error_response_json() {
    let resp = ResponseMessage::error("Connection failed".into());
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":false"));
    assert!(json.contains("\"message\":\"Connection failed\""));
}

#[test]
fn test_status_response_json() {
    let data = StatusData {
        current_angle: 45.0,
        offset_pulses: 10,
        learning_coefficient: 0.1,
        calibration_count: 5,
    };
    let resp = ResponseMessage::success_with_data("Status".into(), data);
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"current_angle\":45.0"));
    assert!(json.contains("\"offset_pulses\":10"));
    assert!(json.contains("\"learning_coefficient\":0.1"));
    assert!(json.contains("\"calibration_count\":5"));
}

// ---------------------------------------------------------------------------
// Test: Response deserialization (Python HMI phải parse được)
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_success_response() {
    let json = r#"{"success":true,"message":"OK"}"#;
    let resp: ResponseMessage = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    assert_eq!(resp.message, "OK");
    assert!(resp.data.is_none());
}

#[test]
fn test_deserialize_status_response() {
    let json = r#"{"success":true,"message":"Status","data":{"current_angle":90.0,"offset_pulses":5,"learning_coefficient":0.1,"calibration_count":3}}"#;
    let resp: ResponseMessage = serde_json::from_str(json).unwrap();
    assert!(resp.success);
    let data = resp.data.unwrap();
    assert!((data.current_angle - 90.0).abs() < f64::EPSILON);
    assert_eq!(data.offset_pulses, 5);
    assert_eq!(data.calibration_count, 3);
}
