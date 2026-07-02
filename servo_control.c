/**
 * Hardware Connections:
 *   - PA8  (TIM1_CH1) → PULSE+ (CSD7 CN1 Pin 1) qua optocoupler
 *   - PA9  (GPIO)     → SIGN+  (CSD7 CN1 Pin 3) qua optocoupler
 *   - PA10 (GPIO)     → SON    (CSD7 CN1 Pin 7) qua optocoupler
 *   - PA11 (GPIO)     → ARST   (CSD7 CN1 Pin 15) qua optocoupler
 *   - PB0  (GPIO IN)  → SRDY   (CSD7 CN1 Pin 9) qua optocoupler
 *   - PB1  (GPIO IN)  → INP    (CSD7 CN1 Pin 11) qua optocoupler
 *   - PB2  (GPIO IN)  → ALM    (CSD7 CN1 Pin 13) qua optocoupler
 * ============================================================================
 */

#include "stm32f4xx_hal.h"
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>

/* ============================================================================
 * Hằng số cấu hình
 * ============================================================================ */

/** Encoder resolution: 23-bit = 8,388,608 pulses per revolution */
#define ENCODER_PPR             8388608UL

/** Tốc độ định mức motor (RPM) */
#define RATED_SPEED_RPM         3000UL

/** Tần số xung mặc định (Hz) */
#define DEFAULT_PULSE_FREQ_HZ   100000UL

/** Tần số xung tối đa (Hz) - theo datasheet CSD7 */
#define MAX_PULSE_FREQ_HZ       4000000UL

/** Tần số system clock (Hz) - STM32F407 = 168 MHz */
#define SYSTEM_CLOCK_HZ         168000000UL

/** Thời gian chờ sau Servo ON (ms) */
#define SERVO_ON_DELAY_MS       500

/** Thời gian xung reset alarm (ms) */
#define ALARM_RESET_PULSE_MS    100

/* ============================================================================
 * Định nghĩa chân GPIO
 * ============================================================================ */

/* Output pins */
#define PULSE_GPIO_PORT     GPIOA
#define PULSE_GPIO_PIN      GPIO_PIN_8      /* TIM1_CH1 */

#define DIR_GPIO_PORT       GPIOA
#define DIR_GPIO_PIN        GPIO_PIN_9

#define SON_GPIO_PORT       GPIOA
#define SON_GPIO_PIN        GPIO_PIN_10

#define ARST_GPIO_PORT      GPIOA
#define ARST_GPIO_PIN       GPIO_PIN_11

/* Input pins */
#define SRDY_GPIO_PORT      GPIOB
#define SRDY_GPIO_PIN       GPIO_PIN_0

#define INP_GPIO_PORT       GPIOB
#define INP_GPIO_PIN        GPIO_PIN_1

#define ALM_GPIO_PORT       GPIOB
#define ALM_GPIO_PIN        GPIO_PIN_2

/* ============================================================================
 * Kiểu dữ liệu
 * ============================================================================ */

/** Trạng thái Servo Drive */
typedef enum {
    SERVO_STATE_IDLE = 0,       /**< Drive chưa được kích hoạt */
    SERVO_STATE_READY,          /**< Drive sẵn sàng nhận lệnh */
    SERVO_STATE_RUNNING,        /**< Motor đang quay */
    SERVO_STATE_IN_POSITION,    /**< Motor ở vị trí mục tiêu */
    SERVO_STATE_ALARM,          /**< Drive báo lỗi */
    SERVO_STATE_ESTOP,          /**< Dừng khẩn cấp */
} servo_state_t;

/** Chiều quay */
typedef enum {
    DIRECTION_CW = 0,           /**< Quay thuận (Clockwise) */
    DIRECTION_CCW = 1,          /**< Quay ngược (Counter-Clockwise) */
} rotation_direction_t;

/** Lệnh điều khiển motor */
typedef struct {
    rotation_direction_t direction;  /**< Chiều quay */
    uint32_t speed_rpm;              /**< Tốc độ mong muốn (RPM) */
    uint32_t target_pulses;          /**< Số xung cần phát (0 = liên tục) */
} motor_command_t;

/** Cấu trúc Servo Controller */
typedef struct {
    TIM_HandleTypeDef *htim_pulse;   /**< Timer handle cho PWM */
    uint32_t tim_channel;            /**< Timer channel (TIM_CHANNEL_1) */
    servo_state_t state;             /**< Trạng thái hiện tại */
    uint32_t pulse_count;            /**< Bộ đếm xung đã phát */
    uint32_t target_count;           /**< Mục tiêu số xung */
} servo_controller_t;

/* ============================================================================
 * Biến toàn cục
 * ============================================================================ */

