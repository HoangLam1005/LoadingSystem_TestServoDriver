// =============================================================================
// LoadingSystem - Calibration Profile (Level 5: Persistence)
// =============================================================================
// Lưu trữ và quản lý dữ liệu hiệu chuẩn (calibration) của đĩa xoay.
// Profile được lưu vĩnh viễn vào file JSON trên đĩa cứng để duy trì
// các hệ số bù sai số qua nhiều phiên làm việc.
// =============================================================================

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::SystemTime;

use crate::error::LoadingError;

/// Một bản ghi lịch sử hiệu chuẩn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationEntry {
    /// Thời điểm hiệu chuẩn (ISO 8601 string)
    pub timestamp: String,
    /// Góc điều chỉnh (độ) - giá trị người dùng nhập
    pub adjustment_degrees: f64,
    /// Số xung delta tương ứng
    pub delta_pulses: i32,
    /// Giá trị offset sau khi cập nhật
    pub resulting_offset: i32,
}

/// Profile hiệu chuẩn đĩa xoay - chứa toàn bộ thông tin tự học
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfile {
    /// Số xung bù hiện tại (được cộng vào mỗi lệnh quay)
    pub offset_pulses: i32,
    /// Hệ số học lỗi thích nghi (Exponential Moving Average coefficient)
    pub learning_coefficient: f64,
    /// Lịch sử các lần hiệu chuẩn
    pub history: Vec<CalibrationEntry>,
}

impl CalibrationProfile {
    /// Tạo profile mới với hệ số học cho trước, offset = 0
    pub fn new(learning_coefficient: f64) -> Self {
        CalibrationProfile {
            offset_pulses: 0,
            learning_coefficient,
            history: Vec::new(),
        }
    }

    /// Thêm bản ghi lịch sử hiệu chuẩn mới
    pub fn add_history_entry(&mut self, adjustment_degrees: f64, delta_pulses: i32) {
        let timestamp = Self::current_timestamp();
        let entry = CalibrationEntry {
            timestamp,
            adjustment_degrees,
            delta_pulses,
            resulting_offset: self.offset_pulses,
        };
        self.history.push(entry);
        tracing::info!(
            "Calibration entry added: adj={:.2}°, delta={}p, offset={}p",
            adjustment_degrees,
            delta_pulses,
            self.offset_pulses
        );
    }

    /// Lấy lịch sử hiệu chuẩn gần nhất (tối đa n entries)
    pub fn get_recent_history(&self, n: usize) -> &[CalibrationEntry] {
        let start = self.history.len().saturating_sub(n);
        &self.history[start..]
    }

    /// Lưu profile vào file JSON
    pub fn save_to_file(&self, path: &str) -> Result<(), LoadingError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| LoadingError::Calibration(format!("Serialize error: {}", e)))?;
        std::fs::write(path, json)
            .map_err(|e| LoadingError::Calibration(format!(
                "Cannot write calibration file '{}': {}", path, e
            )))?;
        tracing::info!("Calibration profile saved to '{}'", path);
        Ok(())
    }

    /// Đọc profile từ file JSON. Nếu file không tồn tại, trả về profile mặc định.
    pub fn load_from_file(path: &str, default_coefficient: f64) -> Result<Self, LoadingError> {
        if Path::new(path).exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| LoadingError::Calibration(format!(
                    "Cannot read calibration file '{}': {}", path, e
                )))?;
            let profile: CalibrationProfile = serde_json::from_str(&content)
                .map_err(|e| LoadingError::Calibration(format!(
                    "Invalid calibration data in '{}': {}", path, e
                )))?;
            tracing::info!(
                "Calibration profile loaded: offset={}p, coefficient={}, history={}",
                profile.offset_pulses,
                profile.learning_coefficient,
                profile.history.len()
            );
            Ok(profile)
        } else {
            tracing::info!(
                "No calibration file found at '{}', creating new profile",
                path
            );
            Ok(CalibrationProfile::new(default_coefficient))
        }
    }

    /// Tạo timestamp ISO 8601 từ system time (không cần crate chrono)
    fn current_timestamp() -> String {
        let duration = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();

        // Chuyển đổi Unix timestamp sang ISO 8601 cơ bản
        let days = secs / 86400;
        let time_secs = secs % 86400;
        let hours = time_secs / 3600;
        let minutes = (time_secs % 3600) / 60;
        let seconds = time_secs % 60;

        // Tính năm/tháng/ngày từ số ngày kể từ epoch (1970-01-01)
        let (year, month, day) = Self::days_to_date(days);

        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hours, minutes, seconds
        )
    }

    /// Chuyển đổi số ngày từ epoch sang (năm, tháng, ngày)
    fn days_to_date(days: u64) -> (u64, u64, u64) {
        // Thuật toán chuyển đổi ngày dương lịch
        let mut y = 1970;
        let mut remaining = days;

        loop {
            let days_in_year = if Self::is_leap_year(y) { 366 } else { 365 };
            if remaining < days_in_year {
                break;
            }
            remaining -= days_in_year;
            y += 1;
        }

        let months_days = if Self::is_leap_year(y) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut m = 0;
        for (i, &days_in_month) in months_days.iter().enumerate() {
            if remaining < days_in_month {
                m = i as u64 + 1;
                break;
            }
            remaining -= days_in_month;
        }

        (y, m, remaining + 1)
    }

    fn is_leap_year(y: u64) -> bool {
        (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
    }
}
