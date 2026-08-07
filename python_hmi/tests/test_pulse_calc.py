# =============================================================================
# Test: Công thức quy đổi Xung ↔ Góc (Python side)
# =============================================================================
# Kiểm chứng công thức Python khớp với Rust:
#   P = round(θ × 131,072 / 360)
# =============================================================================

import unittest
from app import degrees_to_pulses, pulses_to_degrees, ENCODER_RESOLUTION


class TestPulseCalculation(unittest.TestCase):
    """Test công thức quy đổi xung-góc phía Python"""

    def test_encoder_resolution_constant(self):
        self.assertEqual(ENCODER_RESOLUTION, 131_072)

    # ---- degrees_to_pulses ----

    def test_90_degrees(self):
        self.assertEqual(degrees_to_pulses(90.0), 32_768)

    def test_45_degrees(self):
        self.assertEqual(degrees_to_pulses(45.0), 16_384)

    def test_180_degrees(self):
        self.assertEqual(degrees_to_pulses(180.0), 65_536)

    def test_360_degrees(self):
        self.assertEqual(degrees_to_pulses(360.0), 131_072)

    def test_1_degree(self):
        # 1° × 131072 / 360 = 363.89 → round = 364
        self.assertEqual(degrees_to_pulses(1.0), 364)

    def test_0_5_degrees(self):
        # 0.5° × 131072 / 360 = 181.94 → round = 182
        self.assertEqual(degrees_to_pulses(0.5), 182)

    def test_0_degrees(self):
        self.assertEqual(degrees_to_pulses(0.0), 0)

    def test_negative_degrees(self):
        self.assertEqual(degrees_to_pulses(-90.0), -32_768)

    def test_small_angle(self):
        # 0.01° × 131072 / 360 ≈ 3.64 → round = 4
        self.assertEqual(degrees_to_pulses(0.01), 4)

    # ---- pulses_to_degrees ----

    def test_pulses_to_90(self):
        result = pulses_to_degrees(32_768)
        self.assertAlmostEqual(result, 90.0, places=1)

    def test_pulses_to_45(self):
        result = pulses_to_degrees(16_384)
        self.assertAlmostEqual(result, 45.0, places=1)

    def test_pulses_to_360(self):
        result = pulses_to_degrees(131_072)
        self.assertAlmostEqual(result, 360.0, places=1)

    def test_zero_pulses(self):
        self.assertEqual(pulses_to_degrees(0), 0.0)

    # ---- Roundtrip consistency ----

    def test_roundtrip(self):
        """Kiểm tra quy đổi khứ hồi: degrees → pulses → degrees"""
        for angle in [0, 1, 15, 30, 45, 60, 90, 120, 180, 270, 360]:
            pulses = degrees_to_pulses(float(angle))
            back = pulses_to_degrees(pulses)
            self.assertAlmostEqual(
                back, float(angle), delta=0.003,
                msg=f"Roundtrip failed for {angle}°"
            )

    # ---- Consistency with Rust ----

    def test_consistency_with_rust_values(self):
        """Giá trị phải khớp 100% với Rust backend"""
        test_cases = [
            (90.0, 32_768),
            (45.0, 16_384),
            (180.0, 65_536),
            (360.0, 131_072),
            (1.0, 364),
            (0.5, 182),
        ]
        for degrees, expected_pulses in test_cases:
            with self.subTest(degrees=degrees):
                self.assertEqual(
                    degrees_to_pulses(degrees), expected_pulses,
                    f"Mismatch for {degrees}°: expected {expected_pulses}"
                )


if __name__ == "__main__":
    unittest.main()
