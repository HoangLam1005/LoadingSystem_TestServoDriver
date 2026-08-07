# ĐẶC TẢ THUẬT TOÁN & HƯỚNG DẪN VẬN HÀNH MODULE ĐIỀU CHỈNH GIA TỐC S-CURVE CHO ĐĨA XOAY

Tài liệu này giải thích toàn bộ lý thuyết toán học, chi tiết mã nguồn (Rust) và quy trình cài đặt, cấu hình, vận hành trực tiếp trên hệ thống phần cứng (PLC Delta DVP14SS2, Servo Driver RS Automation CSD7, Motor Servo & Mâm đĩa xoay).

---

## 1. TỔNG QUAN VẤN ĐỀ

### 1.1. Hiện trạng hệ thống
Hệ thống LoadingSystem điều khiển mâm đĩa xoay mang 12 lọ vật mẫu thông qua lệnh băm xung vị trí tốc độ cao `DDRVI` của PLC Delta DVP14SS2. Trong thiết kế ban đầu, khi Backend gửi lệnh quay, PLC sẽ lập tức phát xung từ $0\text{ Hz}$ lên tần số tối đa $f_{\text{max}}$ (ví dụ $5,000\text{ Hz}$) trong đúng **1 scan cycle PLC (~5ms)**.

```
Tần số (Hz)
  ▲
  │  ┌────────────────────────┐
  │  │                        │   ← Bước nhảy vận tốc đột ngột (Step change)
  │  │                        │   Gia tốc a = ∞ tại t = 0 và t = T
  └──┴────────────────────────┴───► Thời gian (s)
```

### 1.2. Hậu quả cơ khí & lý do cần thuật toán S-Curve
1. **Sốc quán tính (Jerk spike):** Gia tốc là đạo hàm của vận tốc ($a = \frac{dv}{dt}$). Bước nhảy vận tốc từ $0 \to f_{\text{max}}$ khiến gia tốc tức thời vọt lên vô cùng ($a \to \infty$). Lực quán tính $F = m \cdot a$ đột ngột tác động làm các lọ hạt điều trên mâm đĩa bị xê dịch hoặc rơi rớt.
2. **Giật cơ khí:** Khớp nối giữa trục motor và mâm đĩa có độ rơ (backlash). Cú sốc lực làm các răng cơ khí va đập mạnh, gây tiếng ồn tần số thấp và giảm tuổi thọ truyền động.
3. **Hiện tượng trôi (Overshoot):** Khi dừng từ tốc độ cao về $0\text{ Hz}$ ngay lập tức, quán tính mâm đĩa kéo trôi góc quay, gây sai số định vị.

### 1.3. Giải pháp S-Curve
Thuật toán S-Curve chia quỹ đạo chuyển động thành **$N$ phân đoạn nhỏ (segments)**. Vận tốc băm xung tăng dần theo đường cong hình chữ S khi khởi động, duy trì tốc độ ổn định ở giữa, và giảm dần về tốc độ tối thiểu trước khi dừng.

---

## 2. MÔ HÌNH TOÁN HỌC S-CURVE

### 2.1. Hàm nội suy Hermite smoothstep
Để đảm bảo gia tốc bắt đầu từ $0$ và kết thúc tại $0$ mà không có điểm gấp khúc, thuật toán sử dụng hàm nội suy bậc 3 **Smoothstep**:

$$S(t) = 3t^2 - 2t^3 \quad \text{với } t \in [0, 1]$$

* **Đặc tính đạo hàm:**
  $$\frac{dS}{dt} = 6t - 6t^2 = 6t(1 - t)$$
  * Tại $t = 0 \implies S'(0) = 0$ (Gia tốc ban đầu bằng 0).
  * Tại $t = 1 \implies S'(1) = 0$ (Gia tốc kết thúc bằng 0).
  * Tại $t = 0.5 \implies S'(0.5) = 1.5$ (Gia tốc đạt đỉnh ở giữa pha).

### 2.2. Mô hình phân chia 3 pha
Quãng đường quay được chia thành 3 pha theo tỷ lệ thiết lập (20% tăng tốc - 60% chạy đều - 20% giảm tốc):

