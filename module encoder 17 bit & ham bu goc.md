# ĐẶC TẢ KỸ THUẬT MODULE ĐỌC ENCODER 17-BIT VÀ THUẬT TOÁN BÙ GÓC QUAY NHỎ CHO ĐĨA XOAY (LOADING_SYSTEM)

Tài liệu đặc tả chi tiết hai module kỹ thuật được bổ sung vào hệ thống phần mềm điều khiển đĩa xoay LoadingSystem: 

(1) Module đọc và giải mã dữ liệu từ bộ mã hóa vòng quay 17-bit tích hợp trên Servo Driver CSD7

(2) Thuật toán tính bù sai lệch phi tuyến cho các góc quay nhỏ nhằm triệt tiêu hiệu ứng dead-zone, ma sát tĩnh và backlash cơ khí. 

Mục tiêu của tài liệu này là cung cấp đầy đủ mô hình toán học, kiến trúc mã nguồn và kịch bản kiểm thử để các module AI code generation sinh ra mã nguồn production-ready 100%.

---

## THÔNG TIN CHUNG
* **Dự án:** Hệ thống phân loại tự động & xử lý hạt điều công nghiệp (LoadingSystem)
* **Thành phần đặc tả:** Module đọc Encoder 17-bit & Thuật toán bù góc quay nhỏ cho đĩa xoay
* **Tác giả:** Trần Doãn Hoàng Lâm (MKSOL - HCMUT)
* **Thiết bị phần cứng:** PLC Delta DVP14SS2 (Transistor NPN), Servo Driver RS Automation CSD7 (17-bit Serial Encoder 131,072 xung/vòng)
* **Lớp phần mềm:** Rust Backend (Tokio Async Daemon) → Modbus RTU over RS485 → PLC Delta Core Firmware
* **Vị trí mã nguồn:**
  * `rust_backend/src/hardware/encoder_reader.rs` — Module đọc encoder 17-bit
  * `rust_backend/src/calibration/small_angle_compensation.rs` — Thuật toán bù góc nhỏ

---

## PHẦN I: MODULE ĐỌC VÀ GIẢI MÃ ENCODER 17-BIT

### 1. Bối cảnh kỹ thuật và lý do thiết kế

Servo Driver RS Automation CSD7 tích hợp bộ mã hóa vòng quay tuyệt đối (Absolute Rotary Encoder) độ phân giải 17-bit, tương đương $2^{17} = 131,072$ xung trên một vòng quay đầy đủ ($360°$). Trong kiến trúc phiên bản version 1.3, việc quy đổi giữa xung và góc chỉ được thực hiện bằng hai hàm đơn giản `degrees_to_pulses()` và `pulses_to_degrees()` nằm trực tiếp trong `RotatingDiscController`. Cách tiếp cận này thiếu một module chuyên biệt xử lý các nghiệp vụ phức tạp của encoder 17-bit thực tế:


* **Giải mã dữ liệu raw 17-bit:** Encoder CSD7 trả dữ liệu vị trí tuyệt đối qua 2 thanh ghi Modbus 16-bit. Bit thứ 17 (MSB) nằm ở thanh ghi cao, cần phải ghép nối và mask chính xác.
* **Multi-turn tracking:** Encoder đơn vòng chỉ theo dõi vị trí trong một vòng quay ($0° - 360°$). Khi cốt motor quay qua điểm $0°$, giá trị raw nhảy đột ngột từ $131,071$ về $0$ (hoặc ngược lại). Hệ thống cần phát hiện sự kiện wrap-around và duy trì bộ đếm vòng tích lũy.
* **Xác thực dữ liệu:** Trong môi trường công nghiệp, nhiễu điện từ (EMI) trên đường truyền RS485 có thể làm sai lệch giá trị thanh ghi. Module cần kiểm tra dải hợp lệ trước khi chấp nhận dữ liệu.

### 2. Thông số kỹ thuật encoder 17-bit

| Thông số | Giá trị | Ghi chú |
|:---|:---:|:---|
| Độ phân giải | $2^{17} = 131,072$ xung/vòng | Absolute single-turn |
| Dải giá trị raw | $0 \rightarrow 131,071$ | Unsigned 17-bit |
| Độ chính xác góc tối thiểu | $\frac{360°}{131,072} \approx 0.00275°$ | ~$0.048\text{ mrad}$ |
| Giao diện dữ liệu | 2 thanh ghi Modbus 16-bit | Low word + High word |
| Ngưỡng phát hiện wrap-around | $\frac{131,072}{2} = 65,536$ xung | Nửa vòng encoder |

### 3. Mô hình toán học quy đổi

Gọi $R_{raw}$ là giá trị raw 17-bit đọc được từ encoder, $N_{turn}$ là số vòng quay tích lũy:

**Quy đổi vị trí raw đơn vòng sang góc (độ):**

$$\theta_{single} = \frac{R_{raw} \times 360}{131,072}$$

**Quy đổi vị trí raw đơn vòng sang góc (radian):**

$$\theta_{rad} = \frac{R_{raw} \times 2\pi}{131,072}$$

**Vị trí tuyệt đối tích lũy (tính cả multi-turn):**

$$\theta_{absolute} = N_{turn} \times 360 + \theta_{single}$$

**Quy đổi ngược từ góc mục tiêu sang xung raw:**

$$R_{target} = \text{round}\left(\frac{|\theta| \times 131,072}{360}\right) \bmod 131,072$$

### 4. Thuật toán phát hiện chuyển vòng 

Khi encoder quay qua điểm $0°/360°$, giá trị raw thay đổi đột ngột. Thuật toán sử dụng ngưỡng nửa vòng ($65,536$ xung) để phân biệt chuyển vòng thật với chuyển động bình thường:

