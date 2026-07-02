# MosquitoSortingLab 🦟
> **Automated mosquito sorting system using AI vision and embedded control.**

---

## 1. Project Overview
**MosquitoSortingLab** (phát triển dựa trên kiến trúc **LoadingSystem**) là hệ thống phân loại tự động cấp công nghiệp. Hệ thống tự động nhận diện, phân loại mẫu muỗi (mosquito specimens) và định vị đĩa xoay để đưa chúng vào các lọ thủy tinh tương ứng bằng cách kết hợp:

*   **AI Vision:** Camera 4K thu thập hình ảnh + Mô hình học sâu (YOLO/MobileNet) chạy trên Jetson Nano hoặc Raspberry Pi để nhận diện và gán nhãn loài muỗi theo thời gian thực.
*   **Embedded Control:** Vi điều khiển ESP32 được lập trình bằng ngôn ngữ **Rust** (sử dụng Embassy framework) chịu trách nhiệm nhận dữ liệu phân loại và phát xung Pulse/Direction điều khiển động cơ.
*   **Servo Drive:** Bộ Drive **CSD7-02DX1** (RS Automation, công suất 200W) điều khiển động cơ Servo **CSMT-02BR1ABT3** quay đĩa xoay phân loại (turntable) chính xác tuyệt đối.
*   **Communication:** Kết nối không dây Wifi/Bluetooth đảm nhiệm việc truyền gói tin chứa nhãn phân loại từ bộ xử lý AI Host (RPi) xuống vi điều khiển ESP32.

---

## 2. System Architecture

```
Camera (4K) ──capture──→ Raspberry Pi (AI Inference)
                              │
                    Wifi / Bluetooth (Wireless)
                              │
                              ▼
                         ESP32 (Rust)
                              │
                     Pulse / Direction (24V Logic)
                              │
                              ▼
                   Servo Drive CSD7-02DX1
                              │
                         3-phase AC
                              │
                              ▼
                  Servo Motor CSMT-02BR1ABT3
                              │
                    Đĩa xoay phân loại muỗi
                      (10 lọ thủy tinh)
```

---