```
Tần số (Hz)
  ▲                      Phase 2 (Cruise - 60%)
f_max ───────────────┬───────────────────────────┬───────────────
     ╱               │                           │               ╲
    ╱  Phase 1       │                           │  Phase 3       ╲
   ╱ (Accel - 20%)   │                           │ (Decel - 20%)   ╲
f_min ───────────────┴───────────────────────────┴──────────────────┴──► Thời gian (s)
```

1. **Pha 1 — Tăng tốc:** Tần số tại phân đoạn $i \in [1, N_{\text{accel}}]$:
   $$f_i = f_{\text{min}} + (f_{\text{max}} - f_{\text{min}}) \cdot S\left(\frac{i}{N_{\text{accel}}}\right)$$
2. **Pha 2 — Chạy ổn định (Cruise):** Tần số giữ nguyên:
   $$f_i = f_{\text{max}}$$
3. **Pha 3 — Giảm tốc:** Tần số tại phân đoạn $j \in [1, N_{\text{decel}}]$:
   $$f_j = f_{\text{max}} - (f_{\text{max}} - f_{\text{min}}) \cdot S\left(\frac{j}{N_{\text{decel}}}\right)$$

### 2.3. Thuật toán phân bổ xung bảo toàn
Số xung của mỗi segment $P_i$ được phân bổ tỷ lệ thuận với tần số $f_i$:

$$P_i = \text{round}\left(P_{\text{total}} \cdot \frac{f_i}{\sum_{k=1}^N f_k}\right)$$

Để tránh sai số làm tròn số nguyên làm rơi rớt xung (sai số góc vị trí), phân đoạn cuối cùng nhận toàn bộ số xung còn dư:
$$P_N = P_{\text{total}} - \sum_{k=1}^{N-1} P_k$$

---

## 3. PHÂN TÍCH CHI TIẾT MÃ NGUỒN

Mã nguồn của thuật toán gia tốc nằm tại hai file chính:
* `rust_backend/src/calibration/acceleration_profile.rs` (Bộ sinh profile)
* `rust_backend/src/hardware/rotating_disc.rs` (Bộ điều khiển thực thi)

---

### 3.1. Constant & cấu trúc dữ liệu

```rust
const DEFAULT_MIN_FREQUENCY: u32 = 200;
const DEFAULT_TOTAL_SEGMENTS: u32 = 10;
const DEFAULT_ACCEL_RATIO: f64 = 0.20;
const DEFAULT_DECEL_RATIO: f64 = 0.20;
const MIN_PULSES_FOR_PROFILE: u32 = 500;
```
* `DEFAULT_MIN_FREQUENCY = 200`: Tần số khởi động tối thiểu ($200\text{ Hz}$) giúp Servo thắng ma sát tĩnh ban đầu mượt hơn.
* `DEFAULT_TOTAL_SEGMENTS = 10`: Số phân đoạn tối ưu cân bằng giữa độ trơn chuyển động và lưu lượng truyền thông RS485.
* `MIN_PULSES_FOR_PROFILE = 500`: Ngưỡng xung tối thiểu ($\approx 1.37^\circ$). Nếu chuyển động nhỏ hơn 500 xung, thuật toán tự động fallback về lệnh đơn để tối ưu thời gian.

