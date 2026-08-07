// =============================================================================
// LoadingSystem - Terminal CLI Controller (Điều khiển Servo qua Terminal)
// =============================================================================
// Giao tiếp thẳng: Rust Terminal --> COM5 (RS-485) --> PLC Delta --> Servo
// KHÔNG cần Python HMI, KHÔNG cần IPC Server.
//
// Chạy:
//   cargo run --bin terminal-control
//   cargo run --bin terminal-control -- config.toml
// =============================================================================

use loading_system::calibration::profile::CalibrationProfile;
use loading_system::config::SystemConfig;
use loading_system::hardware::modbus_client::{MockModbusClient, ModbusInterface, RealModbusClient};
use loading_system::hardware::rotating_disc::{Direction, RotatingDiscController};

use std::io::{self, Write};

/// Đọc 1 dòng input từ terminal
fn read_line(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

/// Chạy vòng lặp điều khiển terminal với controller generic
async fn run_terminal<M: ModbusInterface + 'static>(
    mut controller: RotatingDiscController<M>,
    calibration_file: String,
) {
    println!();
    println!("============================================================");
    println!("  🎮 ĐIỀU KHIỂN SERVO QUA RUST TERMINAL");
    println!("  Giao tiếp: Rust --> COM5 (RS-485) --> PLC Delta --> Servo");
    println!("  Encoder: 17-bit (131,072 xung/vòng)");
    println!("============================================================");

    loop {
        let angle = controller.get_current_angle();
        let cal = controller.get_calibration();
        let offset = cal.offset_pulses;

        println!();
        println!("──────────────────────────────────────────────────────────");
        println!("  📍 Vị trí hiện tại: {:.2}° | Offset bù: {} xung", angle, offset);
        println!("──────────────────────────────────────────────────────────");
        println!("  [1] Quay PHẢI (nhập góc)");
        println!("  [2] Quay TRÁI (nhập góc)");
        println!("  [3] Quay VỀ GỐC 0°");
        println!("  [4] Quay PHẢI 90° (nhanh)");
        println!("  [5] Quay TRÁI 90° (nhanh)");
        println!("  [6] Quay PHẢI 30° (nhanh)");
        println!("  [7] Kiểm tra trạng thái (M0 đang di chuyển?)");
        println!("  [8] Fine Tune hiệu chỉnh (Level 5)");
        println!("  [9] Xem thông tin Calibration");
        println!("  [0] Thoát");
        println!("──────────────────────────────────────────────────────────");

        let choice = read_line("  👉 Chọn lệnh: ");

        match choice.as_str() {
            "1" => {
                let deg_str = read_line("  Nhập góc quay PHẢI (độ): ");
                match deg_str.parse::<f64>() {
                    Ok(deg) if deg > 0.0 => {
                        println!("\n  ➡️  Quay PHẢI {:.2}°...", deg);
                        match controller.rotate_degrees(deg, Direction::Right).await {
                            Ok(()) => println!("  ✅ Hoàn thành! Góc hiện tại: {:.2}°", controller.get_current_angle()),
                            Err(e) => println!("  ❌ Lỗi: {}", e),
                        }
                    }
                    _ => println!("  ❌ Giá trị không hợp lệ! Nhập số dương."),
                }
            }
            "2" => {
                let deg_str = read_line("  Nhập góc quay TRÁI (độ): ");
                match deg_str.parse::<f64>() {
                    Ok(deg) if deg > 0.0 => {
                        println!("\n  ⬅️  Quay TRÁI {:.2}°...", deg);
                        match controller.rotate_degrees(deg, Direction::Left).await {
                            Ok(()) => println!("  ✅ Hoàn thành! Góc hiện tại: {:.2}°", controller.get_current_angle()),
                            Err(e) => println!("  ❌ Lỗi: {}", e),
                        }
                    }
                    _ => println!("  ❌ Giá trị không hợp lệ! Nhập số dương."),
                }
            }
            "3" => {
                println!("\n  🏠 Quay VỀ GỐC từ {:.2}°...", controller.get_current_angle());
                match controller.return_to_origin().await {
                    Ok(()) => println!("  ✅ Đã về gốc 0°!"),
                    Err(e) => println!("  ❌ Lỗi: {}", e),
                }
            }
            "4" => {
                println!("\n  ➡️  Quay PHẢI 90°...");
                match controller.rotate_degrees(90.0, Direction::Right).await {
                    Ok(()) => println!("  ✅ Hoàn thành! Góc hiện tại: {:.2}°", controller.get_current_angle()),
                    Err(e) => println!("  ❌ Lỗi: {}", e),
                }
            }
            "5" => {
                println!("\n  ⬅️  Quay TRÁI 90°...");
                match controller.rotate_degrees(90.0, Direction::Left).await {
                    Ok(()) => println!("  ✅ Hoàn thành! Góc hiện tại: {:.2}°", controller.get_current_angle()),
                    Err(e) => println!("  ❌ Lỗi: {}", e),
                }
            }
            "6" => {
                println!("\n  ➡️  Quay PHẢI 30°...");
                match controller.rotate_degrees(30.0, Direction::Right).await {
                    Ok(()) => println!("  ✅ Hoàn thành! Góc hiện tại: {:.2}°", controller.get_current_angle()),
                    Err(e) => println!("  ❌ Lỗi: {}", e),
                }
            }
            "7" => {
                println!("\n  🔍 Kiểm tra trạng thái cờ M0...");
                match controller.is_moving().await {
                    Ok(moving) => {
                        if moving {
                            println!("  📊 M0 = ON → Đĩa xoay ĐANG phát xung (đang di chuyển)!");
                        } else {
                            println!("  📊 M0 = OFF → Đĩa xoay đã DỪNG (sẵn sàng nhận lệnh mới).");
                        }
                    }
                    Err(e) => println!("  ❌ Lỗi đọc M0: {}", e),
                }
            }
            "8" => {
                let deg_str = read_line("  Nhập góc hiệu chỉnh Fine Tune (dương=phải, âm=trái): ");
                match deg_str.parse::<f64>() {
                    Ok(deg) => {
                        controller.inject_manual_override(deg);
                        let cal = controller.get_calibration();
                        println!("  ✅ Đã cập nhật offset bù: {} xung (Số lần học: {})",
                            cal.offset_pulses, cal.history.len());
                        // Lưu calibration
                        if let Err(e) = cal.save_to_file(&calibration_file) {
                            println!("  ⚠️ Không thể lưu calibration: {}", e);
                        } else {
                            println!("  💾 Đã lưu calibration vào '{}'", calibration_file);
                        }
                    }
                    _ => println!("  ❌ Giá trị không hợp lệ!"),
                }
            }
            "9" => {
                let cal = controller.get_calibration();
                println!();
                println!("  ┌─────────────────────────────────────────┐");
                println!("  │  📊 THÔNG TIN CALIBRATION (Level 5)     │");
                println!("  ├─────────────────────────────────────────┤");
                println!("  │  Offset bù:        {:>8} xung        │", cal.offset_pulses);
                println!("  │  Hệ số học:         {:>8.4}            │", cal.learning_coefficient);
                println!("  │  Số lần hiệu chuẩn: {:>5}              │", cal.history.len());
                println!("  │  Góc hiện tại:     {:>8.2}°            │", controller.get_current_angle());
                println!("  │  Encoder:           131,072 xung/vòng  │");
                println!("  └─────────────────────────────────────────┘");
            }
            "0" => {
                println!("\n  👋 Thoát chương trình! Goodbye!");
                break;
            }
            _ => println!("  ❌ Lệnh không hợp lệ!"),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Khởi tạo tracing logger
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    println!("=== LoadingSystem Terminal Controller ===");

    // Đọc config file
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".into());

    let config = SystemConfig::load(&config_path)?;
    println!("Config loaded from '{}'", config_path);

    // Load calibration profile
    let calibration = CalibrationProfile::load_from_file(
        &config.disc.calibration_file,
        config.disc.learning_coefficient,
    )?;

    let calibration_file = config.disc.calibration_file.clone();

    if config.is_mock_mode() {
        println!("Mode: MOCK (mô phỏng, không cần PLC)");
        let mock_client = MockModbusClient::new();
        let controller = RotatingDiscController::new(
            mock_client,
            config.disc.encoder_resolution,
            config.disc.default_frequency,
            calibration,
        );
        run_terminal(controller, calibration_file).await;
    } else {
        println!("Mode: REAL (kết nối PLC qua {})", config.serial.port);
        println!("Serial: {} baud, {},{},{}", 
            config.serial.baud_rate, config.serial.data_bits, config.serial.parity, config.serial.stop_bits);

        let real_client = RealModbusClient::new_with_config(
            &config.serial.port,
            config.serial.baud_rate,
            config.serial.data_bits,
            config.serial.stop_bits,
            &config.serial.parity,
            config.modbus.slave_address,
        )
        .await?;

        let controller = RotatingDiscController::new(
            real_client,
            config.disc.encoder_resolution,
            config.disc.default_frequency,
            calibration,
        );
        run_terminal(controller, calibration_file).await;
    }

    Ok(())
}
