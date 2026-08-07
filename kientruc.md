# ĐẶC TẢ KIẾN TRÚC HỆ THỐNG VÀ KỊCH BẢN KIỂM THỬ KHỐI ĐĨA XOAY (LOADINGSYSTEM)

Tài liệu này định hình toàn bộ cấu trúc phân lớp phần mềm, giải pháp đấu nối phần cứng và kịch bản thực thi thực tế cho khối cơ cấu chấp hành đĩa xoay (Module 3) thuộc hệ thống tự động hóa LoadingSystem. Mục tiêu của tài liệu này là làm bản thiết kế kỹ thuật (Blueprint) có độ chi tiết cao, logic chặt chẽ để nạp trực tiếp vào các mô-đun AI Code Generation (Claude/Gemini Developer) sinh ra mã nguồn production-ready 100% không phát sinh lỗi hệ thống.

---

## THÔNG TIN CHUNG
* **Dự án:** Hệ thống Phân Loại Tự Động & Xử Lý Hạt Điều Công Nghiệp (LoadingSystem)
* **Thành phần đặc tả:** Subsystem Đĩa Xoay Định Vị Vật Thể (Rotating Disc)
* **Tác giả:** Trương Hoàng Nam (Computer Engineering - HCMUT)
* **Nền tảng phần cứng:** PLC Delta DVP14SS2 (Transistor NPN), Servo Driver RS Automation CSD7 (17-bit Serial Encoder)
* **Lớp phần mềm:** HMI Python (customtkinter) -> Backend Engine (Rust Tokio Daemon) -> Modbus RTU over RS485 -> PLC Delta Core Firmware

---

## PHẦN I: KHUNG THAM CHIẾU NĂNG LỰC ĐĨA XOAY (5-LEVEL SYSTEM ROADMAP)

Dựa trên yêu cầu đánh giá thực tập từ kỹ sư trưởng Lê Ngọc Thạch, hệ thống đĩa xoay được phát triển và kiểm chứng tuần tự qua 5 cấp độ nghiệp vụ từ cơ bản đến nâng cao:

```
[Level 1: Pure PLC Code] ──> [Level 2: Clean Rust Library] ──> [Level 3: Cross-Platform Enterprise API]
                                                                        │
[Level 5: Closed-Loop Self-Learning] <── [Level 4: Expert Manual Override] ┘
```

* **Level 1: Điều khiển xoay phải, xoay trái độc lập theo mã nguồn PLC nội bộ.**
  * *Mục tiêu:* Kiểm thử cô lập phần cứng. Toàn bộ logic kích chạy, đảo hướng và định vị xung được thực hiện trực tiếp bằng tập lệnh Ladder Logic trên bộ nhớ Flash của PLC Delta DVP14SS2 thông qua phần mềm WPLSoft. Không phụ thuộc vào kết nối máy tính hay vi điều khiển biên bên ngoài.
* **Level 2: Xây dựng thư viện điều khiển tối giản, mã nguồn sạch bằng ngôn ngữ Rust.**
  * *Mục tiêu:* Thiết kế tư duy kiểm chứng (Validation Mindset) trực quan, tường minh, có cấu trúc module rõ ràng để các nhóm phát triển độc lập có thể dễ dàng review. Thư viện Rust phải cung cấp đầy đủ các hàm nguyên thủy (Primitives) bao gồm: Quay trái góc $	heta$ độ, Quay phải góc $	heta$ độ, và Đưa đĩa xoay về vị trí Home (vị trí gốc 0 độ).
* **Level 3: Xây dựng tầng logic doanh nghiệp (Enterprise Business Logic) bằng Rust Backend và cung cấp hệ thống API đồng bộ.**
  * *Mục tiêu:* Đóng gói toàn bộ nghiệp vụ xử lý bất đồng bộ, lập lịch hàng đợi xung và quản lý tài nguyên cổng serial thành dịch vụ chạy nền. Cung cấp hệ thống API chuẩn (gRPC hoặc Web API nội bộ) để các nhóm lập trình ứng dụng Mobile và giao diện HMI Web/Desktop có thể gọi hàm thực thi trực tiếp từ xa.
