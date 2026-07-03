# Hướng dẫn kiểm thử Task Servo Control.

Tài liệu này hướng dẫn chi tiết quy trình kiểm thử (Unit Test, Integration Test và Hardware Test) cho mã nguồn điều khiển Servo Motor. Thư mục này chứa 3 phiên bản triển khai điều khiển động cơ Servo **RS Automation CSMT-02BR1ABT3** qua bộ Drive **CSD7-02DX1**:

1. **Python (`servo_control.py`):** Chạy trên Host (Raspberry Pi 5 / PC) điều khiển qua giao thức truyền thông Modbus-RTU (RS-485).
2. **Rust (`servo_control.rs`):** Chạy trên vi điều khiển STM32 sử dụng Embassy framework phát xung Pulse/Direction.
3. **C (`servo_control.c`):** Chạy bare-metal HAL/LL trên vi điều khiển STM32 phát xung Pulse/Direction.

---

## 1. Sơ đồ kết nối phần cứng kiểm thử

### 1.1. Ghép nối cho phiên bản Python (Modbus-RTU)
```
┌─────────────────┐       USB       ┌──────────────────┐    RS-485     ┌──────────────┐
│  Raspberry Pi 5 │ ──────────────> │ USB-to-RS485     │ ────────────> │ CSD7 COMM    │
│  (Host PC)      │                 │ Converter (CH340)│   (A+, B-)   │ Port (Tyco)  │
└─────────────────┘                 └──────────────────┘               └──────────────┘
```
* **Đấu nối:** Cổng COMM của Drive CSD7 sử dụng cổng Tyco Mini-I/O 8-pin. Kết nối chân 1 (A+) và chân 2 (B-) về cổng RS-485 của USB Converter. Nối đất chung chân 3 (GND).

### 1.2. Ghép nối cho phiên bản C / Rust (Pulse/Direction)
```
┌──────────────────┐   GPIO (3.3V)   ┌─────────────────┐  Opto (24V)  ┌──────────────┐
│  STM32F4/F7      │ ──────────────> │ Level Shifter   │ ───────────> │ CSD7 CN1     │
│  (PA8, PA9, ...) │                 │ (Opto TLP281)   │              │ (50-pin Port)│
└──────────────────┘                 └─────────────────┘              └──────────────┘
```
* **Phân bổ Pinout trên vi điều khiển STM32:**
  * `PA8` → Đầu ra phát xung **PULSE** (Timer 1 Channel 1 PWM). Kết nối vào chân CN1-Pin 1 (PULSE+) qua opto.
  * `PA9` → Tín hiệu điều khiển chiều **DIRECTION** (GPIO Output). Kết nối vào chân CN1-Pin 3 (SIGN+) qua opto.
  * `PA10` → Tín hiệu kích hoạt **SERVO_ON** (GPIO Output). Kết nối vào chân CN1-Pin 7 (SON) qua opto.
  * `PA11` → Tín hiệu xóa lỗi **ALARM_RESET** (GPIO Output). Kết nối vào chân CN1-Pin 15 (ARST) qua opto.

---

## 2. Kiểm thử phiên bản Python (`servo_control.py`)

### 2.1. Chuẩn bị môi trường
Cài đặt các thư viện phụ thuộc bằng lệnh terminal:
```bash
pip install pymodbus==3.6.0 pyserial==3.5
```

### 2.2. Quy trình thực hiện kiểm thử
1. Cắm USB-to-RS485 converter vào Raspberry Pi 5. Xác định cổng serial:
   ```bash
   ls /dev/ttyUSB*
   # Kết quả mong đợi thường là /dev/ttyUSB0. Nếu khác, sửa hằng số SERIAL_PORT trong file servo_control.py.
   ```
2. Cấp nguồn 220VAC cho Servo Drive CSD7. Đảm bảo màn hình hiển thị trạng thái bình thường (không nhấp nháy báo lỗi ALM).
3. Khởi chạy chương trình kiểm thử tự động (demo):
   ```bash
   python servo_control.py
   ```

### 2.3. Các kịch bản test tuần tự trong chương trình demo
* **TC-PY-01 (Connect):** Thiết lập kết nối Modbus-RTU thành công qua cổng `/dev/ttyUSB0` ở baudrate 38400.
* **TC-PY-02 (Read Status):** Đọc trạng thái ban đầu của drive (Ready, Alarm, Position).
* **TC-PY-03 (Config):** Cấu hình thời gian tăng/giảm tốc (accel/decel time) về mức 500ms.
* **TC-PY-04 (Servo ON):** Kích hoạt Servo ON. Trục động cơ bị khóa cứng, đèn chỉ thị trạng thái Ready chuyển sang màu xanh (hoặc bit status chuyển lên HIGH).
* **TC-PY-05 (JOG CW):** Điều khiển quay thuận (CW) với tốc độ 500 RPM trong 3 giây. Xác nhận trực quan động cơ quay êm, mượt.
* **TC-PY-06 (Stop):** Phát lệnh dừng, động cơ giảm tốc mượt mà về 0 RPM trong 0.5 giây.
* **TC-PY-07 (JOG CCW):** Điều khiển quay ngược (CCW) với tốc độ 300 RPM trong 2 giây.
* **TC-PY-08 (Servo OFF):** Tắt Servo, trục động cơ quay tự do trở lại.
* **TC-PY-09 (Disconnect):** Giải phóng cổng serial kết nối an toàn.

