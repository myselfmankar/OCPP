use std::sync::Arc;

use ocpp_protocol::{Ocpp16, OcppRequest};
use ocpp_store::queue::{OutboundQueue, PendingCall};
use ocpp_transport::{CallFailure, Session};
use tracing::warn;

pub async fn send_or_queue<R>(
    session: &Arc<Session<Ocpp16>>,
    queue: &OutboundQueue,
    req: R,
) -> Result<R::Response, CallFailure>
where
    R: OcppRequest,
{
    let payload = serde_json::to_value(&req).ok();
    match session.call(req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            if let Some(payload) = payload {
                if let Err(queue_err) = queue.enqueue(&PendingCall {
                    action: R::ACTION.to_string(),
                    payload,
                }) {
                    warn!(
                        action = R::ACTION,
                        error = %queue_err,
                        "failed to persist outbound call"
                    );
                }
            }
            Err(e)
        }
    }
}
