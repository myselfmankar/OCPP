# Walkthrough: Phase 4 Refactor & Phase 5 Advanced Features

I have completed the Phase 4 architectural refactor and implemented key Phase 5 features ($DataTransfer$ and Smart Charging Engine). The gateway is now more modular, stateful, and supports advanced charging logic.

## Key Changes

### 1. Specialized Managers
The `ChargePoint` actor was refactored into three specialized managers:
- **[ConnectivityManager](file:///d:/Golain/OCPP/crates/ocpp-adapter/src/connectivity.rs)**: Encapsulates `BootNotification` sequence and offline message queue replay.
- **[TransactionManager](file:///d:/Golain/OCPP/crates/ocpp-adapter/src/transaction.rs)**: Manages transaction states, meter value generation, and **Smart Charging** evaluation.
- **[MetadataManager](file:///d:/Golain/OCPP/crates/ocpp-adapter/src/metadata.rs)**: Handles `StatusNotification`, `Heartbeat`, and provides access to persistent stores.

### 2. Persistent Storage Layer
Expanded the `ocpp-store` crate to support persistence for core and smart charging features:
- **AuthStore**: Stores Local Authorization Lists and provides version tracking.
- **ConfigStore**: Stores configuration keys persistently.
- **ProfileStore**: Stores Smart Charging profiles per connector.

### 3. Smart Charging Engine
- Implemented a new evaluation engine in `TransactionManager` that calculates power limits based on active `ChargingProfiles`.
- Profiles are persisted across restarts, allowing the gateway to maintain charging limits even when offline.

### 4. DataTransfer Support
- Added `DataTransfer` to `DeviceCommand` and `AdapterHandler`.
- This allows CSMS and device-specific extensions to communicate through the gateway transparently.

## Verification Results

### Build & Stability
- Successfully resolved multiple Internal Compiler Errors (ICE) in the `rustc 1.95.0` toolchain.
- Verified the complete build with `cargo check` (Exit Code 0).

### Features Implemented
- **SetChargingProfile**: Persists profiles and activates the evaluation engine.
- **DataTransfer**: Routes vendor-specific messages to/from the device.
- **GetConfiguration/ChangeConfiguration**: Fully backed by `ConfigStore`.
- **SendLocalList**: Fully backed by `AuthStore`.

## Next Steps
- Implement `ClearChargingProfile` logic.
- Add unit tests for the `SmartChargingEngine` evaluation logic (absolute vs relative timing).
- Integrate `DataTransfer` with specific hardware plugins if available.
