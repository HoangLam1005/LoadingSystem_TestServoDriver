# HỆ THỐNG KIỂM THỬ ĐỘNG CƠ RUST + PLC DELTA

Thư mục này chứa toàn bộ ứng dụng kiểm thử tích hợp thời gian thực giữa Laptop (chạy chương trình Rust Master Daemon) và bộ điều khiển PLC Delta DVP-14SS2 để điều khiển Servo Driver RS Automation CSD7 & Động cơ Servo CSMT-02BR trong hệ thống phân loại Loading System.

---

## 1. Tổng quan dự án 

- **Mục đích**: Cung cấp bộ công cụ kiểm thử động học, kiểm thử truyền thông nối tiếp RS485 và nghiệm thu tính năng an toàn cho hệ thống mâm xoay Servo 12 vị trí lọ nhận sản phẩm và biến tần điều khiển băng tải.
- **Kiến trúc truyền thông**: Rust Master kết nối cổng nối tiếp RS485 với PLC Delta qua giao thức Modbus RTU, Baudrate **38,400 bps** (8-N-1), chu kỳ timeout 30ms.
- **Cơ cấu chấp hành**: PLC Delta DVP-28SS2 băm xung ngõ ra tốc độ cao Y0 (Pulse) và Y1 (Direction) sang giắc 50-pin CN1 của Servo Driver CSD7.

---

## 2. Cấu trúc thư mục 

```text
├── plc/
│   ├── plc_test_manual.txt  # Code PLC Delta chế độ thủ công
│   ├── plc_test_auto.txt    # Code PLC Delta chế độ tự động
├── src/
│   ├── main.rs              # Điểm khởi chạy ứng dụng, quản lý menu 10 bài test tương tác
│   ├── modbus.rs            # Struct DeltaPlc, quản lý RS485 38400bps, đọc/ghi 32-bit, clamp 10kHz
│   ├── modbus_registers.rs  # Bảng hằng số địa chỉ Modbus tập trung
│   └── test_cases.rs        # Thuật toán chống Race Condition & 10 bài kiểm thử tích hợp
├── Cargo.toml               # File cấu hình phụ thuộc thư viện Rust (tokio, tokio-modbus, tokio-serial)
├── Cargo.lock               # Quản lý phiên bản chi tiết các crate
└── README.md                # Tài liệu hướng dẫn sử dụng và đặc tả kỹ thuật dự án
```

---

## 3. Cấu hình hệ thống 
### 3.1. Cấu hình trên bảng LED 7 đoạn Driver CSD7

Trước khi chạy test, bắt buộc cài đặt 4 tham số sau trên bảng điều khiển Driver CSD7 và **khởi động lại nguồn Driver**:

| Tham số LED | Giá trị Cài đặt | Giải nghĩa Chức năng Chi tiết |
| :---: | :---: | :--- |
| **`Ft-0.00`** | **`1`** (Chữ **F**) | Chế độ điều khiển Vị trí (Position Control Mode - nhận xung định vị) |
| **`Ft-3.00`** | **`0012`** | Kiểu nhận xung Open Collector 24V NPN, logic phát xung Pulse/Direction |
| **`Ft-3.05`** | **`131072`** | Tử số hộp số điện tử Electronic Gear Ratio (Độ phân giải Encoder 23-bit) |
| **`Ft-3.06`** | **`120000`** | Mẫu số hộp số điện tử (Số xung phát ra tương ứng 1 vòng mâm $360^\circ$) |

### 3.2. Cấu hình Truyền thông Modbus RS-485

| Thông số Truyền thông | Giá trị Cấu hình | Địa chỉ Thanh ghi PLC Delta |
| :--- | :---: | :---: |
| **Baudrate** | **38,400 bps** | `D1120 = H87` |
| **Định dạng khung tin** | **8 Data bits, No Parity, 1 Stop bit (8-N-1)** | `M1120 = SET` |
| **Giao thức** | **Modbus RTU Mode** | `M1143 = SET` |
| **Communication Timeout** | **30 ms** | `D1121 = K30` |
| **Modbus Slave Station ID** | **1** | `D1121 (Default = 1)` |

---

## 4. Bản đồ bộ nhớ truyền thông Modbus RTU

### 4.1. Bảng ánh xạ Coil (M-Relay) — FC01 / FC05

| Địa chỉ PLC | Địa chỉ Modbus (Dec) | Địa chỉ Modbus (Hex) | Hướng truyền | Chức năng kiểm thử |
| :---: | :---: | :---: | :---: | :--- |
| **`M0`** | Coil **`2048`** | `0x0800` | Rust → PLC | Cho phép hệ thống hoạt động (Bật ngõ ra Y5 Servo-ON) |
| **`M1`** | Coil **`2049`** | `0x0801` | Rust → PLC | Dừng hệ thống (Xóa D22 ngắt ngõ ra VFD Y2) |
| **`M2`** | Coil **`2050`** | `0x0802` | Rust → PLC | Dừng Khẩn Cấp E-Stop (Cưỡng bức ngắt Y0 và nhả Y5 Servo-ON) |
| **`M10`** | Coil **`2058`** | `0x080A` | Rust → PLC | Cờ kích hoạt băm xung xoay mâm (Lệnh DDRVI ngõ ra Y0) |
| **`M11`** | Coil **`2059`** | `0x080B` | PLC → Rust | Cờ báo PLC đang bận thực thi băm xung Y0 |
| **`M80`** | Coil **`2128`** | `0x0850` | PLC → Rust | Cờ báo lỗi mất nhịp tim truyền thông Watchdog Heartbeat |

