# BÁO CÁO KẾT QUẢ THỰC TẬP & NGHỆM THU HỆ THỐNG LOADING SYSTEM - VERSION 1.2

**Đơn vị:** Dự Án Tự Động Hóa Dây Chuyền Phân Loại & Đóng Lọ Muỗi Công Nghiệp  
**Đội ngũ Sinh viên Thực tập:**  
1. **Vương Quốc Khánh**
2. **Trương Hoàng Nam**
3. **Trần Doãn Hoàng Lâm**

**Ngày báo cáo:** 22/07/2026  
**Phiên bản hệ thống:** Version 1.2

---

## 1. ĐẶT VẤN ĐỀ VÀ MỤC TIÊU DỰ ÁN

Trong các viện nghiên cứu y học dự phòng và nhà máy công nghệ sinh học, việc phân loại ngẫu nhiên và đóng lọ các dòng muỗi phục vụ thí nghiệm đòi hỏi tính **chính xác tuyệt đối $100\%$**, không được phép lẫn lộn giữa các loài muỗi sinh học. Đồng thời, đĩa xoay mang 12 lọ hứng phải có khả năng di chuyển đến đúng vị trí hứng **trước khi phôi muỗi rơi từ băng chuyền xuống**.

Nhóm sinh viên thực tập đã nghiên cứu, thiết kế và chế tạo thành công **Hệ Thống LoadingSystem Version 1.2** nhằm giải quyết triệt để bài toán này.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      SƠ ĐỒ TỔNG QUAN HỆ THỐNG V1.2                      │
│                                                                         │
│  [Camera AI Vision Sensor] ──► [Băng Chuyền] ──► [Phễu / Máng Gạt 3D]    │
│  (Nhận diện 12 loài muỗi)      (v_belt m/s)      (H_drop m)             │
│                                                       │                 │
│                                                       ▼                 │
│  [PLC Delta DVP14SS2] ◄── [Modbus RTU] ◄── [Đĩa Xoay 12 Lọ 12 Loài]    │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. KẾT QUẢ ĐẠT ĐƯỢC VỀ MẶT KỸ THUẬT (V1.2 HIGHLIGHTS)

### 2.1. Đĩa Xoay 12 Lọ Phân Loại 12 Loài Muỗi Độc Quyền
- Xây dựng sơ đồ 1-to-1 mapping: 12 vị trí lọ trên mâm xoay tương ứng độc quyền với 12 dòng muỗi y tế phổ biến (*Aedes aegypti, Anopheles, Culex, Mansonia,...*).
- Loại bỏ hoàn toàn rủi ro đóng nhầm lọ, đảm bảo tính nguyên vẹn của mẫu sinh học.

### 2.2. Thuật Toán Điều Tốc Servo Tự Động Theo Thời Gian Rơi Vật Lý
- Tích hợp mô hình động lực học rơi tự do:
  $$t_{drop} = \frac{L_{cam}}{v_{belt}} + \sqrt{\frac{2 H_{drop}}{g}}$$
- Đĩa xoay tự động tính toán tần số phát xung $f$ (Hz) gửi xuống PLC Delta:
  - Khi khoảng cách xoay ngắn ($30^\circ$): Motor xoay mượt mà, êm ái trong $1.0\text{s}$.
  - Khi khoảng cách xoay xa ($180^\circ$): Motor tự động tăng tốc (lên tới $20,000\text{ Hz}$) để đón kịp phôi muỗi trước khi rơi.

### 2.3. Mở Rộng Giao Thức IPC JSON & Modbus RTU 8-bit
- Đã khắc phục triệt để lỗi Modbus Timeout do đứt MSB bằng việc chuyển đổi firmware PLC Delta sang mã **`H87`** (9600 bps, 8 Data bits, Even parity, 1 Stop bit, RTU).
- Gói JSON từ HMI truyền trực tiếp tần số $f$ (Hz) xuống các thanh ghi `D102/D103` của PLC.

### 2.4. Trực Quan Hóa HMI 3D & Khung Log Console Chuyên Nghiệp
- Vị trí Camera AI Vision Sensor được đưa về **đầu băng chuyền 3D**, khớp $100\%$ mô hình cơ khí.
- Số lượng muỗi hiển thị **to, đậm ngay CHÍNH GIỮA THÂN LỌ 3D** với 12 màu sắc phân biệt sinh động.
- Bảng Log Console phóng to, phân màu nhãn tag thời gian thực, phục vụ công tác giám sát và nghiệm thu trực tiếp.

---

## 3. BẢNG TỔNG HỢP NGHỆM THU TÍNH NĂNG (CHECKLIST)

| STT | Tính năng nghiệm thu | Trạng thái | Đánh giá / Ghi chú |
|:---:|:---|:---:|:---|
| 1 | Khởi động Backend Daemon MOCK & REAL | **PASS** | Chạy ổn định trên 127.0.0.1:5555 |
| 2 | Kết nối Modbus RTU COM5 9600 8E1 | **PASS** | Phản hồi tức thì < 50ms, không timeout |
| 3 | Thả muỗi ngẫu nhiên 12 loài | **PASS** | Đĩa tự động định vị đúng Lọ loài tương ứng |
| 4 | Thuật toán điều tốc tự động $\le 1.0\text{s}$ | **PASS** | Tự động điều chỉnh $f$ từ $1,000\text{ Hz}$ đến $20,000\text{ Hz}$ |
| 5 | Nút Xác Nhận Cấu Hình Thông Số | **PASS** | Đánh dấu xác nhận và lưu log `[CONFIG]` |
| 6 | Trực quan 3D, Số muỗi giữa thân lọ | **PASS** | Màu sắc đẹp, số đếm nổi bật dễ quan sát |
| 7 | Tự học bù sai số EMA (Level 5) | **PASS** | Tự động lưu `calibration.json` khi Fine Tune |

---

## 4. KẾT LUẬN VÀ HƯỚNG PHÁT TRIỂN

Dưới sự hướng dẫn của các cán bộ quản lý và sự nỗ lực làm việc nhóm của 3 sinh viên thực tập (**Vương Quốc Khánh, Trương Hoàng Nam, Trần Doãn Hoàng Lâm**), hệ thống **LoadingSystem Version 1.2** đã hoàn thành vượt mức các tiêu chí đề ra. 

Hệ thống sẵn sàng được bàn giao và đưa vào vận hành thử nghiệm tại dây chuyền sản xuất thực tế.
