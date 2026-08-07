// =============================================================================
// LoadingSystem - Error Types
// =============================================================================
// Tập trung toàn bộ kiểu lỗi hệ thống vào một enum duy nhất.
// Sử dụng thiserror để tự động derive Display và Error traits.
// =============================================================================

use thiserror::Error;

/// Enum tập trung tất cả lỗi có thể phát sinh trong hệ thống LoadingSystem.
#[derive(Error, Debug)]
pub enum LoadingError {
    /// Lỗi giao tiếp Modbus RTU với PLC
    #[error("Modbus communication error: {0}")]
    Modbus(String),

    /// Lỗi cổng serial RS485
    #[error("Serial port error: {0}")]
    Serial(String),

    /// Lỗi hệ thống hiệu chuẩn (calibration)
    #[error("Calibration error: {0}")]
    Calibration(String),

    /// Lỗi xử lý lệnh điều khiển
    #[error("Command processing error: {0}")]
    Command(String),

    /// Lỗi trạng thái phần cứng (Hardware Motion Guard)
    #[error("Hardware error: {0}")]
    Hardware(String),

    /// Lỗi giao tiếp IPC (TCP/ZeroMQ)
    #[error("IPC communication error: {0}")]
    Ipc(String),

    /// Lỗi đọc/ghi file cấu hình
    #[error("Configuration error: {0}")]
    Config(String),

    /// Lỗi IO hệ thống
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Lỗi JSON serialization/deserialization
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
