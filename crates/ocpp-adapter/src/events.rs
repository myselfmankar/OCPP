use chrono::{DateTime, Utc};
use ocpp_protocol::enums::{AvailabilityType, ChargePointErrorCode, ChargePointStatus, DiagnosticsStatus, FirmwareStatus, StopReason};
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
        reason: Option<StopReason>,
    },
    /// Periodic meter sample (gateway packages this into MeterValues).
    Meter {
        connector_id: i32,
        sample: MeterSample,
    },
    /// Status / fault from the battery.
    Status {
        connector_id: i32,
        status: ChargePointStatus,
        error_code: ChargePointErrorCode,
        info: Option<String>,
    },
    /// Heartbeat from the battery (informational; OCPP heartbeat is independent).
    Alive,
    /// Battery reports current firmware update status (maps to FirmwareStatusNotification).
    FirmwareStatus { status: FirmwareStatus },
    /// Battery reports current diagnostics upload status (maps to DiagnosticsStatusNotification).
    DiagnosticsStatus { status: DiagnosticsStatus },
}

use tokio::sync::oneshot;

/// Outcome of a device command.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceAck {
    Accepted,
    Rejected,
    Failed,
    NotSupported,
}

/// Commands the gateway sends to an internal battery.
#[derive(Serialize, Deserialize)] // Remove Clone as oneshot::Sender is not Clone
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceCommand {
    StartCharging {
        connector_id: Option<i32>,
        id_tag: String,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    StopCharging {
        transaction_id: i32,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    Unlock {
        connector_id: i32,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    Reboot {
        hard: bool,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    SetConfig {
        key: String,
        value: String,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    SetAvailability {
        connector_id: i32,
        availability_type: AvailabilityType,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    /// Ask device to start a firmware download-and-install sequence.
    UpdateFirmware {
        location: String,
        retrieve_date: chrono::DateTime<chrono::Utc>,
        retries: Option<i32>,
        retry_interval: Option<i32>,
        // No ack_tx: OCPP spec says respond immediately; status comes via FirmwareStatus events.
    },
    /// Ask device to collect and upload a diagnostics file.
    GetDiagnostics {
        location: String,
        retries: Option<i32>,
        retry_interval: Option<i32>,
        start_time: Option<chrono::DateTime<chrono::Utc>>,
        stop_time: Option<chrono::DateTime<chrono::Utc>>,
        /// Channel to receive the filename the device will upload (empty = none available).
        #[serde(skip)]
        file_name_tx: Option<tokio::sync::oneshot::Sender<Option<String>>>,
    },
    /// Ask device to clear its local authorization cache.
    ClearCache {
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    ReserveNow {
        connector_id: i32,
        expiry_date: DateTime<Utc>,
        id_tag: String,
        reservation_id: i32,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
    CancelReservation {
        reservation_id: i32,
        #[serde(skip)]
        ack_tx: Option<oneshot::Sender<DeviceAck>>,
    },
}

impl std::fmt::Debug for DeviceCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartCharging { connector_id, id_tag, .. } => f.debug_struct("StartCharging")
                .field("connector_id", connector_id).field("id_tag", id_tag).finish(),
            Self::StopCharging { transaction_id, .. } => f.debug_struct("StopCharging")
                .field("transaction_id", transaction_id).finish(),
            Self::Unlock { connector_id, .. } => f.debug_struct("Unlock")
                .field("connector_id", connector_id).finish(),
            Self::Reboot { hard, .. } => f.debug_struct("Reboot")
                .field("hard", hard).finish(),
            Self::SetConfig { key, value, .. } => f.debug_struct("SetConfig")
                .field("key", key).field("value", value).finish(),
            Self::SetAvailability { connector_id, availability_type, .. } => f.debug_struct("SetAvailability")
                .field("connector_id", connector_id).field("type", availability_type).finish(),
            Self::UpdateFirmware { location, retrieve_date, .. } => f.debug_struct("UpdateFirmware")
                .field("location", location).field("retrieve_date", retrieve_date).finish(),
            Self::GetDiagnostics { location, .. } => f.debug_struct("GetDiagnostics")
                .field("location", location).finish(),
            Self::ClearCache { .. } => f.debug_struct("ClearCache").finish(),
            Self::ReserveNow { connector_id, id_tag, reservation_id, .. } => f.debug_struct("ReserveNow")
                .field("connector_id", connector_id)
                .field("id_tag", id_tag)
                .field("reservation_id", reservation_id)
                .finish(),
            Self::CancelReservation { reservation_id, .. } => f.debug_struct("CancelReservation")
                .field("reservation_id", reservation_id)
                .finish(),
        }
    }
}
