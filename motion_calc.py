#!/usr/bin/env python3
"""
Module tính toán tốc độ quay đĩa xoay và tốc độ băng chuyền.

Module này cung cấp các hàm tính toán động học cho:
  A. Băng chuyền (Conveyor Belt) - tốc độ tuyến tính → RPM → tần số xung
  B. Đĩa xoay (Rotary Disc) - vị trí lọ → góc quay → số xung
  C. Motion Profile (Trapezoidal) - gia tốc / giảm tốc mượt mà
"""

import math
from dataclasses import dataclass, field
from enum import Enum
from typing import List, Tuple, Optional


# ==============================================================================
# Hằng số hệ thống (từ datasheet CSD7 + CSMT-02BR1ABT3)
# ==============================================================================

# Encoder resolution: 23-bit serial encoder
ENCODER_PPR: int = 8_388_608  # pulses per revolution

# Electronic Gear Ratio (mặc định 1:1, có thể cấu hình lại trên drive)
DEFAULT_GEAR_CMX: int = 1  # Numerator
DEFAULT_GEAR_CDV: int = 1  # Denominator

# Tốc độ định mức motor
RATED_SPEED_RPM: int = 3000

# Mô-men xoắn định mức
RATED_TORQUE_NM: float = 0.64

# Tần số xung tối đa (Pulse/Direction mode)
MAX_PULSE_FREQ_HZ: int = 4_000_000

# Số lọ trên đĩa xoay
NUM_JARS: int = 10

# Góc giữa hai lọ liên tiếp (degrees)
ANGLE_PER_JAR: float = 360.0 / NUM_JARS  # = 36.0°


# ==============================================================================
# Kiểu dữ liệu
# ==============================================================================

class RotationDirection(Enum):
    """Chiều quay motor."""
    CW = "clockwise"
    CCW = "counter_clockwise"


@dataclass
class ConveyorParams:
    """Thông số băng chuyền.

    Attributes:
        roller_diameter_mm: Đường kính con lăn dẫn động (mm)
        gear_ratio: Tỷ số truyền cơ khí (motor:roller), ví dụ 5.0 = 5:1
        belt_length_m: Chiều dài băng chuyền (m) - tùy chọn
    """
    roller_diameter_mm: float = 50.0  # 50mm mặc định
    gear_ratio: float = 1.0           # Truyền trực tiếp
    belt_length_m: float = 1.0        # 1m mặc định


@dataclass
class DiscParams:
    """Thông số đĩa xoay.

    Attributes:
        num_jars: Số lọ trên đĩa
        electronic_gear_cmx: Electronic Gear Numerator
        electronic_gear_cdv: Electronic Gear Denominator
        encoder_ppr: Encoder resolution (pulses per revolution)
    """
    num_jars: int = NUM_JARS
    electronic_gear_cmx: int = DEFAULT_GEAR_CMX
    electronic_gear_cdv: int = DEFAULT_GEAR_CDV
    encoder_ppr: int = ENCODER_PPR


@dataclass
class MotionProfile:
    """Trapezoidal Motion Profile - kết quả tính toán.

    Attributes:
        total_pulses: Tổng số xung cần phát
        peak_frequency_hz: Tần số xung đỉnh (Hz)
        accel_pulses: Số xung trong giai đoạn tăng tốc
        cruise_pulses: Số xung trong giai đoạn tốc độ đều
        decel_pulses: Số xung trong giai đoạn giảm tốc
        accel_time_s: Thời gian tăng tốc (s)
        cruise_time_s: Thời gian tốc độ đều (s)
        decel_time_s: Thời gian giảm tốc (s)
        total_time_s: Tổng thời gian di chuyển (s)
        direction: Chiều quay
        is_triangle: True nếu profile là tam giác (không đủ quãng đường cho cruise)
    """
    total_pulses: int = 0
    peak_frequency_hz: float = 0.0
    accel_pulses: int = 0
    cruise_pulses: int = 0
    decel_pulses: int = 0
    accel_time_s: float = 0.0
    cruise_time_s: float = 0.0
    decel_time_s: float = 0.0
    total_time_s: float = 0.0
    direction: RotationDirection = RotationDirection.CW
    is_triangle: bool = False


