//! Persistent local store for the gateway.
//!
//! Holds:
//! - per-ChargePoint outbound `CALL` queue (replayed on reconnect)
//! - per-ChargePoint transaction state and last boot/heartbeat info
//!
//! Backed by `sled` (embedded KV).

pub mod queue;
pub mod state;
pub mod auth;
pub mod config;
pub mod profiles;
pub mod reservations;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sled error: {0}")]
    Sled(#[from] sled::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct Store {
    db: sled::Db,
}

impl Store {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    pub fn queue(&self, cp_id: &str) -> Result<queue::OutboundQueue, StoreError> {
        let tree = self.db.open_tree(format!("queue/{cp_id}"))?;
        Ok(queue::OutboundQueue::new(self.db.clone(), tree))
    }

    pub fn state(&self, cp_id: &str) -> Result<state::CpState, StoreError> {
        let tree = self.db.open_tree(format!("state/{cp_id}"))?;
        Ok(state::CpState::new(tree))
    }

    pub fn auth(&self, cp_id: &str) -> Result<auth::AuthStore, StoreError> {
        let tree = self.db.open_tree(format!("auth/{cp_id}"))?;
        Ok(auth::AuthStore::new(tree))
    }

    pub fn config(&self, cp_id: &str) -> Result<config::ConfigStore, StoreError> {
        let tree = self.db.open_tree(format!("config/{cp_id}"))?;
        Ok(config::ConfigStore::new(tree))
    }

    pub fn profiles(&self, cp_id: &str) -> Result<profiles::ProfileStore, StoreError> {
        let tree = self.db.open_tree(format!("profiles/{cp_id}"))?;
        Ok(profiles::ProfileStore::new(tree))
    }

    pub fn reservations(&self, cp_id: &str) -> Result<reservations::ReservationStore, StoreError> {
        let tree = self.db.open_tree(format!("reservations/{cp_id}"))?;
        Ok(reservations::ReservationStore::new(tree))
    }
}
