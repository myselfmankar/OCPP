use std::sync::Arc;

use ocpp_adapter::{ChargePoint, ChargePointConfig, Device};
use ocpp_internal_grpc::GrpcRegistry;
use ocpp_internal_mqtt::{MqttDevice, MqttDeviceConfig};
use ocpp_store::Store;
use ocpp_transport::{SecurityProfile, SessionConfig};
use tokio::task::JoinSet;
use tracing::{error, info};

use crate::config::{
    DeviceBackendConfig, GatewayConfig, MqttBrokerConfig, SecurityConfig,
};

pub struct Supervisor {
    cfg: GatewayConfig,
    store: Store,
}

impl Supervisor {
    pub fn new(cfg: GatewayConfig) -> anyhow::Result<Self> {
        let store = Store::open(&cfg.store.path)?;
        Ok(Self { cfg, store })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let security = build_security(&self.cfg.csms.security)?;
        let call_timeout = self.cfg.call_timeout();

        // Optional gRPC server for batteries that connect over gRPC.
        let grpc_registry = GrpcRegistry::new();
        if let Some(grpc) = &self.cfg.grpc {
            let registry = grpc_registry.clone();
            let listen = grpc.listen;
            tokio::spawn(async move {
                info!(%listen, "starting gRPC battery-bus server");
                if let Err(e) = tonic::transport::Server::builder()
                    .add_service(registry.service())
                    .serve(listen)
                    .await
                {
                    error!(error=%e, "grpc server stopped");
                }
            });
        }

        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
        let mut tasks: JoinSet<()> = JoinSet::new();

        for cp in &self.cfg.charge_points {
            let device: Arc<dyn Device> = match &cp.device {
                DeviceBackendConfig::Mqtt { battery_id } => {
                    let broker = self
                        .cfg
                        .mqtt
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("MQTT backend used but no [mqtt] config"))?;
                    Arc::new(connect_mqtt(broker, battery_id, &cp.id).await?)
                }
                DeviceBackendConfig::Grpc { battery_id } => {
                    grpc_registry.device_for(battery_id).await
                }
            };

            let url = self
                .cfg
                .csms
                .base_url
                .join(&cp.id)
                .map_err(|e| anyhow::anyhow!("bad CP url for {}: {e}", cp.id))?;
            let mut session = SessionConfig::new(url, security.clone());
            session.call_timeout = call_timeout;

            let cp_cfg = ChargePointConfig {
                cp_id: cp.id.clone(),
                vendor: cp.vendor.clone(),
                model: cp.model.clone(),
                session,
            };

            let actor = ChargePoint::new(cp_cfg, device, self.store.clone());
            let cancel = cancel_rx.clone();
            tasks.spawn(async move {
                actor.run(cancel).await;
            });
        }

        // Graceful shutdown on Ctrl+C.
        tokio::select! {
            _ = tokio::signal::ctrl_c() => info!("Ctrl+C received; shutting down"),
            _ = wait_all(&mut tasks) => info!("all actors exited"),
        }
        let _ = cancel_tx.send(true);
        while tasks.join_next().await.is_some() {}
        Ok(())
    }
}

async fn wait_all(tasks: &mut JoinSet<()>) {
    while tasks.join_next().await.is_some() {}
}

async fn connect_mqtt(
    broker: &MqttBrokerConfig,
    battery_id: &str,
    cp_id: &str,
) -> anyhow::Result<MqttDevice> {
    let cfg = MqttDeviceConfig {
        broker_host: broker.host.clone(),
        broker_port: broker.port,
        client_id: format!("{}-{}", broker.client_id, cp_id),
        username: broker.username.clone(),
        password: broker.password.clone(),
        battery_id: battery_id.to_string(),
        topic_prefix: broker.topic_prefix.clone(),
    };
    Ok(MqttDevice::connect(cfg).await?)
}

fn build_security(s: &SecurityConfig) -> anyhow::Result<SecurityProfile> {
    Ok(match s {
        SecurityConfig::Plain => SecurityProfile::Plain,
        SecurityConfig::TlsBasic { username, password } => SecurityProfile::Tls {
            basic_auth: Some((username.clone(), password.clone())),
        },
        SecurityConfig::Mtls {
            ca_pem_path,
            client_cert_pem_path,
            client_key_pem_path,
        } => SecurityProfile::Mtls {
            ca_pem: match ca_pem_path {
                Some(p) => Some(std::fs::read(p)?),
                None => None,
            },
            client_cert_pem: std::fs::read(client_cert_pem_path)?,
            client_key_pem: std::fs::read(client_key_pem_path)?,
        },
    })
}