@dataclass
class ConveyorResult:
    """Kết quả tính toán cho băng chuyền.

    Attributes:
        motor_rpm: Tốc độ quay motor (RPM)
        pulse_frequency_hz: Tần số xung Pulse/Direction (Hz)
        belt_speed_ms: Tốc độ thực tế của băng (m/s)
        is_within_limits: True nếu tốc độ nằm trong giới hạn cho phép
        warning: Cảnh báo nếu có
    """
    motor_rpm: float = 0.0
    pulse_frequency_hz: float = 0.0
    belt_speed_ms: float = 0.0
    is_within_limits: bool = True
    warning: str = ""


@dataclass
class DiscResult:
    """Kết quả tính toán cho đĩa xoay.

    Attributes:
        target_angle_deg: Góc cần quay (độ)
        total_pulses: Số xung cần phát
        direction: Chiều quay (CW hoặc CCW)
        num_jars_to_move: Số lọ cần di chuyển
        motion_profile: Motion profile (nếu có)
    """
    target_angle_deg: float = 0.0
    total_pulses: int = 0
    direction: RotationDirection = RotationDirection.CW
    num_jars_to_move: int = 0
    motion_profile: Optional[MotionProfile] = None


# ==============================================================================
# Module A: Tính toán tốc độ Băng chuyền (Conveyor Belt)
# ==============================================================================

def calculate_conveyor_speed(
    target_speed_ms: float,
    params: Optional[ConveyorParams] = None,
    electronic_gear_cmx: int = DEFAULT_GEAR_CMX,
    electronic_gear_cdv: int = DEFAULT_GEAR_CDV,
) -> ConveyorResult:
    """Tính toán tốc độ quay motor và tần số xung cho băng chuyền.

    Công thức:
        RPM = (v × 60) / (π × D × gear_ratio)
        Pulse_Freq = (RPM × PPR × CMX) / (60 × CDV)

    Args:
        target_speed_ms: Tốc độ mong muốn của băng chuyền (m/s)
        params: Thông số băng chuyền (roller diameter, gear ratio, ...)
        electronic_gear_cmx: Electronic Gear Numerator
        electronic_gear_cdv: Electronic Gear Denominator

    Returns:
        ConveyorResult với RPM, tần số xung, và trạng thái giới hạn

    Raises:
        ValueError: Nếu đường kính con lăn hoặc gear ratio ≤ 0
    """
    if params is None:
        params = ConveyorParams()

    result = ConveyorResult()

    # Validation
    if params.roller_diameter_mm <= 0:
        raise ValueError(
            f"Đường kính con lăn phải > 0, nhận được: {params.roller_diameter_mm}mm"
        )
    if params.gear_ratio <= 0:
        raise ValueError(
            f"Tỷ số truyền phải > 0, nhận được: {params.gear_ratio}"
        )
    if electronic_gear_cdv <= 0:
        raise ValueError(
            f"Electronic Gear CDV phải > 0, nhận được: {electronic_gear_cdv}"
        )

    # Xử lý tốc độ = 0
    if target_speed_ms == 0:
        result.belt_speed_ms = 0.0
        result.motor_rpm = 0.0
        result.pulse_frequency_hz = 0.0
        result.is_within_limits = True
        return result

    # Tính toán chu vi con lăn
    roller_diameter_m = params.roller_diameter_mm / 1000.0
    circumference_m = math.pi * roller_diameter_m

    # Tính RPM motor
    # RPM = (v × 60) / (π × D × gear_ratio)
    motor_rpm = (abs(target_speed_ms) * 60.0) / (circumference_m * params.gear_ratio)

    # Kiểm tra giới hạn tốc độ
    if motor_rpm > RATED_SPEED_RPM:
        result.warning = (
            f"Tốc độ yêu cầu ({motor_rpm:.1f} RPM) vượt quá tốc độ "
            f"định mức ({RATED_SPEED_RPM} RPM). Đã giới hạn lại."
        )
        motor_rpm = RATED_SPEED_RPM
        result.is_within_limits = False

    # Tính tần số xung
    # Pulse_Freq = (RPM × PPR × CMX) / (60 × CDV)
    pulse_freq = (motor_rpm * ENCODER_PPR * electronic_gear_cmx) / (
        60.0 * electronic_gear_cdv
    )

    # Giới hạn tần số xung tối đa
    if pulse_freq > MAX_PULSE_FREQ_HZ:
        pulse_freq = MAX_PULSE_FREQ_HZ
        # Tính ngược lại RPM thực tế
        motor_rpm = (pulse_freq * 60.0 * electronic_gear_cdv) / (
            ENCODER_PPR * electronic_gear_cmx
        )
        result.warning = (
            f"Tần số xung vượt giới hạn ({MAX_PULSE_FREQ_HZ} Hz). "
            f"RPM thực tế: {motor_rpm:.1f}"
        )
        result.is_within_limits = False

    # Tính tốc độ băng thực tế (sau khi giới hạn)
    actual_speed = (motor_rpm * circumference_m * params.gear_ratio) / 60.0

    result.motor_rpm = motor_rpm
    result.pulse_frequency_hz = pulse_freq
    result.belt_speed_ms = actual_speed

    return result


