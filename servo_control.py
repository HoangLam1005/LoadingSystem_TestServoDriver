#!/usr/bin/env python3
"""
Dependencies:
    pip install pymodbus==3.6.0
    pip install pyserial==3.5

Hardware:
    - Raspberry Pi 5 + USB-to-RS485 converter
    - RS Automation CSD7-02DX1 Servo Drive
    - RS Automation CSMT-02BR1ABT3 Servo Motor (200W)
"""

import time
import logging
from enum import IntEnum, Enum
from dataclasses import dataclass
from typing import Optional

from pymodbus.client import ModbusSerialClient
from pymodbus.exceptions import ModbusException

# ==============================================================================
# Cấu hình Logging
# ==============================================================================
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("ServoControl")


# ==============================================================================
# Hằng số cấu hình
# ==============================================================================

# Cổng RS-485 (USB-to-RS485 converter trên RPi5)
SERIAL_PORT = "/dev/ttyUSB0"
BAUD_RATE = 38400
SLAVE_ID = 1  # Địa chỉ Modbus của CSD7 (mặc định = 1)

# Thông số Servo Motor CSMT-02BR1ABT3
RATED_SPEED_RPM = 3000      # Tốc độ định mức (RPM)
RATED_TORQUE_NM = 0.64      # Mô-men xoắn định mức (N·m)
ENCODER_PPR = 8_388_608     # Encoder 23-bit resolution (pulses/rev)
MAX_PULSE_FREQ = 4_000_000  # Tần số xung tối đa (Hz)


# ==============================================================================
# Modbus Register Addresses (CSD7 - cần xác minh với manual chính thức)
# ==============================================================================

class CSD7Register(IntEnum):
    """Modbus holding register addresses cho CSD7 Servo Drive."""
    # Thanh ghi trạng thái
    STATUS_WORD = 0x0000       # Trạng thái drive (bit field)
    ALARM_CODE = 0x0001        # Mã lỗi hiện tại
    ACTUAL_SPEED = 0x0002      # Tốc độ thực tế (RPM)
    ACTUAL_POSITION = 0x0003   # Vị trí thực tế (pulse count)
    ACTUAL_TORQUE = 0x0004     # Mô-men xoắn thực tế (%)

    # Thanh ghi điều khiển
    CONTROL_WORD = 0x0100      # Lệnh điều khiển (bit field)
    TARGET_SPEED = 0x0102      # Tốc độ mục tiêu (RPM)
    TARGET_POSITION = 0x0104   # Vị trí mục tiêu (pulse count)
    ACCEL_TIME = 0x0106        # Thời gian tăng tốc (ms)
    DECEL_TIME = 0x0108        # Thời gian giảm tốc (ms)

    # Thanh ghi cấu hình
    CONTROL_MODE = 0x0200      # Chế độ điều khiển (0=Position, 1=Speed, 2=Torque)
    ELECTRONIC_GEAR_CMX = 0x0202  # Electronic Gear Numerator
    ELECTRONIC_GEAR_CDV = 0x0204  # Electronic Gear Denominator
    MOTOR_CODE = 0x0206        # Mã motor


class ControlMode(IntEnum):
    """Chế độ điều khiển Servo Drive."""
    POSITION = 0   # Chế độ vị trí (Pulse/Direction)
    SPEED = 1      # Chế độ tốc độ (Analog/Digital)
    TORQUE = 2     # Chế độ mô-men xoắn


class RotationDirection(Enum):
    """Chiều quay motor."""
    CLOCKWISE = "CW"
    COUNTER_CLOCKWISE = "CCW"


# ==============================================================================
# Cấu trúc dữ liệu
# ==============================================================================

