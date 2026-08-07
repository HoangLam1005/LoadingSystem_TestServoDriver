# ĐẶC TẢ THUẬT TOÁN & HƯỚNG DẪN VẬN HÀNH MODULE TỰ ĐỘNG BÙ ĐỘ RƠ VÀ KHE HỞ CƠ KHÍ KHI ĐẢO CHIỀU QUAY

Tài liệu này giải thích toàn bộ lý thuyết toán học liên quan đến thuật toán tự động bù độ rơ và khe hở cơ khí khi đảo chiều quay, chi tiết mã nguồn (Rust) và quy trình cài đặt, cấu hình, vận hành trực tiếp trên hệ thống phần cứng (PLC Delta DVP14SS2, Servo Driver RS Automation CSD7, Motor Servo & mâm đĩa xoay).

---

## 1. TỔNG QUAN VẤN ĐỀ

### 1.1. Hiện trạng hệ thống
Hệ thống LoadingSystem điều khiển mâm đĩa xoay mang 12 lọ vật mẫu thông qua chuỗi truyền động cơ khí:

```
┌────────────────────────┐    Tín hiệu băm xung    ┌────────────────────────┐
│ PLC Delta DVP14SS2     ├────────────────────────►│ Servo Driver CSD7      │
└────────────────────────┘    (PULS / DIR)         └───────────┬────────────┘
                                                               │ Trục Motor
                                                               ▼
┌────────────────────────┐    Trục mâm đĩa         ┌────────────────────────┐
│   Mâm đĩa xoay 12 lọ   │◄────────────────────────┤ Bánh răng nối Driver   │
│                        │                         │     và mâm xoay        │
└────────────────────────┘                         └────────────────────────┘
```

Trong các phiên bản trước, `RotatingDiscController` tiếp nhận lệnh quay từ HMI Python (qua socket IPC) và quy đổi góc quay $\theta$ ra số xung $P_{\text{raw}} = \text{round}(\theta \times 131,072 / 360)$. Tuy nhiên, controller không hề theo dõi hướng quay của lệnh trước đó. 

Khi đĩa xoay thực hiện hai lệnh liên tiếp cùng chiều (phải $\to$ phải), hệ thống hoạt động bình thường. Nhưng khi phát sinh lệnh **đảo chiều quay** (phải $\to$ trái hoặc trái $\to$ phải), hệ thống phát xung ngay lập tức theo số xung lý thuyết mà không tính tới khoảng rơ giữa các bánh răng.

---

### 1.2. Hậu quả cơ khí & lý do cần thuật toán bù độ rơ cơ khí

#### Hiện tượng độ rơ cơ khí:
Trong bất kỳ hệ truyền động cơ khí nào (bánh răng, xích truyền, khớp nối trục), luôn tồn tại một khe hở vật lý giữa các mặt răng ăn khớp (Backlash gap).

```
Khi quay chiều thuận (Right):
  [Mặt răng A] ──► [Mặt răng B]  (Tiếp xúc sát mặt trước)

Khi đảo chiều quay (Left):
  [Mặt răng A] ◄── (Khe hở Backlash - Dead Zone) ◄── [Mặt răng B]
                   │◄───────── ΔB ─────────►│
            Motor xoay qua khe hở nhưng mâm đĩa ĐỨNG YÊN!
```

#### Tác hại thực tế đối với hệ thống:
1. **Mâm đĩa quay thiếu góc khi đảo chiều:** Khi motor đảo chiều, các xung ban đầu chỉ dùng để quét hết khe hở cơ khí. Mâm đĩa hoàn toàn đứng yên trong khoảng thời gian này. Kết quả là lệnh quay không quay đủ góc so với mong muốn.
2. **Sai lệch góc định vị vị trí 12 lọ:** Khác với góc nhỏ bị ảnh hưởng bởi ma sát tĩnh, lỗi backlash xuất hiện ở **mọi góc quay** khi có hành động đảo chiều. Điều này làm vị trí các lọ vật mẫu bị lệch khỏi tâm cảm biến quang nhận diện.
3. **Trôi vị trí gốc (Zero drift):** Qua nhiều chu kỳ đảo chiều liên tục, các khoảng thiếu tích lũy lại khiến vị trí $0^\circ$ của phần mềm lệch hẳn so với vị trí gốc cơ khí thực tế.

