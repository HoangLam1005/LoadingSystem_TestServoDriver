// =============================================================================
// LoadingSystem - System Configuration
// =============================================================================
// Đọc cấu hình hệ thống từ file config.toml.
// Nếu file không tồn tại, sử dụng giá trị mặc định an toàn.
// =============================================================================

use serde::Deserialize;
use std::path::Path;

use crate::error::LoadingError;

/// Cấu hình tổng thể hệ thống
#[derive(Debug, Deserialize, Clone)]
pub struct SystemConfig {
    pub serial: SerialConfig,
    pub modbus: ModbusConfig,
    pub ipc: IpcConfig,
    pub disc: DiscConfig,
}

/// Cấu hình cổng serial RS485
#[derive(Debug, Deserialize, Clone)]
pub struct SerialConfig {
    /// Tên cổng COM (VD: "COM3") hoặc "MOCK" cho chế độ mô phỏng
    pub port: String,
    /// Tốc độ baud (mặc định: 9600)
    pub baud_rate: u32,
    /// Số bit dữ liệu (mặc định: 8 cho Modbus RTU)
    pub data_bits: u8,
    /// Số bit dừng (mặc định: 1)
    pub stop_bits: u8,
    /// Kiểu parity: "none", "even", "odd"
    pub parity: String,
}

/// Cấu hình Modbus RTU
#[derive(Debug, Deserialize, Clone)]
pub struct ModbusConfig {
    /// Địa chỉ Slave PLC Delta (mặc định: 1)
    pub slave_address: u8,
}

/// Cấu hình IPC server (giao tiếp với Python HMI)
#[derive(Debug, Deserialize, Clone)]
pub struct IpcConfig {
    /// Địa chỉ IP lắng nghe (mặc định: "127.0.0.1")
    pub host: String,
    /// Cổng TCP (mặc định: 5555)
    pub port: u16,
}

/// Cấu hình đĩa xoay
#[derive(Debug, Deserialize, Clone)]
pub struct DiscConfig {
    /// Độ phân giải encoder 17-bit (131072 xung/vòng)
    pub encoder_resolution: u32,
    /// Tần số phát xung mặc định (Hz)
    pub default_frequency: u32,
    /// Hệ số học lỗi thích nghi (0.0 - 1.0)
    pub learning_coefficient: f64,
    /// Đường dẫn file lưu trữ calibration profile
    pub calibration_file: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        SystemConfig {
            serial: SerialConfig {
                port: "MOCK".into(),
                baud_rate: 9600,
                data_bits: 8,
                stop_bits: 1,
                parity: "even".into(),
            },
            modbus: ModbusConfig {
                slave_address: 1,
            },
            ipc: IpcConfig {
                host: "127.0.0.1".into(),
                port: 5555,
            },
            disc: DiscConfig {
                encoder_resolution: 131_072,
                default_frequency: 16_384,
                learning_coefficient: 0.1,
                calibration_file: "calibration.json".into(),
            },
        }
    }
}

impl SystemConfig {
    /// Đọc cấu hình từ file TOML. Nếu file không tồn tại, trả về giá trị mặc định.
    pub fn load(path: &str) -> Result<Self, LoadingError> {
        if Path::new(path).exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| LoadingError::Config(format!("Cannot read config file '{}': {}", path, e)))?;
            toml::from_str(&content)
                .map_err(|e| LoadingError::Config(format!("Invalid config format in '{}': {}", path, e)))
        } else {
            tracing::warn!("Config file '{}' not found, using defaults", path);
            Ok(SystemConfig::default())
        }
    }

    /// Kiểm tra xem hệ thống có đang ở chế độ mô phỏng không
    pub fn is_mock_mode(&self) -> bool {
        self.serial.port.eq_ignore_ascii_case("MOCK")
    }
}