# ==============================================================================
# Module B: Tính toán vị trí Đĩa xoay (Rotary Disc)
# ==============================================================================

def calculate_disc_rotation(
    current_jar: int,
    target_jar: int,
    params: Optional[DiscParams] = None,
    max_speed_rpm: int = 500,
    accel_time_s: float = 0.3,
    decel_time_s: float = 0.3,
) -> DiscResult:
    """Tính toán số xung và chiều quay để di chuyển đĩa xoay đến lọ đích.

    Hệ thống chọn chiều quay **ngắn nhất** (CW hoặc CCW) tự động.

    Args:
        current_jar: Vị trí lọ hiện tại (1 đến num_jars)
        target_jar: Vị trí lọ đích (1 đến num_jars)
        params: Thông số đĩa xoay
        max_speed_rpm: Tốc độ quay tối đa (RPM)
        accel_time_s: Thời gian tăng tốc (s)
        decel_time_s: Thời gian giảm tốc (s)

    Returns:
        DiscResult với góc quay, số xung, chiều quay, và motion profile

    Raises:
        ValueError: Nếu vị trí lọ không hợp lệ
    """
    if params is None:
        params = DiscParams()

    num_jars = params.num_jars
    angle_per_jar = 360.0 / num_jars

    # Validation
    if current_jar < 1 or current_jar > num_jars:
        raise ValueError(
            f"Vị trí lọ hiện tại phải từ 1 đến {num_jars}, "
            f"nhận được: {current_jar}"
        )
    if target_jar < 1 or target_jar > num_jars:
        raise ValueError(
            f"Vị trí lọ đích phải từ 1 đến {num_jars}, "
            f"nhận được: {target_jar}"
        )

    result = DiscResult()

    # Trường hợp đã ở vị trí đích
    if current_jar == target_jar:
        result.target_angle_deg = 0.0
        result.total_pulses = 0
        result.num_jars_to_move = 0
        result.direction = RotationDirection.CW
        return result

    # Tính khoảng cách theo chiều CW và CCW
    cw_steps = (target_jar - current_jar) % num_jars
    ccw_steps = (current_jar - target_jar) % num_jars

    # Chọn chiều quay ngắn nhất
    if cw_steps <= ccw_steps:
        direction = RotationDirection.CW
        steps = cw_steps
    else:
        direction = RotationDirection.CCW
        steps = ccw_steps

    # Tính góc quay
    angle_deg = steps * angle_per_jar

    # Tính số xung cần phát
    # pulses = (angle / 360) × PPR × (CMX / CDV)
    effective_ppr = params.encoder_ppr * params.electronic_gear_cmx // params.electronic_gear_cdv
    total_pulses = int((angle_deg / 360.0) * effective_ppr)

    result.target_angle_deg = angle_deg
    result.total_pulses = total_pulses
    result.direction = direction
    result.num_jars_to_move = steps

    # Tính motion profile
    result.motion_profile = calculate_trapezoidal_profile(
        total_pulses=total_pulses,
        max_speed_rpm=max_speed_rpm,
        accel_time_s=accel_time_s,
        decel_time_s=decel_time_s,
        direction=direction,
    )

    return result