```text
  Gọi R_new = giá trị raw mới đọc được
       R_old = giá trị raw lần đọc trước
       Δ = R_new - R_old

  NẾU |Δ| < 65,536:
      → Chuyển động bình thường, không thay đổi turn_count
  NẾU Δ > +65,536:
      → Phát hiện chuyển vòng NGƯỢC (CCW): turn_count -= 1
  NẾU Δ < -65,536:
      → Phát hiện chuyển vòng THUẬN (CW): turn_count += 1
```

### 5. Mã nguồn Rust (`rust_backend/src/hardware/encoder_reader.rs`)

Struct `Encoder17Bit` quản lý toàn bộ trạng thái encoder với API tường minh:

```rust
/// Bộ đọc và giải mã encoder 17-bit trên Servo CSD7
pub struct Encoder17Bit {
    raw_position: u32,    // Giá trị raw đơn vòng (0 → 131,071)
    turn_count: i32,      // Bộ đếm vòng tích lũy (CW dương, CCW âm)
    previous_raw: u32,    // Giá trị raw lần đọc trước
    initialized: bool,    // Cờ khởi tạo lần đọc đầu tiên
}

impl Encoder17Bit {
    // Cập nhật vị trí từ raw 17-bit, tự động phát hiện wrap-around
    pub fn update_position(&mut self, raw_value: u32) -> Result<(), String>;

    // Giải mã vị trí từ cặp thanh ghi Modbus 16-bit (mask 0x1FFFF)
    pub fn decode_from_registers(&mut self, reg_low: u16, reg_high: u16) -> Result<(), String>;

    // Quy đổi vị trí hiện tại sang góc (độ / radian)
    pub fn get_angle_degrees(&self) -> f64;
    pub fn get_angle_radians(&self) -> f64;

    // Vị trí tuyệt đối multi-turn (bao gồm đếm vòng)
    pub fn get_absolute_angle_degrees(&self) -> f64;

    // Quy đổi ngược từ góc sang raw
    pub fn degrees_to_raw(degrees: f64) -> u32;

    // Độ phân giải góc tối thiểu: 360°/131,072 ≈ 0.00275°
    pub fn get_angular_resolution() -> f64;
}
```

---

## PHẦN II: THUẬT TOÁN TÍNH BÙ GÓC QUAY NHỎ CHO ĐĨA XOAY

### 1. Bối cảnh kỹ thuật và phân tích vấn đề

Trong quá trình vận hành thực tế hệ thống đĩa xoay LoadingSystem, quan sát cho thấy sai lệch vị trí khi quay các góc nhỏ ($< 15°$) lớn hơn đáng kể so với góc lớn ($\geq 15°$). Hiện tượng này xuất phát từ tổ hợp 3 nguyên nhân cơ-điện:

```
  ┌─────────────────────────────────────────────────────────────────────┐
  │              TỔ HỢP 3 NGUYÊN NHÂN SAI LỆCH GÓC NHỎ                  │
  │                                                                     │
  │  [1. Dead-zone Encoder] ──► Vùng chết khởi động ≈ 50-80 xung        │
  │         │                                                           │
  │  [2. Ma sát tĩnh]      ──► Lực cản ban đầu mâm đĩa + ổ bi           │
  │         │                                                           │
  │  [3. Backlash cơ khí]  ──► Khe hở bánh răng truyền động             │
  │         │                                                           │
  │         └──► Tỷ lệ sai lệch ∝ 1/θ (nghịch đảo góc quay)             │
  └─────────────────────────────────────────────────────────────────────┘
```

* **Dead-zone encoder ($50-80$ xung):** Khi motor khởi động từ trạng thái đứng yên, encoder cần vượt qua một vùng chết trước khi bắt đầu đếm xung hữu hiệu. Với góc nhỏ ($5°$ tương đương chỉ khoảng $1,820$ xung), dead-zone chiếm tỷ lệ khoảng $4.4\%$. Với góc lớn ($90°$ tương đương $32,768$ xung), tỷ lệ giảm xuống khoảng $0.24\%$.
* **Ma sát tĩnh (Static Friction):** Lực ma sát tĩnh giữa cốt motor và ổ bi đỡ mâm đĩa xoay 12 lọ tạo ra mô-men cản khởi động. Khi góc quay nhỏ, motor phải tiêu tốn một phần năng lượng xung ban đầu để thắng ma sát, dẫn đến thiếu xung thực tế tại điểm đích.
* **Backlash cơ khí:** Khe hở giữa các cặp bánh răng truyền động gây mất mát xung khi đảo chiều hoặc khởi động. Khoảng backlash cố định ($\approx 20-40$ xung) chiếm tỷ lệ lớn hơn trong tổng xung của góc nhỏ.

### 2. Mô hình toán học bù góc nhỏ

Thuật toán sử dụng mô hình suy giảm hàm mũ (Exponential decay) để tính lượng xung bù bổ sung. Mô hình này phản ánh đặc tính thực tế: ảnh hưởng của dead-zone và ma sát tĩnh giảm nhanh khi biên độ góc quay tăng.

Gọi θ là góc quay mục tiêu (độ), P_raw là số xung lý thuyết qua encoder 17-bit:

$$P_{raw} = \text{round}\left(\frac{\theta \times 131,072}{360}\right)$$

**Số xung bù bổ sung cho góc nhỏ:**

$$P_{comp} = \begin{cases} \text{round}\left(K_{base} \times e^{-\alpha \times |\theta|}\right) & \text{khi } |\theta| < \theta_{threshold} \\ 0 & \text{khi } |\theta| \geq \theta_{threshold} \end{cases}$$

