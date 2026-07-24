use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;
use crate::modbus::DeltaPlc;
use crate::modbus_registers::*;

/// Hàm chờ mâm xoay hoàn thành với xử lý chống Tranh chấp Trạng thái (Race Condition).
///
/// SỬA LỖI: Thêm giai đoạn 1 — chờ PLC chuyển sang trạng thái Busy trước
/// khi bắt đầu polling. Mã gốc đọc M10 ngay lập tức, khi Timer T0 PLC
/// chưa kịp SET M10, hàm lập tức return Ok(()) trong khi mâm chưa quay.
pub async fn wait_for_rotation_done(plc: &mut DeltaPlc, timeout_secs: u64) -> Result<(), Box<dyn Error>> {
    let start_time = std::time::Instant::now();

    // Giai đoạn 1: Chờ PLC chuyển sang trạng thái Busy (M10 hoặc M11 = ON)
    // Timeout chờ: 2 giây (đủ cho Timer T0 K10 = 1 giây + margin)
    let mut started = false;
    for _ in 0..20 {
        let m10_active = plc.read_coil(ADDR_M10_PULSE_TRIGGER).await.unwrap_or(false);
        let m11_busy = plc.read_coil(ADDR_M11_SYSTEM_BUSY).await.unwrap_or(false);
        if m10_active || m11_busy {
            started = true;
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    if !started {
        println!("[WARN] PLC không chuyển sang trạng thái bận sau 2 giây.");
    }

    // Giai đoạn 2: Chờ cho đến khi cờ M10 ngắt hẳn (mâm xoay hoàn thành)
    loop {
        let is_pulsing = plc.read_coil(ADDR_M10_PULSE_TRIGGER).await?;
        if !is_pulsing {
            return Ok(());
        }

        if start_time.elapsed() > Duration::from_secs(timeout_secs) {
            return Err("TIMEOUT: Quá thời gian chờ mâm xoay hoàn thành vị trí!".into());
        }

        // SỬA LỖI: Tăng chu kỳ poll từ 50ms lên 350ms để giảm tải bus RS-485
        sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

/// =========================================================================
/// [ NHÓM A: KIỂM THỬ KẾT NỐI VÀ SERVO-ON (SETUP & SAFETY) ]
/// =========================================================================

/// [Test Case 1] Kiểm thử truyền thông ghi đọc Modbus RTU
pub async fn run_modbus_loopback_test(plc: &mut DeltaPlc) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 1] KIỂM THỬ TRUYỀN THÔNG MODBUS RTU ===");
    let test_values = vec![1234, 5678, 9012, 3456, 7890];
    let mut pass_count = 0;

    for val in test_values {
        println!("[TEST] Ghi thử giá trị {} vào D100 (Modbus {})...", val, ADDR_D100_BOTTLE_COUNT);
        plc.write_register(ADDR_D100_BOTTLE_COUNT, val).await?;
        sleep(Duration::from_millis(50)).await;

        let read_val = plc.read_register(ADDR_D100_BOTTLE_COUNT).await?;
        println!("[TEST] Đọc ngược lại từ D100: {}", read_val);
        if read_val == val {
            println!("  -> [OK]");
            pass_count += 1;
        } else {
            println!("  -> [FAIL] Sai lệch dữ liệu!");
        }
        sleep(Duration::from_millis(100)).await;
    }

    if pass_count == 5 {
        println!("==> KẾT QUẢ TEST CASE 1: [PASS] (Đường truyền Modbus hoạt động ổn định và chính xác)");
    } else {
        println!("==> KẾT QUẢ TEST CASE 1: [FAIL] (Có lỗi hoặc mất gói tin trên đường truyền!)");
    }
    Ok(())
}

/// [Test Case 2] Kiểm thử khóa/nhả trục Servo-ON
pub async fn run_servo_on_off_test(plc: &mut DeltaPlc) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 2] KIỂM THỬ KHÓA/NHẢ TRỤC SERVO-ON ===");

    // Gỡ dừng khẩn cấp trước nếu có
    plc.write_coil(ADDR_M2_EMERGENCY_STOP, false).await?;

    println!("[TEST] Kích hoạt cờ cho phép chạy M0 = ON (Servo-ON)...");
    plc.write_coil(ADDR_M0_SYSTEM_ENABLE, true).await?;
    println!("  -> Kiểm tra: Trục motor phải đang BỊ KHÓA CỨNG (Không thể xoay tay).");
    println!("  -> Màn hình Driver hiển thị: F.run");
    sleep(Duration::from_secs(5)).await;

    println!("[TEST] Kích hoạt DỪNG KHẨN CẤP M2 = ON...");
    plc.write_coil(ADDR_M2_EMERGENCY_STOP, true).await?;
    println!("  -> Kiểm tra: Trục motor phải được NHẢ LỎNG (Có thể dùng tay xoay nhẹ mâm dễ dàng).");
    println!("  -> Màn hình Driver hiển thị: F.rdy");
    sleep(Duration::from_secs(5)).await;

    // Trả lại trạng thái bình thường
    plc.write_coil(ADDR_M2_EMERGENCY_STOP, false).await?;
    plc.write_coil(ADDR_M0_SYSTEM_ENABLE, true).await?;
    println!("==> KẾT QUẢ TEST CASE 2: Hoàn thành. Hãy xác nhận xem trục đã khóa/nhả đúng quy trình chưa.");
    Ok(())
}