* **Level 4: Tích hợp cơ chế can thiệp thủ công thời gian thực (Expert Manual Override).**
  * *Mục tiêu:* Khi hệ thống chạy kiểm thử tổng thể, chuyên gia vận hành quan sát chuyển động vật lý của đĩa xoay. Nếu phát hiện đĩa xoay hoạt động sai lệch vị trí do quán tính cơ khí, sai số chế tạo hoặc tích tụ xung, chuyên gia có khả năng bấm nút can thiệp trực tiếp bằng tay trên giao diện HMI để điều chỉnh bù góc đĩa xoay về đúng vị trí đích ngay lập tức.
* **Level 5: Phát triển thuật toán tự học khép kín (Closed-Loop Self-Learning & Auto-Calibration).**
  * *Mục tiêu:* Hệ thống sở hữu khả năng tự động học và ghi nhận hành vi sai lệch. Khi xảy ra sự kiện điều chỉnh bằng tay ở Level 4, thuật toán nền sẽ tự động phân tích độ lệch điện tử ($\Delta	heta$), tính toán lại sai số phân phối xung thực tế và tự động cập nhật hệ số bù (Offset Calibration Factor) vào bộ nhớ vĩnh viễn, bảo đảm ở các chu kỳ hoạt động tiếp theo đĩa xoay sẽ tự động chạy đúng mà không cần con người can thiệp lại.

---

## PHẦN II: THIẾT KẾ PHẦN CỨNG VÀ MÔ HÌNH TOÁN HỌC ĐĨA XOAY

### 1. Sơ đồ ánh xạ chân phần cứng vật lý (Physical Pin Mapping)
Để đảm bảo tính đồng bộ tuyệt đối khi chuyển đổi thiết bị từ DVP28SS2 sang dòng Slim DVP14SS2 ngõ ra Transistor NPN, sơ đồ đấu nối dây vật lý sang cổng 50-pin của Servo Driver CSD7 tuân thủ cấu trúc sau:

* **PLC Delta DVP14SS2 Terminal:**
  * **UP (+24VDC ngõ ra):** Nối vào cực dương **V+** của bộ nguồn tổ ong 24V chung.
  * **ZP (0V ngõ ra):** Nối vào cực âm **V- (GND)** của bộ nguồn tổ ong 24V chung.
  * **Ngõ ra Y0 (High-Speed Pulse):** Nối vào **Chân số 12 (PULS-)** trên cổng DB50 Servo để phát chuỗi xung.
  * **Ngõ ra Y1 (Direction Signal):** Nối vào **Chân số 14 (SIGN-)** trên cổng DB50 Servo để điều khiển hướng quay.
  * **Ngõ ra Y5 (Servo-ON Trigger):** Nối vào **Chân số 3 (INPUT1 / SV-ON)** trên cổng DB50 Servo để giữ cứng trục động cơ.

* **Servo Driver CSD7 Terminal:**
  * **Chân số 1 (Dây Đỏ):** Nối vào cực dương **V+** của nguồn tổ ong 24V.
  * **Chân số 25 (24V SIGN+) và Chân số 49 (24V PULS+):** Đã được nối tắt nội bộ bằng dây màu Vàng vào Chân số 1 để nhận nguồn dương qua điện trở hạn dòng nội bộ $2\,	ext{k}\Omega$.
  * **Chân số 10 (E-STOP):** Hàn chụm xuống 0V (GND) để bypass vòng bảo vệ an toàn trong pha thử nghiệm.

### 2. Mô hình toán học quy đổi Xung - Góc quay (Pulse-to-Degree Physics Model)
Động cơ Servo CSD7 sử dụng bộ mã hóa vòng quay có độ phân giải 17-bit, tương đương với $131,072\,	ext{xung}$ trên một vòng quay tuyệt đối của cốt motor ($360^\circ$).

Gọi $	heta$ là góc cần quay của đĩa xoay (tính bằng độ, giá trị dương tương ứng quay Phải, giá trị âm tương ứng quay Trái). Số lượng xung số nguyên $P$ cần cấp cho lệnh băm xung được tính theo công thức:

$$P = 	ext{round}\left( rac{	heta 	imes 131,072}{360} ight)$$

Tần số phát xung $F$ (Hz) quyết định vận tốc quay góc $\omega$ (độ/giây) của đĩa xoay:

$$F = 	ext{round}\left( rac{\omega 	imes 131,072}{360} ight)$$

---

## PHẦN III: KỊCH BẢN KIỂM THỬ CÔ LẬP PHẦN CỨNG CHẠY BẰNG LÒNG MẠCH PLC (LEVEL 1)