#### Struct `MotionSegment`
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct MotionSegment {
    pub pulses: i32,            // Số xung (dương = quay phải, âm = quay trái)
    pub frequency: u32,         // Tần số phát xung (Hz)
    pub duration_ms: u64,       // Thời gian thực thi dự kiến (ms)
    pub phase: &'static str,    // Pha chuyển động ("accel", "cruise", "decel")
}
```

#### Struct `AccelerationProfile`
```rust
#[derive(Debug, Clone)]
pub struct AccelerationProfile {
    min_frequency: u32,
    total_segments: u32,
    accel_ratio: f64,
    decel_ratio: f64,
}
```

---

### 3.2. Hàm toán học `smoothstep(t: f64) -> f64`

Hàm giải quyết bài toán biến đổi gia tốc giật cục (Jerk). Trong vật lý chuyển động cơ khí, nếu vận tốc tăng trực tiếp dạng tuyến tính (nhảy trực tiếp từ $0\text{ Hz}$ lên $f_{\text{max}}$), gia tốc sẽ bị biến đổi đột ngột tại điểm khởi đầu và điểm kết thúc (đạo hàm không liên tục). Hàm `smoothstep` tạo ra một đường cong nội suy Hermite mượt mà với đạo hàm tại 2 đầu $t=0$ và $t=1$ đều bằng $0$.

* **Tác động đến hệ thống:**
  * **Servo Driver CSD7:** Tín hiệu xung tần số đầu vào tăng mượt, giúp dòng điện cấp vào các cuộn dây Stator của động cơ Servo tăng dần từ từ, tránh hiện tượng quá dòng (current spike) làm báo lỗi Overcurrent trên Driver.
  * **Mâm đĩa & vật mẫu:** Moment xoắn $M = I \cdot \alpha$ tăng dần từ $0$, ma sát tĩnh giữa lọ hạt điều và mâm đĩa không bị phá vỡ, triệt tiêu hiện tượng trượt lọ hay rơi rớt.

```rust
// Hàm toán học nội suy Hermite smoothstep (S-Curve)
// Nhận vào tham số t ∈ [0.0, 1.0], trả về hệ số gia tốc mượt S(t) ∈ [0.0, 1.0]
pub fn smoothstep(t: f64) -> f64 {
    // 1. Ép biên t nghiêm ngặt trong khoảng [0.0, 1.0] để tránh lỗi tràn số hoặc sai số số thực
    let t = t.clamp(0.0, 1.0);
    
    // 2. Tính công thức toán học bậc 3 Hermite: S(t) = 3t² - 2t³ (đạo hàm S'(0) = 0 và S'(1) = 0)
    t * t * (3.0 - 2.0 * t) // Tự động trả về giá trị biểu thức cuối cùng (implicit return)
}
```

---

### 3.3. Thuật toán lập kế hoạch `generate_segments()`

Hàm đóng vai trò **lập kế hoạch quỹ đạo**. Nó giải quyết bài toán chia nhỏ một quãng đường lớn (tổng số xung) thành chuỗi $N$ phân đoạn chuyển động (`Vec<MotionSegment>`) gồm 3 pha rõ ràng (Tăng tốc $\to$ chạy ổn định $\to$ giảm tốc) với các mức tần số khác nhau. Đặc biệt, nó giải quyết triệt để bài toán **sai số làm tròn số nguyên** bằng cách gom xung lẻ vào segment cuối cùng. Ngoài ra, hàm bổ sung tính năng **Short-Move detection** (nếu $< 500\text{ xung}$ tự động chuyển về 1 segment).

* **Tác động đến hệ thống:**
  * **PLC Delta DVP14SS2:** PLC nhận chuỗi phân đoạn có tần số tăng dần $200\text{Hz} \to 5000\text{Hz} \to 200\text{Hz}$, giúp bộ đếm xung tốc độ cao băm xung mượt mà.
  * **Bảo toàn vị trí 100%:** Thuật toán đảm bảo tổng số xung phát ra của $N$ segment bằng chính xác $100\%$ tổng số xung gốc $P_{\text{total}}$, không bị sai lệch dù chỉ $1\text{ xung}$ ($0.00275^\circ$).

```rust
// Hàm lập kế hoạch quỹ đạo chuyển động (Motion Planner)
// Chia tổng số xung thành N phân đoạn (MotionSegment) với các mức tần số S-Curve
pub fn generate_segments(
    &self,
    total_pulses: i32,    // Tổng số xung cần phát (dương = quay phải, âm = quay trái)
    max_frequency: u32,   // Tần số đỉnh tối đa muốn đạt được (Hz)
) -> Option<Vec<MotionSegment>> {
    // 1. Lấy giá trị tuyệt đối của tổng số xung chuyển đổi sang kiểu số nguyên không dấu u32
    let abs_pulses = total_pulses.unsigned_abs();

    // 2. Short-Move Detection: Nếu xung quá nhỏ (< 500 xung ≈ 1.37°), trả về None để fallback dùng single-shot
    if abs_pulses < MIN_PULSES_FOR_PROFILE {
        return None;
    }

    // 3. Xác định dấu hướng quay (1 = quay phải / chiều dương, -1 = quay trái / chiều âm)
    let sign = if total_pulses >= 0 { 1i32 } else { -1i32 };
    
    // 4. Chuyển đổi f_min và f_max sang f64 (đảm bảo f_max lớn hơn f_min ít nhất 100 Hz)
    let f_min = self.min_frequency as f64;
    let f_max = (max_frequency.max(self.min_frequency + 100)) as f64;

    // 5. Tính số lượng phân đoạn cho pha Tăng tốc (accel_ratio = 20%) và Giảm tốc (decel_ratio = 20%)
    let n_accel = ((self.total_segments as f64 * self.accel_ratio).round() as u32).max(1);
    let n_decel = ((self.total_segments as f64 * self.decel_ratio).round() as u32).max(1);
    
    // 6. Tính số lượng phân đoạn cho pha Chạy đều (Cruise) bằng phép trừ chống tràn âm saturating_sub
    let n_cruise = self.total_segments.saturating_sub(n_accel + n_decel).max(1);
    
    // 7. Tổng số phân đoạn thực tế của toàn bộ chuỗi chuyển động
    let n_total = n_accel + n_cruise + n_decel;

    // 8. Khởi tạo vector chứa danh sách tần số, cấp phát bộ nhớ trước với sức chứa n_total để tối ưu hiệu năng
    let mut frequencies: Vec<f64> = Vec::with_capacity(n_total as usize);

    // --- Phase 1: Pha Tăng Tốc (Acceleration) ---
    // Nội suy tần số tăng dần từ f_min đến f_max bằng hàm Smoothstep
    for i in 0..n_accel {
        let t = (i as f64 + 1.0) / n_accel as f64;         // Tỷ lệ tiến trình pha [0.0, 1.0]
        let f = f_min + (f_max - f_min) * smoothstep(t);  // Công thức: f_i = f_min + Δf * S(t)
        frequencies.push(f);                               // Thêm tần số vào danh sách
    }

    // --- Phase 2: Pha Chạy Đều (Cruise) ---
    // Duy trì tần số ổn định ở mức tối đa f_max cho tất cả n_cruise segments
    for _ in 0..n_cruise {
        frequencies.push(f_max);                           // Đẩy f_max vào vector
    }

    // --- Phase 3: Pha Giảm Tốc (Deceleration) ---
    // Nội suy tần số giảm dần từ f_max về f_min bằng hàm Smoothstep
    for i in 0..n_decel {
        let t = (i as f64 + 1.0) / n_decel as f64;         // Tỷ lệ tiến trình pha giảm [0.0, 1.0]
        let f = f_max - (f_max - f_min) * smoothstep(t);  // Công thức: f_j = f_max - Δf * S(t)
        frequencies.push(f);                               // Thêm tần số vào danh sách
    }

    // 9. Tính tổng tất cả tần số trong chuỗi để làm mẫu số chia tỷ lệ phân bổ xung
    let freq_sum: f64 = frequencies.iter().sum();
    
    // 10. Khởi tạo danh sách kết quả chứa MotionSegment và biến theo dõi tổng số xung đã phân bổ
    let mut segments: Vec<MotionSegment> = Vec::with_capacity(n_total as usize);
    let mut allocated_pulses: u32 = 0;

    // 11. Vòng lặp phân bổ xung cho từng phân đoạn trong danh sách tần số
    for (idx, &freq) in frequencies.iter().enumerate() {
        let is_last = idx == frequencies.len() - 1; // Kiểm tra xem có phải segment cuối cùng hay không

        // THUẬT TOÁN BẢO TOÀN TỔNG XUNG: Gom toàn bộ xung còn dư vào segment cuối cùng
        let seg_pulses = if is_last {
            abs_pulses - allocated_pulses           // Segment cuối = Tổng xung ban đầu - Xung đã phân bổ
        } else {
            let p = ((freq / freq_sum) * abs_pulses as f64).round() as u32; // P_i = P_total * (f_i / Σf)
            p.max(1).min(abs_pulses - allocated_pulses)                     // Đảm bảo ít nhất 1 xung
        };

        // Cộng dồn số xung đã phân bổ cho segment này
        allocated_pulses += seg_pulses;
        
        // Ép kiểu tần số về u32 và đảm bảo luôn ≥ f_min (tránh tần số quá thấp làm motor mất moment)
        let freq_u32 = (freq.round() as u32).max(self.min_frequency);

        // Tính thời gian thực thi ước tính cho segment (ms) + 20ms margin an toàn truyền thông RS485
        let duration_ms = if freq_u32 > 0 {
            ((seg_pulses as f64 / freq_u32 as f64) * 1000.0) as u64 + 20
        } else {
            50
        };

        // Gán nhãn pha chuyển động cho mục đích ghi log / debug
        let phase = if idx < n_accel as usize {
            "accel"
        } else if idx < (n_accel + n_cruise) as usize {
            "cruise"
        } else {
            "decel"
        };

        // Tạo cấu trúc MotionSegment hoàn chỉnh và đẩy vào vector kết quả
        segments.push(MotionSegment {
            pulses: sign * (seg_pulses as i32), // Gán dấu hướng quay (+quay phải, -quay trái)
            frequency: freq_u32,                 // Tần số phát xung (Hz)
            duration_ms,                         // Thời gian thực thi (ms)
            phase,                               // Nhãn pha ("accel", "cruise", "decel")
        });
    }

    // Trả về chuỗi phân đoạn chuyển động S-Curve hoàn chỉnh
    Some(segments)
}
```

---

### 3.4. Hàm tích hợp controller `rotate_degrees_smooth()`

Hàm đóng vai trò **thực thi tích hợp phần cứng**. Nó kết nối toàn bộ hệ thống lại với nhau: Nhận góc quay từ ứng dụng $\to$ đưa qua bộ bù sai số AI level 5 $\to$ tính tần số động $\to$ Gọi `generate_segments` $\to$ đẩy từng bản tin Modbus RTU xuống PLC và khóa luồng bất đồng bộ (`async sleep`) để đồng bộ nhịp băm xung.

* **Tác động đến  hệ thống:**
  * **Truyền thông RS485:** Quản lý quá trình truyền tin cực kỳ ổn định. Mỗi lần ghi vào thanh ghi `D100/D101` (xung) và `D102/D103` (tần số) và kích bit `M0`, hệ thống nhường CPU cho các tác vụ khác trong đúng `duration_ms` trước khi ghi đoạn tiếp theo.
  * **Motor Servo:** Quay mượt tuyệt đối từ đầu đến cuối hành trình, dừng chính xác tại vị trí đích mà không trôi góc.


```rust
// Hàm thực thi điều khiển đĩa xoay quay góc θ với gia tốc mượt S-Curve
// Tích hợp bù sai số tự học Level 5, tính tần số động, và giao tiếp Modbus RTU PLC
pub async fn rotate_degrees_smooth(
    &mut self,
    degrees: f64,                                // Góc quay mong muốn (độ dương)
    direction: Direction,                        // Enum hướng quay: Right (phải) hoặc Left (trái)
    accel_profile: Option<AccelerationProfile>,  // Profile tùy chỉnh (hoặc None = dùng mặc định)
) -> Result<(), LoadingError> {
    // 1. Validating góc quay đầu vào: Nếu góc âm, trả về lỗi ngay lập tức
    if degrees < 0.0 {
        return Err(LoadingError::Command(
            "Degrees must be positive. Use Direction to specify rotation direction.".into(),
        ));
    }

    // 2. Quy đổi góc quay ra số xung lý thuyết (17-bit encoder: 131,072 xung / 360°)
    let raw_pulses = self.degrees_to_pulses(degrees);

    // 3. Tích hợp AI Level 5: Cộng/Trừ giá trị offset_pulses bù sai số thích nghi tích lũy
    let final_pulses = match direction {
        Direction::Right => raw_pulses + self.calibration.offset_pulses,   // Quay phải = Xung dương + Offset
        Direction::Left => -(raw_pulses) + self.calibration.offset_pulses,  // Quay trái = Xung âm + Offset
    };

    // 4. Tính toán tần số đỉnh tối đa an toàn f_max dựa trên độ lớn góc quay (1000Hz -> 9000Hz)
    let max_freq = self.calculate_dynamic_frequency(degrees, Some(1.0), Some(1000), Some(9000));
    
    // 5. Khởi tạo profile gia tốc (lấy tham số người dùng truyền vào hoặc dùng mặc định)
    let profile = accel_profile.unwrap_or_default();

    // 6. Quyết định chiến lược phát xung: Dùng S-Curve profile N phân đoạn hay Fallback single-shot 1 lệnh
    let segments = if AccelerationProfile::should_use_profile(final_pulses) {
        match profile.generate_segments(final_pulses, max_freq) {
            Some(segs) => segs,                                           // Lập kế hoạch N segments thành công
            None => AccelerationProfile::single_shot(final_pulses, max_freq), // Fallback nếu xung quá nhỏ
        }
    } else {
        AccelerationProfile::single_shot(final_pulses, max_freq)           // Single-shot cho góc cực nhỏ
    };

    let total_segments = segments.len();

    // 7. Log thông tin khởi chạy quá trình quay mượt S-Curve
    tracing::info!(
        "Smooth rotate {:.2}° {} → {} segments, total_pulses={}, f_max={}Hz",
        degrees, direction, total_segments, final_pulses, max_freq
    );

    // 8. VÒNG LẶP THỰC THI PHẦN CỨNG: Duyệt qua từng phân đoạn MotionSegment và gửi xuống PLC
    for (idx, segment) in segments.iter().enumerate() {
        // Log chi tiết thông số phân đoạn hiện tại
        tracing::debug!(
            "  Segment [{}/{}] phase={}, pulses={}, freq={}Hz, duration={}ms",
            idx + 1, total_segments, segment.phase, segment.pulses, segment.frequency, segment.duration_ms
        );

        // Ghi số xung (D100/D101), tần số (D102/D103) và kích bit ON M0 qua giao thức Modbus RTU RS485
        self.execute_pulse_command(segment.pulses, segment.frequency).await?;

        // Tạm dừng luồng bất đồng bộ (async tokio sleep) đúng bằng duration_ms để chờ PLC băm xung xong
        wait_for_motor_completion(segment.pulses, segment.frequency).await;
    }

    // 9. Cập nhật góc quay theo dõi vị trí tích lũy nội bộ của controller
    match direction {
        Direction::Right => self.current_angle += degrees,
        Direction::Left => self.current_angle -= degrees,
    }

    // 10. Trả về Ok kết thúc lệnh thành công
    Ok(())
}
```
---

## 4. HƯỚNG DẪN CÀI ĐẶT & VẬN HÀNH TRÊN HỆ THỐNG THỰC TẾ

### 4.1. Sơ đồ đấu nối & cấu hình phần cứng

```
┌────────────────────────┐               ┌────────────────────────┐
│ Máy tính điều khiển    │  Cáp USB-RS485│ PLC Delta DVP14SS2     │
│ (Rust Backend Daemon)  ├──────────────►│ Cổng COM2 (D+ / D-)    │
└────────────────────────┘               └───────────┬────────────┘
                                                     │ Xung PULS / DIR
                                                     ▼
                                         ┌────────────────────────┐
                                         │ Servo Driver CSD7      │
                                         │ (RS Automation 17-bit) │
                                         └───────────┬────────────┘
                                                     │ Trục Motor
                                                     ▼
                                         ┌────────────────────────┐
                                         │ Mâm Đĩa Xoay 12 Lọ     │
                                         └────────────────────────┘
