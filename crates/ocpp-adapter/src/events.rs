use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Single meter sample reported by an internal battery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeterSample {
    pub timestamp: DateTime<Utc>,
    /// SoC %, 0..=100
    pub soc: Option<f32>,
    /// Voltage (V)
    pub voltage: Option<f32>,
    /// Current (A)
    pub current: Option<f32>,
    /// Active power (W)
    pub power_w: Option<f32>,
    /// Energy register (Wh) — used as MeterStart/MeterStop.
    pub energy_wh: Option<i32>,
    /// Pack temperature (°C)
    pub temperature_c: Option<f32>,
}

/// Events the gateway receives from an internal battery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceEvent {
    /// Connector physically connected and ready.
    Plugged { connector_id: i32 },
    /// Connector disconnected.
    Unplugged { connector_id: i32 },
    /// Battery wants to start a session: gateway should Authorize then StartTransaction.
    AuthorizeRequest {
        connector_id: i32,
        id_tag: String,
        meter_start: i32,
    },
    /// Charging session ended locally.
    SessionStopped {
        transaction_id: i32,
        meter_stop: i32,
        reason: Option<String>,
    },
    /// Periodic meter sample (gateway packages this into MeterValues).
    Meter {
        connector_id: i32,
        sample: MeterSample,
    },
    /// Status / fault from the battery.
    Status {
        connector_id: i32,
        status: String,        // ChargePointStatus name
        error_code: String,    // ChargePointErrorCode name
        info: Option<String>,
    },
    /// Heartbeat from the battery (informational; OCPP heartbeat is independent).
    Alive,
}

/// Commands the gateway sends to an internal battery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceCommand {
    StartCharging {
        connector_id: Option<i32>,
        id_tag: String,
    },
    StopCharging {
        transaction_id: i32,
    },
    Unlock {
        connector_id: i32,
    },
    Reboot {
        hard: bool,
    },
    SetConfig {
        key: String,
        value: String,
    },
}