static TIM_HandleTypeDef htim1;
static servo_controller_t g_servo;

/* ============================================================================
 * Khởi tạo phần cứng
 * ============================================================================ */

/**
 * @brief Khởi tạo GPIO cho các chân điều khiển servo
 */
static void servo_gpio_init(void)
{
    GPIO_InitTypeDef gpio = {0};

    /* Bật clock cho GPIO port A và B */
    __HAL_RCC_GPIOA_CLK_ENABLE();
    __HAL_RCC_GPIOB_CLK_ENABLE();

    /* Cấu hình PA8 (TIM1_CH1) - Alternate Function cho PWM */
    gpio.Pin = PULSE_GPIO_PIN;
    gpio.Mode = GPIO_MODE_AF_PP;
    gpio.Pull = GPIO_NOPULL;
    gpio.Speed = GPIO_SPEED_FREQ_HIGH;
    gpio.Alternate = GPIO_AF1_TIM1;
    HAL_GPIO_Init(PULSE_GPIO_PORT, &gpio);

    /* Cấu hình PA9, PA10, PA11 - Output Push-Pull */
    gpio.Pin = DIR_GPIO_PIN | SON_GPIO_PIN | ARST_GPIO_PIN;
    gpio.Mode = GPIO_MODE_OUTPUT_PP;
    gpio.Pull = GPIO_NOPULL;
    gpio.Speed = GPIO_SPEED_FREQ_LOW;
    HAL_GPIO_Init(GPIOA, &gpio);

    /* Cấu hình PB0, PB1, PB2 - Input Pull-Down */
    gpio.Pin = SRDY_GPIO_PIN | INP_GPIO_PIN | ALM_GPIO_PIN;
    gpio.Mode = GPIO_MODE_INPUT;
    gpio.Pull = GPIO_PULLDOWN;
    HAL_GPIO_Init(GPIOB, &gpio);

    /* Đặt trạng thái ban đầu: tất cả output = LOW */
    HAL_GPIO_WritePin(DIR_GPIO_PORT, DIR_GPIO_PIN, GPIO_PIN_RESET);
    HAL_GPIO_WritePin(SON_GPIO_PORT, SON_GPIO_PIN, GPIO_PIN_RESET);
    HAL_GPIO_WritePin(ARST_GPIO_PORT, ARST_GPIO_PIN, GPIO_PIN_RESET);
}

/**
 * @brief Khởi tạo Timer 1 cho PWM phát xung Pulse
 * @param freq_hz Tần số PWM mong muốn (Hz)
 */
static void servo_timer_init(uint32_t freq_hz)
{
    TIM_OC_InitTypeDef sConfig = {0};

    __HAL_RCC_TIM1_CLK_ENABLE();

    /* Tính toán prescaler và period
     * Timer clock = SYSTEM_CLOCK_HZ (168 MHz cho STM32F407)
     * PWM frequency = Timer_clock / ((Prescaler + 1) * (Period + 1))
     */
    uint32_t timer_clock = SYSTEM_CLOCK_HZ;
    uint32_t prescaler = 0;
    uint32_t period = 0;

    /* Chọn prescaler sao cho period nằm trong khoảng hợp lý */
    if (freq_hz > 0) {
        /* Thử prescaler = 0 trước */
        period = (timer_clock / freq_hz) - 1;

        /* Nếu period quá lớn (> 65535), tăng prescaler */
        if (period > 65535) {
            prescaler = (timer_clock / (65536UL * freq_hz));
            period = (timer_clock / ((prescaler + 1) * freq_hz)) - 1;
        }
    } else {
        /* Tần số = 0: dùng giá trị mặc định */
        prescaler = 167; /* 168MHz / 168 = 1 MHz */
        period = 9;      /* 1MHz / 10 = 100 kHz */
    }

    htim1.Instance = TIM1;
    htim1.Init.Prescaler = prescaler;
    htim1.Init.CounterMode = TIM_COUNTERMODE_UP;
    htim1.Init.Period = period;
    htim1.Init.ClockDivision = TIM_CLOCKDIVISION_DIV1;
    htim1.Init.RepetitionCounter = 0;
    htim1.Init.AutoReloadPreload = TIM_AUTORELOAD_PRELOAD_ENABLE;
    HAL_TIM_PWM_Init(&htim1);

    /* Cấu hình PWM Channel 1 - Duty Cycle 50% */
    sConfig.OCMode = TIM_OCMODE_PWM1;
    sConfig.Pulse = (period + 1) / 2;  /* 50% duty cycle */
    sConfig.OCPolarity = TIM_OCPOLARITY_HIGH;
    sConfig.OCFastMode = TIM_OCFAST_DISABLE;
    HAL_TIM_PWM_ConfigChannel(&htim1, &sConfig, TIM_CHANNEL_1);
}