Trong đó:
* $K_{base} = 60$ (xung) — Hệ số bù cơ sở, đại diện cho tổng dead-zone + ma sát tĩnh ở trạng thái nghỉ. Giá trị này được hiệu chỉnh thực nghiệm trên bàn thử nghiệm phần cứng.
* $\alpha = 0.15$ — Hệ số suy giảm hàm mũ, quyết định tốc độ giảm bù khi góc tăng.
* $\theta_{threshold} = 15°$ — Ngưỡng phân biệt góc nhỏ. Góc $\geq 15°$ được coi là đủ lớn để sai lệch dead-zone không còn đáng kể.

**Số xung cuối cùng sau bù:**

$$P_{final} = P_{raw} + \text{sign}(\text{direction}) \times P_{comp}$$

### 3. Bảng tham chiếu giá trị bù theo góc quay

| Góc quay $\theta$ (°) | Phân loại | Số xung lý thuyết $P_{raw}$ | Xung bù $P_{comp}$ | Tổng xung $P_{final}$ | Góc bù tương đương |
|:---:|:---:|:---:|:---:|:---:|:---:|
| $1°$ | Micro | $364$ | $51$ | $415$ | $\approx 0.14°$ |
| $3°$ | Micro | $1,092$ | $38$ | $1,130$ | $\approx 0.10°$ |
| $5°$ | Small | $1,820$ | $28$ | $1,848$ | $\approx 0.08°$ |
| $10°$ | Small | $3,641$ | $13$ | $3,654$ | $\approx 0.04°$ |
| $14°$ | Small | $5,097$ | $7$ | $5,104$ | $\approx 0.02°$ |
| $15°$ | Normal | $5,461$ | $0$ | $5,461$ | $0°$ |
| $90°$ | Normal | $32,768$ | $0$ | $32,768$ | $0°$ |

### 4. Phân vùng góc quay (Angle Zone Classification)

Thuật toán phân chia dải góc quay thành 3 vùng hoạt động, mỗi vùng có mức độ bù và chiến lược xử lý khác nhau:

```
  Vùng MICRO (0° - 5°)      Vùng SMALL (5° - 15°)    Vùng NORMAL (≥ 15°)
  ┌─────────────────┐       ┌──────────────────┐       ┌──────────────────┐
  │ Bù tối đa       │       │ Bù suy giảm dần  │       │ Không bù         │
  │ P_comp ≈ 28-60  │──────►│ P_comp ≈ 7-28    │──────►│ P_comp = 0       │
  │ Rủi ro cao      │       │ Rủi ro trung bình│       │ Ổn định          │
  └─────────────────┘       └──────────────────┘       └──────────────────┘
```

### 5. Mã nguồn Rust (`rust_backend/src/calibration/small_angle_compensation.rs`)

Struct `SmallAngleCompensator` đóng gói toàn bộ logic bù góc nhỏ:

```rust
/// Bộ tính bù góc quay nhỏ cho đĩa xoay
pub struct SmallAngleCompensator {
    threshold_degrees: f64,           // Ngưỡng góc nhỏ (mặc định 15°)
    base_compensation_pulses: f64,    // Hệ số bù cơ sở K_base (mặc định 60)
    decay_rate: f64,                  // Hệ số suy giảm α (mặc định 0.15)
    encoder_resolution: u32,          // Độ phân giải encoder (131,072)
}

impl SmallAngleCompensator {
    // Tạo bộ bù với tham số mặc định (hiệu chỉnh cho Servo CSD7)
    pub fn new(encoder_resolution: u32) -> Self;

    // Tạo bộ bù với tham số tùy chỉnh
    pub fn with_params(encoder_resolution: u32, threshold: f64, base: f64, decay: f64) -> Self;

    // Tính số xung bù bổ sung: P_comp = round(K_base × e^(-α × |θ|))
    pub fn compute_compensation_pulses(&self, degrees: f64) -> i32;

    // Tính tổng xung cuối cùng kèm bù: P_raw + sign(dir) × P_comp
    pub fn apply_compensation(&self, degrees: f64, is_positive: bool) -> i32;

    // Phân loại góc: "micro" | "small" | "normal"
    pub fn classify_angle(degrees: f64) -> &'static str;
}
```

---

## PHẦN III: SƠ ĐỒ TÍCH HỢP HAI MODULE VÀO KIẾN TRÚC PHÂN TẦNG HIỆN CÓ

Hai module mới được tích hợp vào kiến trúc phân lớp hiện có của LoadingSystem theo nguyên tắc tách biệt:

```
  ┌────────────────────────────────────────────────────────────────────┐
  │   Layer 3: HMI GUI (Python - customtkinter)                        │
  └──────────────────────────┬─────────────────────────────────────────┘
                             │  ZeroMQ IPC (JSON Strings)
  ┌──────────────────────────▼─────────────────────────────────────────┐
  │   Layer 2: Async Backend Daemon (Rust Tokio Engine)                │
  │                                                                    │
  │   ┌────────────────────────────────────────────────────────────┐   │
  │   │  hardware/                                                 │   │
  │   │    ├── rotating_disc.rs   (Controller chính Level 2-5)     │   │
  │   │    ├── modbus_client.rs   (Giao tiếp Modbus RTU)           │   │
  │   │    └── encoder_reader.rs  [MỚI] Đọc/giải mã encoder 17-bit │   │
  │   ├────────────────────────────────────────────────────────────┤   │
  │   │  calibration/                                              │   │
  │   │    ├── profile.rs         (Lưu trữ calibration vĩnh viễn)  │   │
  │   │    ├── learning.rs        (Thuật toán EMA tự học Level 5)  │   │
  │   │    └── small_angle_compensation.rs [MỚI] Bù góc quay nhỏ   │   │
  │   └────────────────────────────────────────────────────────────┘   │
  └──────────────────────────┬─────────────────────────────────────────┘
                             │  Modbus RTU Frame over RS485
  ┌──────────────────────────▼─────────────────────────────────────────┐
  │   Layer 1: Pure Pulse Executor (PLC Delta Core)                    │
  └────────────────────────────────────────────────────────────────────┘
```

