# LoadingSystem - Hệ Thống Điều Khiển Đĩa Xoay Phân Loại Muỗi Tự Động (Version 1.2)

> **Dự án:** Hệ Thống Phân Loại & Đóng Lọ Muỗi 12 Loài Tự Động Trong Công Nghiệp  
> **Subsystem:** Đĩa Xoay Định Vị Vật Thể & Thuật Toán Điều Tốc Tự Động (Rotating Disc Subsystem)  
> **Phiên bản:** Version 1.2 (Cập nhật ngày 22/07/2026)

---

## 👥 Đội Ngũ Tác Giả & Sinh Viên Thực Tập

- **Vương Quốc Khánh** 
- **Trương Hoàng Nam** 
- **Trần Doãn Hoàng Lâm** 

---

## 🌟 Điểm Mới & Cải Tiến Nổi Bật Trên Phiên Bản 1.2

1. **Phân loại 12 loài muỗi ngẫu nhiên vào 12 lọ riêng biệt (1-to-1 Mapping):**
   - Đĩa xoay chứa 12 lọ mang 12 màu sắc đặc trưng riêng biệt cho 12 loài muỗi.
   - Thả muỗi ngẫu nhiên: Mâm xoay tự động định vị và xoay đến đúng lọ hứng của loài muỗi tương ứng.
2. **Thuật toán điều tốc tự động & Mô hình thời gian rơi vật lý ($t_{drop}$):**
   - Tự động tính $t_{drop} = \frac{L_{cam}}{v_{belt}} + \sqrt{\frac{2 H_{drop}}{g}}$.
   - Tự động tinh chỉnh tần số Servo $f$ (Hz) sao cho mâm xoay về vị trí trúng đích trong **tối đa $1.0\text{s}$** (Góc $30^\circ$ quay chậm mượt, Góc $180^\circ$ quay tốc độ cao).
3. **Mở rộng Giao thức IPC Socket JSON & Modbus RTU 8-bit:**
   - Gói JSON hỗ trợ truyền tham số `"frequency"` từ HMI xuống Rust Daemon.
   - Rust Daemon trích xuất và nạp tần số $f$ (Hz) trực tiếp vào thanh ghi `D102/D103` của PLC Delta.
4. **Nút bấm `[✔ XÁC NHẬN CẤU HÌNH THÔNG SỐ]`:**
   - Đảm bảo kỹ sư chủ động chốt và áp dụng các thông số $L_{cam}, H_{drop}, v_{belt}$, Tần số Hz.
5. **Giao diện HMI 3D & Log Console Phóng To:**
   - Vị trí Camera AI Vision Sensor nằm ở **đầu băng chuyền 3D**.
   - Số lượng muỗi hiển thị **to, đậm ngay CHÍNH GIỮA THÂN LỌ 3D**.
   - Khung Log Console sắc nét, phân màu tag (`DISPENSE`, `ROTATE`, `CONFIG`, `ERROR`).
6. **Clean Async Backend Daemon (Tokio Engine):**
   - Rust Backend chạy thuần dưới dạng dịch vụ ngầm ổn định 24/7.

---

## 🏗️ Kiến Trúc 3 Tầng

```
┌────────────────────────────────────────────────────────┐
│   Layer 3: HMI GUI (Python 3D - customtkinter)         │
│   - Thả muỗi 12 loài ngẫu nhiên & Thuật toán Điều tốc   │
└───────────────────────────┬────────────────────────────┘
                            │  TCP/IPC (JSON Protocol with Frequency)
┌───────────────────────────▼────────────────────────────┐
│   Layer 2: Async Backend Daemon (Rust Tokio Engine)    │
│   - 127.0.0.1:5555 Server, EMA Self-Learning           │
└───────────────────────────┬────────────────────────────┘
                            │  Modbus RTU over RS485 (COM5 9600 8E1)
┌───────────────────────────▼────────────────────────────┐
│   Layer 1: Pure Pulse Executor (PLC Delta Core)        │
│   - MOV H87 D1120 (8-bit RTU), DDRVI D100 D102 Y0 Y1   │
└────────────────────────────────────────────────────────┘
```

