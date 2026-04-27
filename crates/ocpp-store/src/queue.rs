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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    fn open_tmp() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = Store::open(dir.path()).expect("open store");
        (dir, store)
    }

    fn pc(action: &str, n: i32) -> PendingCall {
        PendingCall {
            action: action.to_string(),
            payload: serde_json::json!({ "seq": n }),
        }
    }

    #[test]
    fn enqueue_drain_is_fifo() {
        let (_d, store) = open_tmp();
        let q = store.queue("CP-1").unwrap();
        q.enqueue(&pc("StatusNotification", 1)).unwrap();
        q.enqueue(&pc("MeterValues", 2)).unwrap();
        q.enqueue(&pc("StopTransaction", 3)).unwrap();

        let entries: Vec<_> = q.drain_iter().collect::<Result<_, _>>().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].1.action, "StatusNotification");
        assert_eq!(entries[1].1.action, "MeterValues");
        assert_eq!(entries[2].1.action, "StopTransaction");
        assert_eq!(entries[0].1.payload["seq"], 1);
        assert_eq!(entries[2].1.payload["seq"], 3);
    }

    #[test]
    fn ack_removes_only_acked_entries() {
        let (_d, store) = open_tmp();
        let q = store.queue("CP-1").unwrap();
        let id1 = q.enqueue(&pc("A", 1)).unwrap();
        let _id2 = q.enqueue(&pc("B", 2)).unwrap();
        let _id3 = q.enqueue(&pc("C", 3)).unwrap();

        assert_eq!(q.len(), 3);
        q.ack(id1).unwrap();
        assert_eq!(q.len(), 2);

        let remaining: Vec<_> = q.drain_iter().collect::<Result<_, _>>().unwrap();
        assert_eq!(remaining[0].1.action, "B");
        assert_eq!(remaining[1].1.action, "C");
    }

    #[test]
    fn drain_iter_does_not_remove_entries() {
        // drain_iter is read-only; entries persist until explicitly acked.
        // (Naming follows sled's `Tree::iter`.)
        let (_d, store) = open_tmp();
        let q = store.queue("CP-1").unwrap();
        q.enqueue(&pc("A", 1)).unwrap();
        q.enqueue(&pc("B", 2)).unwrap();

        let _: Vec<_> = q.drain_iter().collect::<Result<_, _>>().unwrap();
        assert_eq!(q.len(), 2, "drain_iter must not remove entries");
    }
}