**Nguyên tắc tích hợp:**

* Module `encoder_reader.rs` thuộc tầng **Hardware** (`hardware/`). Module này cung cấp lớp trừu tượng đọc encoder, được `RotatingDiscController` sử dụng để theo dõi vị trí vật lý thực tế của cốt motor thay vì chỉ dựa vào tracking phần mềm (`current_angle`).

* Module `small_angle_compensation.rs` thuộc tầng **Calibration** (`calibration/`). Module này bổ sung cơ chế bù phi tuyến chuyên biệt cho góc nhỏ, hoạt động song song với cơ chế bù offset tuyến tính EMA đã có ở Level 5.

---

## PHẦN IV: THIẾT KẾ THÍ NGHIỆM KIỂM CHỨNG VÀ KỊCH BẢN KIỂM THỬ ĐƠN VỊ (UNIT TEST VALIDATION FRAMEWORK)

### 1. Phương pháp luận kiểm thử

Để đảm bảo tính đúng đắn toán học và độ tin cậy vận hành của hai module mới (`encoder_reader.rs` và `small_angle_compensation.rs`), bộ kiểm thử đơn vị (Unit Test Suite) được thiết kế theo phương pháp luận kiểm chứng phần mềm nhúng công nghiệp, tuân thủ nguyên tắc **Boundary Value Analysis** (Phân tích giá trị biên) và **Equivalence Class Partitioning** (Phân hoạch lớp tương đương).

Toàn bộ bộ kiểm thử được thực thi tự động thông qua framework `#[cfg(test)]` tích hợp sẵn của Rust Compiler, khởi chạy bằng lệnh `cargo test --lib` tại thư mục `rust_backend/`.

```text
  ┌──────────────────────────────────────────────────────────────────────────┐
  │                    BỘ KIỂM THỬ ĐƠN VỊ - 31 THÍ NGHIỆM                    │
  │                                                                          │
  │  Nhóm A: Thuật toán tự học EMA (Level 5)           ── 7 thí nghiệm       │
  │  Nhóm B: Thuật toán bù góc quay nhỏ                ── 10 thí nghiệm      │
  │  Nhóm C: Module đọc Encoder 17-bit                 ── 14 thí nghiệm      │
  │                                                                          │
  │  Tổng cộng: 31 thí nghiệm cho 3 nhóm chức năng                           │
  └──────────────────────────────────────────────────────────────────────────┘
```

### 2. NHÓM A: THÍ NGHIỆM KIỂM CHỨNG THUẬT TOÁN TỰ HỌC EMA (Module `calibration::learning`) — 7 thí nghiệm

Nhóm thí nghiệm này kiểm chứng tính đúng đắn toán học của thuật toán Exponential Moving Average (EMA) trong module tự học Level 5, bao gồm hàm cập nhật offset `compute_offset_update()`, hàm suy giảm hệ số `compute_decayed_coefficient()` và hàm ước lượng hội tụ `estimate_convergence_steps()`.

#### Thí nghiệm A-1: `test_basic_offset_update` — Xác minh công thức EMA cơ bản

* **Giả thuyết:** Hàm `compute_offset_update()` phải tuân thủ chính xác công thức EMA: `new_offset = current_offset + round(ΔP × λ)`, trong đó ΔP là sai lệch xung và λ là hệ số học.
* **Phương pháp:** Gọi `compute_offset_update(0, 100, 0.1)` với offset ban đầu = 0, sai lệch = 100 xung, hệ số học λ = 0.1.
* **Kết quả kỳ vọng:** 0 + round(100 × 0.1) = 0 + 10 = 10.
* **Tiêu chí đạt:** `assert_eq!(result, 10)` — Giá trị trả về phải bằng chính xác 10.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm A-2: `test_cumulative_offset_update` — Tính cộng dồn tích lũy đa bước

* **Giả thuyết:** Khi nhận nhiều tín hiệu hiệu chuẩn liên tiếp có cùng biên độ sai lệch, offset phải tích lũy tuyến tính theo từng bước EMA. Mỗi bước bổ sung đúng `round(ΔP × λ)` vào offset hiện tại.
* **Phương pháp:** Chuỗi 2 lần gọi liên tiếp:
  * Bước 1: `compute_offset_update(0, 100, 0.1)` → kỳ vọng 10
  * Bước 2: `compute_offset_update(10, 100, 0.1)` → kỳ vọng 20
* **Kết quả kỳ vọng:** Chuỗi giá trị offset: 0 → 10 → 20.
* **Tiêu chí đạt:** `assert_eq!(step1, 10)` và `assert_eq!(step2, 20)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm A-3: `test_negative_delta` — Xử lý sai lệch hướng âm (Lệch trái)

* **Giả thuyết:** Khi đĩa xoay bị lệch về phía bên trái (sai lệch xung âm), thuật toán EMA phải sinh ra offset âm tương ứng để bảo đảm tính đối xứng hướng.
* **Phương pháp:** Gọi `compute_offset_update(0, -100, 0.1)` với sai lệch ΔP = -100 xung.
* **Kết quả kỳ vọng:** 0 + round(-100 × 0.1) = -10.
* **Tiêu chí đạt:** `assert_eq!(result, -10)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm A-4: `test_high_coefficient` — Kiểm tra dải biên hệ số học cực đại

