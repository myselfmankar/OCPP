# OCPP Client Gateway

A Rust OCPP client (Charge Point side) that connects internal battery / EVSE
devices to an OCPP Central System (CSMS) over WebSocket.

## Supported OCPP versions

| Version | Status |
| ------- | ------ |
| OCPP 1.6J (JSON over WebSocket) | Implemented |
| OCPP 2.0.1 | Planned — transport and adapter layers are version-agnostic, 2.0.1 will land as an additional message crate. |

## Run

```bash
cargo run -p ocpp-gateway -- --config gateway.yaml
```

See [crates/ocpp-gateway/gateway.example.yaml](crates/ocpp-gateway/gateway.example.yaml) for a sample configuration.
