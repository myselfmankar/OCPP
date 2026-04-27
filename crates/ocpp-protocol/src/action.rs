use serde::{de::DeserializeOwned, Serialize};

/// A type that can be sent as the payload of an OCPP `CALL` (request).
pub trait OcppRequest: Serialize + DeserializeOwned + Send + Sync + 'static {
    type Response: OcppResponse;
    /// OCPP action name (e.g. `"BootNotification"`).
    const ACTION: &'static str;
}

/// A type that can be returned as the payload of an OCPP `CALLRESULT` (response).
pub trait OcppResponse: Serialize + DeserializeOwned + Send + Sync + 'static {}

/// Convenience enumeration of all actions known to this implementation.
/// Mostly used for routing incoming `CALL`s to typed handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Charge-point initiated
    BootNotification,
    Heartbeat,
    Authorize,
    StartTransaction,
    StopTransaction,
    MeterValues,
    StatusNotification,
    DataTransfer,
    // CSMS initiated
    RemoteStartTransaction,
    RemoteStopTransaction,
    TriggerMessage,
    Reset,
    ChangeConfiguration,
    GetConfiguration,
    UnlockConnector,
    ChangeAvailability,
    // Firmware management
    UpdateFirmware,
    FirmwareStatusNotification,
    // Diagnostics
    GetDiagnostics,
    DiagnosticsStatusNotification,
    // Local authorization
    ClearCache,
    GetLocalListVersion,
    SendLocalList,
    // Reservation
    ReserveNow,
    CancelReservation,
    // Smart charging
    SetChargingProfile,
    ClearChargingProfile,
    GetCompositeSchedule,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::BootNotification => "BootNotification",
            Action::Heartbeat => "Heartbeat",
            Action::Authorize => "Authorize",
            Action::StartTransaction => "StartTransaction",
            Action::StopTransaction => "StopTransaction",
            Action::MeterValues => "MeterValues",
            Action::StatusNotification => "StatusNotification",
            Action::DataTransfer => "DataTransfer",
            Action::RemoteStartTransaction => "RemoteStartTransaction",
            Action::RemoteStopTransaction => "RemoteStopTransaction",
            Action::TriggerMessage => "TriggerMessage",
            Action::Reset => "Reset",
            Action::ChangeConfiguration => "ChangeConfiguration",
            Action::GetConfiguration => "GetConfiguration",
            Action::UnlockConnector => "UnlockConnector",
            Action::ChangeAvailability => "ChangeAvailability",
            Action::UpdateFirmware => "UpdateFirmware",
            Action::FirmwareStatusNotification => "FirmwareStatusNotification",
            Action::GetDiagnostics => "GetDiagnostics",
            Action::DiagnosticsStatusNotification => "DiagnosticsStatusNotification",
            Action::ClearCache => "ClearCache",
            Action::GetLocalListVersion => "GetLocalListVersion",
            Action::SendLocalList => "SendLocalList",
            Action::ReserveNow => "ReserveNow",
            Action::CancelReservation => "CancelReservation",
            Action::SetChargingProfile => "SetChargingProfile",
            Action::ClearChargingProfile => "ClearChargingProfile",
            Action::GetCompositeSchedule => "GetCompositeSchedule",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "BootNotification" => Action::BootNotification,
            "Heartbeat" => Action::Heartbeat,
            "Authorize" => Action::Authorize,
            "StartTransaction" => Action::StartTransaction,
            "StopTransaction" => Action::StopTransaction,
            "MeterValues" => Action::MeterValues,
            "StatusNotification" => Action::StatusNotification,
            "DataTransfer" => Action::DataTransfer,
            "RemoteStartTransaction" => Action::RemoteStartTransaction,
            "RemoteStopTransaction" => Action::RemoteStopTransaction,
            "TriggerMessage" => Action::TriggerMessage,
            "Reset" => Action::Reset,
            "ChangeConfiguration" => Action::ChangeConfiguration,
            "GetConfiguration" => Action::GetConfiguration,
            "UnlockConnector" => Action::UnlockConnector,
            "ChangeAvailability" => Action::ChangeAvailability,
            "UpdateFirmware" => Action::UpdateFirmware,
            "FirmwareStatusNotification" => Action::FirmwareStatusNotification,
            "GetDiagnostics" => Action::GetDiagnostics,
            "DiagnosticsStatusNotification" => Action::DiagnosticsStatusNotification,
            "ClearCache" => Action::ClearCache,
            "GetLocalListVersion" => Action::GetLocalListVersion,
            "SendLocalList" => Action::SendLocalList,
            "ReserveNow" => Action::ReserveNow,
            "CancelReservation" => Action::CancelReservation,
            "SetChargingProfile" => Action::SetChargingProfile,
            "ClearChargingProfile" => Action::ClearChargingProfile,
            "GetCompositeSchedule" => Action::GetCompositeSchedule,
            _ => return None,
        })
    }
}
