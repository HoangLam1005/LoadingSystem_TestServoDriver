# Hướng Dẫn Sử Dụng Hệ Thống LoadingSystem - Version 1.2

**Tác giả:** Vương Quốc Khánh, Trương Hoàng Nam, Trần Doãn Hoàng Lâm  
**Phiên bản:** Version 1.2 (Cập nhật ngày 22/07/2026)

---

## 1. TỔNG QUAN VỀ GIAO DIỆN HMI VERSION 1.2

Giao diện **Python HMI (customtkinter)** của Version 1.2 được nâng cấp toàn diện theo thiết kế công nghiệp **3D Light Industrial Theme (White & Blue)**, tích hợp mô phỏng 4 phân hệ dây chuyền đóng lọ muỗi tự động:

```
┌────────────────────────────────────────────────────────────────────────┐
│ ⚡ LOADING SYSTEM HMI 3D - DỰ ÁN PHÂN LOẠI MUỖI 12 LOÀI    ● Connected │
├────────────────────────────────────────────────────────────────────────┤
│ (1) Ray Tách Muỗi | (2) Băng Chuyền 3D (Cam AI) | (3) Máng | (4) Đĩa3D │
│ 📷 Camera AI đặt ở ĐẦU BĂNG CHUYỀN                12 Lọ 12 Màu         │
├──────────────────────────────────┬─────────────────────────────────────┤
│ ⚙ CẤU HÌNH VẬT LÝ & ĐIỀU TỐC     │ 🔧 CAN THIỆP THỦ CÔNG (LEVEL 4/5)   │
│ L(Cam)=0.5m | H=0.3m | V=0.8m/s  │ [-0.5° Fine Left]  [+0.5° Fine Right│
│ Tần số Hz: [__8192__]            │ Offset: 110 pulses                  │
│ [✔ XÁC NHẬN CẤU HÌNH THÔNG SỐ]   │ Adjustments: 10                     │
│ [🎲 THẢ 1 MUỖI] [▶ TỰ ĐỘNG THẢ]  │ [Lưu Calibration Vĩnh Viễn]         │
├──────────────────────────────────┴─────────────────────────────────────┤
│ 📋 NHẬT KÝ VẬN HÀNH THỜI GIAN THỰC (REAL-TIME LOG CONSOLE) [Xóa Log]    │
│ [11:35:10] [DISPENSE] 🎲 Phễu Nạp: [1. Aedes aegypti] ➔ Lọ #1 (0°)     │
│           ├─ Vị trí: Lọ #7 (180°) ➔ Lọ #1 (0°) | Δθ = -180.0°         │
│           └─ Tần số: f = 20000 Hz | t_motor = 1.64s | t_drop = 0.87s    │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. QUY TRÌNH THẢ MUỖI 12 LOÀI TỰ ĐỘNG (12-JAR DEDICATED SORTING)

Hệ thống quản lý **12 lọ hứng riêng biệt cho 12 loài muỗi** (Lọ 1 đến Lọ 12 tương ứng 1-to-1 với 12 loài muỗi sinh học):

| Lọ # | Tên Loài Muỗi | Màu Sắc Nhận Diện trên HMI 3D |
|:---:|:---|:---:|
| **Lọ 1** | 1. Aedes aegypti (Vằn) | Đỏ tươi (`#dc2626`) |
| **Lọ 2** | 2. Anopheles (Sốt rét) | Xanh dương (`#2563eb`) |
| **Lọ 3** | 3. Culex quinquefasciatus | Xanh lá (`#16a34a`) |
| **Lọ 4** | 4. Mansonia uniformis | Tím đậm (`#9333ea`) |
| **Lọ 5** | 5. Aedes albopictus | Cam tươi (`#ea580c`) |
| **Lọ 6** | 6. Anopheles minimus | Xanh cyan (`#0891b2`) |
| **Lọ 7** | 7. Culex tritaeniorhynchus | Hồng cánh sen (`#db2777`) |
| **Lọ 8** | 8. Armigeres subalbatus | Nâu sẫm (`#78350f`) |
| **Lọ 9** | 9. Coquillettidia | Xám đá (`#475569`) |
| **Lọ 10** | 10. Toxorhynchites | Xanh ngọc (`#0d9488`) |
| **Lọ 11** | 11. Culiseta annulata | Vàng chanh (`#65a30d`) |
| **Lọ 12** | 12. Psorophora ferox | Tím nhạt (`#7c3aed`) |