#### Sự không tương thích của các module hiện có:
| Module hiện có | Chức năng chính | Tại sao KHÔNG giải quyết được Backlash đảo chiều? |
|:---|:---|:---|
| **Level 5 `offset_pulses`** (`profile.rs`) | Bù sai số định vị cố định theo thời gian | Bù một giá trị offset cố định vào mọi lệnh, không phân biệt lệnh đó là cùng chiều hay đảo chiều. |
| **`SmallAngleCompensator`** (`small_angle_compensation.rs`) | Bù ma sát tĩnh & dead-zone ở góc nhỏ | Chỉ kích hoạt khi góc $\theta < 15^\circ$ và suy giảm theo hàm mũ. Trong khi Backlash xuất hiện ở **mọi góc** khi đảo chiều. |
| **`AccelerationProfile`** (`acceleration_profile.rs`) | Tạo đồ thị S-Curve giảm rung lắc | Điều chỉnh tần số băm xung mượt mà, không tác động đến tổng số xung phát ra để bù khe hở. |

---

### 1.3. Giải pháp thuật toán bù độ rơ tự động

Thuật toán giải quyết triệt để bài toán đảo chiều bằng cơ chế 2 lớp:
1. **Lớp real-time tracking (Theo dõi hướng quay):** Lưu giữ trạng thái hướng quay gần nhất $D_{\text{prev}}$. Khi nhận lệnh mới $D_{\text{new}}$, nếu $D_{\text{new}} \neq D_{\text{prev}}$ (đảo chiều), tự động cộng thêm chính xác số xung bù $B$ vào lệnh phát xuống PLC.
2. **Lớp adaptive learning (Tự học thích nghi WMA):** Giá trị độ rơ $B$ không cố định vĩnh viễn mà được học tự động dựa trên phản hồi hiệu chỉnh từ HMI khi cơ khí bị mòn theo thời gian.

---

## 2. MÔ HÌNH TOÁN HỌC & NGUYÊN LÝ HOẠT ĐỘNG

### 2.1. Mô hình theo dõi hướng quay & tính xung bù

Gọi $D \in \{\text{Left}, \text{Right}, \text{Unknown}\}$ là hướng quay.

Thuật toán xác định xung bù $P_{\text{backlash}}$ theo điều kiện:

$$P_{\text{backlash}} = \begin{cases} 
B & \text{nếu } D_{\text{prev}} \neq \text{Unknown} \land D_{\text{new}} \neq D_{\text{prev}} \quad (\text{Đảo chiều xoay}) \\
0 & \text{nếu } D_{\text{prev}} = \text{Unknown} \lor D_{\text{new}} = D_{\text{prev}} \quad (\text{Xoay lần đầu hoặc xoay cùng chiều})
\end{cases}$$

Trong đó:
* $B$: Số xung bù backlash hiện tại (mặc định $B = 109\text{ xung} \approx 0.3^\circ$ cho encoder 17-bit $131,072\text{ xung/vòng}$).
* $D_{\text{prev}}$: Hướng quay của lệnh thực thi liền trước.
* $D_{\text{new}}$: Hướng quay của lệnh hiện tại.

#### Công thức tổng hợp xung phát xuống PLC ($P_{\text{final}}$):
Khi tích hợp cả bù offset tự học Llevel 5 ($P_{\text{offset}}$) và bù backlash ($P_{\text{backlash}}$):

* **Quay phải (Right - xung dương):**
  $$P_{\text{final}} = P_{\text{raw}} + P_{\text{offset}} + P_{\text{backlash}}$$

* **Quay trái (Left - xung âm):**
  $$P_{\text{final}} = -(P_{\text{raw}}) + P_{\text{offset}} - P_{\text{backlash}}$$

---

### 2.2. Thuật toán tự học thích nghi

Theo thời gian vận hành, các bánh răng cơ khí bị mài mòn làm khe hở $B$ tăng dần. Khi chuyên gia can thiệp thủ công trên HMI và gửi giá trị sai lệch thực tế $P_{\text{feedback}}$, hệ thống cập nhật $B$ bằng thuật toán **Weighted Moving Average (WMA)**:

$$B_{\text{new}} = \text{clamp}\Big(\text{round}\big(B_{\text{old}} \cdot (1 - \alpha) + |P_{\text{feedback}}| \cdot \alpha\big), \, B_{\text{min}}, \, B_{\text{max}}\Big)$$

