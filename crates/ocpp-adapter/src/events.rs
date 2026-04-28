use chrono::{DateTime, Utc};
use ocpp_protocol::enums::{
    AvailabilityType, ChargePointErrorCode, ChargePointStatus, DataTransferStatus,
    DiagnosticsStatus, FirmwareStatus, StopReason,
};
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
    /// Response to a command previously sent by this gateway.
    CommandAck {
        command_id: String,
        status: DeviceAck,
    },
    /// Response to a GetDiagnostics command with the file name that will be uploaded.
    DiagnosticsFile {
        command_id: String,
        file_name: Option<String>,
    },
    /// Response to a DataTransfer command.
    DataTransferResult {
        command_id: String,
        status: DataTransferStatus,
        data: Option<String>,
    },
}

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
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceCommand {
    StartCharging {
        command_id: String,
        connector_id: Option<i32>,
        id_tag: String,
    },
    StopCharging {
        command_id: String,
        transaction_id: i32,
    },
    Unlock {
        command_id: String,
        connector_id: i32,
    },
    Reboot {
        command_id: String,
        hard: bool,
    },
    SetConfig {
        command_id: String,
        key: String,
        value: String,
    },
    SetAvailability {
        command_id: String,
        connector_id: i32,
        availability_type: AvailabilityType,
    },
    /// Ask device to start a firmware download-and-install sequence.
    UpdateFirmware {
        location: String,
        retrieve_date: chrono::DateTime<chrono::Utc>,
        retries: Option<i32>,
        retry_interval: Option<i32>,
        // OCPP responds immediately; progress comes via FirmwareStatus events.
    },
    /// Ask device to collect and upload a diagnostics file.
    GetDiagnostics {
        command_id: String,
        location: String,
        retries: Option<i32>,
        retry_interval: Option<i32>,
        start_time: Option<chrono::DateTime<chrono::Utc>>,
        stop_time: Option<chrono::DateTime<chrono::Utc>>,
    },
    /// Ask device to clear its local authorization cache.
    ClearCache {
        command_id: String,
    },
    ReserveNow {
        command_id: String,
        connector_id: i32,
        expiry_date: DateTime<Utc>,
        id_tag: String,
        reservation_id: i32,
    },
    CancelReservation {
        command_id: String,
        reservation_id: i32,
    },
    DataTransfer {
        command_id: String,
        vendor_id: String,
        message_id: Option<String>,
        data: Option<String>,
    },
}

impl std::fmt::Debug for DeviceCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StartCharging {
                command_id,
                connector_id,
                id_tag,
            } => f
                .debug_struct("StartCharging")
                .field("command_id", command_id)
                .field("connector_id", connector_id)
                .field("id_tag", id_tag)
                .finish(),
            Self::StopCharging {
                command_id,
                transaction_id,
            } => f
                .debug_struct("StopCharging")
                .field("command_id", command_id)
                .field("transaction_id", transaction_id)
                .finish(),
            Self::Unlock {
                command_id,
                connector_id,
            } => f
                .debug_struct("Unlock")
                .field("command_id", command_id)
                .field("connector_id", connector_id)
                .finish(),
            Self::Reboot { command_id, hard } => f
                .debug_struct("Reboot")
                .field("command_id", command_id)
                .field("hard", hard)
                .finish(),
            Self::SetConfig {
                command_id,
                key,
                value,
            } => f
                .debug_struct("SetConfig")
                .field("command_id", command_id)
                .field("key", key)
                .field("value", value)
                .finish(),
            Self::SetAvailability {
                command_id,
                connector_id,
                availability_type,
            } => f
                .debug_struct("SetAvailability")
                .field("command_id", command_id)
                .field("connector_id", connector_id)
                .field("type", availability_type)
                .finish(),
            Self::UpdateFirmware {
                location,
                retrieve_date,
                ..
            } => f
                .debug_struct("UpdateFirmware")
                .field("location", location)
                .field("retrieve_date", retrieve_date)
                .finish(),
            Self::GetDiagnostics {
                command_id,
                location,
                ..
            } => f
                .debug_struct("GetDiagnostics")
                .field("command_id", command_id)
                .field("location", location)
                .finish(),
            Self::ClearCache { command_id } => f
                .debug_struct("ClearCache")
                .field("command_id", command_id)
                .finish(),
            Self::ReserveNow {
                command_id,
                connector_id,
                id_tag,
                reservation_id,
                ..
            } => f
                .debug_struct("ReserveNow")
                .field("command_id", command_id)
                .field("connector_id", connector_id)
                .field("id_tag", id_tag)
                .field("reservation_id", reservation_id)
                .finish(),
            Self::CancelReservation {
                command_id,
                reservation_id,
            } => f
                .debug_struct("CancelReservation")
                .field("command_id", command_id)
                .field("reservation_id", reservation_id)
                .finish(),
            Self::DataTransfer {
                command_id,
                vendor_id,
                message_id,
                ..
            } => f
                .debug_struct("DataTransfer")
                .field("command_id", command_id)
                .field("vendor_id", vendor_id)
                .field("message_id", message_id)
                .finish(),
        }
    }
}
