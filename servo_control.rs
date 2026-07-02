#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed as GpioSpeed};
use embassy_stm32::time::Hertz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::{bind_interrupts, peripherals};
use embassy_time::{Duration, Timer};
use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

// ============================================================================
// Hằng số cấu hình Servo Drive CSD7
// ============================================================================

/// Encoder resolution: 23-bit = 8,388,608 pulses per revolution
const ENCODER_PPR: u32 = 8_388_608;

/// Electronic gear ratio (CMX/CDV) - cần tra datasheet chính xác
/// Ví dụ: CMX = 1, CDV = 1 → 1:1
const ELECTRONIC_GEAR_CMX: u32 = 1;
const ELECTRONIC_GEAR_CDV: u32 = 1;

/// Tốc độ định mức motor (RPM)
const RATED_SPEED_RPM: u32 = 3000;

/// Tần số xung mặc định (Hz) cho chế độ Position Mode
const DEFAULT_PULSE_FREQ: u32 = 100_000; // 100 kHz

/// Số xung trên mỗi vòng quay (sau electronic gear)
const PULSES_PER_REV: u32 = ENCODER_PPR * ELECTRONIC_GEAR_CMX / ELECTRONIC_GEAR_CDV;

/// Thời gian tối thiểu chờ sau khi Servo ON (ms)
const SERVO_ON_DELAY_MS: u64 = 500;

// ============================================================================
// Cấu trúc dữ liệu
// ============================================================================

/// Trạng thái của Servo Drive
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServoState {
    /// Drive chưa được cấp nguồn hoặc đang khởi tạo
    Idle,
    /// Drive đã sẵn sàng nhận lệnh (SRDY = ON)
    Ready,
    /// Motor đang quay
    Running,
    /// Motor đang ở vị trí mục tiêu (INP = ON)
    InPosition,
    /// Drive báo lỗi (ALM = ON)
    Alarm,
    /// Dừng khẩn cấp
    EmergencyStop,
}

/// Chiều quay motor
#[derive(Debug, Clone, Copy)]
pub enum RotationDirection {
    /// Quay thuận (Clockwise)
    Clockwise,
    /// Quay ngược (Counter-Clockwise)
    CounterClockwise,
}

/// Lệnh điều khiển motor
#[derive(Debug, Clone, Copy)]
pub struct MotorCommand {
    /// Chiều quay
    pub direction: RotationDirection,
    /// Tốc độ mong muốn (RPM)
    pub speed_rpm: u32,
    /// Số xung cần phát (0 = quay liên tục)
    pub target_pulses: u32,
}

// ============================================================================
// Module điều khiển Servo
// ============================================================================

/// Cấu trúc điều khiển Servo Drive CSD7
pub struct ServoController<'a> {
    /// Chân Pulse (PWM output) - CN1 Pin 1
    pulse_pwm: SimplePwm<'a, peripherals::TIM1>,
    /// Chân Direction (GPIO output) - CN1 Pin 3
    direction_pin: Output<'a>,
    /// Chân Servo ON (GPIO output) - CN1 Pin 7
    servo_on_pin: Output<'a>,
    /// Chân Alarm Reset (GPIO output) - CN1 Pin 15
    alarm_reset_pin: Output<'a>,
    /// Trạng thái hiện tại
    state: ServoState,
}

impl<'a> ServoController<'a> {
    /// Khởi tạo Servo Controller
    ///
    /// # Arguments
    /// * `pulse_pwm` - PWM output cho xung Pulse (Timer 1)
    /// * `direction_pin` - GPIO output cho chiều quay
    /// * `servo_on_pin` - GPIO output cho tín hiệu Servo ON
    /// * `alarm_reset_pin` - GPIO output cho tín hiệu Reset Alarm
    pub fn new(
        pulse_pwm: SimplePwm<'a, peripherals::TIM1>,
        direction_pin: Output<'a>,
        servo_on_pin: Output<'a>,
        alarm_reset_pin: Output<'a>,
    ) -> Self {
        Self {
            pulse_pwm,
            direction_pin,
            servo_on_pin,
            alarm_reset_pin,
            state: ServoState::Idle,
        }
    }

    /// Bật Servo ON - Kích hoạt drive
    ///
    /// Sau khi gọi hàm này, cần chờ ít nhất 500ms
    /// để drive khởi tạo và chuyển sang trạng thái Ready.
    pub async fn servo_on(&mut self) {
        info!("Servo ON - Activating drive...");
        self.servo_on_pin.set_high();
        Timer::after(Duration::from_millis(SERVO_ON_DELAY_MS)).await;
        self.state = ServoState::Ready;
        info!("Servo Ready");
    }

