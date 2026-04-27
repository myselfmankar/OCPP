use futures_util::{SinkExt, StreamExt};
use ocpp_protocol::Frame;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, warn};

use crate::error::TransportError;
use crate::tls::SecurityProfile;

pub type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Channels exposed by `connect`. The transport layer sends `Frame`s on
/// `outbound_tx` and receives them on `inbound_rx`. Closing `outbound_tx`
/// or dropping it will cause the writer task to terminate.
pub struct WsChannels {
    pub inbound_rx: mpsc::Receiver<Frame>,
    pub outbound_tx: mpsc::Sender<Frame>,
    /// Resolves when the underlying connection has been closed for any reason.
    pub closed_rx: tokio::sync::oneshot::Receiver<()>,
}

/// Connect to the CSMS and spawn reader/writer tasks. Returns channels
/// for typed frame IO.
pub async fn connect(
    url: &url::Url,
    subprotocol: &str,
    security: &SecurityProfile,
) -> Result<WsChannels, TransportError> {
    let request = security.build_request(url, subprotocol)?;
    // tungstenite's IntoClientRequest is implemented for http::Request<()>.
    let request = request.into_client_request()?;

    let connector = security.connector()?;
    let cfg = WebSocketConfig::default();
    let (ws, response) =
        connect_async_tls_with_config(request, Some(cfg), false, connector).await?;

    // Verify the server selected our subprotocol.
    let selected = response
        .headers()
        .get(http::header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !selected.eq_ignore_ascii_case(subprotocol) {
        warn!(expected = subprotocol, got = selected, "unexpected ws subprotocol");
    }

    let (inbound_tx, inbound_rx) = mpsc::channel::<Frame>(64);
    let (outbound_tx, outbound_rx) = mpsc::channel::<Frame>(64);
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(run_io(ws, inbound_tx, outbound_rx, closed_tx));

    Ok(WsChannels { inbound_rx, outbound_tx, closed_rx })
}

async fn run_io(
    ws: WsStream,
    inbound_tx: mpsc::Sender<Frame>,
    mut outbound_rx: mpsc::Receiver<Frame>,
    closed_tx: tokio::sync::oneshot::Sender<()>,
) {
    let (mut sink, mut stream) = ws.split();
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(30));
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;

            msg = stream.next() => match msg {
                Some(Ok(Message::Text(t))) => match Frame::from_text(&t) {
                    Ok(f) => {
                        if inbound_tx.send(f).await.is_err() { break; }
                    }
                    Err(e) => warn!(error=%e, raw=%t, "drop malformed frame"),
                },
                Some(Ok(Message::Binary(_))) => warn!("ignoring binary ws frame"),
                Some(Ok(Message::Ping(p))) => {
                    if sink.send(Message::Pong(p)).await.is_err() { break; }
                }
                Some(Ok(Message::Pong(_))) => {}
                Some(Ok(Message::Close(_))) => { debug!("server closed ws"); break; }
                Some(Ok(Message::Frame(_))) => {}
                Some(Err(e)) => { error!(error=%e, "ws read error"); break; }
                None => { debug!("ws stream ended"); break; }
            },

            out = outbound_rx.recv() => match out {
                Some(frame) => match frame.to_text() {
                    Ok(text) => {
                        if let Err(e) = sink.send(Message::Text(text)).await {
                            error!(error=%e, "ws write error");
                            break;
                        }
                    }
                    Err(e) => warn!(error=%e, "could not encode frame"),
                },
                None => { debug!("outbound channel closed"); break; }
            },

            _ = ping.tick() => {
                if sink.send(Message::Ping(Vec::new())).await.is_err() { break; }
            }
        }
    }

    let _ = sink.close().await;
    let _ = closed_tx.send(());
}
