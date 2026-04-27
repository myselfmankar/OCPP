# OCPP Client Review Findings And Critique

Date: 2026-04-27

This document captures the critical review findings from a code review of the OCPP client gateway implementation. The main theme is that the project has a sensible crate layout, but several reliability and protocol-compliance claims are ahead of the implementation.

## Executive Summary

The codebase is structured in a reasonable way: protocol types, transport, persistence, adapter, internal device bridges, and gateway supervision are separated into crates. That is a good starting point.

The current implementation is not yet production-grade as an OCPP client. The most serious issues are around durability and truthfulness of state: queued messages are discarded on reconnect, `StopTransaction` can be lost while local transaction state is deleted, and some CSMS commands are acknowledged as successful before they are actually performed.

The repository's existing `REVIEW.md` overstates coverage. In particular, it claims outbound replay and broader Core Profile support that the code does not currently provide.

## P0 Findings

### 1. Persisted outbound queue is dropped on reconnect

Location: `crates/ocpp-adapter/src/charge_point.rs:373-389`

`replay_queue` iterates the persisted queue and immediately acknowledges each entry without sending it through the OCPP session. This silently discards any queued `StatusNotification`, `MeterValues`, or `StopTransaction` after reconnect.

Impact:

- Offline durability is effectively broken.
- CSMS can miss important transaction and metering events.
- The implementation contradicts the persistence/replay claims in `REVIEW.md`.

Recommended fix:

- Replace ack-and-drop replay with real dispatch by `action`.
- Rehydrate each `PendingCall` into the correct typed request.
- Send in FIFO order.
- Acknowledge only after a successful `CALLRESULT`.
- Stop replay at the first failure to preserve ordering.

### 2. Transaction state is removed before `StopTransaction` is durably delivered

Location: `crates/ocpp-adapter/src/charge_point.rs:251-271`

On `DeviceEvent::SessionStopped`, the actor calls `send_or_queue` for `StopTransaction`, then immediately removes the active transaction from persistent state. If the call failed and was only queued, the local store still forgets the transaction.

Impact:

- A disconnect at session end can leave the CSMS transaction open.
- Local restart/reconnect cannot recover the transaction because the state was already deleted.
- This is the highest-risk lifecycle bug in the adapter.

Recommended fix:

- Make `send_or_queue` return whether the call was actually delivered.
- Remove active transaction state only after `StopTransaction` succeeds.
- If queued, retain enough state to retry and reconcile later.
- Consider storing pending terminal transaction state separately from active charging state.

## P1 Findings

### 3. `TriggerMessage` is acknowledged without any execution path

Location: `crates/ocpp-adapter/src/handler.rs:72-82`

The handler returns `TriggerMessageStatus::Accepted`, but there is no side channel or actor command path that causes the requested message to be emitted.

Impact:

- CSMS receives a success response for work that never happens.
- `TriggerMessage` conformance is only cosmetic.

Recommended fix:

- Add an internal command channel from `AdapterHandler` to the `ChargePoint` actor.
- Implement actual behavior for supported triggers such as `BootNotification`, `Heartbeat`, `StatusNotification`, and `MeterValues`.
- Return `NotImplemented` or `Rejected` for unsupported triggers.

### 4. `UnlockConnector` reports success before unlock outcome is known

Location: `crates/ocpp-adapter/src/handler.rs:118-129`

The handler returns `UnlockStatus::Unlocked` after forwarding the command to the device. OCPP expects this status to reflect the unlock result, not command delivery.

Impact:

- CSMS can be told the connector unlocked even if the device fails.
- Device/backend errors after command dispatch are not represented in the OCPP response.

Recommended fix:

- Add a command acknowledgement path from the device backend.
- Return `Unlocked`, `UnlockFailed`, or `NotSupported` based on the actual device result.
- If the backend cannot provide synchronous acknowledgement, return a conservative status rather than unconditional success.

### 5. WebSocket subprotocol mismatch is only warned, not rejected

Location: `crates/ocpp-transport/src/ws_client.rs:43-51`

The transport checks `Sec-WebSocket-Protocol`, but it only logs a warning when the server did not select the expected OCPP subprotocol.

Impact:

- A configuration or handshake error can turn into later protocol failures.
- The session can proceed on a connection that is not guaranteed to be OCPP 1.6J.

Recommended fix:

- Treat missing or wrong subprotocol selection as a connection error.
- Return a dedicated `TransportError` variant so operators can diagnose it clearly.

### 6. Invalid device status values are silently coerced to healthy defaults

Location: `crates/ocpp-adapter/src/charge_point.rs:298-305`

Device-provided `status` and `error_code` strings are parsed with fallback defaults of `Available` and `NoError`.

Impact:

- Bad backend data can hide real faults.
- Invalid device messages can make the charge point appear healthy.
- Contract errors between the gateway and device layer are difficult to detect.

