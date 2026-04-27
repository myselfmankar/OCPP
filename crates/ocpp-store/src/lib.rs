//! Persistent local store for the gateway.
//!
//! Holds:
//! - per-ChargePoint outbound `CALL` queue (replayed on reconnect)
//! - per-ChargePoint transaction state and last boot/heartbeat info
//!
//! Backed by `sled` (embedded KV).

pub mod queue;
pub mod state;

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
}
