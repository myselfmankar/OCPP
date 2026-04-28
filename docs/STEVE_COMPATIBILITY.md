# SteVe OCPP 1.6J Compatibility

This repository validates OCPP 1.6J compatibility at the protocol boundary:

- every OCPP 1.6J request and response payload is serialized from the Rust types;
- every payload is validated against the bundled OCPP 1.6J JSON schema files under
  `docs/OCPP_1.6/schemas/json`;
- every action is wrapped and parsed as OCPP-J WebSocket frames:
  `CALL [2, uniqueId, action, payload]` and `CALLRESULT [3, uniqueId, payload]`;
- the action registry covers all 28 OCPP 1.6J interactions.

Run the compatibility suite:

```bash
cargo test -p ocpp-protocol --test ocpp_16j_schema_compat
```

Run the full workspace suite:

```bash
cargo test
```

Run the one-shot local and live SteVe E2E suite:

```bash
make test
```

The `make test` target clones SteVe into `target/steve-e2e/steve`, builds and
starts SteVe plus MariaDB through Docker Compose, seeds a charge box/idTag/profile,
then runs the live E2E test against SteVe's OCPP-J endpoint with the `ocpp1.6`
WebSocket subprotocol:

```text
ws://<steve-host>:<port>/steve/websocket/CentralSystemService/<chargeBoxId>
```

By default the E2E script recreates the SteVe containers and database for a
clean run. Set `STEVE_E2E_CLEAN=0` to reuse the current stack while developing.
