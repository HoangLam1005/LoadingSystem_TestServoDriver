# =============================================================================
# LoadingSystem - System Flow Visualizer (3D Isometric Light Industrial Theme)
# =============================================================================
# Mô phỏng 3D Không gian 4 phân hệ đóng lọ muỗi:
#   (1) Ray tách muỗi 3D (Khay nghiêng + Phễu nạp 3D + Camera AI)
#   (2) Băng chuyền 3D (Mặt nghiêng xanh + 2 bánh răng 3D)
#   (3) Cần gạt 3D cấp 1-3
#   (4) ĐĨA XOAY 3D MANG 12 LỌ TỰ ĐỘNG GẮN NHÃN MỖI LOÀI 1 LỌ RIÊNG.
# =============================================================================

import math
import random
import customtkinter as ctk
from typing import Optional, Callable, Dict, List


class SystemVisualizer(ctk.CTkCanvas):
    """
    Canvas mô phỏng 3D Không gian 4 phân hệ đóng lọ muỗi với 12 lọ phân biệt 12 loài muỗi.
    """

    # Bảng màu 3D Công nghiệp
    COLOR_BG = "#ffffff"                # Nền canvas trắng sáng
    COLOR_BORDER = "#cbd5e1"            # Viền khung
    COLOR_PANEL_BG = "#f8fafc"          # Thẻ phân hệ
    COLOR_BELT = "#1976d2"              # Mặt băng chuyền xanh
    COLOR_BELT_SURFACE = "#1565c0"      # Thân băng chuyền xanh đậm

    # Danh sách 12 Loài Muỗi & 12 Màu sắc đại diện nổi bật
    SPECIES_NAMES = [
        "1. Aedes aegypti (Vằn)",
        "2. Anopheles (Sốt rét)",
        "3. Culex quinquefasciatus",
        "4. Mansonia uniformis",
        "5. Aedes albopictus",
        "6. Anopheles minimus",
        "7. Culex tritaeniorhynchus",
        "8. Armigeres subalbatus",
        "9. Coquillettidia",
        "10. Toxorhynchites",
        "11. Culiseta annulata",
        "12. Psorophora ferox"
    ]

    SPECIES_COLORS = [
        "#dc2626", "#2563eb", "#16a34a", "#9333ea",
        "#ea580c", "#0891b2", "#db2777", "#78350f",
        "#475569", "#0d9488", "#65a30d", "#7c3aed"
    ]

    def __init__(self, master, width: int = 860, height: int = 350, **kwargs):
        super().__init__(
            master,
            width=width,
            height=height,
            bg=self.COLOR_BG,
            highlightthickness=0,
            **kwargs,
        )
        self.canvas_width = width
        self.canvas_height = height

        # Tọa độ 4 phân hệ theo trục 3D Perspective
        self.ray_x_start = 35
        self.ray_x_end = 200
        self.ray_y = 170

        self.belt_x_start = 220
        self.belt_x_end = 510
        self.belt_y = 170

        self.chute_x_start = 510
        self.chute_x_end = 575

        self.disc_cx = 715
        self.disc_cy = 185
        self.disc_rx = 115
        self.disc_ry = 58

        # Trạng thái Servo Angle
        self._current_angle = 0.0
        self._target_angle = 0.0
        self._animating_disc = False
        self._anim_step_deg = 0.0
        self._anim_remaining_steps = 0

        # Trạng thái phôi muỗi (Muoi flow state)
        self._muoi_phase = "DISC"
        self._muoi_progress = 1.0
        self._muoi_present = True
        self._animating_muoi = False
        self._sprocket_angle = 0.0
        self._active_species_id = 1  # 1 to 12

        # QUẢN LÝ 12 LỌ RIÊNG BIỆT CHO 12 LOÀI MUỖI (1-to-1 Mapping)
        # Lọ i (1..12) mặc định gán cho Loài i (1..12)
        self.jar_counts: Dict[int, int] = {i: 0 for i in range(1, 13)}
        self.jar_assigned_species: Dict[int, Optional[int]] = {i: i for i in range(1, 13)}
        self.species_assigned_jar: Dict[int, Optional[int]] = {i: i for i in range(1, 13)}

        self._on_count_change: Optional[Callable[[Dict[int, int], Dict[int, Optional[int]]], None]] = None

        self._redraw()

    # =========================================================================
    # Physical Servo Rotation Speed Sync
    # =========================================================================

    def update_angle(self, angle: float, duration_sec: float = 0.5):
        """
        Cập nhật góc quay đĩa 3D với tốc độ đồng bộ với Motor thực tế qua duration_sec.
        """
        self._target_angle = angle
        diff = self._target_angle - self._current_angle
        if abs(diff) > 0.01 and duration_sec > 0.05:
            # Tính số step theo 60 FPS trong khoảng duration_sec
            fps = 60
            total_steps = max(1, int(duration_sec * fps))
            self._anim_step_deg = diff / total_steps
            self._anim_remaining_steps = total_steps
            if not self._animating_disc:
                self._animating_disc = True
                self._animate_disc_step()
        else:
            self._current_angle = angle
            self._redraw()

    def _animate_disc_step(self):
        """Animation xoay 3D đĩa xoay đúng tốc độ"""
        if self._anim_remaining_steps > 0:
            self._current_angle += self._anim_step_deg
            self._anim_remaining_steps -= 1
            self._redraw()
            self.after(16, self._animate_disc_step)  # ~60 FPS (16ms)
        else:
            self._current_angle = self._target_angle
            self._animating_disc = False
            self._redraw()

    # =========================================================================
    # Flow & Species Assignment Logic
    # =========================================================================

    def start_muoi_flow_simulation(
        self,
        species_id: int = 1,
        on_complete: Optional[Callable[[], None]] = None
    ):
        """Khởi chạy mô phỏng 1 con muỗi thuộc loài species_id rơi vào lọ hứng"""
        if self._animating_muoi:
            return

        self._active_species_id = species_id
        self._muoi_phase = "RAY"
        self._muoi_progress = 0.0
        self._muoi_present = True
        self._animating_muoi = True
        self._on_flow_complete = on_complete
        self._animate_muoi_step()

    def _animate_muoi_step(self):
        """Bước di chuyển qua các phân hệ 3D"""
        if not self._animating_muoi:
            return

        speed = 0.03

        if self._muoi_phase == "RAY":
            self._muoi_progress += speed * 1.3
            if self._muoi_progress >= 1.0:
                self._muoi_phase = "BELT"
                self._muoi_progress = 0.0

        elif self._muoi_phase == "BELT":
            self._muoi_progress += speed
            self._sprocket_angle += 14.0
            if self._muoi_progress >= 1.0:
                self._muoi_phase = "CHUTE"
                self._muoi_progress = 0.0

        elif self._muoi_phase == "CHUTE":
            self._muoi_progress += speed * 2.0
            if self._muoi_progress >= 1.0:
                self._muoi_phase = "DISC"
                self._muoi_progress = 1.0
                self._animating_muoi = False

                # Muỗi rơi vào lọ tương ứng loài
                incoming_species = self._active_species_id
                target_jar = self.species_assigned_jar.get(incoming_species, 1)

                self.jar_counts[target_jar] += 1

                if self._on_count_change:
                    self._on_count_change(self.jar_counts, self.jar_assigned_species)

                if hasattr(self, "_on_flow_complete") and self._on_flow_complete:
                    self._on_flow_complete()

        self._redraw()

        if self._animating_muoi:
            self.after(20, self._animate_muoi_step)

    def get_active_jar_index(self) -> int:
        """Xác định số thứ tự lọ (1-12) đang ở vị trí máng nạp"""
        servo_deg = self._current_angle
        jar_idx = (int(round(((-servo_deg + 180) % 360) / 30.0)) % 12) + 1
        return jar_idx

    def set_count_change_callback(self, callback: Callable[[Dict[int, int], Dict[int, Optional[int]]], None]):
        self._on_count_change = callback

    # =========================================================================
    # Pseudo-3D Canvas Rendering
    # =========================================================================

    def _redraw(self):
        self.delete("all")
        self._draw_3d_background_panels()
        self._draw_stage_1_ray_3d()
        self._draw_stage_2_conveyor_3d()
        self._draw_stage_3_chute_3d()
        self._draw_stage_4_disc_3d_12_jars()
        if self._muoi_present:
            self._draw_moving_muoi_3d()

    def _draw_3d_background_panels(self):
        """Vẽ thẻ phân hệ chuẩn 3D Light Industrial Theme"""
        self._draw_3d_box(12, 12, 215, 345, depth=4, fill=self.COLOR_PANEL_BG, outline=self.COLOR_BORDER)
        self.create_text(113, 30, text="(1) RAY TÁCH MUỖI 3D", fill="#0b3c5d", font=("Segoe UI", 10, "bold"))

        self._draw_3d_box(220, 12, 505, 345, depth=4, fill=self.COLOR_PANEL_BG, outline=self.COLOR_BORDER)
        self.create_text(362, 30, text="(2) BĂNG CHUYỀN TỰ ĐỘNG 3D", fill="#0b3c5d", font=("Segoe UI", 10, "bold"))

        self._draw_3d_box(510, 12, 575, 345, depth=4, fill=self.COLOR_PANEL_BG, outline=self.COLOR_BORDER)
        self.create_text(542, 30, text="(3)", fill="#0b3c5d", font=("Segoe UI", 9, "bold"))

        self._draw_3d_box(580, 12, 848, 345, depth=4, fill=self.COLOR_PANEL_BG, outline=self.COLOR_BORDER)
        self.create_text(714, 30, text="(4) ĐĨA XOAY 3D (12 LỌ 12 LOÀI MUỖI)", fill="#0b3c5d", font=("Segoe UI", 10, "bold"))

    def _draw_3d_box(self, x1: float, y1: float, x2: float, y2: float, depth: float = 6, fill: str = "#ffffff", outline: str = "#cbd5e1"):
        """Vẽ hình hộp chữ nhật có khối 3D đổ bóng nhẹ"""
        self.create_polygon(x1, y2, x2, y2, x2 + depth, y2 + depth, x1 + depth, y2 + depth, fill="#e2e8f0", outline="")
        self.create_polygon(x2, y1, x2 + depth, y1 + depth, x2 + depth, y2 + depth, x2, y2, fill="#cbd5e1", outline="")
        self.create_rectangle(x1, y1, x2, y2, fill=fill, outline=outline, width=1)

    def _draw_stage_1_ray_3d(self):
        """(1) Phân hệ Ray 3D"""
        ry = self.ray_y

        # Phễu nạp 3D
        self._draw_3d_box(165, ry - 115, 205, ry - 75, depth=5, fill="#ffffff", outline="#1e293b")
        self.create_rectangle(170, ry - 120, 200, ry - 115, fill="#1976d2", outline="#0d47a1")
        self.create_text(185, ry - 95, text="Phễu Lọ", fill="#0b3c5d", font=("Segoe UI", 8, "bold"))
        self.create_line(185, ry - 75, 185, ry - 55, fill="#1976d2", width=2, arrow="last")

        # 3 Khay ray nghiêng 3D
        for y_offset in [-42, -12, 18]:
            x1, y1 = 30, ry + y_offset
            x2, y2 = 185, ry + y_offset + 18
            self.create_polygon(x1, y1, x2, y2, x2, y2 + 6, x1, y1 + 6, fill="#e2e8f0", outline="#94a3b8")
            self.create_polygon(x1, y1 + 6, x2, y2 + 6, x2 + 4, y2 + 10, x1 + 4, y1 + 10, fill="#cbd5e1", outline="")
            for dot_x in range(int(x1 + 15), int(x2 - 10), 22):
                dot_y = y1 + (dot_x - x1) * 0.11 + 3
                self.create_oval(dot_x - 1.5, dot_y - 1.5, dot_x + 1.5, dot_y + 1.5, fill="#475569", outline="")

    def _draw_stage_2_conveyor_3d(self):
        """(2) Băng chuyền 3D"""
        by = self.belt_y
        bx1, bx2 = self.belt_x_start + 10, self.belt_x_end - 10

        poly_3d = [bx1, by - 24, bx2, by - 24, bx2 + 12, by + 16, bx1 + 12, by + 16]
        self.create_polygon(poly_3d, fill=self.COLOR_BELT_SURFACE, outline="#0d47a1", width=1.5)

        poly_top = [bx1 + 8, by - 18, bx2 - 8, by - 18, bx2 + 4, by + 10, bx1 + 4, by + 10]
        self.create_polygon(poly_top, fill=self.COLOR_BELT, outline="")

        for x in range(int(bx1 + 20), int(bx2 - 20), 38):
            self.create_line(x, by - 16, x + 6, by + 8, fill="#64b5f6", width=2)

        for sp_x in [bx1 + 18, bx2 - 18]:
            self._draw_3d_sprocket(sp_x, by, radius=17, teeth=8, angle_deg=self._sprocket_angle)

        # CAMERA AI VISION SENSOR 3D Ở ĐẦU BĂNG CHUYỀN (CHÍNH XÁC THEO THỰC TẾ)
        cam_x, cam_y = bx1 + 35, by - 75
        # Chân đế camera 3D
        self.create_line(cam_x, cam_y + 10, cam_x, by - 18, fill="#475569", width=2)
        # Tia quét laser Vision xuống đầu băng chuyền
        self.create_polygon(cam_x - 14, by - 14, cam_x + 14, by - 14, cam_x, cam_y + 8, fill="#e0f2fe", outline="#38bdf8", width=1)
        # Thân Camera 3D
        self._draw_3d_box(cam_x - 14, cam_y - 12, cam_x + 14, cam_y + 10, depth=4, fill="#1e293b", outline="#0284c7")
        self.create_oval(cam_x - 6, cam_y - 5, cam_x + 6, cam_y + 5, fill="#0284c7", outline="#ffffff", width=1.5)
        self.create_text(cam_x, cam_y - 20, text="📷 AI Vision Sensor (Đầu Băng Chuyền)", fill="#0369a1", font=("Segoe UI", 8, "bold"))

        self.create_text((bx1 + bx2) // 2, by + 36, text="► BĂNG CHUYỀN 3D TỰ ĐỘNG ►", fill="#0d47a1", font=("Consolas", 9, "bold"))

    def _draw_3d_sprocket(self, cx: float, cy: float, radius: float, teeth: int, angle_deg: float):
        points = []
        for i in range(teeth * 2):
            r = radius if i % 2 == 0 else radius * 0.65
            a = math.radians(angle_deg + i * (360 / (teeth * 2)))
            points.extend([cx + r * math.cos(a), cy + r * math.sin(a) * 0.7])

        self.create_polygon(points, fill="#0b3c5d", outline="#627d98", width=1.5)
        self.create_oval(cx - 4, cy - 3, cx + 4, cy + 3, fill="#ffffff", outline="")

    def _draw_stage_3_chute_3d(self):
        """(3) Cần gạt máng dẫn 3D"""
        x1, y1 = 510, 145
        x2, y2 = 575, 210

        self.create_polygon(x1, y1, x2, y2, x2 - 8, y2 + 14, x1, y1 + 14, fill="#1e88e5", outline="#0d47a1", width=1.5)
        self.create_polygon(x1, y1 + 14, x2 - 8, y2 + 14, x2 - 4, y2 + 18, x1 + 4, y1 + 18, fill="#1565c0", outline="")
        self.create_text(542, y1 - 14, text="Cần gạt 3D", fill="#0d47a1", font=("Segoe UI", 8, "bold"))
        self.create_text(542, y2 + 22, text="Cấp 1-3", fill="#1e88e5", font=("Consolas", 8, "bold"))

    def _draw_stage_4_disc_3d_12_jars(self):
        """(4) ĐĨA XOAY 3D PERSPECTIVE MANG 12 LỌ 12 LOÀI MUỖI NỔI BẬT"""
        cx, cy = self.disc_cx, self.disc_cy
        rx, ry = self.disc_rx, self.disc_ry

        self.create_oval(cx - rx - 4, cy - ry + 12, cx + rx + 4, cy + ry + 16, fill="#94a3b8", outline="")
        self.create_polygon(
            cx - rx, cy, cx + rx, cy,
            cx + rx, cy + 14, cx - rx, cy + 14,
            fill="#cbd5e1", outline="#94a3b8"
        )
        self.create_oval(cx - rx, cy - ry, cx + rx, cy + ry, fill="#f1f5f9", outline="#1565c0", width=2)

        inner_rx, inner_ry = int(rx * 0.78), int(ry * 0.78)
        self.create_oval(cx - inner_rx, cy - inner_ry, cx + inner_rx, cy + inner_ry, fill="#ffffff", outline="#bbdefb", width=1)

        # ---------------------------------------------------------------------
        # Vẽ 12 Lọ 3D (Z-Ordering theo độ sâu Y)
        # ---------------------------------------------------------------------
        jar_orbit_rx = inner_rx * 0.85
        jar_orbit_ry = inner_ry * 0.85

        servo_deg = self._current_angle
        active_jar_index = self.get_active_jar_index()

        jars_to_draw = []
        for idx in range(12):
            jar_num = idx + 1
            base_deg = idx * 30.0
            total_deg = base_deg + servo_deg - 90

            rad = math.radians(total_deg)
            jx = cx + jar_orbit_rx * math.cos(rad)
            jy = cy + jar_orbit_ry * math.sin(rad)
            jars_to_draw.append((jy, jx, jar_num))

        jars_to_draw.sort(key=lambda item: item[0])

        for jy, jx, jar_num in jars_to_draw:
            is_active = (jar_num == active_jar_index)
            self._draw_3d_jar(jx, jy, jar_num, is_active)

        # Kim chỉ hướng Servo 3D
        needle_rad = math.radians(servo_deg - 90)
        nx = cx + (inner_rx * 0.55) * math.cos(needle_rad)
        ny = cy + (inner_ry * 0.55) * math.sin(needle_rad)
        self.create_line(cx, cy, nx, ny, fill="#dc2626", width=2.5, capstyle="round")

        # Tâm đĩa 3D
        self.create_oval(cx - 6, cy - 4, cx + 6, cy + 4, fill="#0d47a1", outline="#ffffff", width=1.5)

        # Hiển thị nhãn loài của Lọ Active hiện tại
        active_species_id = self.jar_assigned_species[active_jar_index]
        active_label = self.SPECIES_NAMES[active_species_id - 1]

        self.create_text(
            cx, cy + ry + 24,
            text=f"SERVO: {servo_deg:.1f}° | LỌ #{active_jar_index}: {active_label}",
            fill="#0d47a1", font=("Consolas", 10, "bold")
        )

    def _draw_3d_jar(self, jx: float, jy: float, jar_num: int, is_active: bool):
        """
        Vẽ 1 Lọ 3D thủy tinh với MÀU RIÊNG CHO TỪNG LỌ & SỐ LƯỢNG NẰM CHÍNH GIỮA LỌ!
        """
        w, h = 20, 26
        species_id = self.jar_assigned_species[jar_num]
        count = self.jar_counts[jar_num]
        species_color = self.SPECIES_COLORS[species_id - 1]

        body_fill = "#f0fdf4" if is_active else "#ffffff"
        border_color = "#16a34a" if is_active else "#64748b"
        border_w = 2.0 if is_active else 1.2

        # 1. Khối nắp 3D mang màu đặc trưng của loài
        self.create_oval(jx - 6, jy - h // 2 - 6, jx + 6, jy - h // 2 - 2, fill=species_color, outline="#000000", width=1)

        # 2. Thân lọ 3D hình trụ
        self.create_rectangle(jx - w // 2, jy - h // 2, jx + w // 2, jy + h // 2, fill=body_fill, outline=border_color, width=border_w)
        self.create_oval(jx - w // 2, jy + h // 2 - 4, jx + w // 2, jy + h // 2 + 3, fill="#e2e8f0", outline=border_color, width=border_w)

        # 3. Dải màu loài ở viền cổ lọ
        self.create_rectangle(jx - w // 2 + 2, jy - h // 2 + 2, jx + w // 2 - 2, jy - h // 2 + 6, fill=species_color, outline="")

        # 4. HIỂN THỊ SỐ LƯỢNG MUỖI NẰM CHÍNH GIỮA LỌ (Centered Large Bold Number)
        center_text = f"{count}"
        text_color = "#15803d" if is_active else "#0f172a"
        # Đổ bóng nhẹ phía sau số để chữ nổi bật
        self.create_text(jx + 1, jy + 2, text=center_text, fill="#ffffff", font=("Consolas", 10, "bold"))
        self.create_text(jx, jy + 1, text=center_text, fill=text_color, font=("Consolas", 10, "bold"))

        # 5. Nhãn số Lọ bên trên nắp lọ
        self.create_text(jx, jy - h // 2 - 10, text=f"Lọ{jar_num}", fill="#1e293b", font=("Segoe UI", 7, "bold"))

    def _draw_moving_muoi_3d(self):
        """Mô phỏng phôi muỗi di chuyển qua 4 phân hệ 3D"""
        if self._muoi_phase == "RAY":
            nx = self.ray_x_start + self._muoi_progress * (self.ray_x_end - self.ray_x_start)
            ny = self.ray_y + 10
            angle = 0.0

        elif self._muoi_phase == "BELT":
            nx = self.belt_x_start + self._muoi_progress * (self.belt_x_end - self.belt_x_start)
            ny = self.belt_y - 20
            angle = 0.0

        elif self._muoi_phase == "CHUTE":
            nx = self.chute_x_start + self._muoi_progress * (self.chute_x_end - self.chute_x_start)
            ny = 145 + self._muoi_progress * 65
            angle = 45.0

        else:  # "DISC"
            nx = self.disc_cx - 55
            ny = self.disc_cy
            angle = self._current_angle

        species_color = self.SPECIES_COLORS[self._active_species_id - 1]
        self._draw_mosquito_3d_icon(nx, ny, angle, species_color)

    def _draw_mosquito_3d_icon(self, cx: float, cy: float, angle_deg: float, color: str):
        """Vẽ phôi muỗi 3D có màu nhận diện loài"""
        self.create_oval(cx - 7, cy - 4, cx + 7, cy + 4, fill=color, outline="#000000", width=1)
        self.create_line(cx - 3, cy - 4, cx - 9, cy - 9, fill="#94a3b8", width=1.5)
        self.create_line(cx + 3, cy - 4, cx + 9, cy - 9, fill="#94a3b8", width=1.5)
