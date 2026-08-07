# =============================================================================
# Test: IPC Client (TCP with Length-Prefixed JSON)
# =============================================================================
# Kiểm tra IpcClient bằng cách tạo mock TCP server trong test.
# =============================================================================

import json
import socket
import struct
import threading
import time
import unittest

from app.zmq_client import IpcClient


class MockTcpServer:
    """Mock TCP server mô phỏng Rust Backend cho testing"""

    def __init__(self, host="127.0.0.1", port=0):
        self.host = host
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind((host, port))
        self.port = self.sock.getsockname()[1]
        self.sock.listen(1)
        self.running = False
        self._thread = None
        self.received_commands = []
        self.response_override = None

    def start(self):
        self.running = True
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self):
        self.running = False
        try:
            self.sock.close()
        except OSError:
            pass

    def _run(self):
        self.sock.settimeout(1.0)
        while self.running:
            try:
                conn, _ = self.sock.accept()
                conn.settimeout(2.0)
                self._handle_client(conn)
            except socket.timeout:
                continue
            except OSError:
                break

    def _handle_client(self, conn):
        try:
            while self.running:
                # Read length prefix
                len_data = self._recv_exact(conn, 4)
                if not len_data:
                    break
                msg_len = struct.unpack(">I", len_data)[0]

                # Read message
                msg_data = self._recv_exact(conn, msg_len)
                if not msg_data:
                    break
                request = json.loads(msg_data.decode("utf-8"))
                self.received_commands.append(request)

                # Send response
                if self.response_override:
                    response = self.response_override
                else:
                    response = self._make_response(request)

                resp_bytes = json.dumps(response).encode("utf-8")
                conn.sendall(struct.pack(">I", len(resp_bytes)))
                conn.sendall(resp_bytes)
        except (socket.timeout, OSError, json.JSONDecodeError):
            pass
        finally:
            conn.close()

    def _recv_exact(self, conn, n):
        data = b""
        while len(data) < n:
            chunk = conn.recv(n - len(data))
            if not chunk:
                return None
            data += chunk
        return data

    def _make_response(self, request):
        cmd = request.get("command", "")
        if cmd == "GET_STATUS":
            return {
                "success": True,
                "message": "Status",
                "data": {
                    "current_angle": 45.0,
                    "offset_pulses": 10,
                    "learning_coefficient": 0.1,
                    "calibration_count": 2,
                },
            }
        elif cmd == "ROTATE_RIGHT":
            return {"success": True, "message": f"Rotated right {request.get('value', 90)}°"}
        elif cmd == "ROTATE_LEFT":
            return {"success": True, "message": f"Rotated left {request.get('value', 90)}°"}
        elif cmd == "RETURN_HOME":
            return {"success": True, "message": "Returned home"}
        elif cmd == "OVERRIDE_ADJUST":
            return {"success": True, "message": f"Override {request.get('value', 0)}°"}
        else:
            return {"success": False, "message": f"Unknown: {cmd}"}


class TestIpcClient(unittest.TestCase):
    """Test IpcClient giao tiếp với mock server"""

    def setUp(self):
        self.server = MockTcpServer()
        self.server.start()
        time.sleep(0.1)
        self.client = IpcClient(host="127.0.0.1", port=self.server.port, timeout=5.0)

    def tearDown(self):
        self.client.disconnect()
        self.server.stop()

    def test_connect(self):
        result = self.client.connect()
        self.assertTrue(result)
        self.assertTrue(self.client.is_connected)

    def test_disconnect(self):
        self.client.connect()
        self.client.disconnect()
        self.assertFalse(self.client.is_connected)

    def test_connect_fail(self):
        bad_client = IpcClient(host="127.0.0.1", port=1, timeout=1.0)
        result = bad_client.connect()
        self.assertFalse(result)

    def test_send_command_not_connected(self):
        with self.assertRaises(ConnectionError):
            self.client.send_command("GET_STATUS")

    def test_rotate_right(self):
        self.client.connect()
        response = self.client.rotate_right(90.0)
        self.assertTrue(response["success"])
        self.assertIn("90", response["message"])
        # Verify server received correct command
        self.assertEqual(self.server.received_commands[-1]["command"], "ROTATE_RIGHT")
        self.assertEqual(self.server.received_commands[-1]["value"], 90.0)

    def test_rotate_left(self):
        self.client.connect()
        response = self.client.rotate_left(45.0)
        self.assertTrue(response["success"])
        self.assertEqual(self.server.received_commands[-1]["command"], "ROTATE_LEFT")
        self.assertEqual(self.server.received_commands[-1]["value"], 45.0)

    def test_return_home(self):
        self.client.connect()
        response = self.client.return_home()
        self.assertTrue(response["success"])
        self.assertEqual(self.server.received_commands[-1]["command"], "RETURN_HOME")

    def test_override_adjust(self):
        self.client.connect()
        response = self.client.override_adjust(0.5)
        self.assertTrue(response["success"])
        self.assertEqual(self.server.received_commands[-1]["command"], "OVERRIDE_ADJUST")
        self.assertEqual(self.server.received_commands[-1]["value"], 0.5)

    def test_get_status(self):
        self.client.connect()
        response = self.client.get_status()
        self.assertTrue(response["success"])
        self.assertIn("data", response)
        data = response["data"]
        self.assertEqual(data["current_angle"], 45.0)
        self.assertEqual(data["offset_pulses"], 10)
        self.assertEqual(data["learning_coefficient"], 0.1)
        self.assertEqual(data["calibration_count"], 2)

    def test_negative_override(self):
        self.client.connect()
        response = self.client.override_adjust(-0.5)
        self.assertTrue(response["success"])
        self.assertEqual(self.server.received_commands[-1]["value"], -0.5)

    def test_multiple_commands_sequence(self):
        self.client.connect()
        self.client.rotate_right(90.0)
        self.client.rotate_left(45.0)
        self.client.return_home()
        self.assertEqual(len(self.server.received_commands), 3)
        self.assertEqual(self.server.received_commands[0]["command"], "ROTATE_RIGHT")
        self.assertEqual(self.server.received_commands[1]["command"], "ROTATE_LEFT")
        self.assertEqual(self.server.received_commands[2]["command"], "RETURN_HOME")


if __name__ == "__main__":
    unittest.main()
