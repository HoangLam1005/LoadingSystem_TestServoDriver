# Hướng Dẫn Build, Compile & Run - LoadingSystem Version 1.2

**Tác giả:** Vương Quốc Khánh, Trương Hoàng Nam, Trần Doãn Hoàng Lâm  
**Phiên bản:** Version 1.2 (Cập nhật ngày 22/07/2026)

---

## Yêu Cầu Hệ Thống

### Phần mềm bắt buộc

| Phần mềm | Phiên bản | Mục đích |
|-----------|-----------|----------|
| **Rust** | ≥ 1.70.0 | Async Backend Daemon (Tokio Engine) |
| **Python** | ≥ 3.10 | HMI GUI 3D (customtkinter) |
| **pip** | ≥ 21.0 | Quản lý package Python |

### Phần mềm tùy chọn (cho phần cứng)

| Phần mềm | Mục đích |
|-----------|----------|
| **WPLSoft** | Nạp firmware PLC Delta (Modbus RTU `H87`/`H67`) |
| **USB-to-RS485 Driver** | Kết nối PLC qua cổng serial COM5 |

---

## 1. Cài Đặt Rust Toolchain

### Windows
```powershell
# Tải và chạy rustup-init.exe từ https://rustup.rs/
# Hoặc dùng winget:
winget install Rustlang.Rustup

# Xác nhận cài đặt:
rustc --version
cargo --version
```

### Linux / macOS
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version
```

---

## 2. Build Rust Backend Daemon (v1.2)

### Debug Build (để phát triển)
```bash
cd rust_backend
cargo build
```

### Release Build (để triển khai công nghiệp)
```bash
cd rust_backend
cargo build --release
```

Binary sẽ nằm tại:
- Debug: `rust_backend/target/debug/loading-daemon`
- Release: `rust_backend/target/release/loading-daemon`

### Chạy Tests
```bash
cd rust_backend
cargo test -- --nocapture
```

---

## 3. Cài Đặt Python HMI (v1.2)

### Cài đặt dependencies
```bash
cd python_hmi
pip install -r requirements.txt
```

### Kiểm tra cú pháp HMI
```bash
cd python_hmi
python -m py_compile main.py
```

---

## 4. Khởi Chạy Hệ Thống Dual-Service (v1.2)

### Bước 1: Cấu hình Backend

Chỉnh sửa file `rust_backend/config.toml`:

```toml
[serial]
# Chế độ MOCK (không cần phần cứng):
port = "MOCK"

# Chế độ thật (kết nối PLC Delta qua RS-485 COM5):
# port = "COM5"
# baud_rate = 9600
# data_bits = 8
# stop_bits = 1
# parity = "even"
```

### Bước 2: Khởi chạy Rust Backend Daemon

```bash
cd rust_backend
cargo run
```

Hoặc chạy binary trực tiếp:
```bash
cd rust_backend
./target/release/loading-daemon
```

Kết quả mong đợi:
```
INFO loading_daemon: === LoadingSystem Backend Daemon ===
INFO loading_daemon: Version: 1.0.0
INFO loading_daemon: Mode: COM5 (9600 baud, 8,even,1)
INFO loading_system::api::ipc_server: IPC server listening on 127.0.0.1:5555
```

### Bước 3: Khởi chạy Python HMI 3D GUI

Mở terminal mới (Terminal 2):
```bash
cd python_hmi
python main.py
```

Với tùy chọn host/port:
```bash
python main.py --host 127.0.0.1 --port 5555
```

---

## 5. Nạp Firmware PLC Delta (Cấu hình 8-bit Modbus RTU)

### Bước 1: Chuẩn bị
- Cài đặt **WPLSoft** từ Delta Electronics
- Kết nối PLC DVP14SS2 qua cáp RS-485 vào cổng domino COM2 (chân S/S, A+, B-)

### Bước 2: Nạp Firmware Modbus RTU Dynamic (Level 2-5)
1. Mở WPLSoft → Chọn model **DVP-14SS2**
2. Nạp đoạn lệnh cài đặt định dạng truyền thông **9600, 8, E, 1, RTU**:
   ```il
   LD M1002
   MOV H87 D1120   ; Cấu hình 9600 bps, 8 Data bits, Even parity, 1 Stop bit
   MOV K1 D1121    ; Slave Address = 1
   SET M1120       ; Giữ trạng thái COM2
   SET M1143       ; Chế độ Modbus RTU (8-bit)
   ```
3. **Ctrl+F7** (Compile) → **Ctrl+F8** (Write to PLC)
4. Rút nguồn 24V PLC 5 giây rồi cắm lại (để PLC nhận cấu hình `H87` mới).
5. Gạt công tắc PLC sang **RUN** (đèn RUN sáng xanh).

---

## 6. Xử Lý Sự Cố (Troubleshooting)

### Lỗi `Modbus read coils timeout (15s)`
1. Kiểm tra mã lệnh trong PLC đã đổi thành **`MOV H87 D1120`** (8-bit) chưa (Nếu dùng `H78` 7-bit sẽ bị đứt MSB byte).
2. Tắt hoàn toàn phần mềm WPLSoft để nhả cổng COM5.
3. Rút nguồn 24V của PLC 5 giây rồi cắm lại.

### Lỗi Socket IPC `127.0.0.1:5555`
1. Đảm bảo Rust Backend Daemon đang chạy trước khi bật Python HMI.
2. Nếu cổng 5555 bị chiếm dụng, kiểm tra tiến trình đang chạy ngầm bằng `netstat -ano | findstr 5555`.