### Quy trình Vận hành:
1. Nhấn nút **`[🎲 THẢ 1 MUỖI NGẪU NHIÊN]`**:
   - Phễu nạp tự động sinh ngẫu nhiên 1 loài muỗi $S \in \{1..12\}$.
   - Hệ thống tra cứu Lọ mục tiêu $J$ tương ứng loài $S$.
   - Thuật toán Auto-Speed tính toán góc quay $\Delta\theta$, thời gian rơi $t_{drop}$ và tần số Servo $f$ (Hz).
   - Gửi gói JSON chứa `frequency` xuống Rust Backend $\rightarrow$ Servo quay mâm xoay về đúng vị trí hứng muỗi.
2. Nhấn nút **`[▶ TỰ ĐỘNG THẢ CONTINUOUS]`**:
   - Hệ thống tự động lặp lại quy trình thả muỗi liên tục sau mỗi $2.5\text{s}$.
   - Nhấn nút một lần nữa để dừng vòng lặp tự động.

---

## 3. THUẬT TOÁN TÍNH TỐC ĐỘ & CẤU HÌNH VẬT LÝ

### 3.1. Các ô nhập thông số vật lý trên Control Panel
- `L(Cam-Rìa) m`: Khoảng cách từ Camera AI (gắn ở đầu băng chuyền) tới rìa băng chuyền (mét, mặc định `0.5m`).
- `H(Rìa-Lọ) m`: Độ cao từ rìa băng chuyền xuống miệng lọ (mét, mặc định `0.3m`).
- `V(Băng chuyền) m/s`: Tốc độ di chuyển của băng chuyền (m/s, mặc định `0.8 m/s`).
- `⚡ Tần số Servo Thủ Công (Hz)`: Ô nhập tần số phát xung thủ công (mặc định `8192 Hz`).

### 3.2. Nút "✔ XÁC NHẬN & ÁP DỤNG CẤU HÌNH THÔNG SỐ"
* Mỗi khi bạn thay đổi các thông số vật lý hoặc tần số thủ công, hãy bấm nút **`[✔ XÁC NHẬN & ÁP DỤNG CẤU HÌNH THÔNG SỐ]`**.
* Hệ thống sẽ tính toán lại thời gian rơi $t_{drop}$, cập nhật thuật toán điều tốc và xuất log màu sắc xác nhận trên Log Console.

---

## 4. CAN THIỆP THỦ CÔNG & TỰ HỌC BÙ SAI SỐ (LEVEL 4 & 5)

1. Khi phát hiện mâm xoay bị lệch góc do quán tính cơ khí:
   - Nhấn nút **`-0.5° Fine Left`** nếu đĩa quay dư sang phải.
   - Nhấn nút **`+0.5° Fine Right`** nếu đĩa quay chưa tới.
2. Thuật toán **Exponential Moving Average (EMA)** trong Rust Backend sẽ tự động cập nhật xung bù `offset_pulses` và lưu vĩnh viễn vào file `calibration.json`.

---

## 5. THAM CHUYỂN BẢNG THÔNG SỐ VÀ CÔNG THỨC

### Công thức Quy đổi Góc $\rightarrow$ Xung & Tần số Servo

$$P = \text{round}\left( \theta \times \frac{131072}{360} \right)$$

$$t_{\text{motor}} = \frac{P}{f_{\text{Hz}}}$$

| Góc Quay ($\theta$) | Số Xung Gốc (17-bit) | Tần số Auto ($f$) | Thời gian Quay ($t$) |
|:---:|:---:|:---:|:---:|
| **$30.0^\circ$** | $10,923\text{ xung}$ | $12,522\text{ Hz}$ | $0.87\text{s}$ ($\le 1.0\text{s}$) |
| **$90.0^\circ$** | $32,768\text{ xung}$ | $20,000\text{ Hz}$ | $1.64\text{s}$ |
| **$180.0^\circ$** | $65,536\text{ xung}$ | $20,000\text{ Hz}$ | $3.28\text{s}$ |

### Bản đồ Cổng & Thanh ghi Modbus RTU PLC Delta
- `D100 - D101` (Modbus `0x1064`): Thanh ghi 32-bit chứa vị trí xung cần băm.
- `D102 - D103` (Modbus `0x1066`): Thanh ghi 32-bit chứa tần số xung $f$ (Hz).
- `M0` (Modbus `0x0800`): Coil kích hoạt lệnh băm xung `DDRVI`.
- `M1029`: Cờ báo hoàn thành băm xung (tự động RST M0).