```

#### Bảng thông số truyền thông Modbus RTU chuẩn trên PLC Delta:
* **Cổng giao tiếp PLC:** COM2 (RS485)
* **Thanh ghi cấu hình truyền thông PLC (`D1120`):** `H87` (9600 bps, 8 Data bits, Even Parity, 1 Stop bit)
* **Địa chỉ Modbus Slave PLC:** `1`
* **Thanh ghi lệnh vị trí:** `D100` & `D101` (32-bit Pulse Count)
* **Thanh ghi lệnh tần số:** `D102` & `D103` (32-bit Frequency Hz)
* **Coil kích hoạt lệnh băm xung `DDRVI`:** `M0`

---

### 4.2. Cấu hình file `config.toml`

Mở file `rust_backend/config.toml` và chỉnh sửa tham số `port` từ `"MOCK"` thành cổng COM thực tế nối với USB-RS485:

#### Trực tiếp cấu hình trên Windows:
```toml
[serial]
# Cổng COM kết nối thực tế trên Windows (Kiểm tra trong Device Manager -> Ports)
port = "COM3"
baud_rate = 9600
data_bits = 8
stop_bits = 1
parity = "even"

[modbus]
slave_address = 1

[ipc]
host = "127.0.0.1"
port = 5555

[disc]
encoder_resolution = 131072
default_frequency = 5000
learning_coefficient = 0.1
calibration_file = "calibration.json"
```

#### Trên Linux / Raspberry Pi:
```toml
[serial]
port = "/dev/ttyUSB0" # Hoặc /dev/ttyS0
baud_rate = 9600
data_bits = 8
stop_bits = 1
parity = "even"
```

---

### 4.3. Biên dịch & khởi chạy Backend Daemon

1. Mở Terminal / PowerShell tại thư mục backend:
   ```bash
   cd E:\Thuc_tap\MKSOL\Projects\git.mksol.com\mks\myworkspace\loading-system\tutor-servo\plc-code\rust_backend
   ```

2. Biên dịch mã nguồn ở chế độ Release (Tối ưu hóa hiệu năng tối đa):
   ```bash
   cargo build --release
   ```

3. Khởi chạy Daemon kết nối phần cứng thực tế:
   ```bash
   cargo run --release -- config.toml
   ```

   **Log hiển thị thành công trên màn hình:**
   ```text
   INFO === LoadingSystem Backend Daemon ===
   INFO Version: 1.0.0
   INFO Config loaded from 'config.toml'
   INFO Mode: COM3
   INFO Connecting to PLC via COM3 at 9600 baud (8,even,1)...
   INFO IPC server listening on 127.0.0.1:5555
   ```

---

### 4.4. Điều khiển đĩa xoay từ Python HMI hoặc script test thực tế

Khi Daemon Rust đang chạy và kết nối với PLC qua `COM3`, bạn có thể gửi lệnh quay mượt S-Curve từ Python HMI hoặc bằng script Python ngắn sau:

#### Script Python điều khiển thực tế (`test_real_hardware.py`):

```python
import socket
import json
import struct
import time