/* ============================================================================
 * API điều khiển Servo
 * ============================================================================ */

/**
 * @brief Khởi tạo Servo Controller
 */
void servo_init(void)
{
    servo_gpio_init();
    servo_timer_init(DEFAULT_PULSE_FREQ_HZ);

    g_servo.htim_pulse = &htim1;
    g_servo.tim_channel = TIM_CHANNEL_1;
    g_servo.state = SERVO_STATE_IDLE;
    g_servo.pulse_count = 0;
    g_servo.target_count = 0;
}

/**
 * @brief Bật Servo ON
 *
 * Kích hoạt drive bằng cách đặt chân SON = HIGH.
 * Chờ 500ms để drive khởi tạo.
 */
void servo_on(void)
{
    HAL_GPIO_WritePin(SON_GPIO_PORT, SON_GPIO_PIN, GPIO_PIN_SET);
    HAL_Delay(SERVO_ON_DELAY_MS);

    /* Kiểm tra SRDY */
    if (HAL_GPIO_ReadPin(SRDY_GPIO_PORT, SRDY_GPIO_PIN) == GPIO_PIN_SET) {
        g_servo.state = SERVO_STATE_READY;
    }
}

/**
 * @brief Tắt Servo OFF
 *
 * Dừng phát xung và tắt tín hiệu SON.
 */
void servo_off(void)
{
    /* Dừng PWM trước */
    HAL_TIM_PWM_Stop(g_servo.htim_pulse, g_servo.tim_channel);

    /* Tắt SON */
    HAL_GPIO_WritePin(SON_GPIO_PORT, SON_GPIO_PIN, GPIO_PIN_RESET);
    g_servo.state = SERVO_STATE_IDLE;
}

/**
 * @brief Đặt chiều quay
 * @param dir Chiều quay (DIRECTION_CW hoặc DIRECTION_CCW)
 */
void servo_set_direction(rotation_direction_t dir)
{
    if (dir == DIRECTION_CW) {
        HAL_GPIO_WritePin(DIR_GPIO_PORT, DIR_GPIO_PIN, GPIO_PIN_RESET);
    } else {
        HAL_GPIO_WritePin(DIR_GPIO_PORT, DIR_GPIO_PIN, GPIO_PIN_SET);
    }
}

/**
 * @brief Phát xung PWM ở tần số cho trước
 * @param freq_hz Tần số xung (Hz)
 */
void servo_start_pulse(uint32_t freq_hz)
{
    if (freq_hz == 0 || freq_hz > MAX_PULSE_FREQ_HZ) {
        return;
    }

    /* Cấu hình lại Timer với tần số mới */
    servo_timer_init(freq_hz);

    /* Bắt đầu phát xung PWM */
    HAL_TIM_PWM_Start(g_servo.htim_pulse, g_servo.tim_channel);
    g_servo.state = SERVO_STATE_RUNNING;
}

/**
 * @brief Dừng phát xung
 */
void servo_stop_pulse(void)
{
    HAL_TIM_PWM_Stop(g_servo.htim_pulse, g_servo.tim_channel);

    if (g_servo.state == SERVO_STATE_RUNNING) {
        g_servo.state = SERVO_STATE_READY;
    }
}

/**
 * @brief Thực thi lệnh điều khiển motor
 * @param cmd Con trỏ tới lệnh điều khiển
 */
void servo_execute(const motor_command_t *cmd)
{
    if (g_servo.state == SERVO_STATE_ALARM ||
        g_servo.state == SERVO_STATE_IDLE) {
        return;
    }

    /* Đặt chiều quay */
    servo_set_direction(cmd->direction);
    HAL_Delay(1); /* Chờ 1ms cho tín hiệu Direction ổn định */

    /* Tính tần số xung từ RPM */
    uint32_t freq_hz = DEFAULT_PULSE_FREQ_HZ;
    if (cmd->speed_rpm > 0 && cmd->speed_rpm <= RATED_SPEED_RPM) {
        freq_hz = (uint32_t)((uint64_t)cmd->speed_rpm * ENCODER_PPR / 60ULL);
        if (freq_hz > MAX_PULSE_FREQ_HZ) {
            freq_hz = MAX_PULSE_FREQ_HZ;
        }
    }

    /* Phát xung */
    servo_start_pulse(freq_hz);

    /* Nếu có target, đợi rồi dừng */
    if (cmd->target_pulses > 0) {
        uint32_t time_ms = (uint32_t)((uint64_t)cmd->target_pulses * 1000ULL / freq_hz);
        HAL_Delay(time_ms);
        servo_stop_pulse();
        g_servo.state = SERVO_STATE_IN_POSITION;
    }
}

