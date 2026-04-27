/// Marker trait for an OCPP version. The transport and adapter layers are
/// generic over `V: OcppVersion` so future versions (e.g. 2.0.1) can be added
/// as separate message crates without rewriting transport.
pub trait OcppVersion: Send + Sync + 'static {
    /// WebSocket subprotocol header (`Sec-WebSocket-Protocol`).
    fn subprotocol() -> &'static str;
}

/// OCPP 1.6 / OCPP-J 1.6.
pub struct Ocpp16;

impl OcppVersion for Ocpp16 {
    fn subprotocol() -> &'static str {
        "ocpp1.6"
    }
}
