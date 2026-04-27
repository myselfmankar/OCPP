use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct GatewayConfig {
    pub csms: CsmsConfig,
    pub store: StoreConfig,
    #[serde(default)]
    pub mqtt: Option<MqttBrokerConfig>,
    #[serde(default)]
    pub grpc: Option<GrpcServerConfig>,
    pub charge_points: Vec<ChargePointEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CsmsConfig {
    /// Base URL up to (but not including) the chargePointId, e.g.
    /// `wss://csms.example.com/ocpp/`.
    pub base_url: Url,
    pub security: SecurityConfig,
    #[serde(default = "default_call_timeout")]
    pub call_timeout_secs: u64,
}

fn default_call_timeout() -> u64 {
    30
}

#[derive(Debug, Deserialize)]
#[serde(tag = "profile", rename_all = "snake_case")]
pub enum SecurityConfig {
    Plain,
    TlsBasic {
        username: String,
        password: String,
    },
    Mtls {
        ca_pem_path: Option<PathBuf>,
        client_cert_pem_path: PathBuf,
        client_key_pem_path: PathBuf,
    },
}

#[derive(Debug, Deserialize)]
pub struct StoreConfig {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct MqttBrokerConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    /// e.g. `"batteries"` -> topics `batteries/<id>/events|commands`
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
    #[serde(default = "default_client_id")]
    pub client_id: String,
}

fn default_topic_prefix() -> String {
    "batteries".into()
}
fn default_client_id() -> String {
    format!("ocpp-gateway-{}", std::process::id())
}

#[derive(Debug, Deserialize)]
pub struct GrpcServerConfig {
    pub listen: SocketAddr,
}

#[derive(Debug, Deserialize)]
pub struct ChargePointEntry {
    /// Charge-point id (also the WS URL suffix).
    pub id: String,
    pub vendor: String,
    pub model: String,
    pub device: DeviceBackendConfig,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeviceBackendConfig {
    Mqtt { battery_id: String },
    Grpc { battery_id: String },
}

impl GatewayConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::from(path))
            .build()?;
        Ok(cfg.try_deserialize()?)
    }

    pub fn call_timeout(&self) -> Duration {
        Duration::from_secs(self.csms.call_timeout_secs)
    }
}
