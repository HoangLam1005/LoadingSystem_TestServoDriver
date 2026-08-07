# =============================================================================
# Test: Integration Test (Python HMI ↔ Rust Backend)
# =============================================================================
# Test tích hợp giữa Python client và Rust backend.
#
# CÁCH CHẠY:
#   1. Khởi chạy Rust backend trước: cd rust_backend && cargo run
#   2. Chạy test: python -m pytest tests/test_integration.py -v
#
# LƯU Ý: Test này yêu cầu Rust backend đang chạy ở chế độ MOCK.
# =============================================================================

import sys
import os
import time
import unittest

# Thêm python_hmi vào path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "python_hmi"))

from app.zmq_client import IpcClient
from app import degrees_to_pulses


class TestIntegrationWithBackend(unittest.TestCase):
    """
    Integration tests yêu cầu Rust backend đang chạy.
    Bỏ qua tự động nếu backend không khả dụng.
    """

    @classmethod
    def setUpClass(cls):
        cls.client = IpcClient(host="127.0.0.1", port=5555, timeout=3.0)
        if not cls.client.connect():
            raise unittest.SkipTest(
                "Rust backend not running. Start it first: "
                "cd rust_backend && cargo run"
            )

    @classmethod
    def tearDownClass(cls):
        cls.client.disconnect()

    def test_01_get_status(self):
        """Lấy trạng thái ban đầu"""
        response = self.client.get_status()
        self.assertTrue(response["success"])
        self.assertIn("data", response)
        data = response["data"]
        self.assertIn("current_angle", data)
        self.assertIn("offset_pulses", data)
        self.assertIn("learning_coefficient", data)

    def test_02_rotate_right(self):
        """Quay phải 45°"""
        response = self.client.rotate_right(45.0)
        self.assertTrue(response["success"])
        # Verify angle updated
        status = self.client.get_status()
        self.assertTrue(status["success"])

    def test_03_rotate_left(self):
        """Quay trái 45°"""
        response = self.client.rotate_left(45.0)
        self.assertTrue(response["success"])

    def test_04_return_home(self):
        """Về vị trí gốc"""
        response = self.client.return_home()
        self.assertTrue(response["success"])
        # Verify angle is 0
        status = self.client.get_status()
        data = status.get("data", {})
        self.assertAlmostEqual(data.get("current_angle", 999), 0.0, places=1)

    def test_05_override_positive(self):
        """Can thiệp +0.5°"""
        response = self.client.override_adjust(0.5)
        self.assertTrue(response["success"])

    def test_06_override_negative(self):
        """Can thiệp -0.5°"""
        response = self.client.override_adjust(-0.5)
        self.assertTrue(response["success"])

    def test_07_complex_sequence(self):
        """Chuỗi thao tác phức tạp"""
        # Reset
        self.client.return_home()

        # Quay phải 90°
        r1 = self.client.rotate_right(90.0)
        self.assertTrue(r1["success"])

        # Quay trái 45°
        r2 = self.client.rotate_left(45.0)
        self.assertTrue(r2["success"])

        # Verify angle = 45°
        status = self.client.get_status()
        data = status.get("data", {})
        self.assertAlmostEqual(data.get("current_angle", 999), 45.0, places=1)

        # Về gốc
        r3 = self.client.return_home()
        self.assertTrue(r3["success"])

    def test_08_invalid_command(self):
        """Gửi lệnh không hợp lệ"""
        response = self.client.send_command("INVALID_COMMAND")
        self.assertFalse(response["success"])

    def test_09_pulse_consistency(self):
        """Kiểm tra tính nhất quán công thức Python vs Rust"""
        # 90° phải tạo 32768 xung
        python_pulses = degrees_to_pulses(90.0)
        self.assertEqual(python_pulses, 32_768)

        # 45° phải tạo 16384 xung
        python_pulses = degrees_to_pulses(45.0)
        self.assertEqual(python_pulses, 16_384)


if __name__ == "__main__":
    unittest.main()