Trong đó:
* $\alpha = 0.15$ (Hệ số học `learning_rate`): Mỗi lần hiệu chỉnh, $15\%$ giá trị sai lệch mới được tích lũy vào $B$, giúp $B$ hội tụ mượt mà mà không bị vọt biên (overshoot) do nhiễu đo lường.
* $B_{\text{min}} = 10\text{ xung} \approx 0.027^\circ$: Sàn tối thiểu bảo vệ $B$ không bị giảm về $0$.
* $B_{\text{max}} = 364\text{ xung} \approx 1.0^\circ$: Trần tối đa bảo vệ hệ thống khỏi các cú nhập sai của vận hành viên, tránh bù quá mức gây đâm cơ khí.

---

## 3. PHÂN TÍCH CHI TIẾT MÃ NGUỒN

Mã nguồn của thuật toán nằm tại 2 file chính trong `rust_backend`:
* `src/calibration/backlash_compensation.rs` (Module tính toán & tự học)
* `src/hardware/rotating_disc.rs` (Bộ điều khiển tích hợp thực thi Modbus)

---

### 3.1. Module `BacklashCompensator` (`backlash_compensation.rs`)

#### Struct `BacklashCompensator`
```rust
#[derive(Debug, Clone)]
pub struct BacklashCompensator {
    backlash_pulses: u32,             // Số xung bù hiện tại (mặc định 109 xung)
    last_direction: TrackedDirection, // Hướng quay lần trước (Unknown / Left / Right)
    learning_rate: f64,               // Hệ số học (mặc định 0.15)
    reversal_count: u32,              // Thống kê số lần đảo chiều quay
}
```

---

#### Hàm  `compute_compensation()`

Hàm này kiểm tra điều kiện đảo chiều quay. Trả về đúng giá trị `backlash_pulses` bù xung nếu phát hiện đảo chiều, hoặc trả về `0` nếu là lần quay đầu tiên hoặc quay cùng chiều.

```rust
// Tính số xung bù backlash cho lệnh quay mới
// Nhận vào hướng quay của lệnh hiện tại (new_direction), trả về số xung cần bù thêm (u32 >= 0)
pub fn compute_compensation(&self, new_direction: TrackedDirection) -> u32 {
    // Sử dụng Pattern Matching kiểm tra hướng quay lần trước (last_direction)
    match self.last_direction {
        // Trường hợp 1: Lần đầu tiên quay sau khi mở máy hoặc sau khi Return to Origin
        // Chưa có hướng quay trước đó -> Chưa xác định được khe hở -> Không bù (trả về 0)
        TrackedDirection::Unknown => 0,
        
        // Trường hợp 2: Hướng quay mới giống với hướng quay lần trước (Cùng chiều quay)
        // Răng bánh răng đã tiếp xúc sát mặt trước -> Không có backlash -> Không bù (trả về 0)
        prev if prev == new_direction => 0,
        
        // Trường hợp 3: Hướng quay mới khác hướng quay lần trước (đảo chiều quay)
        // Trục motor phải quay qua khe hở cơ khí -> Trả về toàn bộ số xung backlash_pulses để bù
        _ => self.backlash_pulses,
    }
}
```

---

#### Hàm `record_direction()`

Hàm này cập nhật trạng thái `last_direction` sau khi lệnh quay đã được ghi xuống PLC thành công, đồng thời tăng bộ đếm số lần đảo chiều `reversal_count` để phục vụ chẩn đoán hệ thống, đảm bảo tính chính xác cho lần tính bù của lệnh quay tiếp theo.

```rust
// Ghi nhận hướng quay sau khi lệnh thực thi thành công xuống PLC
// Đăng ký hướng quay mới và cập nhật bộ đếm thống kê số lần đảo chiều
pub fn record_direction(&mut self, direction: TrackedDirection) {
    // Kiểm tra nếu không phải lần đầu (last_direction != Unknown) VÀ có sự thay đổi hướng quay
    if self.last_direction != TrackedDirection::Unknown
        && self.last_direction != direction
    {
        // Tăng bộ đếm số lần đảo chiều lên 1 (phục vụ thống kê tuổi thọ cơ khí)
        self.reversal_count += 1;
    }
    
    // Cập nhật last_direction bằng hướng quay vừa thực hiện thành công
    self.last_direction = direction;
}
```

---

#### Hàm `update_from_feedback()`

Hàm này tự động điều chỉnh độ lớn $B$ dựa trên phản hồi sai lệch thực tế khi chuyên gia thực hiện Fine Tune trên HMI. Giúp tự học thích nghi qua công thức WMA và tự động ép biên an toàn $[10, 364]\text{ xung}$.

