// =============================================================================
// LoadingSystem - Modbus RTU Client Abstraction
// =============================================================================
// Cung cấp trait ModbusInterface để trừu tượng hóa giao tiếp Modbus,
// cho phép sử dụng cả client thật (RealModbusClient) và client mô phỏng
// (MockModbusClient) cho testing.
//
// Bản đồ địa chỉ Modbus PLC Delta DVP14SS2:
//   D100 (0x1064-0x1065): Thanh ghi vị trí xung (32-bit signed)
//   D102 (0x1066-0x1067): Thanh ghi tần số xung (32-bit unsigned)
//   M0   (0x0800):        Coil kích hoạt phát xung
// =============================================================================

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::LoadingError;

// ---------------------------------------------------------------------------
// Hằng số địa chỉ Modbus
// ---------------------------------------------------------------------------

/// Địa chỉ thanh ghi vị trí xung D100 (32-bit: D100 + D101)
pub const REG_PULSE_POSITION: u16 = 0x1064;

/// Địa chỉ thanh ghi tần số xung D102 (32-bit: D102 + D103)
pub const REG_PULSE_FREQUENCY: u16 = 0x1066;

/// Địa chỉ Coil M0 - kích hoạt phát xung
pub const COIL_TRIGGER_M0: u16 = 0x0800;

// ---------------------------------------------------------------------------
// Trait trừu tượng giao tiếp Modbus
// ---------------------------------------------------------------------------

/// Trait trừu tượng giao tiếp Modbus RTU.
/// Cho phép swap giữa client thật và mock mà không thay đổi business logic.
#[async_trait]
pub trait ModbusInterface: Send {
    /// Ghi nhiều thanh ghi 16-bit liên tiếp (Modbus Function 0x10)
    async fn write_multiple_registers(
        &mut self,
        addr: u16,
        data: &[u16],
    ) -> Result<(), LoadingError>;

    /// Ghi một coil (Modbus Function 0x05)
    async fn write_single_coil(
        &mut self,
        addr: u16,
        value: bool,
    ) -> Result<(), LoadingError>;