# ==============================================================================
# Module C: Trapezoidal Motion Profile
# ==============================================================================

def calculate_trapezoidal_profile(
    total_pulses: int,
    max_speed_rpm: int = 500,
    accel_time_s: float = 0.3,
    decel_time_s: float = 0.3,
    direction: RotationDirection = RotationDirection.CW,
) -> MotionProfile:
    """Tính toán Trapezoidal Motion Profile.

    Trapezoidal profile bao gồm 3 giai đoạn:
    1. Tăng tốc (acceleration) - từ 0 đến max speed
    2. Tốc độ đều (cruise) - giữ max speed
    3. Giảm tốc (deceleration) - từ max speed về 0

    Nếu quãng đường quá ngắn, profile sẽ chuyển thành hình tam giác
    (triangle profile - không có giai đoạn cruise).

    Args:
        total_pulses: Tổng số xung cần phát
        max_speed_rpm: Tốc độ quay tối đa (RPM)
        accel_time_s: Thời gian tăng tốc (s)
        decel_time_s: Thời gian giảm tốc (s)
        direction: Chiều quay

    Returns:
        MotionProfile với chi tiết từng giai đoạn
    """
    profile = MotionProfile()
    profile.total_pulses = total_pulses
    profile.direction = direction

    if total_pulses <= 0 or max_speed_rpm <= 0:
        return profile

    # Giới hạn tốc độ
    safe_rpm = min(max_speed_rpm, RATED_SPEED_RPM)

    # Tính tần số xung đỉnh (peak frequency)
    # peak_freq = (RPM × PPR) / 60
    peak_freq = (safe_rpm * ENCODER_PPR) / 60.0

    # Giới hạn tần số
    if peak_freq > MAX_PULSE_FREQ_HZ:
        peak_freq = MAX_PULSE_FREQ_HZ

    # Tính số xung trong giai đoạn tăng tốc (acceleration)
    # Trong giai đoạn tăng tốc tuyến tính, trung bình tần số = peak_freq/2
    # accel_pulses = (peak_freq / 2) × accel_time_s
    accel_pulses = int(peak_freq * accel_time_s / 2.0)

    # Tính số xung trong giai đoạn giảm tốc (deceleration)
    decel_pulses = int(peak_freq * decel_time_s / 2.0)

    # Kiểm tra xem có đủ quãng đường cho cả accel và decel không
    if accel_pulses + decel_pulses >= total_pulses:
        # Triangle profile: không đủ quãng đường cho cruise
        profile.is_triangle = True

        # Tính lại peak frequency cho triangle profile
        # total_pulses = (peak_freq_new/2) × (accel_time + decel_time)
        total_ramp_time = accel_time_s + decel_time_s
        if total_ramp_time > 0:
            peak_freq = (2.0 * total_pulses) / total_ramp_time
        else:
            peak_freq = 0.0

        # Phân bố lại xung
        ratio = accel_time_s / total_ramp_time if total_ramp_time > 0 else 0.5
        accel_pulses = int(total_pulses * ratio)
        decel_pulses = total_pulses - accel_pulses
        cruise_pulses = 0

        # Tính thời gian thực tế
        profile.accel_time_s = accel_time_s
        profile.decel_time_s = decel_time_s
        profile.cruise_time_s = 0.0
    else:
        # Trapezoidal profile: có giai đoạn cruise
        profile.is_triangle = False
        cruise_pulses = total_pulses - accel_pulses - decel_pulses

        # Tính thời gian cruise
        cruise_time = cruise_pulses / peak_freq if peak_freq > 0 else 0.0

        profile.accel_time_s = accel_time_s
        profile.cruise_time_s = cruise_time
        profile.decel_time_s = decel_time_s

    profile.peak_frequency_hz = peak_freq
    profile.accel_pulses = accel_pulses
    profile.cruise_pulses = cruise_pulses
    profile.decel_pulses = decel_pulses
    profile.total_time_s = profile.accel_time_s + profile.cruise_time_s + profile.decel_time_s

    return profile