def send_ipc_command(host, port, command_dict):
    """Gửi lệnh JSON qua TCP IPC Socket tới Rust Backend Daemon"""
    client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    client.connect((host, port))
    
    payload = json.dumps(command_dict).encode('utf-8')
    # Framing 4-byte length prefix (big-endian)
    header = struct.pack('>I', len(payload))
    
    client.sendall(header + payload)
    
    # Đọc response
    resp_header = client.recv(4)
    resp_len = struct.unpack('>I', resp_header)[0]
    resp_body = client.recv(resp_len).decode('utf-8')
    
    client.close()
    return json.loads(resp_body)

if __name__ == "__main__":
    print("--- KIỂM TRA QUAY ĐĨA S-CURVE TRÊN PHẦN CỨNG THỰC TẾ ---")
    
    # 1. Gửi lệnh quay 90 độ phải (Clockwise) áp dụng S-Curve
    cmd_rotate_90 = {
        "command": "ROTATE_RIGHT",
        "value": 90.0
    }
    
    print("Đang gửi lệnh quay 90° phải...")
    response = send_ipc_command("127.0.0.1", 5555, cmd_rotate_90)
    print("Phản hồi từ Daemon:", response)
    
    time.sleep(3)
    
    # 2. Gửi lệnh quay 180 độ trái (Counter-Clockwise)
    cmd_rotate_180 = {
        "command": "ROTATE_LEFT",
        "value": 180.0
    }
    
    print("Đang gửi lệnh quay 180° trái...")
    response = send_ipc_command("127.0.0.1", 5555, cmd_rotate_180)
    print("Phản hồi từ Daemon:", response)
```

#### Chạy script test phần cứng:
```bash
python test_real_hardware.py
```

---

## 5. CHẨN ĐOÁN & XỬ LÝ LỖI PHẦN CỨNG THỰC TẾ (TROUBLESHOOTING)

| Hiện tượng lỗi | Nguyên nhân lỗi | Cách khắc phục thực tế |
|:---|:---|:---|
| Motor quay bị giật nhẹ ở thời điểm bắt đầu | Tần số khởi động `f_min` ($200\text{ Hz}$) quá thấp so với tải trọng mâm đĩa | Tăng `min_frequency` trong `AccelerationProfile` từ $200\text{ Hz}$ lên $300\text{ Hz} - 400\text{ Hz}$ trong code |
| Mâm đĩa xoay bị rơ / sai góc nhẹ sau nhiều lần quay | Trục nối uốn dẻo hoặc bị trùng ma sát | Thực hiện gửi lệnh `OVERRIDE_ADJUST` từ HMI để AI level 5 tự động học và lưu offset vào `calibration.json` |

---

