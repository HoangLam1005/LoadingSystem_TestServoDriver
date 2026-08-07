# =============================================================================
# LoadingSystem - Main HMI Application (White & Blue Industrial Light Theme)
# =============================================================================
# Giao diện HMI chính mô phỏng 3D Không gian 4 phân hệ theo máy thực tế:
#   (1) Ray tách muỗi 3D (Khay nghiêng + Camera AI Vision Sensor)
#   (2) Băng chuyền 3D (Mặt nghiêng xanh + 2 bánh răng hoa sao 3D)
#   (3) Cần gạt 3D cấp 1-3
#   (4) Đĩa xoay 3D (ĐĨA XOAY 3D MANG 12 LỌ 12 LOÀI MUỖI RIÊNG BIỆT).
#
# Tích hợp Thuật toán Thả 12 loài muỗi ngẫu nhiên, Tự động tính toán tốc độ quay (Max 1s),
# Đồng bộ tốc độ xoay HMI với Motor thực tế, và Bảng LOG hiển thị phóng to chuyên nghiệp.
# =============================================================================

import datetime
import random
import threading
import customtkinter as ctk
from typing import Optional, Dict

from app.zmq_client import IpcClient
from app.system_visualizer import SystemVisualizer
from app.control_panel import ControlPanel
from app.override_panel import OverridePanel


