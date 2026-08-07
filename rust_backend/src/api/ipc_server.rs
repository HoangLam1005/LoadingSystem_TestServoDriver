// =============================================================================
// LoadingSystem - IPC Server (TCP with Length-Prefixed JSON)
// =============================================================================
// Server IPC sử dụng TCP socket với giao thức JSON framing.
// Giao thức tương thích với ZeroMQ REQ/REP pattern.
//
// Framing protocol:
//   [4 bytes: message length (big-endian u32)] + [N bytes: JSON payload]
//
// Server lắng nghe kết nối từ Python HMI, nhận lệnh JSON,
// dispatch tới RotatingDiscController, và trả kết quả JSON.
// =============================================================================

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex as TokioMutex;

use crate::api::commands::{CommandMessage, ResponseMessage, StatusData};
use crate::error::LoadingError;
use crate::hardware::modbus_client::ModbusInterface;
use crate::hardware::rotating_disc::{Direction, RotatingDiscController};

/// IPC Server quản lý giao tiếp giữa Python HMI và Rust Backend.
/// Sử dụng Arc<Mutex> để cho phép chia sẻ controller giữa nhiều client.
pub struct IpcServer<M: ModbusInterface + 'static> {
    host: String,
    port: u16,
    controller: Arc<TokioMutex<RotatingDiscController<M>>>,
    calibration_file: String,
}

impl<M: ModbusInterface + 'static> IpcServer<M> {
    /// Tạo IPC server mới
    ///
    /// # Arguments
    /// * `host` - Địa chỉ IP lắng nghe
    /// * `port` - Cổng TCP
    /// * `controller` - Bộ điều khiển đĩa xoay
    /// * `calibration_file` - Đường dẫn file lưu calibration
    pub fn new(
        host: String,
        port: u16,
        controller: RotatingDiscController<M>,
        calibration_file: String,
    ) -> Self {
        Self::new_shared(host, port, Arc::new(TokioMutex::new(controller)), calibration_file)
    }

    /// Tạo IPC server từ controller Arc đã shared
    pub fn new_shared(
        host: String,
        port: u16,
        controller: Arc<TokioMutex<RotatingDiscController<M>>>,
        calibration_file: String,
    ) -> Self {
        IpcServer {
            host,
            port,
            controller,
            calibration_file,
        }
    }

    /// Khởi chạy server, lắng nghe và xử lý kết nối.
    /// Hàm này chạy vĩnh viễn cho đến khi nhận lệnh SHUTDOWN hoặc bị kill.
    pub async fn run(&self) -> Result<(), LoadingError> {
        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| LoadingError::Ipc(format!("Cannot bind to {}: {}", addr, e)))?;

        tracing::info!("IPC server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    tracing::info!("Client connected: {}", peer);
                    let controller = self.controller.clone();
                    let cal_file = self.calibration_file.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, controller, cal_file).await {
                            tracing::warn!("Client disconnected: {}", e);
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("Accept error: {}", e);
                }
            }
        }
    }
}

