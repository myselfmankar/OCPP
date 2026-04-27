//! OCPP 1.6 enumerations. Names and variants mirror the JSON schemas under
//! `docs/OCPP_1.6/schemas/json/`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegistrationStatus {
    Accepted,
    Pending,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorizationStatus {
    Accepted,
    Blocked,
    Expired,
    Invalid,
    ConcurrentTx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChargePointStatus {
    Available,
    Preparing,
    Charging,
    SuspendedEVSE,
    SuspendedEV,
    Finishing,
    Reserved,
    Unavailable,
    Faulted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChargePointErrorCode {
    ConnectorLockFailure,
    EVCommunicationError,
    GroundFailure,
    HighTemperature,
    InternalError,
    LocalListConflict,
    NoError,
    OtherError,
    OverCurrentFailure,
    OverVoltage,
    PowerMeterFailure,
    PowerSwitchFailure,
    ReaderFailure,
    ResetFailure,
    UnderVoltage,
    WeakSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteStartStopStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetType {
    Hard,
    Soft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationStatus {
    Accepted,
    Rejected,
    RebootRequired,
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnlockStatus {
    Unlocked,
    UnlockFailed,
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerMessageStatus {
    Accepted,
    Rejected,
    NotImplemented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageTrigger {
    BootNotification,
    DiagnosticsStatusNotification,
    FirmwareStatusNotification,
    Heartbeat,
    MeterValues,
    StatusNotification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataTransferStatus {
    Accepted,
    Rejected,
    UnknownMessageId,
    UnknownVendorId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    EmergencyStop,
    EVDisconnected,
    HardReset,
    Local,
    Other,
    PowerLoss,
    Reboot,
    Remote,
    SoftReset,
    UnlockCommand,
    DeAuthorized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadingContext {
    #[serde(rename = "Interruption.Begin")]
    InterruptionBegin,
    #[serde(rename = "Interruption.End")]
    InterruptionEnd,
    #[serde(rename = "Sample.Clock")]
    SampleClock,
    #[serde(rename = "Sample.Periodic")]
    SamplePeriodic,
    #[serde(rename = "Transaction.Begin")]
    TransactionBegin,
    #[serde(rename = "Transaction.End")]
    TransactionEnd,
    Trigger,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueFormat {
    Raw,
    SignedData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Measurand {
    #[serde(rename = "Energy.Active.Export.Register")] EnergyActiveExportRegister,
    #[serde(rename = "Energy.Active.Import.Register")] EnergyActiveImportRegister,
    #[serde(rename = "Energy.Reactive.Export.Register")] EnergyReactiveExportRegister,
    #[serde(rename = "Energy.Reactive.Import.Register")] EnergyReactiveImportRegister,
    #[serde(rename = "Energy.Active.Export.Interval")] EnergyActiveExportInterval,
    #[serde(rename = "Energy.Active.Import.Interval")] EnergyActiveImportInterval,
    #[serde(rename = "Energy.Reactive.Export.Interval")] EnergyReactiveExportInterval,
    #[serde(rename = "Energy.Reactive.Import.Interval")] EnergyReactiveImportInterval,
    #[serde(rename = "Power.Active.Export")] PowerActiveExport,
    #[serde(rename = "Power.Active.Import")] PowerActiveImport,
    #[serde(rename = "Power.Offered")] PowerOffered,
    #[serde(rename = "Power.Reactive.Export")] PowerReactiveExport,
    #[serde(rename = "Power.Reactive.Import")] PowerReactiveImport,
    #[serde(rename = "Power.Factor")] PowerFactor,
    #[serde(rename = "Current.Import")] CurrentImport,
    #[serde(rename = "Current.Export")] CurrentExport,
    #[serde(rename = "Current.Offered")] CurrentOffered,
    Voltage,
    Frequency,
    Temperature,
    SoC,
    RPM,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    L1, L2, L3, N,
    #[serde(rename = "L1-N")] L1N,
    #[serde(rename = "L2-N")] L2N,
    #[serde(rename = "L3-N")] L3N,
    #[serde(rename = "L1-L2")] L1L2,
    #[serde(rename = "L2-L3")] L2L3,
    #[serde(rename = "L3-L1")] L3L1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    Cable, EV, Inlet, Outlet, Body,
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitOfMeasure {
    Wh, kWh, varh, kvarh, W, kW, VA, kVA, var, kvar, A, V, K, Celcius, Fahrenheit, Percent,
}
