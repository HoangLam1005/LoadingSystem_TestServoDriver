# =============================================================================
# Test: JSON Command Protocol
# =============================================================================
# Kiểm tra format JSON command gửi từ Python HMI sang Rust Backend.
# =============================================================================

import json
import unittest


class TestCommandFormat(unittest.TestCase):
    """Test cấu trúc JSON command khớp với giao thức đặc tả"""

    def test_rotate_right_format(self):
        cmd = {"command": "ROTATE_RIGHT", "value": 90.0}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["command"], "ROTATE_RIGHT")
        self.assertEqual(parsed["value"], 90.0)

    def test_rotate_left_format(self):
        cmd = {"command": "ROTATE_LEFT", "value": 45.0}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["command"], "ROTATE_LEFT")
        self.assertEqual(parsed["value"], 45.0)

    def test_return_home_format(self):
        cmd = {"command": "RETURN_HOME"}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["command"], "RETURN_HOME")
        self.assertNotIn("value", parsed)

    def test_override_adjust_format(self):
        """Phải khớp chính xác: {"command": "OVERRIDE_ADJUST", "value": 0.5}"""
        cmd = {"command": "OVERRIDE_ADJUST", "value": 0.5}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["command"], "OVERRIDE_ADJUST")
        self.assertEqual(parsed["value"], 0.5)

    def test_negative_override_format(self):
        cmd = {"command": "OVERRIDE_ADJUST", "value": -0.5}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["value"], -0.5)

    def test_get_status_format(self):
        cmd = {"command": "GET_STATUS"}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["command"], "GET_STATUS")

    def test_shutdown_format(self):
        cmd = {"command": "SHUTDOWN"}
        json_str = json.dumps(cmd)
        parsed = json.loads(json_str)
        self.assertEqual(parsed["command"], "SHUTDOWN")


class TestResponseFormat(unittest.TestCase):
    """Test cấu trúc JSON response từ Rust Backend"""

    def test_success_response(self):
        resp = {"success": True, "message": "Rotated 90°"}
        self.assertTrue(resp["success"])
        self.assertIsInstance(resp["message"], str)

    def test_error_response(self):
        resp = {"success": False, "message": "Connection failed"}
        self.assertFalse(resp["success"])

    def test_status_response_with_data(self):
        resp = {
            "success": True,
            "message": "Status",
            "data": {
                "current_angle": 90.0,
                "offset_pulses": 10,
                "learning_coefficient": 0.1,
                "calibration_count": 5,
            },
        }
        self.assertTrue(resp["success"])
        data = resp["data"]
        self.assertIsInstance(data["current_angle"], float)
        self.assertIsInstance(data["offset_pulses"], int)
        self.assertIsInstance(data["learning_coefficient"], float)
        self.assertIsInstance(data["calibration_count"], int)

    def test_status_data_parsing(self):
        """Simulate parsing a JSON response string"""
        json_str = '{"success":true,"message":"OK","data":{"current_angle":45.0,"offset_pulses":18,"learning_coefficient":0.1,"calibration_count":3}}'
        resp = json.loads(json_str)
        self.assertTrue(resp["success"])
        self.assertEqual(resp["data"]["current_angle"], 45.0)
        self.assertEqual(resp["data"]["offset_pulses"], 18)
        self.assertEqual(resp["data"]["calibration_count"], 3)


if __name__ == "__main__":
    unittest.main()
