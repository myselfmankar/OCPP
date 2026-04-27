use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sled::Tree;

use crate::StoreError;

/// Last accepted BootNotification info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootInfo {
    pub status: String, // RegistrationStatus as string
    pub interval: i32,
    pub last_boot: DateTime<Utc>,
}

/// Active transaction record for replay/resume on reconnect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTransaction {
    pub transaction_id: i32,
    pub connector_id: i32,
    pub id_tag: String,
    pub meter_start: i32,
    pub started_at: DateTime<Utc>,
}

const KEY_BOOT: &[u8] = b"boot";
const PFX_TX: &str = "tx/";

/// Per-ChargePoint state.
pub struct CpState {
    tree: Tree,
}

impl CpState {
    pub(crate) fn new(tree: Tree) -> Self {
        Self { tree }
    }

    pub fn get_boot(&self) -> Result<Option<BootInfo>, StoreError> {
        match self.tree.get(KEY_BOOT)? {
            Some(b) => Ok(Some(serde_json::from_slice(&b)?)),
            None => Ok(None),
        }
    }

    pub fn put_boot(&self, info: &BootInfo) -> Result<(), StoreError> {
        self.tree.insert(KEY_BOOT, serde_json::to_vec(info)?)?;
        self.tree.flush()?;
        Ok(())
    }

    pub fn put_tx(&self, tx: &ActiveTransaction) -> Result<(), StoreError> {
        let key = format!("{PFX_TX}{}", tx.transaction_id);
        self.tree.insert(key.as_bytes(), serde_json::to_vec(tx)?)?;
        self.tree.flush()?;
        Ok(())
    }

    pub fn remove_tx(&self, transaction_id: i32) -> Result<(), StoreError> {
        let key = format!("{PFX_TX}{transaction_id}");
        self.tree.remove(key.as_bytes())?;
        self.tree.flush()?;
        Ok(())
    }

    pub fn list_tx(&self) -> Result<Vec<ActiveTransaction>, StoreError> {
        let mut out = Vec::new();
        for kv in self.tree.scan_prefix(PFX_TX.as_bytes()) {
            let (_, v) = kv?;
            out.push(serde_json::from_slice(&v)?);
        }
        Ok(out)
    }
}