/// Xử lý một client connection
async fn handle_client<M: ModbusInterface>(
    mut stream: TcpStream,
    controller: Arc<TokioMutex<RotatingDiscController<M>>>,
    calibration_file: String,
) -> Result<(), LoadingError> {
    loop {
        // Đọc message từ client
        let request = match read_message(&mut stream).await {
            Ok(msg) => msg,
            Err(_) => {
                tracing::info!("Client disconnected (read error)");
                return Ok(());
            }
        };

        tracing::info!("Received command: {}", request);

        // Parse và dispatch command
        let response = dispatch_command(&request, &controller, &calibration_file).await;
        let response_json = serde_json::to_string(&response)
            .unwrap_or_else(|_| r#"{"success":false,"message":"Serialization error"}"#.to_string());

        tracing::info!("Sending response: {}", response_json);

        // Gửi response
        if let Err(e) = write_message(&mut stream, &response_json).await {
            tracing::warn!("Failed to send response: {}", e);
            return Ok(());
        }

        // Kiểm tra lệnh shutdown
        if request.contains("\"SHUTDOWN\"") {
            tracing::info!("Shutdown command received, saving calibration...");
            let ctrl = controller.lock().await;
            if let Err(e) = ctrl.get_calibration().save_to_file(&calibration_file) {
                tracing::error!("Failed to save calibration: {}", e);
            }
            return Ok(());
        }
    }
}

/// Dispatch lệnh tới controller và trả về response
async fn dispatch_command<M: ModbusInterface>(
    request: &str,
    controller: &Arc<TokioMutex<RotatingDiscController<M>>>,
    calibration_file: &str,
) -> ResponseMessage {
    let cmd: CommandMessage = match serde_json::from_str(request) {
        Ok(c) => c,
        Err(e) => {
            return ResponseMessage::error(format!("Invalid JSON command: {}", e));
        }
    };

    let mut ctrl = controller.lock().await;

    match cmd.command.as_str() {
        "ROTATE_RIGHT" => {
            let degrees = cmd.value.unwrap_or(90.0);
            match ctrl.rotate_degrees_with_freq(degrees, Direction::Right, cmd.frequency).await {
                Ok(()) => ResponseMessage::success(format!(
                    "Rotated right {:.2}°. Current angle: {:.2}°",
                    degrees,
                    ctrl.get_current_angle()
                )),
                Err(e) => ResponseMessage::error(format!("Rotate right failed: {}", e)),
            }
        }

        "ROTATE_LEFT" => {
            let degrees = cmd.value.unwrap_or(90.0);
            match ctrl.rotate_degrees_with_freq(degrees, Direction::Left, cmd.frequency).await {
                Ok(()) => ResponseMessage::success(format!(
                    "Rotated left {:.2}°. Current angle: {:.2}°",
                    degrees,
                    ctrl.get_current_angle()
                )),
                Err(e) => ResponseMessage::error(format!("Rotate left failed: {}", e)),
            }
        }

        "RETURN_HOME" => match ctrl.return_to_origin().await {
            Ok(()) => ResponseMessage::success("Returned to origin (0°)".into()),
            Err(e) => ResponseMessage::error(format!("Return home failed: {}", e)),
        },

        "OVERRIDE_ADJUST" => {
            let degrees = cmd.value.unwrap_or(0.0);
            ctrl.inject_manual_override(degrees);

            // Tự động lưu calibration sau mỗi lần override
            if let Err(e) = ctrl.get_calibration().save_to_file(calibration_file) {
                tracing::warn!("Failed to save calibration after override: {}", e);
            }

            let cal = ctrl.get_calibration();
            ResponseMessage::success(format!(
                "Override applied: {:.2}°. New offset: {} pulses",
                degrees, cal.offset_pulses
            ))
        }

        "GET_STATUS" => {
            let cal = ctrl.get_calibration();
            let data = StatusData {
                current_angle: ctrl.get_current_angle(),
                offset_pulses: cal.offset_pulses,
                learning_coefficient: cal.learning_coefficient,
                calibration_count: cal.history.len(),
            };
            ResponseMessage::success_with_data("Status retrieved".into(), data)
        }

        "SHUTDOWN" => ResponseMessage::success("Shutting down daemon...".into()),

        other => ResponseMessage::error(format!("Unknown command: '{}'", other)),
    }
}

/// Đọc một message với length-prefix framing
async fn read_message(stream: &mut TcpStream) -> Result<String, LoadingError> {
    // Đọc 4 bytes length prefix (big-endian u32)
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .map_err(|e| LoadingError::Ipc(format!("Read length failed: {}", e)))?;

    let msg_len = u32::from_be_bytes(len_buf) as usize;
    if msg_len > 1_048_576 {
        // Giới hạn 1MB
        return Err(LoadingError::Ipc(format!(
            "Message too large: {} bytes",
            msg_len
        )));
    }

    // Đọc message body
    let mut msg_buf = vec![0u8; msg_len];
    stream
        .read_exact(&mut msg_buf)
        .await
        .map_err(|e| LoadingError::Ipc(format!("Read message failed: {}", e)))?;

    String::from_utf8(msg_buf)
        .map_err(|e| LoadingError::Ipc(format!("Invalid UTF-8 message: {}", e)))
}

/// Ghi một message với length-prefix framing
async fn write_message(stream: &mut TcpStream, message: &str) -> Result<(), LoadingError> {
    let msg_bytes = message.as_bytes();
    let len_bytes = (msg_bytes.len() as u32).to_be_bytes();

    stream
        .write_all(&len_bytes)
        .await
        .map_err(|e| LoadingError::Ipc(format!("Write length failed: {}", e)))?;
    stream
        .write_all(msg_bytes)
        .await
        .map_err(|e| LoadingError::Ipc(format!("Write message failed: {}", e)))?;
    stream
        .flush()
        .await
        .map_err(|e| LoadingError::Ipc(format!("Flush failed: {}", e)))?;

    Ok(())
}
