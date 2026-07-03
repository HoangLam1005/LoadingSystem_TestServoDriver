# Task Speed Calculation Module ⚙️
> **Module tính toán động học và tạo biên dạng gia tốc hình thang cho đĩa xoay và băng chuyền.**

---

## 1. Giới thiệu tổng quan
Module `motion_calc.py` nằm trong phân hệ **Embedded Control (Module 3)** của dự án **MosquitoSortingLab**. Module này cung cấp các thuật toán tính toán động học cốt lõi để chuyển đổi các yêu cầu chuyển động từ tầng AI (vận tốc dài m/s hoặc số lọ đích) thành các tham số điều khiển trực tiếp cho động cơ Servo (vòng/phút - RPM và tần số phát xung - Hz).

### Chức năng chính:
*   **Tính toán động học băng chuyền (Conveyor Belt):** Đổi từ tốc độ dài đặt ($v$ m/s) sang RPM của động cơ và tần số phát xung tương ứng sau hộp số giảm tốc cơ khí.
*   **Tính toán động học đĩa xoay (Rotary Disc):** Xác định chiều quay tối ưu nhất (quay thuận CW hoặc quay ngược CCW) để di chuyển giữa 10 vị trí lọ thủy tinh trên đĩa xoay theo quãng đường ngắn nhất.
*   **Tạo biên dạng hình thang (Trapezoidal Motion Profile):** Tự động sinh phân bổ số xung và tần số cho 3 giai đoạn (Tăng tốc -> Tốc độ đều -> Giảm tốc) giúp đĩa xoay khởi động và dừng mượt mà, tránh rung lắc cơ khí. Tự động chuyển đổi thành biên dạng hình tam giác (Triangle Profile) nếu quãng đường di chuyển quá ngắn.

---

## 2. Cấu trúc thư mục làm việc
```
feature/Speed_Calculation_Demo
├── motion_calc.py                        # Mã nguồn thuật toán chính
├── Hướng dẫn giải thích code Task 3.docx  # Tài liệu giải thích chi tiết chương trình
└── README.md                             # Tài liệu giới thiệu nhanh này
```

---

## 3. Hướng dẫn sử dụng nhanh

### 3.1. Chạy chương trình Demo
Chương trình tích hợp sẵn hàm demo thực tế. Chạy trực tiếp file code để quan sát kết quả tính toán động học:
```bash
python motion_calc.py
```

### 3.2. Ví dụ tích hợp code
```python
from motion_calc import (
    calculate_conveyor_speed, ConveyorParams,
    calculate_disc_rotation, DiscParams
)

# 1. Cấu hình & tính toán băng chuyền (con lăn Ø50mm, hộp số giảm tốc 5:1)
conveyor_cfg = ConveyorParams(roller_diameter_mm=50.0, gear_ratio=5.0)
res_conv = calculate_conveyor_speed(target_speed_ms=0.1, params=conveyor_cfg)
print(f"Motor RPM: {res_conv.motor_rpm:.2f} RPM | Pulse Freq: {res_conv.pulse_frequency_hz:.0f} Hz")

# 2. Cấu hình & tính toán quay đĩa xoay (di chuyển từ lọ 1 đến lọ 4)
disc_cfg = DiscParams(num_jars=10)
res_disc = calculate_disc_rotation(current_jar=1, target_jar=4, params=disc_cfg)
print(f"Tổng số xung: {res_disc.total_pulses} | Chiều quay: {res_disc.direction.name}")
```

---

## 4. Công thức toán học cốt lõi

### Động học băng chuyền:
$$\text{RPM} = \frac{v \times 60}{\pi \times D \times \text{gear\_ratio}}$$

$$\text{Pulse\_Freq (Hz)} = \frac{\text{RPM} \times \text{PPR} \times \text{CMX}}{60 \times \text{CDV}}$$

### Động học đĩa xoay:
$$\text{Pulses} = \frac{\text{Angle\_deg}}{360^\circ} \times \text{PPR} \times \frac{\text{CMX}}{\text{CDV}}$$

$$\text{Pulses\_accel} = \frac{\text{Peak\_frequency} \times \text{Accel\_time}}{2}$$

---