class LoadingSystemHMI(ctk.CTk):
    """
    Cửa sổ chính HMI LoadingSystem (3D Light Industrial Theme: White & Blue).
    """

    APP_TITLE = "LoadingSystem HMI - Mosquito Machine 3D (Ray ➔ Conveyor ➔ Chute ➔ 12-Jar Dynamic Carousel)"
    APP_WIDTH = 980
    APP_HEIGHT = 820

    def __init__(
        self,
        backend_host: str = "127.0.0.1",
        backend_port: int = 5555,
    ):
        super().__init__()

        # Cấu hình cửa sổ
        self.title(self.APP_TITLE)
        self.geometry(f"{self.APP_WIDTH}x{self.APP_HEIGHT}")
        self.minsize(920, 760)
        self.resizable(True, True)

        # Theme sáng công nghiệp
        ctk.set_appearance_mode("light")
        ctk.set_default_color_theme("blue")
        self.configure(fg_color="#f0f4f8")

        # IPC client
        self._client = IpcClient(host=backend_host, port=backend_port)
        self._current_angle = 0.0

        # Auto Feed Loop State
        self._auto_loop_running = False
        self._auto_loop_timer = None

        # Xây dựng giao diện
        self._build_ui()

        # Kết nối callback bảng đếm muỗi & gán nhãn
        self._sys_viz.set_count_change_callback(self._override_panel.update_jar_counts)

        # Log khởi tạo
        self.log_msg("SYSTEM", "Hệ thống HMI Đóng Lọ Muỗi 3D đã khởi tạo thành công.")
        self.log_msg("CONFIG", "Thuật toán điều tốc tự động & Phân loại 12 loài muỗi đã kích hoạt.")

        # Tự động kết nối khi khởi động
        self.after(500, self._auto_connect)

        # Handle đóng cửa sổ
        self.protocol("WM_DELETE_WINDOW", self._on_close)

    def _build_ui(self):
        """Xây dựng toàn bộ giao diện HMI"""

        # ---- Header ----
        header = ctk.CTkFrame(self, fg_color="#0b3c5d", height=48, corner_radius=0)
        header.pack(fill="x")
        header.pack_propagate(False)

        ctk.CTkLabel(
            header,
            text="⚡ HỆ THỐNG ĐÓNG LỌ MUỖI 3D - THUẬT TOÁN TÍNH TỐC ĐỘ TỰ ĐỘNG (MAX 1S)",
            font=("Segoe UI", 14, "bold"),
            text_color="#ffffff",
        ).pack(side="left", padx=16, pady=8)

        # Connection indicator
        self._conn_frame = ctk.CTkFrame(header, fg_color="transparent")
        self._conn_frame.pack(side="right", padx=16)

        self._conn_dot = ctk.CTkLabel(
            self._conn_frame,
            text="●",
            font=("Segoe UI", 14),
            text_color="#ef4444",
        )
        self._conn_dot.pack(side="left", padx=(0, 4))

        self._conn_label = ctk.CTkLabel(
            self._conn_frame,
            text="Ngắt kết nối",
            font=("Segoe UI", 10, "bold"),
            text_color="#cbd5e1",
        )
        self._conn_label.pack(side="left")

        # ---- Main content frame ----
        content = ctk.CTkFrame(self, fg_color="transparent")
        content.pack(fill="both", expand=True, padx=8, pady=4)

        # ---- System Visualizer 3D (Top Section) ----
        viz_frame = ctk.CTkFrame(content, fg_color="#ffffff", border_color="#cbd5e1", border_width=1, corner_radius=10)
        viz_frame.pack(fill="x", pady=(0, 4))

        self._sys_viz = SystemVisualizer(viz_frame, width=950, height=330)
        self._sys_viz.pack(pady=4, padx=4)

        # ---- Middle Section: Control Panels (Side-by-Side) ----
        mid_frame = ctk.CTkFrame(content, fg_color="transparent")
        mid_frame.pack(fill="x", pady=(0, 4))
        mid_frame.grid_columnconfigure((0, 1), weight=1)

        # Control Panel (Left)
        self._control_panel = ControlPanel(
            mid_frame,
            on_rotate_right=self._cmd_rotate_right,
            on_rotate_left=self._cmd_rotate_left,
            on_return_home=self._cmd_return_home,
            on_feed_nut=self._cmd_feed_nut,
            on_toggle_auto_loop=self._cmd_toggle_auto_loop,
            on_confirm_config=self._cmd_confirm_config,
        )
        self._control_panel.grid(row=0, column=0, padx=(0, 2), sticky="nsew")

        # Override Panel (Right)
        self._override_panel = OverridePanel(
            mid_frame,
            on_override=self._cmd_override,
        )
        self._override_panel.grid(row=0, column=1, padx=(2, 0), sticky="nsew")

        # ---- Bottom Section: HIỂN THỊ LOG PHÓNG TO (Expanded Log Console Window) ----
        log_frame = ctk.CTkFrame(content, fg_color="#ffffff", border_color="#cbd5e1", border_width=1, corner_radius=8)
        log_frame.pack(fill="both", expand=True)

        log_title_box = ctk.CTkFrame(log_frame, fg_color="#f1f5f9", height=24, corner_radius=0)
        log_title_box.pack(fill="x")
        log_title_box.pack_propagate(False)

        ctk.CTkLabel(
            log_title_box,
            text="📋 NHẬT KÝ VẬN HÀNH THỜI GIAN THỰC (REAL-TIME LOG CONSOLE)",
            font=("Segoe UI", 9, "bold"),
            text_color="#0b3c5d",
        ).pack(side="left", padx=8, pady=2)

        self.btn_clear_log = ctk.CTkButton(
            log_title_box,
            text="Xóa Log",
            font=("Segoe UI", 9),
            height=18,
            width=60,
            fg_color="#94a3b8",
            hover_color="#64748b",
            command=self._clear_log,
        )
        self.btn_clear_log.pack(side="right", padx=6, pady=2)

        # Log Text Box PHÓNG TO với font chữ Console sắc nét
        self.log_textbox = ctk.CTkTextbox(
            log_frame,
            font=("Consolas", 10),
            fg_color="#0f172a",
            text_color="#f8fafc",
            corner_radius=4,
            wrap="word",
        )
        self.log_textbox.pack(fill="both", expand=True, padx=4, pady=4)

        # Configure Text Tags cho màu sắc Log
        self.log_textbox.tag_config("TIME", foreground="#94a3b8")
        self.log_textbox.tag_config("INFO", foreground="#38bdf8")
        self.log_textbox.tag_config("DISPENSE", foreground="#4ade80")
        self.log_textbox.tag_config("ROTATE", foreground="#facc15")
        self.log_textbox.tag_config("ERROR", foreground="#f87171")
        self.log_textbox.tag_config("CONFIG", foreground="#c084fc")

        # ---- Status Bar ----
        self._status_bar = ctk.CTkFrame(self, fg_color="#0b3c5d", height=26, corner_radius=0)
        self._status_bar.pack(fill="x", side="bottom")
        self._status_bar.pack_propagate(False)

        self._status_label = ctk.CTkLabel(
            self._status_bar,
            text="Sẵn sàng vận hành hệ thống 3D.",
            font=("Segoe UI", 9),
            text_color="#e2e8f0",
        )
        self._status_label.pack(side="left", padx=12, pady=2)

        self._connect_btn = ctk.CTkButton(
            self._status_bar,
            text="Kết nối",
            font=("Segoe UI", 9, "bold"),
            fg_color="#1d6491",
            hover_color="#2b7cb0",
            text_color="#ffffff",
            height=20,
            width=80,
            corner_radius=4,
            command=self._toggle_connection,
        )
        self._connect_btn.pack(side="right", padx=10, pady=2)

        # Disable controls initially
        self._control_panel.set_enabled(False)
        self._override_panel.set_enabled(False)

    # =========================================================================
    # Logging System
    # =========================================================================

    def log_msg(self, tag: str, message: str):
        """Ghi log có dấu thời gian và phân màu trực quan"""
        now = datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3]
        line = f"[{now}] [{tag:<8}] {message}\n"

        self.log_textbox.configure(state="normal")
        self.log_textbox.insert("end", f"[{now}] ", "TIME")
        self.log_textbox.insert("end", f"[{tag:<8}] ", tag if tag in ["INFO", "DISPENSE", "ROTATE", "ERROR", "CONFIG"] else "INFO")
        self.log_textbox.insert("end", f"{message}\n")
        self.log_textbox.see("end")
        self.log_textbox.configure(state="disabled")

    def _clear_log(self):
        self.log_textbox.configure(state="normal")
        self.log_textbox.delete("1.0", "end")
        self.log_textbox.configure(state="disabled")

    # =========================================================================
    # Connection Management
    # =========================================================================

    def _auto_connect(self):
        self._set_status("Đang kết nối tới Rust Backend Daemon (127.0.0.1:5555)...")
        threading.Thread(target=self._connect_async, daemon=True).start()

    def _connect_async(self):
        success = self._client.connect()
        self.after(0, lambda: self._on_connect_result(success))

    def _on_connect_result(self, success: bool):
        if success:
            self._conn_dot.configure(text_color="#22c55e")
            self._conn_label.configure(text="Đã kết nối")
            self._connect_btn.configure(text="Ngắt kết nối")
            self._control_panel.set_enabled(True)
            self._override_panel.set_enabled(True)
            self._set_status("Đã kết nối thành công tới Rust Backend Server (127.0.0.1:5555)")
            self.log_msg("INFO", "Kết nối Socket TCP IPC thành công tới Rust Backend Daemon.")
            self._refresh_status()
        else:
            self._conn_dot.configure(text_color="#ef4444")
            self._conn_label.configure(text="Ngắt kết nối")
            self._connect_btn.configure(text="Kết nối")
            self._control_panel.set_enabled(False)
            self._override_panel.set_enabled(False)
            self._stop_auto_loop()
            self._set_status("Mất kết nối - Hãy kiểm tra daemon Rust backend đang chạy!")
            self.log_msg("ERROR", "Không thể kết nối IPC Server (127.0.0.1:5555). Hãy khởi chạy Rust Backend.")

    def _toggle_connection(self):
        if self._client.is_connected:
            self._client.disconnect()
            self._on_connect_result(False)
        else:
            self._auto_connect()

    # =========================================================================
    # MOSQUITO DISPENSING & AUTOMATIC SPEED CALCULATION ALGORITHM
    # =========================================================================

    def _cmd_feed_nut(self):
        """
        THẢ RANDOM 12 LOÀI MUỖI VÀO 12 LỌ RIÊNG BIỆT:
        - Mỗi lọ chỉ chứa duy nhất 1 loài muỗi (1-to-1 mapping).
        - Đĩa tự động tính toán tần số f (Hz) và quay đúng vị trí trong MAX 1S (hoặc t_drop).
        """
        # 1. Chọn ngẫu nhiên 1 trong 12 loài muỗi
        incoming_species = random.randint(1, 12)
        sp_name = SystemVisualizer.SPECIES_NAMES[incoming_species - 1]

        # 2. Lọ đích cố định đại diện cho loài muỗi này (Lọ 1..12)
        target_jar = self._sys_viz.species_assigned_jar.get(incoming_species, 1)

        # 3. Tính toán vị trí góc trước khi xoay và điểm rơi mục tiêu
        start_jar = self._sys_viz.get_active_jar_index()
        start_angle = self._current_angle

        # Tọa độ chuẩn của Lọ mục tiêu (Lọ 1 = 0°, Lọ 2 = 30°, ..., Lọ 12 = 330°)
        target_angle = (target_jar - 1) * 30.0

        # Tính góc quay chênh lệch delta_deg (-180° đến +180°)
        delta_deg = target_angle - (start_angle % 360.0)
        if delta_deg > 180.0:
            delta_deg -= 360.0
        elif delta_deg < -180.0:
            delta_deg += 360.0

        # 4. THUẬT TOÁN TÍNH TỐC ĐỘ TỰ ĐỘNG (Auto-Speed Algorithm):
        # Tính toán tần số Hz và thời gian quay sao cho <= 1.0s (hoặc t_drop)
        opt_freq, rot_duration = self._control_panel.calculate_optimal_frequency(delta_deg)
        t_drop = self._control_panel.get_physical_drop_time()

        # 5. GHI LOG HIỂN THỊ CHI TIẾT
        log_detail = (
            f"🎲 Phễu Nạp: [{sp_name}] ➔ Mục tiêu: Lọ #{target_jar} ({target_angle:.0f}°)\n"
            f"   ├─ Vị trí trước: Lọ #{start_jar} ({start_angle:.1f}°) ➔ Điểm rơi: Lọ #{target_jar} ({target_angle:.1f}°)\n"
            f"   ├─ Góc cần quay: Δθ = {delta_deg:+.1f}° | Tần số Servo: f = {opt_freq} Hz\n"
            f"   └─ Thời gian quay Motor: {rot_duration:.2f}s | Thời gian muỗi rơi t_drop: {t_drop:.2f}s"
        )
        self.log_msg("DISPENSE", log_detail)
        self._set_status(f"Thả Muỗi [{sp_name}]: Xoay {delta_deg:+.1f}° về Lọ #{target_jar} ({opt_freq} Hz, {rot_duration:.2f}s)...")

        # 6. Gửi lệnh quay Servo kèm tần số tối ưu xuống Rust/PLC
        if abs(delta_deg) > 0.1:
            if delta_deg > 0:
                self._send_command_async("ROTATE_RIGHT", abs(delta_deg), frequency=opt_freq)
            else:
                self._send_command_async("ROTATE_LEFT", abs(delta_deg), frequency=opt_freq)
            # Cập nhật đĩa 3D HMI xoay mượt đồng bộ đúng thời gian rot_duration
            self._sys_viz.update_angle(target_angle, duration_sec=rot_duration)

        # 7. Mô phỏng phôi muỗi 3D di chuyển
        self._sys_viz.start_muoi_flow_simulation(
            species_id=incoming_species,
            on_complete=lambda: self._set_status(f"✓ Muỗi [{sp_name}] đã rơi trúng Lọ #{target_jar}!")
        )

    def _cmd_confirm_config(self, freq: int, t_drop: float):
        msg = f"✔ ĐÃ XÁC NHẬN CẤU HÌNH: Tần số Servo = {freq} Hz | t_drop = {t_drop:.2f}s"
        self.log_msg("CONFIG", msg)
        self._set_status(f"✔ Đã áp dụng cấu hình mới: {freq} Hz, t_drop={t_drop:.2f}s")

    # =========================================================================
    # Continuous Auto Feed Loop
    # =========================================================================

    def _cmd_toggle_auto_loop(self, running: bool):
        if running:
            self._auto_loop_running = True
            self.log_msg("CONFIG", "▶ ĐÃ BẬT VÒNG LẶP TỰ ĐỘNG THẢ MUỖI LIÊN TỤC.")
            self._set_status("▶ ĐANG CHẠY VÒNG LẶP TỰ ĐỘNG THẢ MUỖI LIÊN TỤC...")
            self._auto_feed_step()
        else:
            self._stop_auto_loop()

    def _auto_feed_step(self):
        if not self._auto_loop_running or not self._client.is_connected:
            return

        self._cmd_feed_nut()

        # Hẹn giờ thả con tiếp theo sau 2.5 giây
        self._auto_loop_timer = self.after(2500, self._auto_feed_step)

    def _stop_auto_loop(self):
        self._auto_loop_running = False
        if self._auto_loop_timer:
            self.after_cancel(self._auto_loop_timer)
            self._auto_loop_timer = None
        self._control_panel.set_auto_loop_state(False)
        self.log_msg("CONFIG", "⏹ ĐÃ DỪNG VÒNG LẶP TỰ ĐỘNG THẢ MUỖI.")
        self._set_status("⏹ Đã dừng vòng lặp tự động thả muỗi.")

    # =========================================================================
    # Servo Command Handlers
    # =========================================================================

    def _send_command_async(self, command: str, value: Optional[float] = None, frequency: Optional[int] = None):
        def _run():
            try:
                response = self._client.send_command(command, value, frequency)
                self.after(0, lambda: self._on_command_response(response))
            except ConnectionError as e:
                err_msg = str(e)
                self.after(0, lambda: self._on_command_error(err_msg))

        threading.Thread(target=_run, daemon=True).start()

    def _on_command_response(self, response: dict):
        success = response.get("success", False)
        message = response.get("message", "Unknown response")
        if not success:
            self.log_msg("ERROR", f"Lỗi phản hồi PLC: {message}")
            self._set_status(f"✗ Lỗi PLC: {message}")
        self._refresh_status()

    def _on_command_error(self, error: str):
        self.log_msg("ERROR", f"Lỗi Socket IPC: {error}")
        self._set_status(f"✗ Lỗi IPC: {error}")
        self._on_connect_result(False)

    def _cmd_rotate_right(self, degrees: float, freq: int = 16384):
        self.log_msg("ROTATE", f"Lệnh quay PHẢI {degrees:.1f}° (Tần số: {freq} Hz)")
        self._set_status(f"Đang quay phải đĩa xoay {degrees:.1f}° ({freq} Hz)...")
        self._send_command_async("ROTATE_RIGHT", degrees, frequency=freq)
        self._current_angle += degrees  # Cập nhật góc ngay lập tức (optimistic update)
        self._sys_viz.update_angle(self._current_angle, duration_sec=0.5)

    def _cmd_rotate_left(self, degrees: float, freq: int = 16384):
        self.log_msg("ROTATE", f"Lệnh quay TRÁI {degrees:.1f}° (Tần số: {freq} Hz)")
        self._set_status(f"Đang quay trái đĩa xoay {degrees:.1f}° ({freq} Hz)...")
        self._send_command_async("ROTATE_LEFT", degrees, frequency=freq)
        self._current_angle -= degrees  # Cập nhật góc ngay lập tức (optimistic update)
        self._sys_viz.update_angle(self._current_angle, duration_sec=0.5)

    def _cmd_return_home(self):
        self.log_msg("ROTATE", "Lệnh quay VỀ GỐC 0°")
        self._set_status("Đang quay đĩa xoay về vị trí gốc 0°...")
        self._send_command_async("RETURN_HOME")
        self._current_angle = 0.0  # Cập nhật góc ngay lập tức (optimistic update)
        self._sys_viz.update_angle(0.0, duration_sec=0.6)

    def _cmd_override(self, degrees: float):
        direction = "phải" if degrees > 0 else "trái"
        self.log_msg("CONFIG", f"Hiệu chỉnh thủ công Fine Tune: {degrees:+.1f}° sang {direction}")
        self._set_status(f"Hiệu chỉnh thủ công Level 4/5: {degrees:+.1f}° sang {direction}...")
        self._send_command_async("OVERRIDE_ADJUST", degrees)

    # =========================================================================
    # Status Updates
    # =========================================================================

    def _refresh_status(self):
        def _run():
            try:
                response = self._client.get_status()
                self.after(0, lambda: self._update_status_display(response))
            except ConnectionError:
                pass

        threading.Thread(target=_run, daemon=True).start()

    def _update_status_display(self, response: dict):
        if not response.get("success"):
            return

        data = response.get("data", {})
        if not data:
            return

        angle = data.get("current_angle", 0.0)
        self._current_angle = angle

        self._override_panel.update_calibration_info(
            offset_pulses=data.get("offset_pulses", 0),
            learning_coefficient=data.get("learning_coefficient", 0.1),
            calibration_count=data.get("calibration_count", 0),
        )

    def _set_status(self, message: str):
        self._status_label.configure(text=message)

    def _on_close(self):
        self._stop_auto_loop()
        if self._client.is_connected:
            self._client.disconnect()
        self.destroy()
