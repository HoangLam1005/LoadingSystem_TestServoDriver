mod modbus;
mod modbus_registers;
mod test_cases;

use std::error::Error;
use std::io::{self, Write};
use modbus::DeltaPlc;
use modbus_registers::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("=========================================================");
    println!("   HỆ THỐNG KIỂM THỬ ĐỘNG CƠ RUST + PLC DELTA (v2.0)   ");
    println!("   PLC: DVP-28SS2 | Driver: CSD7 | Baudrate: 38400 bps  ");
    println!("=========================================================");

    // 1. Nhập cổng COM từ người dùng
    print!("Vui lòng nhập cổng COM kết nối PLC (ví dụ: COM4 hoặc /dev/ttyUSB0): ");
    io::stdout().flush()?;
    let mut port_name = String::new();
    io::stdin().read_line(&mut port_name)?;
    let port_name = port_name.trim();

    // 2. Kết nối Modbus PLC
    let mut plc = match DeltaPlc::connect(port_name).await {
        Ok(client) => client,
        Err(e) => {
            eprintln!("[ERROR] Không thể kết nối tới PLC trên cổng {}: {}", port_name, e);
            eprintln!("Vui lòng kiểm tra lại dây cáp, cổng COM hoặc trạng thái nguồn của PLC.");
            return Ok(());
        }
    };

    // Khởi tạo trạng thái ban đầu: Gỡ dừng khẩn cấp, cho phép hệ thống hoạt động
    println!("[INIT] Gỡ dừng khẩn cấp M2 = OFF...");
    plc.write_coil(ADDR_M2_EMERGENCY_STOP, false).await?;
    println!("[INIT] Bật cờ cho phép chạy M0 = ON...");
    plc.write_coil(ADDR_M0_SYSTEM_ENABLE, true).await?;

    let default_speed = 10000; // SỬA LỖI: Tốc độ mặc định 10 kHz (max cho DVP-28SS2)

    // 3. Vòng lặp giao diện Menu tương tác
    loop {
        println!("\n=========================================================");
        println!("       MENU KIỂM THỬ PLC & MOTOR SERVO CSD7 (v2.0)      ");
        println!("=========================================================");
        println!(" [ NHÓM A: KIỂM THỬ KẾT NỐI VÀ SERVO-ON (SETUP & SAFETY) ]");
        println!("   [1] Test truyền thông Modbus RTU (Ghi/Đọc thanh ghi D100)");
        println!("   [2] Test khóa/nhả trục Servo-ON (Kích Y5 và Dừng khẩn M2)");
        println!("");
        println!(" [ NHÓM B: KIỂM THỬ CHUYỂN ĐỘNG THỦ CÔNG (MANUAL CONTROL) ]");
        println!("   [3] Test JOG quay liên tục (phím Enter để dừng)");
        println!("   [4] Test chọn chiều quay và góc quay chủ động (30°, 90°, 180°, 360°)");
        println!("");
        println!(" [ NHÓM C: KIỂM THỬ ĐỘNG HỌC NÂNG CAO (DYNAMIC & SPEED) ]");
        println!("   [5] Test dốc tăng/giảm tốc mềm (Gia tốc dốc D1343)");
        println!("   [6] Test quét tốc độ tần số phát xung (1 kHz -> 5 kHz -> 10 kHz)");
        println!("   [7] Test đảo chiều quay thuận/nghịch 30° đột ngột");
        println!("");
        println!(" [ NHÓM D: KIỂM THỬ TỰ ĐỘNG TUẦN HOÀN (RELIABILITY) ]");
        println!("   [8] Test chu kỳ tự động tuần hoàn xoay 12 lần (xoay 30°, dừng 1s)");
        println!("");
        println!(" [ NHÓM E: HỆ THỐNG AN TOÀN LIÊN ĐỘNG (EMERGENCY CONTROL) ]");
        println!("   [9] KÍCH HOẠT DỪNG KHẨN CẤP NGAY LẬP TỨC (M2 = ON, Ngắt Y5)");
        println!("   [10] NHẢ TRẠNG THÁI DỪNG KHẨN CẤP (M2 = OFF, Khôi phục)");
        println!("   [0] Thoát chương trình");
        println!("=========================================================");
        print!("Nhập lựa chọn của bạn (0-10): ");
        io::stdout().flush()?;

        let mut choice = String::new();
        io::stdin().read_line(&mut choice)?;
        let choice = choice.trim();

        match choice {
            "1" => {
                if let Err(e) = test_cases::run_modbus_loopback_test(&mut plc).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "2" => {
                if let Err(e) = test_cases::run_servo_on_off_test(&mut plc).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "3" => {
                if let Err(e) = test_cases::run_continuous_rotation_test(&mut plc, default_speed).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "4" => {
                if let Err(e) = test_cases::run_direction_selection_test(&mut plc, default_speed).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "5" => {
                if let Err(e) = test_cases::run_acceleration_ramp_test(&mut plc, default_speed).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "6" => {
                if let Err(e) = test_cases::run_speed_sweep_test(&mut plc).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "7" => {
                if let Err(e) = test_cases::run_reversal_test(&mut plc, default_speed).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "8" => {
                if let Err(e) = test_cases::run_auto_cycle_test(&mut plc, default_speed).await {
                    eprintln!("[TEST ERROR] {}", e);
                }
            }
            "9" => {
                println!("[ALERT] >>> KÍCH HOẠT DỪNG KHẨN CẤP M2 = ON! <<<");
                if let Err(e) = plc.write_coil(ADDR_M2_EMERGENCY_STOP, true).await {
                    eprintln!("[ERROR] Không thể gửi lệnh dừng khẩn: {}", e);
                } else {
                    println!("[ALERT] Đã khóa băm xung Y0 và ngắt cổng Servo-ON Y5!");
                }
            }
            "10" => {
                println!("[INFO] Giải phóng trạng thái dừng khẩn cấp M2 = OFF...");
                if let Err(e) = plc.write_coil(ADDR_M2_EMERGENCY_STOP, false).await {
                    eprintln!("[ERROR] Không thể gửi lệnh nhả dừng khẩn: {}", e);
                } else {
                    println!("[INFO] Đã khôi phục trạng thái sẵn sàng.");
                    // Bật lại cờ cho phép hệ thống sau khi nhả E-Stop
                    plc.write_coil(ADDR_M0_SYSTEM_ENABLE, true).await?;
                }
            }
            "0" => {
                println!("Đang thoát chương trình kiểm thử. Tạm biệt!");
                break;
            }
            _ => {
                println!("Lựa chọn không hợp lệ, vui lòng chọn từ 0 đến 10.");
            }
        }
    }

    Ok(())
}
