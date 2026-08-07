// =============================================================================
// LoadingSystem - IPC Command/Response Protocol
// =============================================================================
// Định nghĩa cấu trúc JSON cho giao tiếp giữa Python HMI và Rust Backend.
//
// Giao thức:
//   Python HMI → Rust Backend: CommandMessage (JSON)
//   Rust Backend → Python HMI: ResponseMessage (JSON)
//
// Ví dụ commands:
//   {"command": "ROTATE_RIGHT", "value": 90.0}
//   {"command": "ROTATE_LEFT", "value": 45.0}
//   {"command": "RETURN_HOME"}
//   {"command": "OVERRIDE_ADJUST", "value": 0.5}
//   {"command": "GET_STATUS"}
//   {"command": "SHUTDOWN"}
// =============================================================================

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Command: HMI → Backend
// ---------------------------------------------------------------------------

/// Lệnh điều khiển từ Python HMI gửi tới Rust Backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMessage {
    /// Tên lệnh: ROTATE_RIGHT, ROTATE_LEFT, RETURN_HOME,
    ///            OVERRIDE_ADJUST, GET_STATUS, SHUTDOWN
    pub command: String,

    /// Giá trị kèm theo (tùy lệnh):
    /// - ROTATE_RIGHT/ROTATE_LEFT: góc quay (độ)
    /// - OVERRIDE_ADJUST: góc hiệu chỉnh (độ)
    /// - Các lệnh khác: None
    #[serde(default)]
    pub value: Option<f64>,

    /// Tần số phát xung Servo tùy chỉnh (Hz) cho lệnh ROTATE_RIGHT/ROTATE_LEFT:
    #[serde(default)]
    pub frequency: Option<u32>,
}

impl CommandMessage {
    /// Tạo lệnh quay phải
    pub fn rotate_right(degrees: f64) -> Self {
        CommandMessage {
            command: "ROTATE_RIGHT".into(),
            value: Some(degrees),
            frequency: None,
        }
    }

    /// Tạo lệnh quay trái
    pub fn rotate_left(degrees: f64) -> Self {
        CommandMessage {
            command: "ROTATE_LEFT".into(),
            value: Some(degrees),
            frequency: None,
        }
    }

    /// Tạo lệnh về gốc
    pub fn return_home() -> Self {
        CommandMessage {
            command: "RETURN_HOME".into(),
            value: None,
            frequency: None,
        }
    }

    /// Tạo lệnh can thiệp thủ công
    pub fn override_adjust(degrees: f64) -> Self {
        CommandMessage {
            command: "OVERRIDE_ADJUST".into(),
            value: Some(degrees),
            frequency: None,
        }
    }

    /// Tạo lệnh lấy trạng thái
    pub fn get_status() -> Self {
        CommandMessage {
            command: "GET_STATUS".into(),
            value: None,
            frequency: None,
        }
    }

    /// Tạo lệnh tắt daemon
    pub fn shutdown() -> Self {
        CommandMessage {
            command: "SHUTDOWN".into(),
            value: None,
            frequency: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Response: Backend → HMI
// ---------------------------------------------------------------------------

/// Phản hồi từ Rust Backend gửi lại Python HMI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    /// Trạng thái thực hiện (true = thành công)
    pub success: bool,
    /// Thông báo mô tả kết quả
    pub message: String,
    /// Dữ liệu trạng thái (chỉ có khi command = GET_STATUS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<StatusData>,
}

/// Dữ liệu trạng thái hệ thống
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusData {
    /// Góc quay tích lũy hiện tại (độ)
    pub current_angle: f64,
    /// Giá trị offset bù hiện tại (xung)
    pub offset_pulses: i32,
    /// Hệ số học hiện tại
    pub learning_coefficient: f64,
    /// Số lần hiệu chuẩn đã thực hiện
    pub calibration_count: usize,
}

impl ResponseMessage {
    /// Tạo response thành công
    pub fn success(message: String) -> Self {
        ResponseMessage {
            success: true,
            message,
            data: None,
        }
    }

    /// Tạo response thành công kèm dữ liệu trạng thái
    pub fn success_with_data(message: String, data: StatusData) -> Self {
        ResponseMessage {
            success: true,
            message,
            data: Some(data),
        }
    }

    /// Tạo response lỗi
    pub fn error(message: String) -> Self {
        ResponseMessage {
            success: false,
            message,
            data: None,
        }
    }
}