---

## 📊 5 Cấp Độ Chức Năng (Levels)

| Level | Tên | Mô tả |
|:---:|:---|:---|
| **1** | Pure PLC Code | Kiểm thử cô lập phần cứng bằng Ladder Logic (`standalone_test.il`) |
| **2** | Clean Rust Library | Thư viện Rust với primitives: quay trái/phải/home (`rotating_disc.rs`) |
| **3** | Enterprise API | Backend daemon + IPC server TCP JSON cho HMI (`ipc_server.rs`) |
| **4** | Manual Override | Can thiệp thủ công Fine Tune ±0.5° (`override_panel.py`) |
| **5** | Self-Learning | Thuật toán EMA tự học khép kín bù sai số vĩnh viễn (`learning.rs`) |

---

## 📁 Cấu Trúc Thư Mục Dự Án

```
loading_system/
├── plc_firmware/               # Layer 1: Firmware PLC Delta DVP14SS2 (Instruction List)
│   ├── standalone_test.il      #   Kiểm thử cô lập (Level 1)
│   └── dynamic_mode.il         #   Firmware Modbus RTU 8-bit H87 (Level 2-5)
├── rust_backend/               # Layer 2: Rust Async Backend Daemon
│   ├── Cargo.toml              #   Cargo manifest (default-run = "loading-daemon")
│   ├── config.toml             #   Cấu hình COM5, Baud 9600, 8E1
│   └── src/                    #   Mã nguồn Rust Tokio Engine
├── python_hmi/                 # Layer 3: Python HMI GUI 3D
│   ├── main.py                 #   Entry point HMI GUI
│   └── app/                    #   Visualizer 3D, Log Console, Control Panel
├── docs_v1.1/                  # Tài liệu lưu trữ phiên bản 1.1
└── docs_v1.2/                  # Tài liệu báo cáo chi tiết phiên bản 1.2
    ├── BUILD.md                #   Hướng dẫn Build, Compile & Run v1.2
    ├── USAGE.md                #   Hướng dẫn Sử Dụng Hệ Thống v1.2
    ├── code_architecture_guide.md # Đặc tả Kiến Trúc Mã Nguồn v1.2
    ├── presentation_report.md  #   Báo Cáo Nghiệm Thu Thực Tập v1.2
    └── v1.2_improvements_report.md # Báo Cáo Cải Tiến So Với v1.1
```

---

## 🚀 Quick Start (Hướng Dẫn Nhanh)

### 1. Khởi chạy Rust Backend Daemon
```bash
cd rust_backend
cargo run
```

### 2. Khởi chạy Python HMI 3D GUI
```bash
cd python_hmi
python main.py
```

---

## 📚 Tài Liệu Chi Tiết Version 1.2

- 📄 [Báo Cáo Cải Tiến So Với v1.1](docs_v1.2/v1.2_improvements_report.md)
- 🛠️ [Hướng Dẫn Build & Biên Dịch v1.2](docs_v1.2/BUILD.md)
- 🎮 [Hướng Dẫn Sử Dụng Hệ Thống v1.2](docs_v1.2/USAGE.md)
- 📐 [Đặc Tả Kiến Trúc Mã Nguồn v1.2](docs_v1.2/code_architecture_guide.md)
- 📊 [Báo Cáo Kết Quả Thực Tập & Nghiệm Thu v1.2](docs_v1.2/presentation_report.md)

---

## 📌 Thông Tin Nền Tảng

- **Nhóm Thực Tập:** Vương Quốc Khánh, Trương Hoàng Nam, Trần Doãn Hoàng Lâm
- **Phần Cứng:** PLC Delta DVP14SS2 + Servo Driver CSD7 (17-bit Encoder) + USB-to-RS485 COM5
- **Công Nghệ:** Rust (Tokio Async, tokio-modbus) + Python (customtkinter, Socket TCP IPC) + Modbus RTU 8E1