```rust
// Cập nhật giá trị backlash từ phản hồi sai lệch thực tế từ HMI (Adaptive Learning)
// Nhận vào số xung sai lệch đo được (feedback_pulses) và tính toán giá trị bù rơ mới
pub fn update_from_feedback(&mut self, feedback_pulses: i32) {
    // 1. Lấy giá trị tuyệt đối của xung sai lệch (không phụ thuộc dấu âm/dương)
    let abs_feedback = feedback_pulses.unsigned_abs();

    // 2. Tính toán giá trị backlash mới bằng thuật toán Weighted Moving Average (WMA)
    // B_new = B_old * (1 - learning_rate) + |feedback| * learning_rate
    let new_backlash = (self.backlash_pulses as f64 * (1.0 - self.learning_rate))
        + (abs_feedback as f64 * self.learning_rate);

    // 3. Làm tròn số thực về u32 và ép giá trị trong khoảng an toàn [MIN_BACKLASH, MAX_BACKLASH]
    // Giúp bảo vệ hệ thống không bị vọt biên khi có nhập liệu sai từ người dùng
    self.backlash_pulses =
        (new_backlash.round() as u32).clamp(MIN_BACKLASH_PULSES, MAX_BACKLASH_PULSES);
}
```

---

### 3.2. Tích hợp controller (`hardware/rotating_disc.rs`)

#### Hàm `rotate_degrees_with_freq()` (Cập nhật tích hợp Backlash)

Ở các phiên bản trước, đĩa xoay quay với tần số cố định, nay được bổ sung bước tính toán và cộng/trừ xung bù backlash trước khi đóng gói bản tin Modbus gửi PLC.

* **Khác biệt so với Version trước:**
  * *Version trước:* `final_pulses = raw_pulses + offset_pulses`
  * *Version mới:* `final_pulses = raw_pulses + offset_pulses ± backlash_comp`

* **Tác động đến hệ thống:**
  * **PLC Delta:** Nhận đúng số xung đã bao gồm bù backlash qua thanh ghi `D100/D101`.
  * **Motor Servo:** Quay thêm khoảng bù $B$ khi đảo chiều, giúp mâm đĩa xoay đúng góc yêu cầu.

```rust
// Quay đĩa xoay một góc θ theo hướng chỉ định kèm tần số phát xung tùy chỉnh
// Đã tích hợp bù sai số tự học level 5 và bù độ rơ cơ khí khi đảo chiều 
pub async fn rotate_degrees_with_freq(
    &mut self,
    degrees: f64,                // Góc quay mục tiêu (độ dương)
    direction: Direction,        // Hướng quay (Right / Left)
    custom_freq: Option<u32>,    // Tần số tùy chỉnh (hoặc None để tự tính tần số động)
) -> Result<(), LoadingError> {
    // 1. Kiểm tra góc quay không được âm
    if degrees < 0.0 {
        return Err(LoadingError::Command(
            "Degrees must be positive. Use Direction to specify rotation direction.".into(),
        ));
    }

    // 2. Quy đổi góc quay ra số xung lý thuyết (17-bit: 131,072 xung/vòng)
    let raw_pulses = self.degrees_to_pulses(degrees);

    // 3. Chuyển đổi enum Direction sang TrackedDirection để bộ bù backlash xử lý
    let tracked_dir = match direction {
        Direction::Right => TrackedDirection::Right,
        Direction::Left => TrackedDirection::Left,
    };

    // 4. Gọi BacklashCompensator tính số xung bù: Trả về > 0 nếu đảo chiều, trả về 0 nếu cùng chiều
    let backlash_comp = self.backlash.compute_compensation(tracked_dir) as i32;

    // 5. Áp dụng tổng hợp 2 lớp bù: Calibration Offset (Level 5) + Backlash Compensation
    // - Quay Phải (Right): Xung dương + Offset + Backlash
    // - Quay Trái (Left): Xung âm + Offset - Backlash (Trừ backlash làm tăng biên độ xung âm)
    let final_pulses = match direction {
        Direction::Right => raw_pulses + self.calibration.offset_pulses + backlash_comp,
        Direction::Left => -(raw_pulses) + self.calibration.offset_pulses - backlash_comp,
    };

    // 6. Tính toán hoặc lấy tần số phát xung
    let freq = custom_freq.unwrap_or_else(|| self.calculate_dynamic_frequency(degrees, Some(1.0), Some(1000), Some(9000)));

    // 7. Log thông tin lệnh quay chi tiết (Bao gồm số xung raw, offset và backlash)
    tracing::info!(
        "Rotate {:.2}° {} @ {}Hz → raw={}p, offset={}p, backlash={}p, final={}p",
        degrees, direction, freq, raw_pulses, self.calibration.offset_pulses, backlash_comp, final_pulses
    );

    // 8. Thực thi ghi thanh ghi D100/D101 (xung), D102/D103 (tần số) và ON M0 qua Modbus RTU
    self.execute_pulse_command(final_pulses, freq).await?;

    // 9. Chờ motor quay xong
    wait_for_motor_completion(final_pulses, freq).await;

    // 10. Ghi nhận hướng quay thành công vào bộ bù backlash để làm mốc cho lệnh tiếp theo
    self.backlash.record_direction(tracked_dir);

    // 11. Cập nhật góc theo dõi nội bộ
    match direction {
        Direction::Right => self.current_angle += degrees,
        Direction::Left => self.current_angle -= degrees,
    }

    Ok(())
}
```