# ==============================================================================
# Hàm tiện ích
# ==============================================================================

def rpm_to_pulse_freq(rpm: float, ppr: int = ENCODER_PPR) -> float:
    """Chuyển đổi RPM sang tần số xung (Hz).

    Args:
        rpm: Tốc độ quay (RPM)
        ppr: Pulses per revolution

    Returns:
        Tần số xung (Hz)
    """
    return (rpm * ppr) / 60.0


def pulse_freq_to_rpm(freq_hz: float, ppr: int = ENCODER_PPR) -> float:
    """Chuyển đổi tần số xung (Hz) sang RPM.

    Args:
        freq_hz: Tần số xung (Hz)
        ppr: Pulses per revolution

    Returns:
        Tốc độ quay (RPM)
    """
    if ppr == 0:
        return 0.0
    return (freq_hz * 60.0) / ppr


def angle_to_pulses(angle_deg: float, ppr: int = ENCODER_PPR) -> int:
    """Chuyển đổi góc (độ) sang số xung.

    Args:
        angle_deg: Góc quay (độ)
        ppr: Pulses per revolution

    Returns:
        Số xung tương ứng
    """
    return int((angle_deg / 360.0) * ppr)


def pulses_to_angle(pulses: int, ppr: int = ENCODER_PPR) -> float:
    """Chuyển đổi số xung sang góc (độ).

    Args:
        pulses: Số xung
        ppr: Pulses per revolution

    Returns:
        Góc tương ứng (độ)
    """
    if ppr == 0:
        return 0.0
    return (pulses / ppr) * 360.0


def calculate_travel_time(
    distance_pulses: int,
    speed_rpm: float,
    ppr: int = ENCODER_PPR,
) -> float:
    """Tính thời gian di chuyển (không tính gia tốc/giảm tốc).

    Args:
        distance_pulses: Quãng đường (số xung)
        speed_rpm: Tốc độ quay (RPM)
        ppr: Pulses per revolution

    Returns:
        Thời gian (giây)
    """
    if speed_rpm <= 0 or ppr <= 0:
        return 0.0
    freq = rpm_to_pulse_freq(speed_rpm, ppr)
    if freq <= 0:
        return 0.0
    return distance_pulses / freq


# ==============================================================================
# Demo
# ==============================================================================