* **Giả thuyết:** Khi hệ số học λ = 1.0 (học tức thì 100%), toàn bộ sai lệch phải được hấp thụ trong một bước duy nhất. Khi λ = 0.5, đúng 50% sai lệch được hấp thụ.
* **Phương pháp:**
  * Gọi `compute_offset_update(0, 100, 1.0)` — kỳ vọng 100
  * Gọi `compute_offset_update(0, 100, 0.5)` — kỳ vọng 50
* **Kết quả kỳ vọng:** 100 và 50 tương ứng.
* **Tiêu chí đạt:** `assert_eq!(result_1, 100)` và `assert_eq!(result_05, 50)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm A-5: `test_zero_delta` — Ổn định khi không có sai lệch

* **Giả thuyết:** Khi tín hiệu hiệu ghi nhận sai lệch bằng 0 (đĩa xoay đã đúng vị trí), offset hiện tại phải được bảo toàn nguyên vẹn, không phát sinh lỗi cộng dồn.
* **Phương pháp:** Gọi `compute_offset_update(42, 0, 0.1)` với offset hiện tại = 42, sai lệch = 0.
* **Kết quả kỳ vọng:** 42 + round(0 × 0.1) = 42 (không thay đổi).
* **Tiêu chí đạt:** `assert_eq!(result, 42)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm A-6: `test_coefficient_decay` — Suy giảm hệ số học theo số lần hiệu chuẩn

