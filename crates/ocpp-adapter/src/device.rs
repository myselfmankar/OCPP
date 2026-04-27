use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::mpsc;

use crate::events::{DeviceCommand, DeviceEvent};

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("device closed")]
    Closed,
    #[error("device backend error: {0}")]
    Backend(String),
}

/// Internal-protocol-agnostic battery handle.
///
/// Implementations:
/// - `ocpp-internal-mqtt::MqttDevice`
/// - `ocpp-internal-grpc::GrpcDevice`
#[async_trait]
pub trait Device: Send + Sync + 'static {
    /// Subscribe to events from this device. The returned receiver yields
    /// events until the device disconnects.
    async fn events(&self) -> Result<mpsc::Receiver<DeviceEvent>, DeviceError>;

    /// Send a command to the device.
    async fn send(&self, cmd: DeviceCommand) -> Result<(), DeviceError>;
}
