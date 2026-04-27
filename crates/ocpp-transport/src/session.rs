use std::sync::Arc;
use std::time::Duration;

use ocpp_protocol::{Call, Frame, OcppRequest, OcppVersion};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::correlator::Correlator;
use crate::dispatcher::{dispatch, CsmsHandler};
use crate::error::{CallFailure, TransportError};
use crate::tls::SecurityProfile;
use crate::ws_client::{self, WsChannels};

/// Configuration for opening a `Session`.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub url: url::Url,
    pub security: SecurityProfile,
    pub call_timeout: Duration,
}

impl SessionConfig {
    pub fn new(url: url::Url, security: SecurityProfile) -> Self {
        Self {
            url,
            security,
            call_timeout: Duration::from_secs(30),
        }
    }
}

/// One OCPP session over a single WebSocket connection.
///
/// Responsibilities:
/// - Send typed `OcppRequest` and await typed responses (with correlation+timeout).
/// - Dispatch incoming CSMS `CALL`s to a `CsmsHandler`.
///
/// Generic over `V: OcppVersion` so the same session machinery can drive
/// OCPP 1.6 today and 2.0.1 in the future.
pub struct Session<V: OcppVersion> {
    cfg: SessionConfig,
    outbound_tx: mpsc::Sender<Frame>,
    correlator: Correlator,
    _version: std::marker::PhantomData<V>,
}

impl<V: OcppVersion> Session<V> {
    /// Open a new session by connecting to the CSMS and spawning the
    /// inbound dispatcher. The returned `Session` is the handle used to
    /// issue calls. `closed` resolves when the underlying connection ends.
    pub async fn connect(
        cfg: SessionConfig,
        handler: Arc<dyn CsmsHandler>,
    ) -> Result<(Self, tokio::sync::oneshot::Receiver<()>), TransportError> {
        let WsChannels {
            inbound_rx,
            outbound_tx,
            closed_rx,
        } = ws_client::connect(&cfg.url, V::subprotocol(), &cfg.security).await?;

        let correlator = Correlator::new();
        tokio::spawn(run_inbound(
            inbound_rx,
            outbound_tx.clone(),
            correlator.clone(),
            handler,
        ));

        let session = Self {
            cfg,
            outbound_tx,
            correlator,
            _version: std::marker::PhantomData,
        };
        Ok((session, closed_rx))
    }

    /// Send an OCPP request and await the typed response.
    pub async fn call<R: OcppRequest>(&self, req: R) -> Result<R::Response, CallFailure> {
        let unique_id = Uuid::new_v4().to_string();
        let payload = serde_json::to_value(&req).map_err(|e| {
            CallFailure::Transport(TransportError::Protocol(
                ocpp_protocol::ProtocolError::Json(e),
            ))
        })?;
        let frame = Frame::Call(Call {
            unique_id: unique_id.clone(),
            action: R::ACTION.to_string(),
            payload,
        });

        let rx = self.correlator.register(unique_id.clone()).await;
        self.outbound_tx
            .send(frame)
            .await
            .map_err(|_| CallFailure::Transport(TransportError::Closed))?;

        let reply = match timeout(self.cfg.call_timeout, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => return Err(CallFailure::Transport(TransportError::Closed)),
            Err(_) => return Err(CallFailure::Transport(TransportError::Timeout)),
        };

        match reply {
            Ok(value) => serde_json::from_value::<R::Response>(value)
                .map_err(|e| CallFailure::BadResponse(e.to_string())),
            Err(call_err) => Err(CallFailure::CallError {
                code: call_err.error_code,
                description: call_err.error_description,
                details: call_err.error_details,
            }),
        }
    }
}

async fn run_inbound(
    mut inbound_rx: mpsc::Receiver<Frame>,
    outbound_tx: mpsc::Sender<Frame>,
    correlator: Correlator,
    handler: Arc<dyn CsmsHandler>,
) {
    while let Some(frame) = inbound_rx.recv().await {
        match frame {
            Frame::Result(r) => {
                if !correlator.complete_result(&r.unique_id, r.payload).await {
                    warn!(id = %r.unique_id, "no pending call for CallResult");
                }
            }
            Frame::Error(e) => {
                let id = e.unique_id.clone();
                if !correlator.complete_error(e).await {
                    warn!(id = %id, "no pending call for CallError");
                }
            }
            Frame::Call(c) => {
                let h = handler.clone();
                let outbound_tx = outbound_tx.clone();
                tokio::spawn(async move {
                    let reply = dispatch(h.as_ref(), c).await;
                    if outbound_tx.send(reply).await.is_err() {
                        error!("could not send dispatch reply: outbound closed");
                    }
                });
            }
        }
    }
    debug!("inbound dispatcher exiting; cancelling pending calls");
    correlator.cancel_all("websocket disconnected").await;
    drop(outbound_tx); // signals writer task to exit
}