---

#### Hàm `rotate_degrees_smooth()` (Cập nhật tích hợp Backlash vào S-Curve)

Hàm này kết hợp 3 thuật toán: **Gia tốc S-Curve** (triệt tiêu rung lắc) + **Offset tự học Level 5** + **Bù độ rơ Backlash khi đảo chiều**.

* **Tác động đến hệ thống:**
  Đĩa xoay vừa khởi động/dừng mượt mà không rung mâm đĩa, vừa không bị thiếu góc khi đảo chiều quay.

```rust
// Quay đĩa xoay một góc θ với gia tốc S-Curve mượt và bù độ rơ Backlash khi đảo chiều
pub async fn rotate_degrees_smooth(
    &mut self,
    degrees: f64,
    direction: Direction,
    accel_profile: Option<AccelerationProfile>,
) -> Result<(), LoadingError> {
    if degrees < 0.0 {
        return Err(LoadingError::Command(
            "Degrees must be positive. Use Direction to specify rotation direction.".into(),
        ));
    }

    // 1. Quy đổi góc ra xung lý thuyết
    let raw_pulses = self.degrees_to_pulses(degrees);

    // 2. Chuyển đổi hướng quay sang TrackedDirection
    let tracked_dir = match direction {
        Direction::Right => TrackedDirection::Right,
        Direction::Left => TrackedDirection::Left,
    };

    // 3. Tính số xung bù backlash (trả về 109p nếu đảo chiều, 0p nếu cùng chiều)
    let backlash_comp = self.backlash.compute_compensation(tracked_dir) as i32;

    // 4. Áp dụng tổng hợp bù offset và bù backlash cho tổng xung của S-Curve
    let final_pulses = match direction {
        Direction::Right => raw_pulses + self.calibration.offset_pulses + backlash_comp,
        Direction::Left => -(raw_pulses) + self.calibration.offset_pulses - backlash_comp,
    };

    let max_freq = self.calculate_dynamic_frequency(degrees, Some(1.0), Some(1000), Some(9000));
    let profile = accel_profile.unwrap_or_default();

    // 5. Sinh chuỗi phân đoạn S-Curve từ tổng xung final_pulses đã được bù backlash
    let segments = if AccelerationProfile::should_use_profile(final_pulses) {
        match profile.generate_segments(final_pulses, max_freq) {
            Some(segs) => segs,
            None => AccelerationProfile::single_shot(final_pulses, max_freq),
        }
    } else {
        AccelerationProfile::single_shot(final_pulses, max_freq)
    };

    let total_segments = segments.len();

    tracing::info!(
        "Smooth rotate {:.2}° {} → {} segments, total_pulses={}, backlash={}p, f_max={}Hz",
        degrees, direction, total_segments, final_pulses, backlash_comp, max_freq
    );

    // 6. Thực thi tuần tự các phân đoạn qua Modbus RTU
    for (idx, segment) in segments.iter().enumerate() {
        tracing::debug!(
            "  Segment [{}/{}] phase={}, pulses={}, freq={}Hz, duration={}ms",
            idx + 1, total_segments, segment.phase, segment.pulses, segment.frequency, segment.duration_ms
        );

        self.execute_pulse_command(segment.pulses, segment.frequency).await?;
        wait_for_motor_completion(segment.pulses, segment.frequency).await;
    }

    // 7. Ghi nhận hướng quay thành công sau khi chạy xong tất cả segments
    self.backlash.record_direction(tracked_dir);

    // 8. Cập nhật góc tích lũy nội bộ
    match direction {
        Direction::Right => self.current_angle += degrees,
        Direction::Left => self.current_angle -= degrees,
    }

    Ok(())
}
```