    /// Tắt Servo OFF - Vô hiệu hóa drive
    ///
    /// Motor sẽ mất mô-men và quay tự do.
    /// Phanh từ (nếu có) sẽ tự động đóng.
    pub fn servo_off(&mut self) {
        info!("Servo OFF - Deactivating drive");
        self.stop_pulse();
        self.servo_on_pin.set_low();
        self.state = ServoState::Idle;
    }

    /// Đặt chiều quay
    ///
    /// # Arguments
    /// * `dir` - Chiều quay mong muốn (CW hoặc CCW)
    pub fn set_direction(&mut self, dir: RotationDirection) {
        match dir {
            RotationDirection::Clockwise => {
                self.direction_pin.set_low(); // SIGN = LOW → CW
                info!("Direction: Clockwise (CW)");
            }
            RotationDirection::CounterClockwise => {
                self.direction_pin.set_high(); // SIGN = HIGH → CCW
                info!("Direction: Counter-Clockwise (CCW)");
            }
        }
    }

    /// Phát xung ở tần số cho trước
    ///
    /// Tần số xung quyết định tốc độ quay motor.
    /// Công thức: RPM = (pulse_freq × 60) / pulses_per_rev
    ///
    /// # Arguments
    /// * `freq_hz` - Tần số xung (Hz)
    pub fn start_pulse(&mut self, freq_hz: u32) {
        if freq_hz == 0 {
            warn!("Pulse frequency is 0, stopping motor");
            self.stop_pulse();
            return;
        }

        // Giới hạn tần số không vượt quá khả năng drive
        let safe_freq = freq_hz.min(4_000_000); // Max 4 Mpps theo datasheet CSD7

        info!("Starting pulse at {} Hz", safe_freq);

        // Cấu hình PWM: duty cycle 50% cho xung vuông chuẩn
        self.pulse_pwm.set_frequency(Hertz(safe_freq));
        self.pulse_pwm.set_duty(embassy_stm32::timer::Channel::Ch1, 50);
        self.pulse_pwm.enable(embassy_stm32::timer::Channel::Ch1);

        self.state = ServoState::Running;
    }

    /// Dừng phát xung - Motor giảm tốc và dừng
    pub fn stop_pulse(&mut self) {
        info!("Stopping pulse output");
        self.pulse_pwm.disable(embassy_stm32::timer::Channel::Ch1);

        if self.state == ServoState::Running {
            self.state = ServoState::Ready;
        }
    }

    /// Quay motor với tốc độ và chiều xác định
    ///
    /// # Arguments
    /// * `cmd` - Lệnh điều khiển motor
    pub async fn execute_command(&mut self, cmd: MotorCommand) {
        if self.state == ServoState::Alarm {
            error!("Cannot execute: Drive is in ALARM state!");
            return;
        }

        if self.state == ServoState::Idle {
            error!("Cannot execute: Drive is OFF! Call servo_on() first.");
            return;
        }

        // Đặt chiều quay
        self.set_direction(cmd.direction);

        // Chờ 1ms để tín hiệu Direction ổn định trước khi phát xung
        Timer::after(Duration::from_millis(1)).await;

        // Tính tần số xung từ RPM mong muốn
        // freq = (RPM × pulses_per_rev) / 60
        let freq_hz = if cmd.speed_rpm > 0 {
            (cmd.speed_rpm as u64 * PULSES_PER_REV as u64 / 60) as u32
        } else {
            DEFAULT_PULSE_FREQ
        };

        info!(
            "Executing: dir={}, speed={}RPM, freq={}Hz, pulses={}",
            match cmd.direction {
                RotationDirection::Clockwise => "CW",
                RotationDirection::CounterClockwise => "CCW",
            },
            cmd.speed_rpm,
            freq_hz,
            cmd.target_pulses
        );

        // Phát xung
        self.start_pulse(freq_hz);

        // Nếu có target pulses, đếm và dừng
        if cmd.target_pulses > 0 {
            // Tính thời gian cần phát xung
            // time_ms = (target_pulses / freq_hz) × 1000
            let time_ms = (cmd.target_pulses as u64 * 1000) / freq_hz as u64;
            info!("Running for {} ms to reach {} pulses", time_ms, cmd.target_pulses);

            Timer::after(Duration::from_millis(time_ms)).await;
            self.stop_pulse();
            self.state = ServoState::InPosition;
            info!("Motion complete - In Position");
        }
    }

    /// Reset alarm trên drive
    ///
    /// Gửi xung reset ngắn (100ms) tới chân ARST.
    pub async fn reset_alarm(&mut self) {
        info!("Resetting alarm...");
        self.alarm_reset_pin.set_high();
        Timer::after(Duration::from_millis(100)).await;
        self.alarm_reset_pin.set_low();
        Timer::after(Duration::from_millis(200)).await;
        self.state = ServoState::Ready;
        info!("Alarm reset complete");
    }