    /// Đọc nhiều thanh ghi holding (Modbus Function 0x03)
    async fn read_holding_registers(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<u16>, LoadingError>;

    /// Đọc trạng thái coils (Modbus Function 0x01)
    async fn read_coils(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<bool>, LoadingError>;
}

// ---------------------------------------------------------------------------
// Real Modbus Client (kết nối PLC thật qua RS485)
// ---------------------------------------------------------------------------

/// Client Modbus RTU thật, giao tiếp qua cổng serial RS485.
/// Sử dụng tokio-modbus và tokio-serial cho async I/O.
pub struct RealModbusClient {
    ctx: tokio_modbus::client::Context,
}

impl RealModbusClient {
    /// Tạo kết nối Modbus RTU mới tới PLC Delta qua cổng serial với đầy đủ tham số cấu hình.
    pub async fn new_with_config(
        port_path: &str,
        baud_rate: u32,
        data_bits: u8,
        stop_bits: u8,
        parity: &str,
        slave_address: u8,
    ) -> Result<Self, LoadingError> {
        let db = match data_bits {
            5 => tokio_serial::DataBits::Five,
            6 => tokio_serial::DataBits::Six,
            7 => tokio_serial::DataBits::Seven,
            _ => tokio_serial::DataBits::Eight,
        };

        let sb = match stop_bits {
            2 => tokio_serial::StopBits::Two,
            _ => tokio_serial::StopBits::One,
        };

        let pr = match parity.to_lowercase().as_str() {
            "none" => tokio_serial::Parity::None,
            "odd" => tokio_serial::Parity::Odd,
            _ => tokio_serial::Parity::Even,
        };

        let builder = tokio_serial::new(port_path, baud_rate)
            .data_bits(db)
            .parity(pr)
            .stop_bits(sb);

        let port = tokio_serial::SerialStream::open(&builder)
            .map_err(|e| LoadingError::Serial(format!(
                "Cannot open serial port '{}' at {} baud ({},{},{}): {}",
                port_path, baud_rate, data_bits, parity, stop_bits, e
            )))?;

        let slave = tokio_modbus::slave::Slave(slave_address);
        let ctx = tokio_modbus::client::rtu::attach_slave(port, slave);

        tracing::info!(
            "Modbus RTU connected: port={}, baud={}, data_bits={}, parity={}, stop_bits={}, slave={}",
            port_path, baud_rate, data_bits, parity, stop_bits, slave_address
        );

        Ok(RealModbusClient { ctx })
    }

    /// Tạo kết nối Modbus RTU mới tới PLC Delta qua cổng serial (mặc định 8,E,1).
    pub async fn new(
        port_path: &str,
        baud_rate: u32,
        slave_address: u8,
    ) -> Result<Self, LoadingError> {
        Self::new_with_config(port_path, baud_rate, 8, 1, "even", slave_address).await
    }
}

#[async_trait]
impl ModbusInterface for RealModbusClient {
    async fn write_multiple_registers(
        &mut self,
        addr: u16,
        data: &[u16],
    ) -> Result<(), LoadingError> {
        use tokio_modbus::prelude::Writer;
        use std::time::Duration;

        let future = self.ctx.write_multiple_registers(addr, data);
        let _ = tokio::time::timeout(Duration::from_secs(15), future)
            .await
            .map_err(|_e| LoadingError::Modbus("Modbus write registers timeout (15s)".into()))?
            .map_err(|e| LoadingError::Modbus(format!(
                "Write registers failed at 0x{:04X}: {}", addr, e
            )))?;

        tracing::debug!("Wrote {} registers at 0x{:04X}: {:?}", data.len(), addr, data);
        Ok(())
    }

    async fn write_single_coil(
        &mut self,
        addr: u16,
        value: bool,
    ) -> Result<(), LoadingError> {
        use tokio_modbus::prelude::Writer;
        use std::time::Duration;

        let future = self.ctx.write_single_coil(addr, value);
        let _ = tokio::time::timeout(Duration::from_secs(15), future)
            .await
            .map_err(|_e| LoadingError::Modbus("Modbus write coil timeout (15s)".into()))?
            .map_err(|e| LoadingError::Modbus(format!(
                "Write coil failed at 0x{:04X}: {}", addr, e
            )))?;

        tracing::debug!("Wrote coil at 0x{:04X} = {}", addr, value);
        Ok(())
    }

    async fn read_holding_registers(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<u16>, LoadingError> {
        use tokio_modbus::prelude::Reader;
        use std::time::Duration;

        let future = self.ctx.read_holding_registers(addr, count);
        let result = tokio::time::timeout(Duration::from_secs(15), future)
            .await
            .map_err(|_e| LoadingError::Modbus("Modbus read registers timeout (15s)".into()))?
            .map_err(|e| LoadingError::Modbus(format!(
                "Read registers failed at 0x{:04X}: {}", addr, e
            )))?;

        let data = result.unwrap_or_default();
        tracing::debug!("Read {} registers at 0x{:04X}: {:?}", count, addr, data);
        Ok(data)
    }

    async fn read_coils(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<bool>, LoadingError> {
        use tokio_modbus::prelude::Reader;
        use std::time::Duration;

        let future = self.ctx.read_coils(addr, count);
        let result = tokio::time::timeout(Duration::from_secs(15), future)
            .await
            .map_err(|_e| LoadingError::Modbus("Modbus read coils timeout (15s)".into()))?
            .map_err(|e| LoadingError::Modbus(format!(
                "Read coils failed at 0x{:04X}: {}", addr, e
            )))?;

        let data = result.unwrap_or_default();
        tracing::debug!("Read {} coils at 0x{:04X}: {:?}", count, addr, data);
        Ok(data)
    }
}

// ---------------------------------------------------------------------------
// Mock Modbus Client (cho testing không cần phần cứng)
// ---------------------------------------------------------------------------

/// Mock Modbus client ghi nhận toàn bộ thao tác ghi/đọc để kiểm tra logic.
/// Thread-safe thông qua Arc<Mutex>.
#[derive(Clone, Debug)]
pub struct MockModbusClient {
    /// Bản đồ thanh ghi: addr → Vec<u16>
    pub registers: Arc<Mutex<HashMap<u16, Vec<u16>>>>,
    /// Bản đồ coil: addr → bool
    pub coils: Arc<Mutex<HashMap<u16, bool>>>,
    /// Đếm số lần ghi thanh ghi
    pub write_register_count: Arc<Mutex<u32>>,
    /// Đếm số lần ghi coil
    pub write_coil_count: Arc<Mutex<u32>>,
}

impl MockModbusClient {
    /// Tạo mock client mới với bộ nhớ rỗng
    pub fn new() -> Self {
        MockModbusClient {
            registers: Arc::new(Mutex::new(HashMap::new())),
            coils: Arc::new(Mutex::new(HashMap::new())),
            write_register_count: Arc::new(Mutex::new(0)),
            write_coil_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Đọc giá trị thanh ghi đã ghi (cho assertion trong tests)
    pub fn get_registers(&self, addr: u16) -> Option<Vec<u16>> {
        self.registers.lock().unwrap().get(&addr).cloned()
    }

    /// Đọc giá trị coil đã ghi (cho assertion trong tests)
    pub fn get_coil(&self, addr: u16) -> Option<bool> {
        self.coils.lock().unwrap().get(&addr).copied()
    }

    /// Lấy số lần ghi thanh ghi
    pub fn get_write_register_count(&self) -> u32 {
        *self.write_register_count.lock().unwrap()
    }

    /// Lấy số lần ghi coil
    pub fn get_write_coil_count(&self) -> u32 {
        *self.write_coil_count.lock().unwrap()
    }

    /// Tái tạo giá trị 32-bit signed từ cặp thanh ghi 16-bit (low, high)
    pub fn reconstruct_i32_from_registers(&self, addr: u16) -> Option<i32> {
        let regs = self.registers.lock().unwrap();
        if let Some(data) = regs.get(&addr) {
            if data.len() >= 2 {
                let low = data[0] as u32;
                let high = data[1] as u32;
                return Some(((high << 16) | low) as i32);
            }
        }
        None
    }

    /// Tái tạo giá trị 32-bit unsigned từ cặp thanh ghi 16-bit
    pub fn reconstruct_u32_from_registers(&self, addr: u16) -> Option<u32> {
        let regs = self.registers.lock().unwrap();
        if let Some(data) = regs.get(&addr) {
            if data.len() >= 4 {
                let f_low = data[2] as u32;
                let f_high = data[3] as u32;
                return Some((f_high << 16) | f_low);
            }
        }
        None
    }
}

impl Default for MockModbusClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModbusInterface for MockModbusClient {
    async fn write_multiple_registers(
        &mut self,
        addr: u16,
        data: &[u16],
    ) -> Result<(), LoadingError> {
        self.registers.lock().unwrap().insert(addr, data.to_vec());
        *self.write_register_count.lock().unwrap() += 1;
        Ok(())
    }

    async fn write_single_coil(
        &mut self,
        addr: u16,
        value: bool,
    ) -> Result<(), LoadingError> {
        self.coils.lock().unwrap().insert(addr, value);
        *self.write_coil_count.lock().unwrap() += 1;
        Ok(())
    }

    async fn read_holding_registers(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<u16>, LoadingError> {
        let regs = self.registers.lock().unwrap();
        if let Some(data) = regs.get(&addr) {
            Ok(data[..std::cmp::min(count as usize, data.len())].to_vec())
        } else {
            Ok(vec![0; count as usize])
        }
    }

    async fn read_coils(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<bool>, LoadingError> {
        let mut coils = self.coils.lock().unwrap();
        let mut result = Vec::with_capacity(count as usize);
        for i in 0..count {
            let target_addr = addr + i;
            if target_addr == COIL_TRIGGER_M0 {
                // Trong môi trường mock synchronous, băm xung kết thúc tức thì nên M0 rảnh (false)
                result.push(false);
                coils.insert(target_addr, false);
            } else {
                result.push(*coils.get(&target_addr).unwrap_or(&false));
            }
        }
        Ok(result)
    }
}
