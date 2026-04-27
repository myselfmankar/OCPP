use serde::{Deserialize, Serialize};
use sled::{Db, Tree};

use crate::StoreError;

/// One persisted outbound `CALL`. The transport rehydrates these as raw
/// JSON into a `Frame::Call` and resends them in order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCall {
    pub action: String,
    pub payload: serde_json::Value,
}

/// FIFO queue of outbound calls, keyed by monotonically increasing IDs.
pub struct OutboundQueue {
    db: Db,
    tree: Tree,
}

impl OutboundQueue {
    pub(crate) fn new(db: Db, tree: Tree) -> Self {
        Self { db, tree }
    }

    /// Append a call, returning its store key.
    pub fn enqueue(&self, call: &PendingCall) -> Result<u64, StoreError> {
        let id = self.db.generate_id()?;
        let bytes = serde_json::to_vec(call)?;
        self.tree.insert(id.to_be_bytes(), bytes)?;
        self.tree.flush()?;
        Ok(id)
    }

    pub fn ack(&self, id: u64) -> Result<(), StoreError> {
        self.tree.remove(id.to_be_bytes())?;
        self.tree.flush()?;
        Ok(())
    }

    /// Iterate all pending calls in FIFO order.
    pub fn drain_iter(&self) -> impl Iterator<Item = Result<(u64, PendingCall), StoreError>> + '_ {
        self.tree.iter().map(|res| {
            let (k, v) = res?;
            let mut id_bytes = [0u8; 8];
            id_bytes.copy_from_slice(&k[..8.min(k.len())]);
            let id = u64::from_be_bytes(id_bytes);
            let call: PendingCall = serde_json::from_slice(&v)?;
            Ok((id, call))
        })
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
}