/// =========================================================================
/// [ NHÓM B: KIỂM THỬ CHUYỂN ĐỘNG THỦ CÔNG (MANUAL CONTROL) ]
/// =========================================================================

/// [Test Case 3] Kiểm thử quay liên tục (Continuous Rotation)
pub async fn run_continuous_rotation_test(plc: &mut DeltaPlc, speed_hz: i32) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 3] KIỂM THỬ MOTOR QUAY LIÊN TỤC ===");
    println!("[TEST] Kích hoạt quay thuận liên tục (Phát 999,999 xung)...");

    plc.trigger_rotation(999999, speed_hz).await?;
    println!("Motor đang quay liên tục. Bạn hãy nhấn Enter trên bàn phím để phát lệnh dừng...");

    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    println!("[TEST] Nhả cờ chạy M10 để dừng quay...");
    plc.write_coil(ADDR_M10_PULSE_TRIGGER, false).await?;

    sleep(Duration::from_secs(1)).await;
    println!("==> KẾT QUẢ TEST CASE 3: Đã phát lệnh dừng.");
    Ok(())
}

/// [Test Case 4] Kiểm thử chọn chiều quay và góc quay chủ động
pub async fn run_direction_selection_test(plc: &mut DeltaPlc, speed_hz: i32) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 4] KIỂM THỬ CHỌN CHIỀU QUAY VÀ GÓC QUAY CHỦ ĐỘNG ===");

    // 1. Chọn chiều quay
    println!("Chọn chiều quay:");
    println!(" [1] Quay thuận (CW - Theo chiều kim đồng hồ)");
    println!(" [2] Quay nghịch (CCW - Ngược chiều kim đồng hồ)");
    print!("Lựa chọn của bạn (1-2): ");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut dir_choice = String::new();
    std::io::stdin().read_line(&mut dir_choice)?;
    let dir_choice = dir_choice.trim();

    let dir_sign = match dir_choice {
        "1" => 1,
        "2" => -1,
        _ => {
            println!("Lựa chọn không hợp lệ, mặc định chọn quay Thuận (CW).");
            1
        }
    };

    // 2. Chọn góc quay
    println!("\nChọn góc quay:");
    println!(" [1] 30 độ (Xoay 1 lọ - 10,000 xung)");
    println!(" [2] 90 độ (Xoay 3 lọ - 30,000 xung)");
    println!(" [3] 180 độ (Xoay 6 lọ - 60,000 xung)");
    println!(" [4] 360 độ (Xoay 12 lọ - 120,000 xung)");
    print!("Lựa chọn của bạn (1-4): ");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut angle_choice = String::new();
    std::io::stdin().read_line(&mut angle_choice)?;
    let angle_choice = angle_choice.trim();

    let base_pulses = match angle_choice {
        "1" => 10000,
        "2" => 30000,
        "3" => 60000,
        "4" => 120000,
        _ => {
            println!("Lựa chọn không hợp lệ, mặc định chọn quay 30 độ.");
            10000
        }
    };

    let target_pulses = base_pulses * dir_sign;
    let dir_text = if dir_sign > 0 { "Thuận (CW)" } else { "Nghịch (CCW)" };
    let degree_text = match base_pulses {
        10000 => "30°",
        30000 => "90°",
        60000 => "180°",
        120000 => "360°",
        _ => "30°",
    };

    println!("\n[TEST] Bắt đầu quay {} góc {} (Phát {} xung) với tốc độ {} Hz...", dir_text, degree_text, target_pulses, speed_hz);
    plc.trigger_rotation(target_pulses, speed_hz).await?;

    match wait_for_rotation_done(plc, 30).await {
        Ok(_) => {
            let count = plc.read_register(ADDR_D100_BOTTLE_COUNT).await?;
            println!("  -> [OK] Đã quay xong! Bộ đếm PLC D100: {}", count);
        }
        Err(e) => {
            println!("  -> [FAIL] Quá thời gian chờ hoặc có lỗi: {}", e);
        }
    }

    println!("==> KẾT QUẢ TEST CASE 4: Hoàn thành kịch bản kiểm thử hướng quay chủ động.");
    Ok(())
}

