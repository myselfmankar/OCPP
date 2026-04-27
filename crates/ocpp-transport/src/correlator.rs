use std::collections::HashMap;
use std::sync::Arc;

use ocpp_protocol::{CallError, CallErrorCode};
use serde_json::Value;
use tokio::sync::{oneshot, Mutex};

/// Result of a pending `CALL`: either the JSON payload of a `CALLRESULT` or
/// the contents of a `CALLERROR`.
pub type CallReply = Result<Value, CallError>;

/// Tracks pending `CALL`s by `uniqueId`.
#[derive(Clone, Default)]
pub struct Correlator {
    inner: Arc<Mutex<HashMap<String, oneshot::Sender<CallReply>>>>,
}

impl Correlator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending call; returns a receiver to await the reply.
    pub async fn register(&self, unique_id: String) -> oneshot::Receiver<CallReply> {
        let (tx, rx) = oneshot::channel();
        self.inner.lock().await.insert(unique_id, tx);
        rx
    }

    pub async fn complete_result(&self, unique_id: &str, payload: Value) -> bool {
        if let Some(tx) = self.inner.lock().await.remove(unique_id) {
            let _ = tx.send(Ok(payload));
            true
        } else {
            false
        }
    }

    pub async fn complete_error(&self, err: CallError) -> bool {
        let id = err.unique_id.clone();
        if let Some(tx) = self.inner.lock().await.remove(&id) {
            let _ = tx.send(Err(err));
            true
        } else {
            false
        }
    }

    /// Cancel all in-flight calls (e.g. on disconnect) with a synthetic
    /// `InternalError` so callers can fail fast.
    pub async fn cancel_all(&self, reason: &str) {
        let mut g = self.inner.lock().await;
        for (id, tx) in g.drain() {
            let _ = tx.send(Err(CallError {
                unique_id: id,
                error_code: CallErrorCode::InternalError,
                error_description: reason.to_string(),
                error_details: Value::Null,
            }));
        }
    }
}
