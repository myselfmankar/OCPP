# OCPP 1.6J Edge Gateway (Rust)

Rust middleware that bridges multiple internal batteries (MQTT or gRPC) to a fixed
OCPP 1.6J CSMS over WebSocket. Each battery is represented as one ChargePoint
with its own WebSocket connection.

## Layout

```
crates/
  ocpp-protocol/        # Frames, messages, enums (no IO)
  ocpp-transport/       # WebSocket client, TLS, correlator, session
  ocpp-store/           # sled-backed offline queue + transaction state
  ocpp-adapter/         # Device trait + ChargePoint actor + CsmsHandler
  ocpp-internal-mqtt/   # Device impl over MQTT (rumqttc)
  ocpp-internal-grpc/   # Device impl over gRPC (tonic)
  ocpp-gateway/         # Binary: config + supervisor
```

## Run

```powershell
cargo run -p ocpp-gateway -- --config gateway.yaml
```

See `crates/ocpp-gateway/gateway.example.yaml` for a sample configuration.

## OCPP versioning

OCPP 1.6J is the only version implemented today. The transport and adapter layers
are generic over an `OcppVersion` trait so OCPP 2.0.1 can be added as a separate
message crate without rewriting transport or adapter.