* **Giả thuyết:** Hàm `compute_decayed_coefficient()` phải giảm hệ số học theo cấp số nhân `λ_new = λ_base × r^n` (với r = 0.95, n = số lần hiệu chuẩn), nhưng luôn duy trì giá trị dương lớn hơn sàn tối thiểu 0.01.
* **Phương pháp:** Gọi `compute_decayed_coefficient(0.1, 10, 0.95)` sau 10 lần hiệu chuẩn.
* **Kết quả kỳ vọng:** 0.1 × 0.95^10 ≈ 0.0598 — nhỏ hơn 0.1 nhưng lớn hơn 0.01.
* **Tiêu chí đạt:** `assert!(decayed < 0.1)` và `assert!(decayed > 0.01)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm A-7: `test_coefficient_decay_floor` — Sàn an toàn tối thiểu của hệ số học

* **Giả thuyết:** Sau rất nhiều lần hiệu chuẩn (n = 1000), hệ số học bị suy giảm cực mạnh nhưng hàm phải đảm bảo giá trị sàn ≥ 0.01 để hệ thống luôn duy trì khả năng tự học, tránh tình trạng "đóng băng" hoàn toàn.
* **Phương pháp:** Gọi `compute_decayed_coefficient(0.1, 1000, 0.95)`.
* **Kết quả kỳ vọng:** 0.1 × 0.95^1000 ≈ 5.29 × 10^-23 → bị kẹp lên sàn 0.01.
* **Tiêu chí đạt:** `assert!(decayed >= 0.01)`.
* **Kết quả thực tế:** ✅ **PASSED**

### 3. NHÓM B: THÍ NGHIỆM KIỂM CHỨNG THUẬT TOÁN BÙ GÓC QUAY NHỎ (Module `calibration::small_angle_compensation`) — 10 thí nghiệm

Nhóm thí nghiệm này kiểm chứng mô hình suy giảm hàm mũ `P_comp = K_base × e^(-α × |θ|)` và các hàm phụ trợ phân loại, quy đổi của struct `SmallAngleCompensator`.

#### Thí nghiệm B-1: `test_no_compensation_above_threshold` — Triệt tiêu bù tại và vượt ngưỡng

* **Giả thuyết:** Với mọi góc quay |θ| ≥ θ_threshold = 15°, hàm `compute_compensation_pulses()` phải trả về giá trị bù bằng đúng 0 xung. Đĩa xoay ở dải góc bình thường không cần can thiệp bù bổ sung vì ảnh hưởng của dead-zone và ma sát tĩnh là không đáng kể.
* **Phương pháp:**
  * Gọi `compute_compensation_pulses(15.0)` — tại đúng ngưỡng biên
  * Gọi `compute_compensation_pulses(90.0)` — góc lớn tiêu chuẩn
* **Kết quả kỳ vọng:** Cả hai trả về 0.
* **Tiêu chí đạt:** `assert_eq!(comp_15, 0)` và `assert_eq!(comp_90, 0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-2: `test_compensation_decreases_with_angle` — Tính đơn điệu giảm của hàm mũ

* **Giả thuyết:** Hàm bù `P_comp(θ) = K_base × e^(-α × |θ|)` là hàm đơn điệu giảm nghiêm ngặt trên miền [0°, θ_threshold). Giá trị bù tại góc nhỏ hơn phải luôn lớn hơn giá trị bù tại góc lớn hơn: P_comp(1°) > P_comp(5°) > P_comp(10°) > 0.
* **Phương pháp:** Tính bù cho 3 góc đại diện 1°, 5°, 10° và so sánh thứ tự.
* **Kết quả kỳ vọng:**
  * P_comp(1°) = round(60 × e^(-0.15 × 1)) ≈ 51 xung
  * P_comp(5°) = round(60 × e^(-0.15 × 5)) ≈ 28 xung
  * P_comp(10°) = round(60 × e^(-0.15 × 10)) ≈ 13 xung
* **Tiêu chí đạt:** `assert!(comp_1 > comp_5)`, `assert!(comp_5 > comp_10)`, `assert!(comp_10 > 0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-3: `test_compensation_at_zero_degrees` — Giá trị cực đại tại gốc 0°

* **Giả thuyết:** Tại θ = 0°, hàm mũ đạt giá trị cực đại e^0 = 1, do đó P_comp(0°) = K_base × 1 = K_base = 60 xung. Đây là trường hợp biên trên (upper boundary) của dải bù, tương ứng với trạng thái motor đứng yên hoàn toàn khi dead-zone và ma sát tĩnh ảnh hưởng tối đa.
* **Phương pháp:** Gọi `compute_compensation_pulses(0.0)`.
* **Kết quả kỳ vọng:** 60 xung.
* **Tiêu chí đạt:** `assert_eq!(result, 60)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-4: `test_apply_compensation_positive_direction` — Tích hợp bù vào xung thực tế (Hướng dương)

* **Giả thuyết:** Hàm `apply_compensation()` phải cộng xung bù vào xung lý thuyết khi quay phải (is_positive = true). Tổng xung cuối cùng cho góc nhỏ phải lớn hơn giá trị raw thuần.
* **Phương pháp:** Gọi `apply_compensation(5.0, true)`. Giá trị raw P_raw(5°) = round(5 × 131072 / 360) = 1820 xung.
* **Kết quả kỳ vọng:** P_final = 1820 + P_comp(5°) = 1820 + 28 = 1848 > 1820.
* **Tiêu chí đạt:** `assert!(result > 1820)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-5: `test_apply_compensation_negative_direction` — Đảo dấu xung khi quay trái

* **Giả thuyết:** Khi hướng quay là trái (is_positive = false), hàm phải đảo dấu toàn bộ tổng xung thành giá trị âm, bảo đảm Servo Driver CSD7 nhận đúng chiều quay ngược kim đồng hồ.
* **Phương pháp:** Gọi `apply_compensation(5.0, false)`.
* **Kết quả kỳ vọng:** P_final = -(1820 + 28) = -1848 < 0.
* **Tiêu chí đạt:** `assert!(result < 0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-6: `test_apply_compensation_large_angle_no_extra` — Không bù cho góc lớn

* **Giả thuyết:** Với góc quay ≥ 15° (vùng Normal), P_comp = 0 nên tổng xung cuối cùng phải bằng chính xác giá trị raw lý thuyết, không có bất kỳ xung bù nào được cộng thêm.
* **Phương pháp:** Gọi `apply_compensation(90.0, true)`. Giá trị raw P_raw(90°) = 32768.
* **Kết quả kỳ vọng:** P_final = 32768 + 0 = 32768.
* **Tiêu chí đạt:** `assert_eq!(result, 32768)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-7: `test_custom_params` — Tham số hóa linh hoạt bộ bù

* **Giả thuyết:** Constructor `with_params()` phải cho phép tùy chỉnh toàn bộ 3 tham số (K_base, α, θ_threshold) và các giá trị tùy chỉnh phải được lưu trữ và sử dụng chính xác trong tính toán.
* **Phương pháp:** Tạo `SmallAngleCompensator::with_params(131072, 20.0, 100.0, 0.2)`. Kiểm tra getter trả về đúng giá trị, và góc 19° (dưới ngưỡng mới 20°) vẫn được bù.
* **Kết quả kỳ vọng:** threshold = 20.0, base = 100.0, decay = 0.2, comp(19°) > 0.
* **Tiêu chí đạt:** `assert_eq!` cho 3 getter và `assert!(comp_19 > 0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-8: `test_classify_angle` — Phân loại vùng hoạt động góc quay

* **Giả thuyết:** Hàm `classify_angle()` phải phân loại chính xác 3 vùng: [0°, 5°) → `"micro"`, [5°, 15°) → `"small"`, [15°, +∞) → `"normal"`. Phân vùng này quyết định chiến lược vận hành và mức độ cảnh báo trên HMI.
* **Phương pháp:**
  * `classify_angle(2.0)` → kỳ vọng `"micro"`
  * `classify_angle(10.0)` → kỳ vọng `"small"`
  * `classify_angle(45.0)` → kỳ vọng `"normal"`
* **Tiêu chí đạt:** `assert_eq!` cho 3 trường hợp đại diện.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-9: `test_negative_degrees_treated_as_positive` — Tính đối xứng dấu góc quay

* **Giả thuyết:** Hàm bù phải xử lý đối xứng: góc âm -θ và góc dương +θ phải cho ra cùng giá trị bù P_comp, vì dead-zone và ma sát tĩnh không phụ thuộc vào chiều quay. Hàm sử dụng |θ| nội bộ.
* **Phương pháp:** So sánh `compute_compensation_pulses(5.0)` và `compute_compensation_pulses(-5.0)`.
* **Kết quả kỳ vọng:** Hai giá trị bằng nhau.
* **Tiêu chí đạt:** `assert_eq!(comp_pos, comp_neg)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm B-10: `test_compensation_to_degrees` — Quy đổi xung bù sang góc bù tương đương

* **Giả thuyết:** Hàm `compensation_to_degrees()` phải quy đổi chính xác số xung bù sang góc (độ) tương đương thông qua công thức `θ_comp = P_comp × 360 / 131072`. Với thiết lập mặc định, góc bù cho 5° phải nằm trong khoảng (0°, 1°) — đủ nhỏ để không gây quay lố.
* **Phương pháp:** Gọi `compensation_to_degrees(5.0)`.
* **Kết quả kỳ vọng:** 0° < θ_comp < 1°.
* **Tiêu chí đạt:** `assert!(deg > 0.0 && deg < 1.0)`.
* **Kết quả thực tế:** ✅ **PASSED**

### 4. NHÓM C: THÍ NGHIỆM KIỂM CHỨNG MODULE ĐỌC ENCODER 17-BIT (Module `hardware::encoder_reader`) — 14 thí nghiệm

Nhóm thí nghiệm này kiểm chứng toàn bộ pipeline xử lý encoder: từ khởi tạo, cập nhật raw, giải mã thanh ghi Modbus, phát hiện chuyển vòng, quy đổi góc, đến reset trạng thái.

#### Thí nghiệm C-1: `test_new_encoder_defaults` — Trạng thái khởi tạo ban đầu

* **Giả thuyết:** Khi mới được tạo, struct `Encoder17Bit` phải ở trạng thái zeroed: `raw_position = 0`, `turn_count = 0`, `angle = 0.0°`. Không được có giá trị rác từ bộ nhớ chưa khởi tạo.
* **Phương pháp:** Tạo `Encoder17Bit::new()` và đọc toàn bộ trường.
* **Kết quả kỳ vọng:** `raw = 0`, `turns = 0`, `angle = 0.0`.
* **Tiêu chí đạt:** `assert_eq!` cho 3 trường.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-2: `test_update_position_valid` — Cập nhật vị trí hợp lệ và quy đổi góc

* **Giả thuyết:** Khi cập nhật raw = 32,768 (tương đương 90.0° lý thuyết theo công thức `θ = 32768 × 360 / 131072`), hàm `get_angle_degrees()` phải trả về giá trị xấp xỉ 90.0° với sai số tuyệt đối < 0.01°.
* **Phương pháp:** Gọi `update_position(32_768)` → `get_angle_degrees()`.
* **Kết quả kỳ vọng:** 32768 × 360 / 131072 = 90.0° (chính xác, do 32768 = 131072 / 4).
* **Tiêu chí đạt:** `assert_eq!(raw, 32768)` và `assert!((angle - 90.0).abs() < 0.01)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-3: `test_update_position_overflow_rejected` — Từ chối dữ liệu vượt dải 17-bit

* **Giả thuyết:** Giá trị raw ≥ 131,072 (= 2^17) nằm ngoài dải biểu diễn hợp lệ của encoder 17-bit và phải bị từ chối với thông báo lỗi. Cơ chế này bảo vệ hệ thống khỏi dữ liệu nhiễu EMI trên đường truyền RS485.
* **Phương pháp:**
  * Gọi `update_position(131_072)` — tại điểm biên tràn
  * Gọi `update_position(200_000)` — giá trị rác điển hình do nhiễu
* **Kết quả kỳ vọng:** Cả hai trả về `Err(...)`.
* **Tiêu chí đạt:** `assert!(result.is_err())` cho cả 2 trường hợp.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-4: `test_wrap_around_clockwise` — Phát hiện chuyển vòng thuận (CW)

* **Giả thuyết:** Khi cốt motor quay thuận (CW) qua điểm 0°/360°, giá trị raw nhảy từ vùng cao (130,000) xuống vùng thấp (500), tạo delta âm Δ = 500 - 130000 = -129,500. Do |Δ| = 129,500 > 65,536 (ngưỡng nửa vòng), thuật toán phải nhận diện đây là chuyển vòng CW và tăng `turn_count` lên +1.
* **Phương pháp:** Chuỗi gọi `update_position(130_000)` → `update_position(500)`.
* **Kết quả kỳ vọng:** `turn_count = 1`.
* **Tiêu chí đạt:** `assert_eq!(enc.get_turn_count(), 1)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-5: `test_wrap_around_counter_clockwise` — Phát hiện chuyển vòng ngược (CCW)

* **Giả thuyết:** Khi cốt motor quay ngược (CCW) qua điểm 360°/0°, giá trị raw nhảy từ vùng thấp (500) lên vùng cao (130,000), tạo delta dương Δ = 130000 - 500 = 129,500 > 65,536. Thuật toán phải nhận diện đây là chuyển vòng CCW và giảm `turn_count` xuống -1.
* **Phương pháp:** Chuỗi gọi `update_position(500)` → `update_position(130_000)`.
* **Kết quả kỳ vọng:** `turn_count = -1`.
* **Tiêu chí đạt:** `assert_eq!(enc.get_turn_count(), -1)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-6: `test_decode_from_registers` — Giải mã thanh ghi Modbus 16-bit tiêu chuẩn

* **Giả thuyết:** Hàm `decode_from_registers()` phải ghép chính xác 2 thanh ghi 16-bit thành giá trị 32-bit rồi mask 17 bit thấp nhất (`& 0x0001_FFFF`). Với `reg_low = 0x8000, reg_high = 0x0000`, giá trị kết quả = 0 × 2^16 + 32768 = 32,768.
* **Phương pháp:** Gọi `decode_from_registers(0x8000, 0x0000)`.
* **Kết quả kỳ vọng:** `raw_position = 32,768`.
* **Tiêu chí đạt:** `assert_eq!(enc.get_raw_position(), 32_768)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-7: `test_decode_from_registers_17th_bit` — Giải mã Bit thứ 17 ở thanh ghi cao

* **Giả thuyết:** Bit thứ 17 (MSB của encoder) nằm ở bit 0 của thanh ghi cao (`reg_high`). Với `reg_low = 0x0000, reg_high = 0x0001`, giá trị ghép = 1 × 2^16 + 0 = 65,536. Sau khi mask `& 0x1FFFF`, kết quả vẫn là 65,536 vì 65,536 < 131,072.
* **Phương pháp:** Gọi `decode_from_registers(0x0000, 0x0001)`.
* **Kết quả kỳ vọng:** `raw_position = 65,536`.
* **Tiêu chí đạt:** `assert_eq!(enc.get_raw_position(), 65_536)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-8: `test_absolute_angle_multi_turn` — Tính góc tuyệt đối tích lũy đa vòng

* **Giả thuyết:** Sau khi phát hiện chuyển vòng CW (`turn_count = 1`), hàm `get_absolute_angle_degrees()` phải trả về giá trị > 360°, tính theo công thức `θ_abs = N_turn × 360 + θ_single`.
* **Phương pháp:** Chuỗi gọi `update_position(130_000)` → `update_position(100)` (kích hoạt CW wrap).
* **Kết quả kỳ vọng:** θ_abs = 1 × 360 + (100 × 360 / 131072) ≈ 360.27° > 360°.
* **Tiêu chí đạt:** `assert!(abs_angle > 360.0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-9: `test_degrees_to_raw_90` — Quy đổi ngược 90° sang xung Raw

* **Giả thuyết:** Hàm tĩnh `degrees_to_raw(90.0)` phải trả về chính xác 90 × 131072 / 360 = 32,768 xung. Giá trị này là số nguyên chính xác do 131072 chia hết cho 4.
* **Phương pháp:** Gọi `Encoder17Bit::degrees_to_raw(90.0)`.
* **Kết quả kỳ vọng:** 32,768.
* **Tiêu chí đạt:** `assert_eq!(result, 32_768)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-10: `test_degrees_to_raw_360_wraps` — Wrap-around modulo khi quy đổi 360°

* **Giả thuyết:** Một vòng quay đầy đủ 360° tương đương 131,072 xung, nhưng sau phép modulo `% 131,072` phải trả về 0 (quay đủ 1 vòng = trở về gốc).
* **Phương pháp:** Gọi `Encoder17Bit::degrees_to_raw(360.0)`.
* **Kết quả kỳ vọng:** 0.
* **Tiêu chí đạt:** `assert_eq!(result, 0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-11: `test_angular_resolution` — Xác minh độ phân giải góc lý thuyết

* **Giả thuyết:** Độ phân giải góc tối thiểu của encoder 17-bit phải bằng 360 / 131072 ≈ 0.00274658° (≈ 0.048 mrad). Giá trị này xác định ngưỡng phân biệt góc nhỏ nhất mà hệ thống có thể đo lường.
* **Phương pháp:** Gọi `Encoder17Bit::get_angular_resolution()`.
* **Kết quả kỳ vọng:** ≈ 0.00274658° (sai số < 0.0001).
* **Tiêu chí đạt:** `assert!((res - 0.00274658).abs() < 0.0001)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-12: `test_radians_conversion` — Quy đổi sang đơn vị Radian

* **Giả thuyết:** Tại vị trí 90° (raw = 32768), hàm `get_angle_radians()` phải trả về π/2 ≈ 1.5708 radian với sai số < 0.01 rad.
* **Phương pháp:** Gọi `update_position(32_768)` → `get_angle_radians()`.
* **Kết quả kỳ vọng:** 32768 × 2π / 131072 = π/2.
* **Tiêu chí đạt:** `assert!((rad - π/2).abs() < 0.01)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-13: `test_reset` — Reset trạng thái Encoder về gốc

* **Giả thuyết:** Hàm `reset()` phải đưa toàn bộ trạng thái encoder về điều kiện ban đầu: `raw_position = 0`, `turn_count = 0`, `initialized = false`. Thao tác này tương đương với việc tái khởi động encoder từ đầu mà không cần tạo instance mới.
* **Phương pháp:** Cập nhật vị trí `50,000` → `130,000` (thay đổi trạng thái), sau đó gọi `reset()`.
* **Kết quả kỳ vọng:** `raw = 0`, `turns = 0`.
* **Tiêu chí đạt:** `assert_eq!(raw, 0)` và `assert_eq!(turns, 0)`.
* **Kết quả thực tế:** ✅ **PASSED**

#### Thí nghiệm C-14: `test_convergence_estimation` — Ước lượng số bước hội tụ thuật toán EMA

* **Giả thuyết:** Hàm `estimate_convergence_steps()` phải ước tính chính xác số lần hiệu chuẩn cần thiết để offset đạt giá trị mục tiêu, dựa trên công thức `n = ⌈|target - current| / step_size⌉` với `step_size = |ΔP| × λ`.
* **Phương pháp:** Gọi `estimate_convergence_steps(100, 0, 100, 0.1)`. Mỗi bước bù = round(100 × 0.1) = 10 xung. Cần 100 / 10 = 10 bước.
* **Kết quả kỳ vọng:** 10 bước.
* **Tiêu chí đạt:** `assert_eq!(steps, 10)`.
* **Kết quả thực tế:** ✅ **PASSED**

---

### 5. BẢNG TỔNG HỢP KẾT QUẢ THÍ NGHIỆM

| Nhóm | Module | Số thí nghiệm | Đạt | Không đạt | Tỷ lệ thành công |
|:---:|:---|:---:|:---:|:---:|:---:|
| **A** | `calibration::learning` (EMA tự học Level 5) | 7 | 7 | 0 | **100%** |
| **B** | `calibration::small_angle_compensation` (Bù góc nhỏ) | 10 | 10 | 0 | **100%** |
| **C** | `hardware::encoder_reader` (Đọc encoder 17-bit) | 14 | 14 | 0 | **100%** |
| | **TỔNG CỘNG** | **31** | **31** | **0** | **100%** |

**Lệnh thực thi kiểm thử:**

```bash
$ cd rust_backend
$ cargo test --lib

running 31 tests
...
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Kết luận:** Toàn bộ 31/31 thí nghiệm đơn vị đều đạt kết quả **PASSED SUCCESS**, xác nhận tính đúng đắn toán học của mô hình suy giảm hàm mũ `P_comp = K_base × e^(-α|θ|)`, tính chính xác của pipeline giải mã encoder 17-bit (bao gồm cả phát hiện chuyển vòng wrap-around và mask bit `0x1FFFF`), và tính ổn định của thuật toán EMA tự học Level 5. Bộ mã nguồn đạt tiêu chuẩn nghiệm thu 100% để tích hợp vào hệ thống sản xuất LoadingSystem.


