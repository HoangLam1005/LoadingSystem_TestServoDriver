# =============================================================================
# LoadingSystem - IPC Client (TCP with Length-Prefixed JSON)
# =============================================================================
# Client giao tiếp với Rust Backend Daemon qua TCP socket.
# Giao thức: [4 bytes length prefix (big-endian)] + [JSON payload]
#
# Tương thích với ZeroMQ REQ/REP pattern:
#   Client gửi command → Server xử lý → Client nhận response
# =============================================================================

import json
import socket
import struct
import threading
from typing import Any, Optional


class IpcClient:
    """
    Client IPC giao tiếp với Rust Backend Daemon.
    Thread-safe thông qua lock.
    """

    def __init__(self, host: str = "127.0.0.1", port: int = 5555, timeout: float = 20.0):
        """
        Khởi tạo client.

        Args:
            host: Địa chỉ IP của Rust Backend
            port: Cổng TCP
            timeout: Timeout cho mỗi thao tác (giây)
        """
        self.host = host
        self.port = port
        self.timeout = timeout
        self._sock: Optional[socket.socket] = None
        self._lock = threading.Lock()
        self._connected = False

    @property
    def is_connected(self) -> bool:
        """Kiểm tra trạng thái kết nối"""
        return self._connected

    def connect(self) -> bool:
        """
        Kết nối tới Rust Backend.

        Returns:
            True nếu kết nối thành công
        """
        with self._lock:
            try:
                self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                self._sock.settimeout(self.timeout)
                self._sock.connect((self.host, self.port))
                self._connected = True
                return True
            except (socket.error, OSError) as e:
                self._connected = False
                self._sock = None
                print(f"[IPC] Connection failed: {e}")
                return False

    def disconnect(self):
        """Ngắt kết nối"""
        with self._lock:
            if self._sock:
                try:
                    self._sock.close()
                except OSError:
                    pass
                self._sock = None
            self._connected = False

    def send_command(
        self,
        command: str,
        value: Optional[float] = None,
        frequency: Optional[int] = None
    ) -> dict:
        """
        Gửi lệnh tới Rust Backend và nhận phản hồi.

        Args:
            command: Tên lệnh (ROTATE_RIGHT, ROTATE_LEFT, RETURN_HOME, etc.)
            value: Giá trị kèm theo (tùy lệnh)
            frequency: Tần số phát xung tùy chỉnh (Hz)

        Returns:
            Dict chứa response từ server: {"success": bool, "message": str, "data": ...}

        Raises:
            ConnectionError: Nếu chưa kết nối hoặc mất kết nối
        """
        with self._lock:
            if not self._connected or not self._sock:
                raise ConnectionError("Not connected to backend")

            try:
                # Tạo JSON command
                msg = {"command": command}
                if value is not None:
                    msg["value"] = value
                if frequency is not None:
                    msg["frequency"] = int(frequency)

                # Gửi message với length prefix
                data = json.dumps(msg).encode("utf-8")
                self._sock.sendall(struct.pack(">I", len(data)))
                self._sock.sendall(data)

                # Nhận response
                len_data = self._recv_exact(4)
                resp_len = struct.unpack(">I", len_data)[0]
                resp_data = self._recv_exact(resp_len)

                return json.loads(resp_data.decode("utf-8"))

            except (socket.error, OSError, json.JSONDecodeError) as e:
                self._connected = False
                raise ConnectionError(f"Communication error: {e}")

    def _recv_exact(self, n: int) -> bytes:
        """Đọc chính xác n bytes từ socket"""
        data = b""
        while len(data) < n:
            chunk = self._sock.recv(n - len(data))
            if not chunk:
                raise ConnectionError("Connection closed by server")
            data += chunk
        return data

    # =========================================================================
    # Convenience methods cho các lệnh thường dùng
    # =========================================================================

    def rotate_right(self, degrees: float = 90.0) -> dict:
        """Quay phải một góc"""
        return self.send_command("ROTATE_RIGHT", degrees)

    def rotate_left(self, degrees: float = 90.0) -> dict:
        """Quay trái một góc"""
        return self.send_command("ROTATE_LEFT", degrees)

    def return_home(self) -> dict:
        """Đưa về vị trí gốc"""
        return self.send_command("RETURN_HOME")

    def override_adjust(self, degrees: float) -> dict:
        """Can thiệp thủ công (Fine Tune)"""
        return self.send_command("OVERRIDE_ADJUST", degrees)

    def get_status(self) -> dict:
        """Lấy trạng thái hệ thống"""
        return self.send_command("GET_STATUS")

    def shutdown_backend(self) -> dict:
        """Tắt daemon"""
        return self.send_command("SHUTDOWN")