---

## 3. Kiểm thử phiên bản Rust (`servo_control.rs`)

### 3.1. Chuẩn bị môi trường
Yêu cầu máy phát triển đã cài đặt Rust toolchain và ST-Link/J-Link để nạp code:
```bash
# Cài đặt công cụ nạp flash probe-rs (nếu chưa có)
cargo install probe-rs --features cli
```

### 3.2. Quy trình thực hiện kiểm thử
1. Di chuyển terminal tới thư mục chứa project Rust (nơi chứa file `Cargo.toml`).
2. Kết nối mạch STM32 với máy tính qua cổng debug ST-Link.
3. Cấp nguồn cho mạch STM32 và bộ Driver CSD7.
4. Biên dịch và nạp firmware trực tiếp xuống chip, đồng thời lắng nghe log gỡ lỗi (RTT log):
   ```bash
   cargo run --release
   ```
5. Theo dõi log in ra trên console thông qua RTT.

### 3.3. Các kịch bản test tuần tự của firmware Rust
* **TC-RS-01 (Init):** Cấu hình thành công bộ phát xung phần cứng Timer 1 (PWM PA8) và các chân đầu ra GPIO (PA9, PA10, PA11).
* **TC-RS-02 (Servo ON):** Tín hiệu chân PA10 lên mức HIGH, chờ 500ms để drive chuyển trạng thái.
* **TC-RS-03 (Move Position CW):** Phát chính xác số xung tương ứng để quay thuận **360°** với tốc độ 500 RPM.
* **TC-RS-04 (Delay):** Motor dừng đúng vị trí và giữ nguyên vị trí trong vòng 2 giây.
* **TC-RS-05 (Move Position CCW):** Đổi chân chiều PA9 lên HIGH (CCW) và phát xung quay ngược **180°** với tốc độ 300 RPM.
* **TC-RS-06 (Continuous Run):** Phát xung liên tục CW với tốc độ 1000 RPM trong 3 giây, sau đó cắt xung dừng động cơ.
* **TC-RS-07 (Servo OFF):** Chân PA10 hạ về LOW, ngắt lực giữ động cơ.

---

## 4. Kiểm thử phiên bản C (`servo_control.c`)

### 4.1. Chuẩn bị môi trường
Yêu cầu phần mềm STM32CubeIDE hoặc Keil uVision để biên dịch và nạp chương trình.

### 4.2. Quy trình thực hiện kiểm thử
1. Mở file `servo_control.c` trong dự án STM32CubeIDE.
2. Biên dịch dự án bằng phím tắt `Ctrl + B` (hoặc nhấn Project -> Build Project). Đảm bảo không có lỗi biên dịch (0 errors).
3. Kết nối kit STM32 với máy tính qua ST-Link, nhấn `F11` để chuyển sang chế độ Debug và nạp code xuống chip.
4. Nhấn `F8` (Resume) để bắt đầu chạy chương trình.
5. Quan sát chuyển động của trục motor và kiểm tra dạng sóng xung phát ra ở chân `PA8` bằng máy hiện sóng (Oscilloscope).

### 4.3. Tiêu chí kiểm định dạng sóng xung trên Oscilloscope
* Tần số phát xung phải trùng khớp với tính toán tốc độ quay.
* Độ lệch chu kỳ nhiệm vụ (Duty cycle) của chân phát xung phải duy trì ổn định ở mức **50% ± 1%** để đảm bảo truyền tín hiệu xung vuông chính xác cao qua Optocoupler.
* Các sườn xung sắc nét, sườn lên (rising edge) < 100ns để optocoupler có thể bắt kịp tần số xung.

---

## 5. Tiêu chí đánh giá kết quả kiểm thử (Pass/Fail)

| Mã testcase | Nội dung kiểm tra | Ngưỡng PASS | Ngưỡng FAIL |
| :--- | :--- | :--- | :--- |
| **TC-01** | Kết nối truyền thông (Modbus / Debug link) | Kết nối thành công, đọc được dữ liệu trạng thái ban đầu của Drive. | Không kết nối được, báo lỗi Timeout hoặc Hardware Error. |
| **TC-02** | Trạng thái Servo ON/OFF | Trục motor khóa cứng khi ON, quay tự do hoàn toàn khi OFF. Đèn SRDY sáng. | Trục không khóa cứng, CSD7 báo lỗi Alarm (nhấp nháy đèn đỏ). |
| **TC-03** | Điều khiển tốc độ (JOG Mode) | Động cơ quay đều theo đúng chiều CW/CCW. Tốc độ thực tế sai số ≤ 2% tốc độ đặt. | Động cơ không quay hoặc quay sai chiều. Sai số tốc độ > 5%. |
| **TC-04** | Điều khiển vị trí (Position Mode) | Motor quay đúng góc đặt (360° và 180°), sai số vị trí ≤ 0.5°. | Sai số góc quay > 1.0° hoặc motor quay quá hành trình. |
| **TC-05** | Dừng khẩn cấp (Emergency Stop) | Motor ngắt lực và dừng ngay lập tức khi gửi lệnh hoặc nhấn nút khẩn cấp. | Motor tiếp tục quay hoặc trôi tự do không kiểm soát. |
