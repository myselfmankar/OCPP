//! Adapter layer: maps internal battery events <-> OCPP messages.
//!
//! The adapter owns one [`ChargePoint`] actor per battery. Each actor:
//! - drives a [`Session`](ocpp_transport::Session) to a CSMS,
//! - subscribes to [`DeviceEvent`]s from a [`Device`] (MQTT/gRPC backend),
//! - implements [`CsmsHandler`](ocpp_transport::CsmsHandler) to translate
//!   CSMS-initiated commands into [`DeviceCommand`]s.

pub mod charge_point;
pub(crate) mod connectivity;
pub mod device;
pub mod events;
pub mod handler;
pub(crate) mod metadata;
pub(crate) mod outbound;
pub(crate) mod transaction;

pub use charge_point::{ChargePoint, ChargePointConfig};
pub use device::{Device, DeviceError};
pub use events::{DeviceCommand, DeviceEvent, MeterSample};
pub use handler::AdapterHandler;
