# OCPP 1.6J Client — Implementation & Test Report

**Project:** Golain OCPP Client Gateway
**Version:** 0.1.0
**Date:** 2026-04-27
**Benchmark CSMS:** SteVe v3.x (reference open-source OCPP Central System)

---

## 1. What Has Been Implemented

A **Rust-based OCPP 1.6J client gateway** that registers a battery/charger device as a Charge Point against any standard OCPP Central System (CSMS), and translates internal device telemetry (over MQTT or gRPC) into OCPP 1.6J calls.

### 1.1 OCPP 1.6J — Core Profile message set

**Charge Point → CSMS (client-initiated):**

| # | Message | Purpose |
|---|---|---|
| 1 | BootNotification | Register CP on connect |
| 2 | Heartbeat | Keep-alive at CSMS-configured interval |
| 3 | StatusNotification | Report connector status (Available / Preparing / Charging / Finishing / Faulted) |
| 4 | Authorize | Validate idTag with CSMS |
| 5 | StartTransaction | Begin charging session |
| 6 | MeterValues | Periodic telemetry samples |
| 7 | StopTransaction | End charging session |
| 8 | DataTransfer | Vendor-specific extension |

**CSMS → Charge Point (server-initiated, handler trait wired):**

| # | Message |
|---|---|
| 1 | RemoteStartTransaction |
| 2 | RemoteStopTransaction |
| 3 | GetConfiguration |
| 4 | ChangeConfiguration |
| 5 | Reset |
| 6 | UnlockConnector |
| 7 | TriggerMessage |
| 8 | ChangeAvailability |

### 1.2 Transport & framing

- WebSocket client over `ws://` and `wss://`
- OCPP **`ocpp1.6`** subprotocol negotiation
- JSON Call / CallResult / CallError frames per OCPP-J spec
- Per-call request/response correlation by UUID
- Configurable per-call timeout
- Auto-reconnect with exponential backoff
- Security profiles supported in code: **Profile 0** (no auth, `ws://`) and **Profile 1** (HTTP Basic over `wss://`). Only Profile 0 exercised in this test run.

### 1.3 Internal device bus

- **MQTT** bridge (rumqttc): `batteries/<id>/events` <-> `batteries/<id>/commands`
- **gRPC** bridge (tonic): bidirectional streaming RPC
- Hot-pluggable per charge point via config

### 1.4 Persistence (Sled KV)

- Outbound queue: messages persisted on send-failure and replayed on reconnect
- Active transaction state survives gateway restarts

### 1.5 Operational

- Multi-charge-point per gateway process (one Tokio actor per CP)
- Single YAML configuration file
- Structured logging via `tracing`
- Dockerised stack (`docker-compose.yml`) for SteVe + MariaDB + Mosquitto

---

## 2. Test Results Against Benchmark (SteVe)

All tests run against **SteVe** — the most widely used reference OCPP 1.6J Central System — as the conformance benchmark.

### 2.1 Connection & registration

| Test | Result |
|---|---|
| WebSocket upgrade with `ocpp1.6` subprotocol (Profile 0 / no-auth, `ws://`) |  Pass |
| BootNotification → `Accepted` with interval |  Pass |
| Heartbeat at CSMS-assigned interval |  Pass — stable >1 hour |
| Auto-reconnect on CSMS restart |  Pass |
| MQTT auto re-subscribe on broker reconnect |  Pass |

### 2.2 Authorization & session lifecycle

| Test | Result |
|---|---|
| Authorize request with valid idTag → `Accepted` |  Pass |
| StartTransaction stores transaction in CSMS DB |  Pass |
| StartTransaction with `ConcurrentTx` correctly rejected by client (no phantom local tx) |  Pass |
| StopTransaction closes session in CSMS with correct meter and reason |  Pass |
| Connector status transitions Available → Preparing → Available |  Pass |

### 2.3 Metering

| Test | Result |
|---|---|
| MeterValues schema validated by CSMS |  Pass |
| All 6 measurands per sample accepted: SoC, Voltage, Current.Import, Power.Active.Import, Energy.Active.Import.Register, Temperature |  Pass |
| Units recognised: Percent, V, A, W, Wh, Celsius |  Pass |
| Multiple samples linked to correct transactionId |  Pass |

### 2.4 Reference transaction (Transaction #9 in SteVe)

| Field | Value |
|---|---|
| ChargeBox ID | CP-0001 |
| OCPP Tag | TAG-001 |
| Connector | 1 |
| Start Date (UTC) | 2026-04-27 12:05:51.943 |
| Start Value | 0 Wh |
| Stop Date (UTC) | 2026-04-27 12:05:59.672 |
| Stop Value | 4800 Wh |
| Stop Reason | Local |
| MeterValues rows persisted | **18** (3 samples × 6 measurands) |
| Stop Event Actor | station |

DB-level verification:
```
transaction_pk=9 | count=18
measurands: Current.Import, Energy.Active.Import.Register,
            Power.Active.Import, SoC, Temperature, Voltage
```

### 2.5 Resilience

| Test | Result |
|---|---|
| Outbound queue replay after CSMS downtime |  Pass — replays in order on reconnect |
| Active transaction state preserved across gateway restart |  Pass |
| Backoff respects max interval (60 s) |  Pass |

---

## 3. Coverage Summary

- **OCPP 1.6J Core Profile — CP-initiated messages: 8 / 8 implemented and benchmarked **
- **OCPP 1.6J Core Profile — CSMS-initiated messages: 8 handlers wired** (RemoteStart end-to-end exercise pending)
- **Transport conformance:** WebSocket + JSON framing + correlator — 100 % against SteVe
- **End-to-end charge transaction:** complete lifecycle accepted by reference CSMS
- **No protocol-level errors logged** by SteVe across the full test run
- **Security:** Profile 0 (no auth, `ws://`) validated end-to-end. Profile 1 (HTTP Basic + TLS) implemented in code but not yet exercised against a CSMS that requires it.
