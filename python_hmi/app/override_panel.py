# =============================================================================
# LoadingSystem - Override Panel (White & Blue Industrial Light Theme)
# =============================================================================
# Panel Level 4 & 5: Can thiệp thủ công + Bảng đếm muỗi tích lũy 12 lọ (Gắn nhãn động).
# =============================================================================

import customtkinter as ctk
from typing import Callable, Optional, Dict


class OverridePanel(ctk.CTkFrame):
    """
    Panel Level 4 & 5: Can thiệp thủ công và theo dõi 12 Lọ Muỗi tích lũy động.
    """

    def __init__(
        self,
        master,
        on_override: Optional[Callable[[float], None]] = None,
        **kwargs,
    ):
        super().__init__(master, **kwargs)
        self._on_override = on_override
        self.configure(fg_color="#ffffff", border_color="#cbd5e1", border_width=1, corner_radius=12)
        self._build_ui()

    def _build_ui(self):
        """Xây dựng giao diện Light Industrial Theme"""

        # ---- Tiêu đề ----
        title = ctk.CTkLabel(
            self,
            text="📊 BẢNG THEO DÕI 12 LỌ & TỰ HỌC EMA",
            font=("Segoe UI", 13, "bold"),
            text_color="#0b3c5d",
        )
        title.pack(pady=(10, 2), padx=10)

        # ---- Frame nút Fine Tune Level 4 ----
        tune_frame = ctk.CTkFrame(self, fg_color="transparent")
        tune_frame.pack(pady=2, padx=10, fill="x")
        tune_frame.grid_columnconfigure((0, 1), weight=1)

        self.btn_tune_left = ctk.CTkButton(
            tune_frame,
            text="-0.5° HIỆU CHỈNH TRÁI",
            font=("Segoe UI", 10, "bold"),
            fg_color="#d32f2f",
            hover_color="#e53935",
            text_color="#ffffff",
            height=32,
            corner_radius=6,
            command=lambda: self._handle_override(-0.5),
        )
        self.btn_tune_left.grid(row=0, column=0, padx=2, sticky="ew")

        self.btn_tune_right = ctk.CTkButton(
            tune_frame,
            text="+0.5° HIỆU CHỈNH PHẢI",
            font=("Segoe UI", 10, "bold"),
            fg_color="#2e7d32",
            hover_color="#388e3c",
            text_color="#ffffff",
            height=32,
            corner_radius=6,
            command=lambda: self._handle_override(0.5),
        )
        self.btn_tune_right.grid(row=0, column=1, padx=2, sticky="ew")

        # ---- Separator ----
        sep = ctk.CTkFrame(self, fg_color="#e2e8f0", height=1)
        sep.pack(fill="x", padx=10, pady=4)

        # ---- Calibration Info (Offset & EMA) ----
        cal_frame = ctk.CTkFrame(self, fg_color="#f8fafc", border_color="#e2e8f0", border_width=1, corner_radius=6)
        cal_frame.pack(pady=2, padx=10, fill="x")
        cal_frame.grid_columnconfigure((0, 1, 2), weight=1)

        # Offset
        self.offset_label = ctk.CTkLabel(cal_frame, text="Offset: 0 p", font=("Consolas", 10, "bold"), text_color="#b45309")
        self.offset_label.grid(row=0, column=0, padx=4, pady=4)

        # EMA Rate
        self.coeff_label = ctk.CTkLabel(cal_frame, text="Rate: 0.100", font=("Consolas", 10, "bold"), text_color="#1976d2")
        self.coeff_label.grid(row=0, column=1, padx=4, pady=4)

        # Adjust count
        self.count_label = ctk.CTkLabel(cal_frame, text="Tự học: 0 lần", font=("Consolas", 10, "bold"), text_color="#2e7d32")
        self.count_label.grid(row=0, column=2, padx=4, pady=4)

        # ---- 12 Jars Summary Table (Grid 4x3) ----
        grid_title = ctk.CTkLabel(self, text="Thống kê Muỗi Tích Lũy 12 Lọ (Gắn Nhãn Tự Động):", font=("Segoe UI", 10, "bold"), text_color="#475569")
        grid_title.pack(anchor="w", padx=10, pady=(4, 2))

        self.table_frame = ctk.CTkFrame(self, fg_color="#f8fafc", border_color="#e2e8f0", border_width=1, corner_radius=6)
        self.table_frame.pack(pady=(0, 8), padx=10, fill="both", expand=True)
        self.table_frame.grid_columnconfigure((0, 1, 2, 3), weight=1)

        self.jar_labels: Dict[int, ctk.CTkLabel] = {}
        for idx in range(12):
            jar_num = idx + 1
            row = idx // 4
            col = idx % 4

            lbl = ctk.CTkLabel(
                self.table_frame,
                text=f"Lọ #{jar_num}: (Trống)",
                font=("Consolas", 8, "bold"),
                text_color="#64748b",
                fg_color="#ffffff",
                corner_radius=4,
                height=22,
            )
            lbl.grid(row=row, column=col, padx=2, pady=2, sticky="ew")
            self.jar_labels[jar_num] = lbl

    def _handle_override(self, degrees: float):
        if self._on_override:
            self._on_override(degrees)

    def update_calibration_info(
        self, offset_pulses: int, learning_coefficient: float, calibration_count: int
    ):
        self.offset_label.configure(text=f"Offset: {offset_pulses}p")
        self.coeff_label.configure(text=f"Rate: {learning_coefficient:.3f}")
        self.count_label.configure(text=f"Tự học: {calibration_count}l")

    def update_jar_counts(self, counts: Dict[int, int], assigned_species: Dict[int, Optional[int]]):
        """Cập nhật bảng đếm số muỗi & nhãn tên loài muỗi cho 12 lọ"""
        from app.system_visualizer import SystemVisualizer
        for jar_num, count in counts.items():
            if jar_num in self.jar_labels:
                sid = assigned_species.get(jar_num)
                if sid is not None:
                    sp_short = SystemVisualizer.SPECIES_NAMES[sid - 1].split(". ")[1].split(" (")[0]
                    self.jar_labels[jar_num].configure(
                        text=f"Lọ #{jar_num}: {sp_short} ({count}c)",
                        text_color="#2e7d32",
                        fg_color="#e8f5e9"
                    )
                else:
                    self.jar_labels[jar_num].configure(
                        text=f"Lọ #{jar_num}: (Trống)",
                        text_color="#64748b",
                        fg_color="#ffffff"
                    )

    def set_enabled(self, enabled: bool):
        state = "normal" if enabled else "disabled"
        self.btn_tune_left.configure(state=state)
        self.btn_tune_right.configure(state=state)