Mục đích của kịch bản này là kiểm tra tính sống sót 100% của phần cứng mới (PLC DVP14SS2, mạch dập mass ngõ ra Transistor, Driver CSD7 và Động cơ) bằng cách nạp trực tiếp mã nguồn Ladder Logic độc lập xuống PLC.

### 1. Bảng phân phối vùng nhớ nội bộ (Internal Memory Allocation)
Để thực hiện test trên phần mềm WPLSoft thông qua việc cưỡng ép trạng thái (Force ON/OFF các bit), ta quy hoạch các tiếp điểm ảo sau:
* `M1000`: Cờ hệ thống, luôn đóng (ON) khi PLC RUN $ightarrow$ Dùng để kích hoạt Servo-ON (`Y5`).
* `M10`: Tiếp điểm kích hoạt kịch bản "Quay Phải $90^\circ$".
* `M11`: Tiếp điểm kích hoạt kịch bản "Quay Trái $90^\circ$".
* `M12`: Tiếp điểm kích hoạt kịch bản "Quay về gốc ban đầu ($0^\circ$)".
* `M0`: Tiếp điểm trung gian nội bộ ra lệnh phát xung.
* `D100` (32-bit, gồm `D100` và `D101`): Thanh ghi lưu trữ số lượng xung vị trí dịch chuyển.
* `D102` (32-bit, gồm `D102` và `D103`): Thanh ghi lưu trữ tần số phát xung (Tốc độ).

### 2. Mã nguồn Instruction List (IL) của chương trình PLC độc lập
Bạn tạo một dự án mới trên WPLSoft với Model dòng SS2, chuyển sang chế độ gõ mã lệnh (Instruction List) và dán đoạn mã chuẩn hóa sau:

```text
// NETWORK 1: LUÔN LUÔN KÍCH HOẠT VÒNG GIỮ TRỤC SERVO MOTOR
LD M1000
OUT Y5

// NETWORK 2: KỊCH BẢN QUAY PHẢI 90 ĐỘ (Tương ứng +32,768 xung, Tốc độ 5,000 Hz)
LD M10
DMOV K32768 D100
DMOV K5000 D102
SET M0
RST M10

// NETWORK 3: KỊCH BẢN QUAY TRÁI 90 ĐỘ (Tương ứng -32,768 xung, Tốc độ 5,000 Hz)
LD M11
DMOV K-32768 D100
DMOV K5000 D102
SET M0
RST M11

// NETWORK 4: KỊCH BẢN QUAY VỀ VỊ TRÍ GỐC O ĐỘ (Tuyệt đối hóa tọa độ về K0)
LD M12
DMOV K0 D100
DMOV K5000 D102
SET M0
RST M12

// NETWORK 5: KHỐI CHẤP HÀNH PHÁT XUNG TỐC ĐỘ CAO RA PHẦN CỨNG
LD M0
DDRVI D100 D102 Y0 Y1
RST M0
```

### 3. Các bước kiểm thử trên bàn kỹ thuật:
1. Gõ đoạn mã trên vào WPLSoft $ightarrow$ Bấm **Ctrl+F7** (Compile) $ightarrow$ Bấm **Ctrl+F8** (Write to PLC).
2. Chuyển PLC sang chế độ hoạt động bằng nút gạt trên vỏ hoặc bấm **Ctrl+F11** (RUN). Đèn `Y5` sáng rực, màn hình Servo CSD7 chuyển từ `F-rdy` sang `run-00`, cốt động cơ khóa cứng.
3. Bật **Ctrl+F4** (Online Mode) để giám sát.
4. Chuột phải vào `M10` $ightarrow$ Chọn **Set ON**. Tiếp điểm `M10` sẽ nạp tọa độ dương vào `D100` rồi tự động reset. Lệnh `DDRVI` được kích hoạt, băm xung ra chân `Y0`, trục đĩa xoay lập tức quay một góc chính xác $90^\circ$ về phía bên Phải.
5. Chuột phải vào `M11` $ightarrow$ Chọn **Set ON**. Trục đĩa xoay đảo ngược hướng quay và dịch chuyển một góc $90^\circ$ về phía bên Trái.
6. Khi nhấn `M12`, đĩa xoay lập tức quay trả ngược lại chính xác vị trí ban đầu lúc chưa cấp nguồn. Nếu cơ cấu cơ khí đáp ứng trơn tru, phần cứng đã đạt độ ổn định 100%.

---

## PHẦN IV: CẤU TRÚC PHÂN LỚP ĐIỀU KHIỂN ĐỘNG (PYTHON - RUST - MODBUS - PLC)

