# =============================================================================
# LoadingSystem - Disc Visualizer (Canvas vẽ đĩa xoay realtime)
# =============================================================================
# Vẽ đĩa xoay hình tròn với kim chỉ góc quay hiện tại.
# Hỗ trợ animation mượt khi cập nhật vị trí.
# =============================================================================

import math
import customtkinter as ctk


class DiscVisualizer(ctk.CTkCanvas):
    """
    Canvas vẽ đĩa xoay với kim chỉ hướng realtime.
    Hiển thị góc quay hiện tại bằng kim chỉ đỏ trên nền tròn.
    """

    # Bảng màu
    COLOR_BG = "#1a1a2e"          # Nền canvas
    COLOR_DISC_OUTER = "#16213e"  # Vòng ngoài đĩa
    COLOR_DISC_INNER = "#0f3460"  # Vòng trong đĩa
    COLOR_NEEDLE = "#e94560"      # Kim chỉ
    COLOR_CENTER = "#e94560"      # Tâm
    COLOR_TICK_MAJOR = "#a8a8a8"  # Vạch chia chính (0°, 90°, 180°, 270°)
    COLOR_TICK_MINOR = "#505050"  # Vạch chia phụ
    COLOR_LABEL = "#ffffff"       # Nhãn số
    COLOR_ANGLE_TEXT = "#e94560"  # Hiển thị góc

    def __init__(self, master, size: int = 280, **kwargs):
        """
        Args:
            master: Widget cha
            size: Kích thước canvas (pixel)
        """
        super().__init__(
            master,
            width=size,
            height=size,
            bg=self.COLOR_BG,
            highlightthickness=0,
            **kwargs,
        )
        self.size = size
        self.center = size // 2
        self.radius = int(size * 0.40)
        self._current_angle = 0.0
        self._target_angle = 0.0
        self._animating = False

        self._draw_disc()

    def update_angle(self, angle: float, animate: bool = True):
        """
        Cập nhật góc quay hiển thị.

        Args:
            angle: Góc mới (độ)
            animate: Có animation mượt không
        """
        self._target_angle = angle
        if animate and abs(self._target_angle - self._current_angle) > 0.5:
            if not self._animating:
                self._animating = True
                self._animate_step()
        else:
            self._current_angle = angle
            self._draw_disc()

    def _animate_step(self):
        """Bước animation - tiến dần tới góc mục tiêu"""
        diff = self._target_angle - self._current_angle
        if abs(diff) < 0.5:
            self._current_angle = self._target_angle
            self._animating = False
            self._draw_disc()
            return

        # Easing: tiến 15% khoảng cách còn lại mỗi frame
        self._current_angle += diff * 0.15
        self._draw_disc()
        self.after(16, self._animate_step)  # ~60 FPS

    def _draw_disc(self):
        """Vẽ lại toàn bộ đĩa xoay"""
        self.delete("all")

        cx, cy = self.center, self.center
        r = self.radius

        # Vòng ngoài (shadow)
        self.create_oval(
            cx - r - 4, cy - r - 4, cx + r + 4, cy + r + 4,
            fill="#0a0a1a", outline=""
        )

        # Vòng ngoài đĩa
        self.create_oval(
            cx - r, cy - r, cx + r, cy + r,
            fill=self.COLOR_DISC_OUTER, outline="#2a2a4e", width=2
        )

        # Vòng trong đĩa
        inner_r = int(r * 0.85)
        self.create_oval(
            cx - inner_r, cy - inner_r, cx + inner_r, cy + inner_r,
            fill=self.COLOR_DISC_INNER, outline="#1a3a6e", width=1
        )

        # Vạch chia
        for deg in range(0, 360, 10):
            is_major = deg % 90 == 0
            is_mid = deg % 30 == 0

            rad = math.radians(deg - 90)  # 0° ở trên
            if is_major:
                r1, r2 = inner_r * 0.75, inner_r * 0.95
                color = self.COLOR_TICK_MAJOR
                width = 2
            elif is_mid:
                r1, r2 = inner_r * 0.82, inner_r * 0.95
                color = self.COLOR_TICK_MINOR
                width = 1.5
            else:
                r1, r2 = inner_r * 0.88, inner_r * 0.95
                color = self.COLOR_TICK_MINOR
                width = 1

            x1 = cx + r1 * math.cos(rad)
            y1 = cy + r1 * math.sin(rad)
            x2 = cx + r2 * math.cos(rad)
            y2 = cy + r2 * math.sin(rad)
            self.create_line(x1, y1, x2, y2, fill=color, width=width)

        # Nhãn góc (0°, 90°, 180°, 270°)
        label_r = inner_r * 0.62
        for deg, label in [(0, "0°"), (90, "90°"), (180, "180°"), (270, "270°")]:
            rad = math.radians(deg - 90)
            lx = cx + label_r * math.cos(rad)
            ly = cy + label_r * math.sin(rad)
            self.create_text(
                lx, ly, text=label,
                fill=self.COLOR_LABEL, font=("Consolas", 9, "bold")
            )

        # Kim chỉ góc
        needle_angle = math.radians(self._current_angle - 90)
        needle_len = inner_r * 0.70
        nx = cx + needle_len * math.cos(needle_angle)
        ny = cy + needle_len * math.sin(needle_angle)
        self.create_line(
            cx, cy, nx, ny,
            fill=self.COLOR_NEEDLE, width=3, capstyle="round"
        )

        # Tâm đĩa
        center_r = 6
        self.create_oval(
            cx - center_r, cy - center_r, cx + center_r, cy + center_r,
            fill=self.COLOR_CENTER, outline="#ff6b7a"
        )

        # Hiển thị góc số
        angle_text = f"{self._current_angle:.1f}°"
        self.create_text(
            cx, cy + r + 20,
            text=angle_text,
            fill=self.COLOR_ANGLE_TEXT,
            font=("Consolas", 14, "bold")
        )
