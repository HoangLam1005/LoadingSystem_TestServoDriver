// =============================================================================
// LoadingSystem - Backend Daemon Entry Point
// =============================================================================
// Khởi chạy Rust Tokio Async Backend Daemon.
// Lắng nghe kết nối IPC Socket từ Python HMI (127.0.0.1:5555).
// =============================================================================

use loading_system::api::ipc_server::IpcServer;
use loading_system::calibration::profile::CalibrationProfile;
use loading_system::config::SystemConfig;
use loading_system::hardware::modbus_client::{MockModbusClient, ModbusInterface, RealModbusClient};
use loading_system::hardware::rotating_disc::RotatingDiscController;

/// Hàm helper chạy server với controller generic
async fn run_server<M: ModbusInterface + 'static>(
    controller: RotatingDiscController<M>,
    config: &SystemConfig,
) -> anyhow::Result<()> {
    let server = IpcServer::new(
        config.ipc.host.clone(),
        config.ipc.port,
        controller,
        config.disc.calibration_file.clone(),
    );

    tracing::info!("IPC Server đang chạy tại {}:{}", config.ipc.host, config.ipc.port);

    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Ctrl+C received, shutting down gracefully...");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Khởi tạo tracing logger
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(true)
        .with_thread_ids(false)
        .init();

    tracing::info!("=== LoadingSystem Backend Daemon ===");
    tracing::info!("Version: 1.0.0");

    // 2. Đọc config file
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".into());

    let config = SystemConfig::load(&config_path)?;
    tracing::info!("Config loaded from '{}'", config_path);
    tracing::info!("Mode: {}", if config.is_mock_mode() { "MOCK (simulated)" } else { &config.serial.port });

    // 3. Load calibration profile
    let calibration = CalibrationProfile::load_from_file(
        &config.disc.calibration_file,
        config.disc.learning_coefficient,
    )?;

    // 4. Tạo controller và khởi chạy server
    if config.is_mock_mode() {
        tracing::info!("Starting in MOCK mode (no hardware required)");
        let mock_client = MockModbusClient::new();
        let controller = RotatingDiscController::new(
            mock_client,
            config.disc.encoder_resolution,
            config.disc.default_frequency,
            calibration,
        );
        run_server(controller, &config).await?;
    } else {
        tracing::info!(
            "Connecting to PLC via {} at {} baud ({},{},{})...",
            config.serial.port,
            config.serial.baud_rate,
            config.serial.data_bits,
            config.serial.parity,
            config.serial.stop_bits
        );
        let real_client = RealModbusClient::new_with_config(
            &config.serial.port,
            config.serial.baud_rate,
            config.serial.data_bits,
            config.serial.stop_bits,
            &config.serial.parity,
            config.modbus.slave_address,
        )
        .await?;

        let controller = RotatingDiscController::new(
            real_client,
            config.disc.encoder_resolution,
            config.disc.default_frequency,
            calibration,
        );
        run_server(controller, &config).await?;
    }

    tracing::info!("Daemon stopped");
    Ok(())
}