Sau khi kiểm thử phần cứng thành công ở Phần III, ta xóa bỏ hoàn toàn các Network kịch bản cứng (`M10`, `M11`, `M12`) trong PLC. Lúc này, cấu trúc điều khiển chuyển sang chế độ Động (Dynamic Register Matrix).

### 1. Kiến trúc phân tầng dữ liệu
```
 ┌────────────────────────────────────────────────────────┐
 │   Layer 3: HMI GUI (Python - customtkinter)            │
 └───────────────────────────┬────────────────────────────┘
                             │  ZeroMQ IPC (JSON Strings)
 ┌───────────────────────────▼────────────────────────────┐
 │   Layer 2: Async Backend Daemon (Rust Tokio Engine)    │
 └───────────────────────────┬────────────────────────────┘
                             │  Modbus RTU Frame over RS485
 ┌───────────────────────────▼────────────────────────────┐
 │   Layer 1: Pure Pulse Executor (PLC Delta Core)       │
 └────────────────────────────────────────────────────────┘
```

### 2. Bản đồ thanh ghi Modbus PLC Delta DVP14SS2 (Layer 1 Firmware)
Chương trình PLC được rút gọn về trạng thái tối giản tối đa để giải phóng tài nguyên CPU của PLC, biến PLC thành một thiết bị cơ bắp thuần túy:

```text
// FIRMWARE NẠP VĨNH VIỄN TRÊN PLC DELTA CHO CHẾ ĐỘ CHẠY ĐỘNG
LD M1000
OUT Y5

LD M0
DDRVI D100 D102 Y0 Y1
RST M0
```

*Địa chỉ ánh xạ Modbus HEX của các thanh ghi dữ liệu trên PLC Delta:*
* Thanh ghi vị trí xung `D100` (32-bit): Địa chỉ Modbus gồm cặp ô nhớ `0x1064` và `0x1065`.
* Thanh ghi tốc độ xung `D102` (32-bit): Địa chỉ Modbus gồm cặp ô nhớ `0x1066` và `0x1067`.
* Tiếp điểm kích hoạt phát xung `M0` (Bit): Địa chỉ Modbus Coil `0x0800`.

### 3. Tầng Core Driver và Kinh doanh của bộ mã nguồn Rust (Layer 2 Backend)
Mô-đun Rust sẽ chạy như một luồng bất đồng bộ độc lập sử dụng `tokio` và `tokio-modbus`. Dưới đây là kiến trúc mã nguồn thư viện sạch (Mẫu gợi ý cho Claude hiện thực hóa):

```rust
// File: src/hardware/rotating_disc.rs
use tokio_modbus::client::sync::Context;
use tokio_modbus::prelude::*;

pub enum Direction {
    Left,
    Right,
}

pub struct DiscCalibrationProfile {
    pub offset_pulses: i32,
    pub error_learning_coefficient: f64,
}

pub struct RotatingDiscController {
    modbus_context: Context,
    encoder_resolution: u32,
    calibration: DiscCalibrationProfile,
}

impl RotatingDiscController {
    pub fn new(port_path: &str, slave_address: u8) -> Self {
        // Khởi tạo kết nối Serial Modbus RTU qua RS485
        let slave = Slave(slave_address);
        let builder = tokio_serial::new(port_path, 9600)
            .data_bits(tokio_serial::DataBits::Seven)
            .parity(tokio_serial::Parity::Even)
            .stop_bits(tokio_serial::StopBits::One);
        
        let port = tokio_serial::暢::open(&builder).unwrap();
        let ctx = sync::rtu::connect_暢(port, slave).unwrap();

        RotatingDiscController {
            modbus_context: ctx,
            encoder_resolution: 131072,
            calibration: DiscCalibrationProfile { offset_pulses: 0, error_learning_coefficient: 0.1 }
        }
    }

    // Hàm cơ bản cấp độ Level 2: Quy đổi góc sang xung và đẩy xuống Modbus
    pub async fn rotate_degrees(&mut self, degrees: f64, dir: Direction) -> Result<(), std::io::Error> {
        let raw_pulses = ((degrees * self.encoder_resolution as f64) / 360.0).round() as i32;
        
        // Tính toán sai số thực tế dựa trên Level 5 tự học
        let final_pulses = match dir {
            Direction::Right => raw_pulses + self.calibration.offset_pulses,
            Direction::Left => -raw_pulses + self.calibration.offset_pulses,
        };

        // Phân tách số nguyên 32-bit thành hai từ 16-bit để ghi vào Modbus Register
        let p_high = ((final_pulses >> 16) & 0xFFFF) as u16;
        let p_low = (final_pulses & 0xFFFF) as u16;
        
        // Mặc định cấu hình tần số băm xung ở mức an toàn 4000Hz
        let f_high = 0u16;
        let f_low = 4000u16;

        // Ghi đồng bộ vào cụm thanh ghi D100, D101, D102, D103 (Địa chỉ gốc 0x1064)
        self.modbus_context.write_multiple_registers(0x1064, &[p_low, p_high, f_low, f_high]).await?;
        
        // Kích hoạt cuộn coi M0 (Địa chỉ 0x0800) để ra lệnh cho PLC băm xung ra chân cứng Y0
        self.modbus_context.write_single_coil(0x0800, true).await?;
        
        Ok(())
    }

    pub async fn return_to_origin(&mut self) -> Result<(), std::io::Error> {
        self.modbus_context.write_multiple_registers(0x1064, &[0, 0, 4000, 0]).await?;
        self.modbus_context.write_single_coil(0x0800, true).await?;
        Ok(())
    }

    // Cơ chế Level 4 & Level 5: Can thiệp thủ công và lưu trữ sai số tự học
    pub fn inject_manual_override_offset(&mut self, adjustment_degrees: f64) {
        let delta_pulses = ((adjustment_degrees * self.encoder_resolution as f64) / 360.0).round() as i32;
        // Học lỗi thích nghi khép kín
        self.calibration.offset_pulses += ((delta_pulses as f64) * self.calibration.error_learning_coefficient).round() as i32;
    }
}
```