@dataclass
class ServoStatus:
    """Trạng thái của Servo Drive."""
    is_ready: bool = False          # SRDY: Drive sẵn sàng
    is_servo_on: bool = False       # SON: Servo đang bật
    is_in_position: bool = False    # INP: Motor ở vị trí mục tiêu
    is_alarm: bool = False          # ALM: Có lỗi
    alarm_code: int = 0             # Mã lỗi
    actual_speed: int = 0           # Tốc độ thực tế (RPM)
    actual_position: int = 0        # Vị trí thực tế (pulse)


# ==============================================================================
# Lớp điều khiển Servo Drive CSD7
# ==============================================================================

class ServoController:
    """Điều khiển Servo Drive CSD7-02DX1 qua Modbus-RTU (RS-485).

    Lớp này cung cấp các phương thức cơ bản để:
    - Kết nối/ngắt kết nối với drive
    - Bật/tắt servo (Servo ON/OFF)
    - Quay motor theo chiều và tốc độ mong muốn
    - Đọc trạng thái drive (Ready, Alarm, Position)
    - Reset alarm
    - Dừng khẩn cấp

    Example:
        >>> controller = ServoController(port="/dev/ttyUSB0")
        >>> controller.connect()
        >>> controller.servo_on()
        >>> controller.jog(direction=RotationDirection.CLOCKWISE, speed_rpm=500)
        >>> controller.stop()
        >>> controller.servo_off()
        >>> controller.disconnect()
    """

    def __init__(
        self,
        port: str = SERIAL_PORT,
        baudrate: int = BAUD_RATE,
        slave_id: int = SLAVE_ID,
    ):
        """Khởi tạo Servo Controller.

        Args:
            port: Cổng serial (ví dụ: "/dev/ttyUSB0")
            baudrate: Tốc độ truyền (9600, 19200, 38400, 57600)
            slave_id: Địa chỉ Modbus của drive (1-247)
        """
        self.port = port
        self.baudrate = baudrate
        self.slave_id = slave_id
        self.client: Optional[ModbusSerialClient] = None
        self._is_connected = False

    def connect(self) -> bool:
        """Kết nối tới Servo Drive qua RS-485.

        Returns:
            True nếu kết nối thành công, False nếu thất bại.
        """
        try:
            self.client = ModbusSerialClient(
                port=self.port,
                baudrate=self.baudrate,
                parity="N",
                stopbits=1,
                bytesize=8,
                timeout=1,
            )
            self._is_connected = self.client.connect()

            if self._is_connected:
                logger.info(
                    f"Kết nối thành công: {self.port} @ {self.baudrate} bps, "
                    f"Slave ID: {self.slave_id}"
                )
            else:
                logger.error(f"Không thể kết nối tới {self.port}")

            return self._is_connected

        except Exception as e:
            logger.error(f"Lỗi kết nối: {e}")
            return False

    def disconnect(self) -> None:
        """Ngắt kết nối RS-485."""
        if self.client:
            self.client.close()
            self._is_connected = False
            logger.info("Đã ngắt kết nối RS-485")

    def _write_register(self, address: int, value: int) -> bool:
        """Ghi giá trị vào Modbus holding register.

        Args:
            address: Địa chỉ register
            value: Giá trị cần ghi (0-65535)

        Returns:
            True nếu ghi thành công
        """
        if not self._is_connected or not self.client:
            logger.error("Chưa kết nối! Gọi connect() trước.")
            return False

        try:
            result = self.client.write_register(
                address=address, value=value, slave=self.slave_id
            )
            if result.isError():
                logger.error(f"Lỗi ghi register 0x{address:04X}: {result}")
                return False
            return True

        except ModbusException as e:
            logger.error(f"Modbus exception khi ghi register 0x{address:04X}: {e}")
            return False

    def _read_registers(self, address: int, count: int = 1) -> Optional[list]:
        """Đọc Modbus holding register(s).

        Args:
            address: Địa chỉ register bắt đầu
            count: Số lượng register cần đọc

        Returns:
            Danh sách giá trị register, hoặc None nếu lỗi
        """
        if not self._is_connected or not self.client:
            logger.error("Chưa kết nối! Gọi connect() trước.")
            return None

        try:
            result = self.client.read_holding_registers(
                address=address, count=count, slave=self.slave_id
            )
            if result.isError():
                logger.error(f"Lỗi đọc register 0x{address:04X}: {result}")
                return None
            return result.registers

        except ModbusException as e:
            logger.error(f"Modbus exception khi đọc register 0x{address:04X}: {e}")
            return None

    # ========================================================================
    # Các lệnh điều khiển cơ bản
    # ========================================================================

    def servo_on(self) -> bool:
        """Bật Servo ON - Kích hoạt drive.

        Sau khi gọi hàm này, drive sẽ cấp dòng cho motor
        và chuyển sang trạng thái sẵn sàng (Ready).

        Returns:
            True nếu Servo ON thành công
        """
        logger.info(">>> Servo ON - Đang kích hoạt drive...")

        # Ghi bit Servo ON vào Control Word
        success = self._write_register(CSD7Register.CONTROL_WORD, 0x0001)

        if success:
            # Chờ drive khởi tạo
            time.sleep(0.5)
            logger.info(">>> Servo ON thành công - Drive sẵn sàng")
        else:
            logger.error(">>> Servo ON thất bại!")

        return success

    def servo_off(self) -> bool:
        """Tắt Servo OFF - Vô hiệu hóa drive.

        Motor sẽ mất mô-men và quay tự do.
        Phanh từ (nếu có) sẽ tự động đóng.

        Returns:
            True nếu Servo OFF thành công
        """
        logger.info(">>> Servo OFF - Đang tắt drive...")
        success = self._write_register(CSD7Register.CONTROL_WORD, 0x0000)

        if success:
            logger.info(">>> Servo OFF thành công")

        return success

    def jog(self, direction: RotationDirection, speed_rpm: int) -> bool:
        """Quay motor liên tục theo chiều và tốc độ cho trước (JOG mode).

        Args:
            direction: Chiều quay (CW hoặc CCW)
            speed_rpm: Tốc độ mong muốn (RPM), 0 < speed ≤ 3000

        Returns:
            True nếu lệnh JOG thành công
        """
        # Kiểm tra giới hạn tốc độ
        safe_speed = min(abs(speed_rpm), RATED_SPEED_RPM)

        # Đặt chiều quay bằng dấu của speed
        if direction == RotationDirection.COUNTER_CLOCKWISE:
            # Tốc độ âm = quay ngược (trong Modbus, dùng signed int16)
            safe_speed = 65536 - safe_speed  # Two's complement cho int16

        logger.info(
            f">>> JOG: Chiều={direction.value}, "
            f"Tốc độ={speed_rpm} RPM"
        )

        return self._write_register(CSD7Register.TARGET_SPEED, safe_speed)

    def move_to_position(self, target_pulses: int) -> bool:
        """Di chuyển motor tới vị trí mục tiêu (Position Mode).

        Args:
            target_pulses: Vị trí mục tiêu (đơn vị: pulse count)

        Returns:
            True nếu lệnh thành công
        """
        logger.info(f">>> Move to position: {target_pulses} pulses")

        # Ghi vị trí mục tiêu (có thể cần 2 register cho 32-bit)
        high_word = (target_pulses >> 16) & 0xFFFF
        low_word = target_pulses & 0xFFFF

        success = self._write_register(CSD7Register.TARGET_POSITION, low_word)
        if high_word > 0:
            success = success and self._write_register(
                CSD7Register.TARGET_POSITION + 1, high_word
            )

        return success

    def stop(self) -> bool:
        """Dừng motor (giảm tốc theo Decel Time đã cài đặt).

        Returns:
            True nếu lệnh dừng thành công
        """
        logger.info(">>> STOP - Đang dừng motor...")
        return self._write_register(CSD7Register.TARGET_SPEED, 0)

    def emergency_stop(self) -> None:
        """Dừng khẩn cấp - Tắt Servo ngay lập tức.

        Motor sẽ dừng đột ngột (không giảm tốc).
        Chỉ sử dụng trong trường hợp nguy hiểm!
        """
        logger.warning("⚠️  EMERGENCY STOP - Dừng khẩn cấp!")
        self.servo_off()

    def reset_alarm(self) -> bool:
        """Reset alarm trên drive.

        Returns:
            True nếu reset thành công
        """
        logger.info(">>> Đang reset alarm...")

        # Ghi bit Alarm Reset
        success = self._write_register(CSD7Register.CONTROL_WORD, 0x0080)
        time.sleep(0.2)

        # Xóa bit reset
        self._write_register(CSD7Register.CONTROL_WORD, 0x0000)
        time.sleep(0.3)

        if success:
            logger.info(">>> Alarm đã được reset")

        return success

    # ========================================================================
    # Đọc trạng thái
    # ========================================================================

    def read_status(self) -> Optional[ServoStatus]:
        """Đọc trạng thái hiện tại của Servo Drive.

        Returns:
            ServoStatus object hoặc None nếu lỗi
        """
        # Đọc Status Word
        status_regs = self._read_registers(CSD7Register.STATUS_WORD, count=5)

        if status_regs is None:
            return None

        status = ServoStatus(
            is_ready=bool(status_regs[0] & 0x0001),
            is_servo_on=bool(status_regs[0] & 0x0002),
            is_in_position=bool(status_regs[0] & 0x0004),
            is_alarm=bool(status_regs[0] & 0x0008),
            alarm_code=status_regs[1],
            actual_speed=status_regs[2],
            actual_position=status_regs[3],
        )

        return status

    def print_status(self) -> None:
        """In trạng thái drive ra console."""
        status = self.read_status()

        if status is None:
            logger.error("Không thể đọc trạng thái drive!")
            return

        print("\n" + "=" * 50)
        print("       TRẠNG THÁI SERVO DRIVE CSD7")
        print("=" * 50)
        print(f"  Servo Ready  : {'✅ YES' if status.is_ready else '❌ NO'}")
        print(f"  Servo ON     : {'✅ YES' if status.is_servo_on else '❌ NO'}")
        print(f"  In Position  : {'✅ YES' if status.is_in_position else '❌ NO'}")
        print(f"  Alarm        : {'🚨 YES' if status.is_alarm else '✅ NO'}")
        if status.is_alarm:
            print(f"  Alarm Code   : 0x{status.alarm_code:04X}")
        print(f"  Speed (RPM)  : {status.actual_speed}")
        print(f"  Position     : {status.actual_position}")
        print("=" * 50 + "\n")

    # ========================================================================
    # Cấu hình
    # ========================================================================

    def set_acceleration(self, accel_ms: int, decel_ms: int) -> bool:
        """Đặt thời gian tăng tốc / giảm tốc.

        Args:
            accel_ms: Thời gian tăng tốc (ms), 0 → RATED_SPEED
            decel_ms: Thời gian giảm tốc (ms), RATED_SPEED → 0

        Returns:
            True nếu cài đặt thành công
        """
        logger.info(f">>> Acceleration: {accel_ms}ms, Deceleration: {decel_ms}ms")
        s1 = self._write_register(CSD7Register.ACCEL_TIME, accel_ms)
        s2 = self._write_register(CSD7Register.DECEL_TIME, decel_ms)
        return s1 and s2

    def set_electronic_gear(self, cmx: int, cdv: int) -> bool:
        """Đặt tỷ số Electronic Gear (CMX/CDV).

        Tỷ số này quy đổi giữa số xung lệnh và số xung encoder.
        Ví dụ: CMX=1, CDV=100 → 1 xung lệnh = 100 xung encoder

        Args:
            cmx: Tử số (Numerator)
            cdv: Mẫu số (Denominator)

        Returns:
            True nếu cài đặt thành công
        """
        logger.info(f">>> Electronic Gear: CMX={cmx}, CDV={cdv} (ratio={cmx/cdv:.4f})")
        s1 = self._write_register(CSD7Register.ELECTRONIC_GEAR_CMX, cmx)
        s2 = self._write_register(CSD7Register.ELECTRONIC_GEAR_CDV, cdv)
        return s1 and s2