/// =========================================================================
/// [ NHÓM C: KIỂM THỬ ĐỘNG HỌC NÂNG CAO (DYNAMIC & SPEED) ]
/// =========================================================================

/// [Test Case 5] Kiểm thử dốc tăng/giảm tốc mềm (Ramp Time D1343)
///
/// SỬA LỖI: Ghi vào ADDR_D1343_RAMP_TIME_MS (5439) thay vì 8535
pub async fn run_acceleration_ramp_test(plc: &mut DeltaPlc, speed_hz: i32) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 5] KIỂM THỬ DỐC TĂNG/GIẢM TỐC MỀM ===");

    print!("Nhập thời gian tăng/giảm tốc mong muốn (mili-giây, ví dụ: 200): ");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut ramp_input = String::new();
    std::io::stdin().read_line(&mut ramp_input)?;
    let ramp_time: u16 = match ramp_input.trim().parse() {
        Ok(t) => t,
        Err(_) => {
            println!("Định dạng không hợp lệ, mặc định chọn 150 ms.");
            150
        }
    };

    // Kiểm tra giới hạn ramp time
    let clamped_ramp = ramp_time.clamp(MIN_RAMP_TIME_MS, MAX_RAMP_TIME_MS);
    if clamped_ramp != ramp_time {
        println!("[WARN] Ramp time {} ms ngoài dải cho phép, điều chỉnh về {} ms", ramp_time, clamped_ramp);
    }

    println!(
        "[TEST] Ghi thời gian dốc {} ms vào thanh ghi D1343 (Modbus {})...",
        clamped_ramp, ADDR_D1343_RAMP_TIME_MS
    );
    // SỬA LỖI: Địa chỉ Modbus đúng của D1343 là 5439, KHÔNG PHẢI 8535
    plc.write_register(ADDR_D1343_RAMP_TIME_MS, clamped_ramp).await?;

    println!("[TEST] Quay mâm xoay 90 độ (30,000 xung) để kiểm tra độ êm khi khởi động và dừng...");
    plc.trigger_rotation(30000, speed_hz).await?;

    match wait_for_rotation_done(plc, 15).await {
        Ok(_) => {
            println!("  -> [OK] Quay xong! Hãy quan sát xem mâm khởi động và dừng có êm ái hơn không.");
        }
        Err(e) => {
            println!("  -> [FAIL] Quá thời gian chờ: {}", e);
        }
    }

    Ok(())
}

