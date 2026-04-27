//! `Device` implementation backed by a gRPC bidi-stream.
//!
//! The gateway runs the gRPC server; each battery is a client that opens a
//! `BatteryBus.Stream` RPC. Events flow client→server, commands server→client.

use async_trait::async_trait;
use futures::Stream;
use ocpp_adapter::{Device, DeviceCommand, DeviceError, DeviceEvent};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, info, warn};

pub mod pb {
    tonic::include_proto!("ocpp.gateway.v1");
}

use pb::battery_bus_server::{BatteryBus, BatteryBusServer};
use pb::{CommandEnvelope, EventEnvelope};

/// Server-side endpoint and per-battery `Device` factory.
///
/// Typical usage in the gateway binary:
/// ```ignore
/// let registry = GrpcRegistry::new();
/// let device = registry.device_for("battery-1");
/// tonic::transport::Server::builder()
///     .add_service(registry.service())
///     .serve(addr)
///     .await?;
/// ```
#[derive(Clone, Default)]
pub struct GrpcRegistry {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    devices: std::collections::HashMap<String, Arc<GrpcDeviceShared>>,
}

struct GrpcDeviceShared {
    /// Multi-producer for inbound events (server-side broadcasts to subscribers).
    events_tx: tokio::sync::broadcast::Sender<DeviceEvent>,
    /// Single command sink. The active stream task picks commands from here
    /// and forwards them to the connected battery.
    cmd_tx: Mutex<Option<mpsc::Sender<DeviceCommand>>>,
}

impl GrpcRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn service(&self) -> BatteryBusServer<GrpcService> {
        BatteryBusServer::new(GrpcService {
            registry: self.clone(),
        })
    }

    /// Get (or create) the `Device` handle for a battery id. Safe to call
    /// before the battery actually connects.
    pub async fn device_for(&self, battery_id: &str) -> Arc<GrpcDevice> {
        let mut g = self.inner.lock().await;
        let shared = g
            .devices
            .entry(battery_id.to_string())
            .or_insert_with(|| {
                let (tx, _) = tokio::sync::broadcast::channel(64);
                Arc::new(GrpcDeviceShared {
                    events_tx: tx,
                    cmd_tx: Mutex::new(None),
                })
            })
            .clone();
        Arc::new(GrpcDevice {
            battery_id: battery_id.to_string(),
            shared,
        })
    }
}

pub struct GrpcDevice {
    battery_id: String,
    shared: Arc<GrpcDeviceShared>,
}

#[async_trait]
impl Device for GrpcDevice {
    async fn events(&self) -> Result<mpsc::Receiver<DeviceEvent>, DeviceError> {
        let mut rx = self.shared.events_tx.subscribe();
        let (tx, out) = mpsc::channel(64);
        tokio::spawn(async move {
            while let Ok(ev) = rx.recv().await {
                if tx.send(ev).await.is_err() {
                    break;
                }
            }
            debug!("grpc subscriber ending");
        });
        Ok(out)
    }

    async fn send(&self, cmd: DeviceCommand) -> Result<(), DeviceError> {
        let g = self.shared.cmd_tx.lock().await;
        let tx = g
            .as_ref()
            .ok_or_else(|| DeviceError::Backend(format!("battery {} not connected", self.battery_id)))?;
        tx.send(cmd)
            .await
            .map_err(|_| DeviceError::Closed)
    }
}

pub struct GrpcService {
    registry: GrpcRegistry,
}

#[async_trait]
impl BatteryBus for GrpcService {
    type StreamStream =
        Pin<Box<dyn Stream<Item = Result<CommandEnvelope, Status>> + Send + 'static>>;

    async fn stream(
        &self,
        req: Request<Streaming<EventEnvelope>>,
    ) -> Result<Response<Self::StreamStream>, Status> {
        let mut inbound = req.into_inner();

        // Peek the first message to learn the battery_id, then route.
        let first = inbound
            .message()
            .await?
            .ok_or_else(|| Status::invalid_argument("empty stream"))?;
        let battery_id = first.battery_id.clone();
        info!(%battery_id, "battery connected over gRPC");

        let device = self.registry.device_for(&battery_id).await;
        let (cmd_tx, cmd_rx) = mpsc::channel::<DeviceCommand>(32);
        {
            let mut g = device.shared.cmd_tx.lock().await;
            *g = Some(cmd_tx);
        }

        // Replay the first envelope into the broadcast.
        if let Ok(ev) = serde_json::from_str::<DeviceEvent>(&first.event_json) {
            let _ = device.shared.events_tx.send(ev);
        } else {
            warn!("first envelope had unparsable event_json");
        }

        let shared = device.shared.clone();
        // Spawn a task that consumes the rest of the inbound stream.
        tokio::spawn(async move {
            while let Ok(Some(env)) = inbound.message().await {
                match serde_json::from_str::<DeviceEvent>(&env.event_json) {
                    Ok(ev) => {
                        let _ = shared.events_tx.send(ev);
                    }
                    Err(e) => warn!(error=%e, "bad event_json"),
                }
            }
            // Disconnect: clear the command sink.
            let mut g = shared.cmd_tx.lock().await;
            *g = None;
            info!("battery disconnected");
        });

        let outbound = ReceiverStream::new(cmd_rx).map(|cmd| {
            let json = serde_json::to_string(&cmd).unwrap_or_default();
            Ok(CommandEnvelope { command_json: json })
        });

        Ok(Response::new(Box::pin(outbound)))
    }
}