# ==============================================================================
# Hàm tiện ích
# ==============================================================================

def angle_to_pulses(angle_degrees: float, ppr: int = ENCODER_PPR) -> int:
    """Tính số xung cần phát để quay một góc.

    Args:
        angle_degrees: Góc quay (độ), có thể âm cho quay ngược
        ppr: Pulses per revolution

    Returns:
        Số xung cần phát
    """
    return int((angle_degrees / 360.0) * ppr)


def velocity_to_rpm(
    velocity_ms: float,
    roller_diameter_mm: float,
    gear_ratio: float = 1.0,
) -> int:
    """Tính RPM từ tốc độ tuyến tính.

    Args:
        velocity_ms: Tốc độ tuyến tính (m/s)
        roller_diameter_mm: Đường kính con lăn (mm)
        gear_ratio: Tỷ số truyền hộp số

    Returns:
        Tốc độ quay motor (RPM)
    """
    import math

    if roller_diameter_mm <= 0 or gear_ratio <= 0:
        return 0

    diameter_m = roller_diameter_mm / 1000.0
    circumference = math.pi * diameter_m
    rpm = (velocity_ms * 60.0) / (circumference * gear_ratio)

    return min(int(rpm), RATED_SPEED_RPM)


# ==============================================================================
# Demo: Chương trình chính
# ==============================================================================