def main():
    """Demo các chức năng tính toán."""
    print("=" * 70)
    print("  LoadingSystem - Motion Calculation Module Demo")
    print("  Module 3: Tính toán tốc độ băng chuyền & đĩa xoay")
    print("=" * 70)

    # ----- Demo A: Băng chuyền -----
    print("\n" + "─" * 70)
    print("  A. TÍNH TOÁN TỐC ĐỘ BĂNG CHUYỀN")
    print("─" * 70)

    conveyor = ConveyorParams(
        roller_diameter_mm=50.0,  # Con lăn Ø50mm
        gear_ratio=5.0,           # Hộp số 5:1
    )

    test_speeds = [0.0, 0.05, 0.1, 0.5, 1.0, 2.0]
    print(f"\n  Thông số: Ø con lăn = {conveyor.roller_diameter_mm}mm, "
          f"Tỷ số truyền = {conveyor.gear_ratio}:1")
    print(f"  {'Tốc độ (m/s)':>14} | {'Motor RPM':>12} | {'Pulse Freq (Hz)':>16} | {'Trạng thái':>12}")
    print(f"  {'-'*14}-+-{'-'*12}-+-{'-'*16}-+-{'-'*12}")

    for speed in test_speeds:
        result = calculate_conveyor_speed(speed, conveyor)
        status = "✅ OK" if result.is_within_limits else "⚠️ Limit"
        print(
            f"  {speed:>14.3f} | {result.motor_rpm:>12.1f} | "
            f"{result.pulse_frequency_hz:>16.0f} | {status:>12}"
        )

    # ----- Demo B: Đĩa xoay -----
    print("\n" + "─" * 70)
    print("  B. TÍNH TOÁN VỊ TRÍ ĐĨA XOAY")
    print("─" * 70)

    disc = DiscParams(num_jars=10)

    test_moves = [(1, 2), (1, 5), (1, 10), (3, 8), (7, 2), (5, 5)]
    print(f"\n  Hệ thống: {disc.num_jars} lọ, "
          f"góc mỗi lọ = {360.0/disc.num_jars:.1f}°")
    print(f"  {'Từ→Đến':>10} | {'Chiều':>6} | {'Góc (°)':>10} | "
          f"{'Số xung':>12} | {'Thời gian (s)':>14}")
    print(f"  {'-'*10}-+-{'-'*6}-+-{'-'*10}-+-{'-'*12}-+-{'-'*14}")

    for current, target in test_moves:
        result = calculate_disc_rotation(current, target, disc)
        dir_str = "CW" if result.direction == RotationDirection.CW else "CCW"
        total_time = result.motion_profile.total_time_s if result.motion_profile else 0.0
        print(
            f"  {current:>4} → {target:<4} | {dir_str:>6} | {result.target_angle_deg:>10.1f} | "
            f"{result.total_pulses:>12,d} | {total_time:>14.3f}"
        )

    # ----- Demo C: Motion Profile -----
    print("\n" + "─" * 70)
    print("  C. TRAPEZOIDAL MOTION PROFILE")
    print("─" * 70)

    # Profile cho di chuyển từ lọ 1 đến lọ 5 (180°)
    result = calculate_disc_rotation(1, 5, disc, max_speed_rpm=500)
    profile = result.motion_profile

    if profile:
        print(f"\n  Di chuyển: Lọ 1 → Lọ 5 ({result.target_angle_deg}°)")
        print(f"  Loại profile: {'Tam giác (Triangle)' if profile.is_triangle else 'Hình thang (Trapezoidal)'}")
        print(f"  Tổng xung: {profile.total_pulses:,d}")
        print(f"  Tần số đỉnh: {profile.peak_frequency_hz:,.0f} Hz")
        print(f"  Giai đoạn tăng tốc: {profile.accel_pulses:,d} xung ({profile.accel_time_s:.3f}s)")
        print(f"  Giai đoạn tốc độ đều: {profile.cruise_pulses:,d} xung ({profile.cruise_time_s:.3f}s)")
        print(f"  Giai đoạn giảm tốc: {profile.decel_pulses:,d} xung ({profile.decel_time_s:.3f}s)")
        print(f"  Tổng thời gian: {profile.total_time_s:.3f}s")

    print("\n" + "=" * 70)
    print("  Demo hoàn tất! ✅")
    print("=" * 70)


if __name__ == "__main__":
    main()
