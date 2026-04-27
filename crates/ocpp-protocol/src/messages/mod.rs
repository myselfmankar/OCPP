//! OCPP 1.6 message structs. Field names mirror the JSON schemas under
//! `docs/OCPP_1.6/schemas/json/` (camelCase via `serde(rename_all = "camelCase")`).

pub mod authorize;
pub mod boot_notification;
pub mod change_configuration;
pub mod data_transfer;
pub mod get_configuration;
pub mod heartbeat;
pub mod meter_values;
pub mod remote_start_transaction;
pub mod remote_stop_transaction;
pub mod reset;
pub mod start_transaction;
pub mod status_notification;
pub mod stop_transaction;
pub mod trigger_message;
pub mod unlock_connector;

pub use authorize::*;
pub use boot_notification::*;
pub use change_configuration::*;
pub use data_transfer::*;
pub use get_configuration::*;
pub use heartbeat::*;
pub use meter_values::*;
pub use remote_start_transaction::*;
pub use remote_stop_transaction::*;
pub use reset::*;
pub use start_transaction::*;
pub use status_notification::*;
pub use stop_transaction::*;
pub use trigger_message::*;
pub use unlock_connector::*;