---

#### Hàm `return_to_origin()` (Cập nhật reset backlash tracking)

```rust
pub async fn return_to_origin(&mut self) -> Result<(), LoadingError> {
    let raw_return = -self.degrees_to_pulses(self.current_angle);
    let return_pulses = raw_return - self.calibration.offset_pulses;
    let freq = self.default_frequency;

    tracing::info!(
        "Return to origin from {:.2}° → raw={}p, offset={}p, final={}p",
        self.current_angle, raw_return, self.calibration.offset_pulses, return_pulses
    );

    self.execute_pulse_command(return_pulses, freq).await?;
    wait_for_motor_completion(return_pulses, freq).await;

    // RESET hướng quay về Unknown vì đĩa đã về vị trí gốc 0°
    // Lệnh quay tiếp theo sau khi về gốc sẽ được coi như lần đầu (không tính backlash)
    self.backlash.reset_direction();
    self.current_angle = 0.0;

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

#### Cấu hình cổng truyền thông Modbus RTU trên PLC:
* **COM Port (Windows):** `COM3` (hoặc kiểm tra trong Device Manager)
* **Baud Rate:** `9600` | **Data bits:** `8` | **Parity:** `Even` | **Stop bits:** `1`
* **PLC Modbus Address:** `1`
* **Thanh ghi vị trí xung:** `D100` (Low Word), `D101` (High Word)
* **Thanh ghi tần số Hz:** `D102` (Low Word), `D103` (High Word)
* **Bit kích hoạt lệnh `DDRVI`:** `M0`

---

### 4.2. Kiểm thử Unit Tests & biên dịch phần mềm

1. Mở Terminal / PowerShell tại thư mục backend:
   ```bash
   cd rust_backend
   ```

2. Biên dịch phiên bản Release tối ưu hóa cho phần cứng thực:
   ```bash
   cargo build --release
   ```

3. Khởi chạy Backend kết nối trực tiếp PLC:
   ```bash
   cargo run --release -- config.toml
   ```

---

### 4.3. Quy trình kiểm tra vận hành

Để kiểm chứng thuật toán bù backlash hoạt động chính xác trên hệ thống thật:

#### Bước 1: Kiểm tra phản hồi lệnh cùng chiều (Không bù)
* Gửi lệnh quay **Phải 30°** (Right):
  * **Log Backend:** `Rotate 30.00° Right ... → raw=10923p, offset=0p, backlash=0p, final=10923p`
  * **Giải thích:** Lần đầu quay từ `Unknown` $\to$ `backlash = 0p`.
* Gửi tiếp lệnh quay **Phải 30°** (Right):
  * **Log Backend:** `Rotate 30.00° Right ... → raw=10923p, offset=0p, backlash=0p, final=10923p`
  * **Giải thích:** Cùng chiều quay (`Right` $\to$ `Right`) $\to$ `backlash = 0p`.

#### Bước 2: Kiểm tra phản hồi lệnh đảo chiều (Tự động bù 109 xung)
* Gửi lệnh quay **Trái 30°** (Left):
  * **Log Backend:** `Rotate 30.00° Left ... → raw=10923p, offset=0p, backlash=109p, final=-11032p`
  * **Giải thích:** Đảo chiều quay (`Right` $\to$ `Left`) $\to$ Tự động cộng thêm $109\text{ xung}$ bù độ rơ. Tổng xung âm là $-(10923 + 109) = -11032\text{ xung}$.
  * **Quan sát thực tế:** Mâm đĩa xoay đúng chính xác $30.00^\circ$ về vị trí lọ trước đó, không còn hiện tượng lệch $0.3^\circ$ do khe hở bánh răng.

#### Bước 3: Kiểm tra tính năng tự học khi có sai lệch cơ khí
* Khi bánh răng bị mòn thêm làm khe hở thực tế tăng lên $150\text{ xung}$:
  * Chuyên gia nhấn Fine Tune trên HMI nhập $+0.41^\circ$ ($\approx 150\text{ xung}$).
  * Hàm `update_from_feedback(150)` tự động tính:
    $$B_{\text{new}} = 109 \times 0.85 + 150 \times 0.15 = 92.65 + 22.5 = 115\text{ xung}$$
  * Giá trị backlash mới $115\text{ xung}$ được cập nhật cho các lần đảo chiều tiếp theo, giúp hệ thống luôn tự thích nghi với độ mài mòn cơ khí.

