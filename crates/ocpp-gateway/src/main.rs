mod config;
mod supervisor;

use std::path::PathBuf;

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::config::GatewayConfig;
use crate::supervisor::Supervisor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .init();

    let path = std::env::args()
        .nth(2)
        .or_else(|| std::env::args().nth(1).filter(|a| !a.starts_with("--")))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("gateway.yaml"));

    let cfg = GatewayConfig::load(&path)?;
    Supervisor::new(cfg)?.run().await
}