/**
 * @brief Reset alarm
 */
void servo_reset_alarm(void)
{
    HAL_GPIO_WritePin(ARST_GPIO_PORT, ARST_GPIO_PIN, GPIO_PIN_SET);
    HAL_Delay(ALARM_RESET_PULSE_MS);
    HAL_GPIO_WritePin(ARST_GPIO_PORT, ARST_GPIO_PIN, GPIO_PIN_RESET);
    HAL_Delay(200);
    g_servo.state = SERVO_STATE_READY;
}

/**
 * @brief Dừng khẩn cấp
 */
void servo_emergency_stop(void)
{
    servo_stop_pulse();
    servo_off();
    g_servo.state = SERVO_STATE_ESTOP;
}

/**
 * @brief Đọc trạng thái drive từ chân I/O
 * @return Trạng thái hiện tại
 */
servo_state_t servo_read_io_status(void)
{
    /* Kiểm tra ALM trước (ưu tiên cao nhất) */
    if (HAL_GPIO_ReadPin(ALM_GPIO_PORT, ALM_GPIO_PIN) == GPIO_PIN_SET) {
        g_servo.state = SERVO_STATE_ALARM;
        return SERVO_STATE_ALARM;
    }

    /* Kiểm tra INP */
    if (HAL_GPIO_ReadPin(INP_GPIO_PORT, INP_GPIO_PIN) == GPIO_PIN_SET) {
        if (g_servo.state != SERVO_STATE_RUNNING) {
            g_servo.state = SERVO_STATE_IN_POSITION;
        }
    }

    /* Kiểm tra SRDY */
    if (HAL_GPIO_ReadPin(SRDY_GPIO_PORT, SRDY_GPIO_PIN) == GPIO_PIN_SET) {
        if (g_servo.state == SERVO_STATE_IDLE) {
            g_servo.state = SERVO_STATE_READY;
        }
    }

    return g_servo.state;
}

/**
 * @brief Lấy trạng thái hiện tại
 */
servo_state_t servo_get_state(void)
{
    return g_servo.state;
}

/* ============================================================================
 * Hàm tiện ích
 * ============================================================================ */

/**
 * @brief Tính số xung để quay một góc
 * @param angle_degrees Góc quay (độ)
 * @return Số xung cần phát
 */
uint32_t angle_to_pulses(float angle_degrees)
{
    return (uint32_t)((angle_degrees / 360.0f) * (float)ENCODER_PPR);
}

/**
 * @brief Tính RPM từ tốc độ tuyến tính
 * @param velocity_ms Tốc độ tuyến tính (m/s)
 * @param roller_diameter_mm Đường kính con lăn (mm)
 * @param gear_ratio Tỷ số truyền hộp số
 * @return RPM
 */
uint32_t velocity_to_rpm(float velocity_ms, float roller_diameter_mm, float gear_ratio)
{
    if (roller_diameter_mm <= 0.0f || gear_ratio <= 0.0f) {
        return 0;
    }

    float diameter_m = roller_diameter_mm / 1000.0f;
    float circumference = 3.14159265f * diameter_m;
    float rpm = (velocity_ms * 60.0f) / (circumference * gear_ratio);

    if (rpm > (float)RATED_SPEED_RPM) {
        rpm = (float)RATED_SPEED_RPM;
    }
    if (rpm < 0.0f) {
        rpm = 0.0f;
    }

    return (uint32_t)rpm;
}

/* ============================================================================
 * Main - Demo
 * ============================================================================ */

int main(void)
{
    HAL_Init();

    /* TODO: Cấu hình System Clock 168 MHz */
    /* SystemClock_Config(); */

    /* Khởi tạo servo controller */
    servo_init();

    /* === Demo === */

    /* Bước 1: Servo ON */
    servo_on();

    /* Bước 2: Quay CW 360° ở tốc độ mặc định */
    motor_command_t cmd1 = {
        .direction = DIRECTION_CW,
        .speed_rpm = 500,
        .target_pulses = angle_to_pulses(360.0f),
    };
    servo_execute(&cmd1);

    HAL_Delay(2000);

    /* Bước 3: Quay CCW 180° */
    motor_command_t cmd2 = {
        .direction = DIRECTION_CCW,
        .speed_rpm = 300,
        .target_pulses = angle_to_pulses(180.0f),
    };
    servo_execute(&cmd2);

    /* Bước 4: Servo OFF */
    servo_off();

    while (1) {
        /* Vòng lặp chính - kiểm tra trạng thái */
        servo_read_io_status();
        HAL_Delay(100);
    }
}
