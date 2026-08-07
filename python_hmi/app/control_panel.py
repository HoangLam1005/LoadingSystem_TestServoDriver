# =============================================================================
# LoadingSystem - Control Panel (White & Blue Industrial Light Theme)
# =============================================================================
# Panel điều khiển nạp phôi muỗi ngẫu nhiên & Phân loại AI:
#   - Chỉnh Tần số phát xung (Frequency Hz) bằng tay
#   - Cấu hình Thông số Vật lý (Cam AI ➔ Rìa Băng Chuyền ➔ Lọ Hứng)
#   - Thuật toán Tự động Tính toán Tốc độ & Thời gian quay (Tối đa 1s)
#   - Nút Thả 1 con muỗi ngẫu nhiên & Auto Loop Continuous
#   - Điều khiển Servo quay đĩa 12 lọ hứng muỗi
# =============================================================================

import math
import customtkinter as ctk
from typing import Callable, Optional, Tuple


class ControlPanel(ctk.CTkFrame):
    """
    Bảng điều khiển chính nạp muỗi AI & Servo 12 Lọ với Thuật toán Tự Động Tính Tốc Độ.
    """

    def __init__(
        self,
        master,
        on_rotate_right: Optional[Callable[[float, int], None]] = None,
        on_rotate_left: Optional[Callable[[float, int], None]] = None,
        on_return_home: Optional[Callable[[], None]] = None,
        on_feed_nut: Optional[Callable[[], None]] = None,
        on_toggle_auto_loop: Optional[Callable[[bool], None]] = None,
        on_confirm_config: Optional[Callable[[int, float], None]] = None,
        **kwargs,
    ):
        super().__init__(master, **kwargs)
        self._on_rotate_right = on_rotate_right
        self._on_rotate_left = on_rotate_left
        self._on_return_home = on_return_home
        self._on_feed_nut = on_feed_nut
        self._on_toggle_auto_loop = on_toggle_auto_loop
        self._on_confirm_config = on_confirm_config
        self._is_auto_loop_running = False

        self.configure(fg_color="#ffffff", border_color="#cbd5e1", border_width=1, corner_radius=12)
        self._build_ui()

    def _build_ui(self):
        """Xây dựng giao diện Light Industrial Theme"""

        # ---- Tiêu đề ----
        title = ctk.CTkLabel(
            self,
            text="⚙️ CẤU HÌNH THÔNG SỐ VẬT LÝ & THUẬT TOÁN ĐIỀU TỐC",
            font=("Segoe UI", 12, "bold"),
            text_color="#0b3c5d",
        )
        title.pack(pady=(8, 2), padx=10)

        # ---- Box 1: Cấu hình Vật lý Cam AI & Băng chuyền ----
        phys_frame = ctk.CTkFrame(self, fg_color="#f8fafc", border_color="#e2e8f0", border_width=1, corner_radius=8)
        phys_frame.pack(pady=3, padx=8, fill="x")

        # Row 1: L_cam (Camera -> Rìa) & H_drop (Rìa -> Lọ)
        row1 = ctk.CTkFrame(phys_frame, fg_color="transparent")
        row1.pack(fill="x", padx=8, pady=3)
        row1.grid_columnconfigure((0, 1), weight=1)

        l_box = ctk.CTkFrame(row1, fg_color="transparent")
        l_box.grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(l_box, text="L(Cam-Rìa) m:", font=("Segoe UI", 9, "bold"), text_color="#334155").pack(side="left")
        self.entry_l_cam = ctk.CTkEntry(l_box, width=52, font=("Consolas", 10), height=22)
        self.entry_l_cam.pack(side="left", padx=4)
        self.entry_l_cam.insert(0, "0.5")

        h_box = ctk.CTkFrame(row1, fg_color="transparent")
        h_box.grid(row=0, column=1, sticky="e")
        ctk.CTkLabel(h_box, text="H(Rìa-Lọ) m:", font=("Segoe UI", 9, "bold"), text_color="#334155").pack(side="left")
        self.entry_h_drop = ctk.CTkEntry(h_box, width=52, font=("Consolas", 10), height=22)
        self.entry_h_drop.pack(side="left", padx=4)
        self.entry_h_drop.insert(0, "0.3")

        # Row 2: Tốc độ Băng chuyền & Thời gian rơi t_drop readout
        row2 = ctk.CTkFrame(phys_frame, fg_color="transparent")
        row2.pack(fill="x", padx=8, pady=(0, 4))

        ctk.CTkLabel(row2, text="V(Băng chuyền) m/s:", font=("Segoe UI", 9, "bold"), text_color="#334155").pack(side="left")
        self.entry_v_belt = ctk.CTkEntry(row2, width=52, font=("Consolas", 10), height=22)
        self.entry_v_belt.pack(side="left", padx=4)
        self.entry_v_belt.insert(0, "0.8")

        self.lbl_t_drop = ctk.CTkLabel(row2, text="⏱ t_drop: 0.87s", font=("Consolas", 9, "bold"), text_color="#0d9488")
        self.lbl_t_drop.pack(side="right", padx=4)

        # Bind event update t_drop
        self.entry_l_cam.bind("<KeyRelease>", self._recalc_drop_time)
        self.entry_h_drop.bind("<KeyRelease>", self._recalc_drop_time)
        self.entry_v_belt.bind("<KeyRelease>", self._recalc_drop_time)

        # ---- Box 2: Tần số Servo Manual (Hz) & Xác Nhận ----
        manual_frame = ctk.CTkFrame(self, fg_color="#f8fafc", border_color="#e2e8f0", border_width=1, corner_radius=8)
        manual_frame.pack(pady=3, padx=8, fill="x")

        freq_row = ctk.CTkFrame(manual_frame, fg_color="transparent")
        freq_row.pack(fill="x", padx=8, pady=4)

        ctk.CTkLabel(freq_row, text="⚡ Tần số Servo Thủ Công (Hz):", font=("Segoe UI", 9, "bold"), text_color="#0b3c5d").pack(side="left")
        self.entry_freq = ctk.CTkEntry(freq_row, width=90, font=("Consolas", 10, "bold"), height=22)
        self.entry_freq.pack(side="left", padx=6)
        self.entry_freq.insert(0, "16384")

        # Nút XÁC NHẬN THÔNG SỐ
        self.btn_confirm_config = ctk.CTkButton(
            manual_frame,
            text="✔ XÁC NHẬN & ÁP DỤNG CẤU HÌNH THÔNG SỐ",
            font=("Segoe UI", 9, "bold"),
            fg_color="#0d9488",
            hover_color="#0f766e",
            text_color="#ffffff",
            height=24,
            corner_radius=5,
            command=self._handle_confirm_config,
        )
        self.btn_confirm_config.pack(fill="x", padx=8, pady=(2, 6))

        # ---- Nút Nạp Muỗi Đơn & Auto Loop ----
        act_frame = ctk.CTkFrame(self, fg_color="transparent")
        act_frame.pack(pady=2, padx=8, fill="x")
        act_frame.grid_columnconfigure((0, 1), weight=1)

        self.btn_feed_one = ctk.CTkButton(
            act_frame,
            text="🎲 THẢ 1 MUỖI NGẪU NHIÊN",
            font=("Segoe UI", 10, "bold"),
            fg_color="#00838f",
            hover_color="#0097a7",
            text_color="#ffffff",
            height=30,
            corner_radius=6,
            command=self._handle_feed_one,
        )
        self.btn_feed_one.grid(row=0, column=0, padx=2, sticky="ew")

        self.btn_auto_loop = ctk.CTkButton(
            act_frame,
            text="▶ TỰ ĐỘNG THẢ CONTINUOUS",
            font=("Segoe UI", 10, "bold"),
            fg_color="#2e7d32",
            hover_color="#388e3c",
            text_color="#ffffff",
            height=30,
            corner_radius=6,
            command=self._handle_toggle_auto_loop,
        )
        self.btn_auto_loop.grid(row=0, column=1, padx=2, sticky="ew")

        # ---- Separator ----
        sep = ctk.CTkFrame(self, fg_color="#e2e8f0", height=1)
        sep.pack(fill="x", padx=10, pady=4)

        # ---- Frame Điều Khiển Servo Trực Tiếp ----
        angle_frame = ctk.CTkFrame(self, fg_color="transparent")
        angle_frame.pack(pady=2, padx=8, fill="x")

        ctk.CTkLabel(angle_frame, text="Góc Servo (°):", font=("Segoe UI", 9, "bold"), text_color="#334155").pack(side="left", padx=(4, 4))

        self.angle_entry = ctk.CTkEntry(
            angle_frame,
            width=70,
            font=("Consolas", 10, "bold"),
            fg_color="#ffffff",
            border_color="#cbd5e1",
            text_color="#0b3c5d",
            height=24,
        )
        self.angle_entry.pack(side="left", expand=True, fill="x", padx=(0, 4))
        self.angle_entry.insert(0, "30.0")

        # Nút Quay Trái, Về Gốc, Quay Phải
        btn_frame = ctk.CTkFrame(self, fg_color="transparent")
        btn_frame.pack(pady=2, padx=8, fill="x")
        btn_frame.grid_columnconfigure((0, 1, 2), weight=1)

        self.btn_left = ctk.CTkButton(
            btn_frame,
            text="◄ TRÁI",
            font=("Segoe UI", 9, "bold"),
            fg_color="#1d6491",
            hover_color="#2b7cb0",
            text_color="#ffffff",
            height=28,
            corner_radius=5,
            command=self._handle_rotate_left,
        )
        self.btn_left.grid(row=0, column=0, padx=2, sticky="ew")

        self.btn_home = ctk.CTkButton(
            btn_frame,
            text="⌂ 0°",
            font=("Segoe UI", 9, "bold"),
            fg_color="#486581",
            hover_color="#627d98",
            text_color="#ffffff",
            height=28,
            corner_radius=5,
            command=self._handle_return_home,
        )
        self.btn_home.grid(row=0, column=1, padx=2, sticky="ew")

        self.btn_right = ctk.CTkButton(
            btn_frame,
            text="PHẢI ►",
            font=("Segoe UI", 9, "bold"),
            fg_color="#1d6491",
            hover_color="#2b7cb0",
            text_color="#ffffff",
            height=28,
            corner_radius=5,
            command=self._handle_rotate_right,
        )
        self.btn_right.grid(row=0, column=2, padx=2, sticky="ew")

    # =========================================================================
    # Physical Calculations & Automatic Speed Adjustment Algorithm
    # =========================================================================

    def get_physical_drop_time(self) -> float:
        """
        Tính thời gian muỗi rơi từ Cam AI đến lọ hứng:
        t_drop = (L_cam / v_belt) + sqrt(2 * H_drop / 9.81)
        """
        try:
            l_cam = float(self.entry_l_cam.get())
            h_drop = float(self.entry_h_drop.get())
            v_belt = float(self.entry_v_belt.get())
            if v_belt <= 0:
                v_belt = 0.8

            t_belt = l_cam / v_belt
            t_fall = math.sqrt((2.0 * h_drop) / 9.81)
            return t_belt + t_fall
        except ValueError:
            return 0.87

    def _recalc_drop_time(self, event=None):
        t_drop = self.get_physical_drop_time()
        self.lbl_t_drop.configure(text=f"⏱ t_drop: {t_drop:.2f}s")

    def calculate_optimal_frequency(self, delta_degrees: float) -> Tuple[int, float]:
        """
        THUẬT TOÁN TÍNH TỐC ĐỘ TỰ ĐỘNG CHÍNH XÁC VỚI THỰC TẾ:
        - Mục tiêu: Đĩa quay xong đúng góc delta_degrees trong thời gian max 1.0s (hoặc t_drop).
        - Nếu góc xa (vd 180°): Tốc độ tự động nâng cao (tối đa 1s là về đúng vị trí).
        - Nếu góc gần (vd 30°): Tốc độ tự động giảm chậm sao cho vừa trúng 1s là về đúng vị trí.

        Returns: (frequency_hz, duration_seconds)
        """
        delta_deg = abs(delta_degrees)
        if delta_deg < 0.1:
            return self.get_manual_frequency(), 0.05

        pulses = round(delta_deg * 131072.0 / 360.0)

        # Thời gian mục tiêu: Tối đa 1.0 giây (hoặc t_drop nếu t_drop < 1.0s)
        t_drop = self.get_physical_drop_time()
        t_target = min(1.0, t_drop)
        if t_target <= 0.1:
            t_target = 1.0

        # Tính tần số Hz yêu cầu: f = pulses / t_target
        calc_freq = int(round(pulses / t_target))

        # Giới hạn tần số Servo an toàn (từ 1000 Hz đến 20000 Hz)
        min_freq = 1000
        max_freq = 20000
        final_freq = max(min_freq, min(max_freq, calc_freq))

        # Tính lại thời gian quay thực tế
        actual_duration = pulses / final_freq
        return final_freq, actual_duration

    def get_manual_frequency(self) -> int:
        try:
            val = int(self.entry_freq.get())
            return max(500, min(30000, val))
        except ValueError:
            return 8192

    def _handle_confirm_config(self):
        self._recalc_drop_time()
        if self._on_confirm_config:
            self._on_confirm_config(
                self.get_manual_frequency(),
                self.get_physical_drop_time()
            )

    def _handle_feed_one(self):
        if self._on_feed_nut:
            self._on_feed_nut()

    def _handle_toggle_auto_loop(self):
        self._is_auto_loop_running = not self._is_auto_loop_running
        if self._is_auto_loop_running:
            self.btn_auto_loop.configure(text="⏹ DỪNG TỰ ĐỘNG THẢ", fg_color="#d32f2f", hover_color="#e53935")
        else:
            self.btn_auto_loop.configure(text="▶ TỰ ĐỘNG THẢ CONTINUOUS", fg_color="#2e7d32", hover_color="#388e3c")

        if self._on_toggle_auto_loop:
            self._on_toggle_auto_loop(self._is_auto_loop_running)

    def set_auto_loop_state(self, running: bool):
        self._is_auto_loop_running = running
        if running:
            self.btn_auto_loop.configure(text="⏹ DỪNG TỰ ĐỘNG THẢ", fg_color="#d32f2f", hover_color="#e53935")
        else:
            self.btn_auto_loop.configure(text="▶ TỰ ĐỘNG THẢ CONTINUOUS", fg_color="#2e7d32", hover_color="#388e3c")

    def _get_angle(self) -> float:
        try:
            val = float(self.angle_entry.get())
            return abs(val)
        except ValueError:
            return 30.0

    def _handle_rotate_right(self):
        if self._on_rotate_right:
            self._on_rotate_right(self._get_angle(), self.get_manual_frequency())

    def _handle_rotate_left(self):
        if self._on_rotate_left:
            self._on_rotate_left(self._get_angle(), self.get_manual_frequency())

    def _handle_return_home(self):
        if self._on_return_home:
            self._on_return_home()

    def set_enabled(self, enabled: bool):
        state = "normal" if enabled else "disabled"
        self.btn_left.configure(state=state)
        self.btn_right.configure(state=state)
        self.btn_home.configure(state=state)
        self.btn_feed_one.configure(state=state)
        self.btn_auto_loop.configure(state=state)