    /// Dừng khẩn cấp - Ngay lập tức cắt xung và tắt servo
    pub fn emergency_stop(&mut self) {
        error!("⚠ EMERGENCY STOP ACTIVATED!");
        self.stop_pulse();
        self.servo_off();
        self.state = ServoState::EmergencyStop;
    }

    /// Lấy trạng thái hiện tại
    pub fn get_state(&self) -> ServoState {
        self.state
    }

    /// Tính số xung cần phát để quay một góc (đơn vị: độ)
    ///
    /// # Arguments
    /// * `angle_degrees` - Góc quay (0.0 - 360.0)
    ///
    /// # Returns
    /// Số xung cần phát
    pub fn angle_to_pulses(angle_degrees: f32) -> u32 {
        ((angle_degrees / 360.0) * PULSES_PER_REV as f32) as u32
    }

    /// Tính RPM từ tốc độ tuyến tính (m/s) và đường kính con lăn
    ///
    /// # Arguments
    /// * `velocity_ms` - Tốc độ tuyến tính (m/s)
    /// * `roller_diameter_mm` - Đường kính con lăn (mm)
    /// * `gear_ratio` - Tỷ số truyền hộp số
    ///
    /// # Returns
    /// Tốc độ quay motor (RPM)
    pub fn velocity_to_rpm(velocity_ms: f32, roller_diameter_mm: f32, gear_ratio: f32) -> u32 {
        if roller_diameter_mm <= 0.0 || gear_ratio <= 0.0 {
            return 0;
        }

        let diameter_m = roller_diameter_mm / 1000.0;
        let circumference = core::f32::consts::PI * diameter_m;
        let rpm = (velocity_ms * 60.0) / (circumference * gear_ratio);

        rpm.min(RATED_SPEED_RPM as f32).max(0.0) as u32
    }
}

// ============================================================================
// Entry point (Embassy main task)
// ============================================================================

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("LoadingSystem - Servo Control (Rust/Embassy) starting...");

    let p = embassy_stm32::init(Default::default());

    // Cấu hình các chân I/O
    // PULSE → Timer 1 Channel 1 (PA8)
    // DIRECTION → PA9 (GPIO Output)
    // SERVO_ON → PA10 (GPIO Output)
    // ALARM_RESET → PA11 (GPIO Output)

    let direction = Output::new(p.PA9, Level::Low, GpioSpeed::Medium);
    let servo_on = Output::new(p.PA10, Level::Low, GpioSpeed::Medium);
    let alarm_reset = Output::new(p.PA11, Level::Low, GpioSpeed::Medium);

    // Cấu hình PWM trên Timer 1 Channel 1
    let pwm_pin = PwmPin::new_ch1(p.PA8, embassy_stm32::gpio::OutputType::PushPull);
    let pwm = SimplePwm::new(
        p.TIM1,
        Some(pwm_pin),
        None,
        None,
        None,
        Hertz(DEFAULT_PULSE_FREQ),
        Default::default(),
    );

    // Khởi tạo Servo Controller
    let mut servo = ServoController::new(pwm, direction, servo_on, alarm_reset);

    // === Demo: Quy trình điều khiển cơ bản ===

    // Bước 1: Servo ON
    servo.servo_on().await;

    // Bước 2: Quay thuận (CW) 360° ở 500 RPM
    let cmd_cw = MotorCommand {
        direction: RotationDirection::Clockwise,
        speed_rpm: 500,
        target_pulses: ServoController::angle_to_pulses(360.0),
    };
    servo.execute_command(cmd_cw).await;

    // Chờ 2 giây
    Timer::after(Duration::from_secs(2)).await;

    // Bước 3: Quay ngược (CCW) 180° ở 300 RPM
    let cmd_ccw = MotorCommand {
        direction: RotationDirection::CounterClockwise,
        speed_rpm: 300,
        target_pulses: ServoController::angle_to_pulses(180.0),
    };
    servo.execute_command(cmd_ccw).await;

    // Bước 4: Quay liên tục CW ở 1000 RPM (3 giây rồi dừng)
    let cmd_continuous = MotorCommand {
        direction: RotationDirection::Clockwise,
        speed_rpm: 1000,
        target_pulses: 0, // 0 = quay liên tục
    };
    servo.execute_command(cmd_continuous).await;
    Timer::after(Duration::from_secs(3)).await;
    servo.stop_pulse();

    // Bước 5: Servo OFF
    servo.servo_off();

    info!("Demo complete!");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}