/// [Test Case 6] Kiểm thử quét các tần số phát xung (Speed Sweep)
///
/// SỬA LỖI: Giảm dải tốc độ từ [5k, 20k, 50k] xuống [1k, 5k, 10k]
/// DVP-28SS2 transistor output chỉ hỗ trợ max 10 kHz
pub async fn run_speed_sweep_test(plc: &mut DeltaPlc) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 6] KIỂM THỬ VẬN TỐC TỪ 1 kHz ĐẾN 10 kHz ===");
    // SỬA LỖI: Tần số tối đa giảm về 10 kHz (giới hạn phần cứng DVP-28SS2)
    let speeds = vec![1000, 5000, 10000];
    let pulses = 30000; // Quay 90 độ

    for hz in speeds {
        println!("\n[TEST] Quay góc 90 độ với tốc độ {} Hz...", hz);
        plc.trigger_rotation(pulses, hz).await?;

        let start_time = std::time::Instant::now();
        wait_for_rotation_done(plc, 30).await?;
        let elapsed = start_time.elapsed().as_secs_f64();

        println!("  -> Quay xong! Thời gian thực tế: {:.2} giây", elapsed);
        sleep(Duration::from_secs(2)).await;
    }

    println!("==> KẾT QUẢ TEST CASE 6: Hoàn thành chuỗi quét tốc độ.");
    Ok(())
}

/// [Test Case 7] Kiểm thử đảo chiều quay đột ngột
pub async fn run_reversal_test(plc: &mut DeltaPlc, speed_hz: i32) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 7] KIỂM THỬ ĐẢO CHIỀU QUAY ĐỘT NGỘT ===");
    let pulses = 10000; // 30 độ

    for i in 1..=3 {
        println!("[Vòng {}/3] Quay thuận 30 độ...", i);
        plc.trigger_rotation(pulses, speed_hz).await?;
        wait_for_rotation_done(plc, 10).await?;

        println!("[Vòng {}/3] Quay ngược 30 độ lập tức...", i);
        plc.trigger_rotation(-pulses, speed_hz).await?;
        wait_for_rotation_done(plc, 10).await?;

        sleep(Duration::from_millis(500)).await;
    }

    println!("==> KẾT QUẢ TEST CASE 7: Hoàn thành test đảo chiều đột ngột.");
    Ok(())
}

/// =========================================================================
/// [ NHÓM D: KIỂM THỬ TỰ ĐỘNG TUẦN HOÀN (RELIABILITY) ]
/// =========================================================================

/// [Test Case 8] Kiểm thử chạy tự động tuần hoàn xoay 30 độ dừng 1s (lặp 12 lần)
pub async fn run_auto_cycle_test(plc: &mut DeltaPlc, speed_hz: i32) -> Result<(), Box<dyn Error>> {
    println!("\n=== [TEST CASE 8] KIỂM THỬ CHU KỲ TỰ ĐỘNG TUẦN HOÀN ===");
    println!("Chương trình sẽ tự động xoay 12 lần, mỗi lần 30 độ, dừng 1 giây giữa các lọ.");
    println!("Vui lòng đánh dấu vạch xuất phát trên mâm để đối chiếu sai số sau 1 vòng quay.");

    // Reset bộ đếm số lọ trên PLC về 0 trước khi chạy
    plc.write_register(ADDR_D100_BOTTLE_COUNT, 0).await?;

    for step in 1..=12 {
        println!("[Bước {}/12] Xoay mâm 30 độ...", step);
        plc.trigger_rotation(10000, speed_hz).await?;

        wait_for_rotation_done(plc, 10).await?;
        let count = plc.read_register(ADDR_D100_BOTTLE_COUNT).await?;
        println!("  -> Xoay xong bước {}. Bộ đếm PLC D100: {}", step, count);

        sleep(Duration::from_secs(1)).await;
    }

    println!("\n==> KẾT QUẢ TEST CASE 8: Hoàn thành 1 vòng tuần hoàn 12 bước.");
    println!("Hãy kiểm tra xem vạch dấu trên mâm xoay có trùng khớp hoàn hảo với điểm xuất phát ban đầu không.");
    Ok(())
}
