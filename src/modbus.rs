use std::error::Error;
use std::time::Duration;
use tokio_serial::SerialStream;
use tokio_modbus::prelude::*;
use tokio_modbus::client::rtu;

use crate::modbus_registers::*;

pub struct DeltaPlc {
    ctx: tokio_modbus::client::Context,
}

impl DeltaPlc {
    /// Kết nối đến Delta PLC qua cổng Serial RS-485
    /// SỬA LỖI: Nâng Baudrate từ 9600 lên 38400 bps (khớp PLC D1120 = H87)
    /// SỬA LỖI: Tăng Timeout từ 500ms lên 1000ms (khớp PLC D1121 = K30)
    pub async fn connect(port_name: &str) -> Result<Self, Box<dyn Error>> {
        println!(
            "[MODBUS] Đang kết nối PLC Delta DVP-28SS2 trên {} ({} bps)...",
            port_name, MODBUS_BAUD_RATE
        );

        let settings = tokio_serial::new(port_name, MODBUS_BAUD_RATE)
            .data_bits(tokio_serial::DataBits::Eight)
            .parity(tokio_serial::Parity::None)
            .stop_bits(tokio_serial::StopBits::One)
            .timeout(Duration::from_millis(MODBUS_TIMEOUT_MS));

        let stream = SerialStream::open(&settings)?;
        let slave = Slave(MODBUS_SLAVE_ID);
        let ctx = rtu::attach_slave(stream, slave);

        let plc = DeltaPlc { ctx };
        println!("[MODBUS] ✓ Kết nối thành công đến Delta PLC!");
        Ok(plc)
    }

    /// Đọc trạng thái của 1 Coil (M-relay)
    pub async fn read_coil(&mut self, addr: u16) -> Result<bool, Box<dyn Error>> {
        let response = self.ctx.read_coils(addr, 1).await?;
        match response {
            Ok(coils) => Ok(*coils.first().unwrap_or(&false)),
            Err(e) => Err(format!("Lỗi đọc Coil {}: {:?}", addr, e).into()),
        }
    }

    /// Ghi trạng thái của 1 Coil (M-relay)
    pub async fn write_coil(&mut self, addr: u16, value: bool) -> Result<(), Box<dyn Error>> {
        self.ctx.write_single_coil(addr, value).await??;
        Ok(())
    }

    /// Đọc giá trị 16-bit của 1 Holding Register (D-register)
    pub async fn read_register(&mut self, addr: u16) -> Result<u16, Box<dyn Error>> {
        let response = self.ctx.read_holding_registers(addr, 1).await?;
        match response {
            Ok(regs) => Ok(*regs.first().unwrap_or(&0)),
            Err(e) => Err(format!("Lỗi đọc Register {}: {:?}", addr, e).into()),
        }
    }

    /// Ghi giá trị 16-bit vào 1 Holding Register (D-register)
    pub async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), Box<dyn Error>> {
        self.ctx.write_single_register(addr, value).await??;
        Ok(())
    }

    /// Đọc giá trị 32-bit có dấu ghép từ 2 thanh ghi liên tiếp (D-register)
    pub async fn read_register_32(&mut self, addr: u16) -> Result<i32, Box<dyn Error>> {
        let response = self.ctx.read_holding_registers(addr, 2).await?;
        match response {
            Ok(regs) => {
                if regs.len() < 2 {
                    return Err("Không đủ dữ liệu phản hồi 32-bit".into());
                }
                let low = regs[0] as u32;
                let high = regs[1] as u32;
                let combined = (high << 16) | low;
                Ok(combined as i32)
            }
            Err(e) => Err(format!("Lỗi đọc Register 32-bit {}: {:?}", addr, e).into()),
        }
    }

    /// Ghi giá trị 32-bit có dấu vào 2 thanh ghi liên tiếp (D-register)
    pub async fn write_register_32(&mut self, addr: u16, value: i32) -> Result<(), Box<dyn Error>> {
        let low = (value & 0xFFFF) as u16;
        let high = ((value >> 16) & 0xFFFF) as u16;
        let values = vec![low, high];
        self.ctx.write_multiple_registers(addr, &values).await??;
        Ok(())
    }

    /// Khởi động băm xung mâm xoay với kiểm tra giới hạn an toàn phần cứng.
    ///
    /// SỬA LỖI: Thêm clamp() giới hạn tần số max 10 kHz cho DVP-28SS2
    pub async fn trigger_rotation(&mut self, pulses: i32, speed_hz: i32) -> Result<(), Box<dyn Error>> {
        // Kiểm tra giới hạn an toàn phần cứng DVP-28SS2
        let clamped_speed = speed_hz.clamp(MIN_SAFE_FREQUENCY_HZ, MAX_SAFE_FREQUENCY_HZ);
        if clamped_speed != speed_hz {
            println!(
                "[WARN] Tốc độ {} Hz vượt giới hạn phần cứng DVP-28SS2 (max {} Hz), tự động điều chỉnh về {} Hz",
                speed_hz, MAX_SAFE_FREQUENCY_HZ, clamped_speed
            );
        }

        // Ghi số xung 32-bit vào D10-D11
        self.write_register_32(ADDR_D10_TARGET_PULSES_32, pulses).await?;
        // Ghi tốc độ 32-bit vào D14-D15
        self.write_register_32(ADDR_D14_TARGET_SPEED_32, clamped_speed).await?;
        // Bật cờ M10 kích hoạt phát xung
        self.write_coil(ADDR_M10_PULSE_TRIGGER, true).await?;
        Ok(())
    }
}