def main():
    """Chương trình demo điều khiển servo cơ bản."""
    print("=" * 60)
    print("  LoadingSystem - Servo Control Demo (Python/Modbus-RTU)")
    print("  Target: RS Automation CSD7-02DX1 + CSMT-02BR1ABT3")
    print("=" * 60)

    # Khởi tạo controller
    controller = ServoController(
        port=SERIAL_PORT,
        baudrate=BAUD_RATE,
        slave_id=SLAVE_ID,
    )

    try:
        # Bước 1: Kết nối
        if not controller.connect():
            logger.error("Không thể kết nối! Kiểm tra cáp RS-485 và cổng USB.")
            return

        # Bước 2: Đọc trạng thái ban đầu
        controller.print_status()

        # Bước 3: Cấu hình acceleration
        controller.set_acceleration(accel_ms=500, decel_ms=500)

        # Bước 4: Servo ON
        controller.servo_on()
        controller.print_status()

        # Bước 5: JOG - Quay thuận CW ở 500 RPM
        print("\n>>> [Demo] Quay thuận (CW) @ 500 RPM trong 3 giây...")
        controller.jog(RotationDirection.CLOCKWISE, speed_rpm=500)
        time.sleep(3)

        # Bước 6: Dừng
        controller.stop()
        time.sleep(1)

        # Bước 7: JOG - Quay ngược CCW ở 300 RPM
        print("\n>>> [Demo] Quay ngược (CCW) @ 300 RPM trong 2 giây...")
        controller.jog(RotationDirection.COUNTER_CLOCKWISE, speed_rpm=300)
        time.sleep(2)

        # Bước 8: Dừng
        controller.stop()
        time.sleep(1)

        # Bước 9: Đọc trạng thái cuối
        controller.print_status()

        # Bước 10: Servo OFF
        controller.servo_off()

        print("\n>>> [Demo] Hoàn tất! ✅")

    except KeyboardInterrupt:
        print("\n>>> [Demo] Người dùng nhấn Ctrl+C - Dừng khẩn cấp!")
        controller.emergency_stop()

    finally:
        controller.disconnect()


if __name__ == "__main__":
    main()
