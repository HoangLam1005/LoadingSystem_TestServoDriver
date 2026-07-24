/// =========================================================================
/// DỰ ÁN LOADING SYSTEM — BẢNG ÁNH XẠ ĐỊA CHỈ MODBUS PLC DELTA DVP-28SS2
/// =========================================================================
/// Tập trung toàn bộ địa chỉ Modbus tại một module duy nhất.
/// Loại bỏ hoàn toàn Magic Numbers rải rác trong code.
///
/// Quy tắc quy đổi địa chỉ Modbus cho PLC Delta DVP Series:
///   - M coils:         base = 0x0800 (2048)   → M{n} = 2048 + n
///   - D registers:     base = 0x1000 (4096)   → D{n} = 4096 + n
///   - D1000-D1999:     base = 0x13E8 (5096)   → D{n} = 5096 + (n - 1000)
///   - Y output coils:  base = 0x0500 (1280)

// ============================================================================
// BẢNG ÁNH XẠ COIL (M-RELAY) — Giao tiếp qua Modbus FC01/FC05
// ============================================================================

/// M0 (Modbus 2048) — Cờ cho phép hệ thống hoạt động (Servo-ON Y5)
pub const ADDR_M0_SYSTEM_ENABLE: u16 = 2048;

/// M2 (Modbus 2050) — Cờ kích hoạt Dừng Khẩn Cấp E-Stop
pub const ADDR_M2_EMERGENCY_STOP: u16 = 2050;

/// M10 (Modbus 2058) — Cờ kích hoạt lệnh băm xung mâm xoay (DDRVI)
pub const ADDR_M10_PULSE_TRIGGER: u16 = 2058;

/// M11 (Modbus 2059) — Cờ trạng thái hệ thống đang bận phát xung
pub const ADDR_M11_SYSTEM_BUSY: u16 = 2059;

/// M80 (Modbus 2128) — Cờ lỗi Watchdog Heartbeat (PLC tự SET khi mất nhịp)
pub const ADDR_M80_WATCHDOG_FAULT: u16 = 2128;

/// M100 (Modbus 2148) — Cờ chế độ tự động tuần hoàn
pub const ADDR_M100_AUTO_MODE: u16 = 2148;

// ============================================================================
// BẢNG ÁNH XẠ HOLDING REGISTER (D-REGISTER) — Giao tiếp qua Modbus FC03/FC06/FC16
// ============================================================================

/// D10-D11 (Modbus 4106) — Số xung mục tiêu 32-bit có dấu (DDRVI S1)
/// Dương = CW (quay thuận), Âm = CCW (quay ngược)
pub const ADDR_D10_TARGET_PULSES_32: u16 = 4106;

/// D14-D15 (Modbus 4110) — Tần số phát xung mục tiêu 32-bit (DDRVI S2)
pub const ADDR_D14_TARGET_SPEED_32: u16 = 4110;

/// D100 (Modbus 4196) — Bộ đếm số lọ đã xoay thành công (16-bit)
pub const ADDR_D100_BOTTLE_COUNT: u16 = 4196;

/// D1000 (Modbus 5096) — Watchdog Heartbeat Counter ghi từ Rust
pub const ADDR_D1000_WATCHDOG_HEARTBEAT: u16 = 5096;

/// D1343 (Modbus 5439) — Thời gian dốc tăng/giảm tốc Ramp Time (ms)
/// ⚠️ SỬA LỖI: Địa chỉ Modbus đúng = 5096 + (1343 - 1000) = 5439
///    Mã nguồn gốc sai: 8535 → ghi vào vùng nhớ không hợp lệ D4439!
pub const ADDR_D1343_RAMP_TIME_MS: u16 = 5439;

// ============================================================================
// GIỚI HẠN AN TOÀN PHẦN CỨNG PLC DVP-28SS2
// ============================================================================

/// Tần số phát xung tối đa cho ngõ ra Y0 của DVP-28SS2 (DVP28SS211T)
/// Datasheet: High-speed Output Y0/Y1 max 10 kHz (transistor output)
pub const MAX_SAFE_FREQUENCY_HZ: i32 = 10_000;

/// Tần số phát xung tối thiểu (tránh dừng hẳn motor gây giật)
pub const MIN_SAFE_FREQUENCY_HZ: i32 = 100;

/// Thời gian dốc tối đa cho phép (ms)
pub const MAX_RAMP_TIME_MS: u16 = 5000;

/// Thời gian dốc tối thiểu (ms)
pub const MIN_RAMP_TIME_MS: u16 = 10;

// ============================================================================
// CẤU HÌNH TRUYỀN THÔNG MODBUS RS-485
// ============================================================================

/// Baudrate chuẩn hóa (SỬA LỖI: nâng từ 9600 lên 38400 bps)
pub const MODBUS_BAUD_RATE: u32 = 38_400;

/// Modbus Slave ID của PLC Delta
pub const MODBUS_SLAVE_ID: u8 = 1;

/// Timeout truyền thông phía Rust Master (ms)
pub const MODBUS_TIMEOUT_MS: u64 = 1000;

// ============================================================================
// CẤU HÌNH WATCHDOG HEARTBEAT
// ============================================================================

/// Chu kỳ gửi Watchdog Heartbeat từ Rust (ms)
pub const WATCHDOG_INTERVAL_MS: u64 = 400;

/// Chu kỳ poll chờ mâm xoay hoàn thành (ms) — tối ưu hóa cho RS-485
pub const POLL_INTERVAL_MS: u64 = 350;