### 4. Giao diện người máy Python HMI Frontend (Layer 3)
Viết bằng `customtkinter`, giao diện HMI tối giản (Blocky Design) liên kết với Rust Core thông qua Socket ZeroMQ. Giao diện cung cấp:
* Hệ thống hiển thị trạng thái góc quay hiện tại của đĩa xoay.
* Các khối chức năng điều khiển tự động nhận lệnh từ thuật toán AI Vision.
* **Cụm nút bấm Level 4 & 5 (Manual Override Control Panel):** Gồm nút nhấn `[+0.5° Fine Tune Right]` và `[-0.5° Fine Tune Left]`. Khi chuyên gia nhấn các nút này, Python lập tức bắn chuỗi ký tự JSON chứa mã lệnh can thiệp thủ công sang Rust Backend:
  `{"command": "OVERRIDE_ADJUST", "value": 0.5}`
  Bộ điều khiển Rust sẽ tiếp nhận bản tin này, gọi hàm `inject_manual_override_offset(0.5)` để tính toán hiệu số xung, nạp bù dòng điện áp dịch chuyển cốt motor ngay trong chu kỳ máy hiện hành và tự động học lỗi để tối ưu hóa vĩnh viễn cho các chu kỳ tiếp theo.

---

## PHẦN V: TIÊU CHÍ BÀN GIAO THÀNH CÔNG (SUCCESS DEFINITION)
Mã nguồn được sinh ra từ kiến trúc này được coi là thành công 100% nếu thỏa mãn trọn vẹn các bài Test Case sau:
1. **Test Case 1 (Isolated Verification):** Kích hoạt trực tiếp `M10` trên WPLSoft, đĩa xoay vật lý phải quay đúng $90^\circ$ và khóa cứng vị trí, không bị rung lắc cơ khí.
2. **Test Case 2 (Rust Communication Integration):** Khởi chạy daemon Rust, gọi hàm `rotate_degrees(45.0, Direction::Right)`, PLC phải nhận được giá trị $16384$ trong cụm thanh ghi `D100-D101`, đèn ngõ ra `Y0` và `Y5` hoạt động chính xác.
3. **Test Case 3 (Closed-Loop Validation):** Mô phỏng lỗi đĩa xoay bị chạy lố góc do quán tính cơ khí băng tải, người dùng nhấn nút hiệu chỉnh tinh chỉnh trên giao diện Python HMI. Hệ thống phải dịch chuyển bù ngay lập tức và lưu lại hệ số hiệu chỉnh vào profile. Ở lần kích chạy tiếp theo, đĩa xoay tự động dừng ở điểm đích đã được hiệu chỉnh sai số, hoàn tất yêu cầu nghiệm thu năng lượng của mức Level 5.