### 4.2. Bảng ánh xạ Holding Register (D-Register) — FC03 / FC06 / FC16

| Địa chỉ PLC | Địa chỉ Modbus (Dec) | Địa chỉ Modbus (Hex) | Kiểu dữ liệu | Chức năng kiểm thử |
| :---: | :---: | :---: | :---: | :--- |
| **`D10 - D11`** | Register **`4106`** | `0x100A` | 32-bit Signed Int | Số xung mục tiêu (`+` là quay CW, `-` là quay CCW) |
| **`D14 - D15`** | Register **`4110`** | `0x100E` | 32-bit Signed Int | Tần số phát xung (Tốc độ quay Servo Hz, max 10,000 Hz) |
| **`D100`** | Register **`4196`** | `0x1064` | 16-bit Unsigned | Bộ đếm số lần mâm xoay đã quay định vị thành công |
| **`D1000`** | Register **`5096`** | `0x13E8` | 16-bit Unsigned | Thanh ghi nhận nhịp tim Watchdog gửi từ Rust mỗi 400ms |
| **`D1343`** | Register **`5439`** | `0x153F` | 16-bit Unsigned | Thời gian dốc tăng/giảm tốc gia tốc mềm (mili-giây) |

---

## 5. Sơ đồ đấu nối dây vật lý

| Cổng PLC Delta | Giắc 50-pin CN1 Driver CSD7 | Chức năng đấu nối |
| :---: | :---: | :--- |
| **`Y0`** | Chân **`12`** (PULS-) | Ngõ ra phát xung tốc độ cao NPN |
| **`Y1`** | Chân **`14`** (SIGN-) | Ngõ ra chỉ hướng quay NPN |
| **`Y5`** | Chân **`3`** (INPUT1 / SV-ON) | Ngõ ra điều khiển rơ-le kích mát Servo-ON |
| **`UP`** (PLC) | Cực dương **`+24VDC`** | Cấp nguồn dương nuôi ngõ ra Transistor PLC |
| **`ZP`** (PLC) | Cực âm **`0VDC` (GND)** | Cấp nguồn âm chung mass hệ thống |
| *Cầu nối chung* | Chân **`1`**, **`25`**, **`49`** | Nối cầu chung về cực dương **`+24VDC`** nguồn tổ ong |
| *Bảo vệ E-Stop* | Chân **`10`** (Driver) | Nối xuống cực âm **`0VDC` (GND)** để cho phép Servo ON |

---

## 6. Thuật toán cốt lõi & tính năng an toàn

1. **Thuật toán tìm đường đi ngắn nhất vòng tròn 12 Lọ**:
   - Công thức tính khoảng cách góc thuận (CW): `d_cw = (target - current + 12) % 12`
   - Số bước chọn đường ngắn nhất: `steps = d_cw` (nếu `d_cw <= 6`), ngược lại `steps = d_cw - 12`
   - Số xung phát tương ứng: `Pulses = steps * 10,000` (xung)

2. **Thuật toán xử lý Race Condition 2 giai đoạn (`wait_for_rotation_done`)**:
   - *Giai đoạn 1*: Chờ cờ `M10`/`M11` chuyển sang `true` (Busy State).
   - *Giai đoạn 2*: Polling chu kỳ **350ms** chờ cờ `M10` tự động Reset về `false` (Done State).

3. **Cơ chế khóa liên động an toàn E-Stop (ISO 13849-1)**:
   - Tín hiệu `M2 = ON` lập tức tạm dừng phát xung (`M1334 = SET`), ngắt Servo-ON `Y5` và cưỡng bức `RST M10`, `RST M11`. Khi nhả `M2 = OFF`, mâm xoay **không bao giờ tự chạy lại đột ngột**.

4. **Cơ chế giám sát nhịp tim Watchdog Heartbeat**:
   - Rust gửi nhịp đếm tăng dần xuống `D1000` mỗi 400ms. Nếu cáp RS485 bị đứt hoặc Rust crash quá 1.0s, Timer T1 trong PLC tự kích `M80 = ON`, tự ngắt Servo-ON `Y5` và ngắt chạy băng tải.

5. **Khống chế tần số phần cứng Transistor 10 kHz**:
   - Hàm `trigger_rotation()` trong Rust thực thi `speed_hz.clamp(100, 10000)` để ngăn ngừa vượt quá giới hạn vật lý 10 kHz của ngõ ra Y0/Y1 PLC Delta DVP-28SS2.

---

## 7. Hướng dẫn vận hành & kiểm thử

### Bước 1: Nạp chương trình vào PLC
1. Mở phần mềm **WPLSoft** trên laptop.
2. Chọn **File** -> **Open** -> chọn định dạng **`Text Files (*.txt)`**.
3. Chọn tệp **`plc_test_manual.txt`** (hoặc `plc_test_auto.txt` nếu test chế độ tự động có Watchdog). Nhấn **Open**.
4. Biên dịch bằng phím **Ctrl + F7** (Compiler -> Ladder to Instruction).
5. Nạp xuống PLC bằng phím **Ctrl + F8** (Write to PLC). Gạt công tắc vật lý mặt PLC sang **RUN**.

### Bước 2: Chạy ứng dụng kiểm thử trên Laptop (Rust)
1. Cắm cáp USB-to-RS485 nối từ cổng COM2 của PLC vào cổng USB laptop.
2. Kiểm tra tên cổng COM trong Device Manager (ví dụ: `COM4`).
3. Click đúp chuột vào tệp **`run_test.bat`** (hoặc gõ `cargo run --release`).
4. Nhập tên cổng COM (ví dụ: `COM4`) và nhấn **Enter**.