Recommended fix:

- Make `DeviceEvent::Status` use typed `ChargePointStatus` and `ChargePointErrorCode` directly, or validate at deserialization.
- Reject/log invalid values loudly.
- Avoid defaulting to healthy states on parse failure.

## Missing OCPP Coverage

The bundled OCPP 1.6 schemas include actions that are not represented in `crates/ocpp-protocol/src/action.rs` or the message modules.

Missing actions:

- `CancelReservation`
- `ChangeAvailability`
- `ClearCache`
- `ClearChargingProfile`
- `DiagnosticsStatusNotification`
- `FirmwareStatusNotification`
- `GetCompositeSchedule`
- `GetDiagnostics`
- `GetLocalListVersion`
- `ReserveNow`
- `SendLocalList`
- `SetChargingProfile`
- `UpdateFirmware`

Important correction:

- `REVIEW.md` claims `ChangeAvailability` is wired, but the code does not implement it.
- `REVIEW.md` claims outbound queue replay passes, but the current replay code acknowledges and drops queued entries.

## Design Critique

### Crate boundaries are good, but the actor is too broad

The workspace split is sensible:

- `ocpp-protocol` owns frames, actions, enums, and message structs.
- `ocpp-transport` owns WebSocket IO, correlation, and dispatch.
- `ocpp-store` owns persisted state and queues.
- `ocpp-adapter` translates device events and CSMS commands.
- `ocpp-gateway` wires configuration and supervision.

The weak point is `ChargePoint` in `crates/ocpp-adapter/src/charge_point.rs`. It currently owns reconnection, boot flow, heartbeat scheduling, transaction state, outbound queueing, and device event translation. That concentration is where the durability bugs came from.

Recommended direction:

- Split transaction lifecycle handling out of the actor.
- Split outbound durable delivery into its own component.
- Keep the actor as orchestration glue rather than the owner of every state transition.

### The version-agnostic claim is overstated

`Session<V: OcppVersion>` is generic, but the actual routing and adapter logic are OCPP 1.6-specific:

- `Action` is a concrete OCPP 1.6 action enum.
- `CsmsHandler` methods are concrete OCPP 1.6 message types.
- `AdapterHandler` directly imports OCPP 1.6 messages and enums.
- `ChargePoint` is hard-coded to `Session<Ocpp16>`.

This is not a bad OCPP 1.6 design, but OCPP 2.0.1 will not be added by simply dropping in another message crate. The dispatch and adapter boundaries would need to become version-aware too.

Recommended direction:

- Be explicit that the current adapter is OCPP 1.6J-specific.
- If 2.0.1 is a real roadmap item, introduce version-specific adapter modules rather than pretending the current handler trait is version-neutral.

### Command delivery is confused with command completion

Several CSMS handlers treat "message delivered to internal device bus" as equivalent to "device performed the requested command." That is not accurate for OCPP operations whose response status represents an outcome.

Examples:

- `RemoteStartTransaction` returns `Accepted` once the command is sent to the device.
- `RemoteStopTransaction` returns `Accepted` once the command is sent to the device.
- `UnlockConnector` returns `Unlocked` once the command is sent to the device.
- `ChangeConfiguration` returns `Accepted` once the command is sent to the device.

Recommended direction:

- Model device command acknowledgements explicitly.
- Use timeout-bounded request/response semantics for commands that need immediate OCPP response status.
- Return conservative responses when the backend cannot prove completion.

### Internal event schema is too loose

`DeviceEvent` uses plain strings for protocol concepts such as status, error code, and stop reason. That pushes validation into ad hoc parsing at the adapter boundary.

Recommended direction:

- Use OCPP protocol enums directly where the internal device contract is intentionally OCPP-shaped.
- If the device contract should remain independent, define internal typed enums and map them explicitly to OCPP enums.
- Treat unknown values as backend contract errors, not healthy defaults.

## Test Coverage Gaps

`cargo test` currently passes, but the in-tree test coverage is minimal. It only exercises frame round-tripping in `ocpp-protocol`.

Missing high-value tests:

- Queue replay sends persisted calls in FIFO order.
- Queue replay only acknowledges entries after successful delivery.
- Failed `StopTransaction` does not remove active transaction state.
- Reconnect resumes active transaction state safely.
- `TriggerMessage` causes the requested outbound message or returns the correct unsupported status.
- WebSocket connection fails on missing or wrong OCPP subprotocol.
- Invalid device status/error values are rejected or surfaced.
- CSMS-initiated command handlers return status based on device acknowledgement.

## Verification Notes

Commands run during review:

```sh
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Results:

- `cargo test` passed.
- Only 3 unit tests ran, all in the protocol frame module.
- `cargo clippy --all-targets --all-features -- -D warnings` failed on `clippy::result_large_err` in `crates/ocpp-transport/src/tls.rs`.

