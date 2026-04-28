use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use ocpp_protocol::enums::*;
use ocpp_protocol::messages::*;
use ocpp_protocol::Ocpp16;
use ocpp_transport::dispatcher::HandlerResult;
use ocpp_transport::{CsmsHandler, SecurityProfile, Session, SessionConfig};
use serde_json::json;
use tokio::sync::Mutex;

const CP_ID: &str = "CP-0001";
const ID_TAG: &str = "E2E-TAG";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn steve_ocpp16j_live_e2e_all_interactions() {
    if std::env::var("STEVE_E2E").as_deref() != Ok("1") {
        eprintln!("skipping live SteVe E2E; set STEVE_E2E=1");
        return;
    }

    let ws_url = std::env::var("STEVE_WS_URL").unwrap_or_else(|_| {
        format!("ws://localhost:18180/steve/websocket/CentralSystemService/{CP_ID}")
    });

    let handler = Arc::new(E2eHandler::default());
    let mut cfg = SessionConfig::new(ws_url.parse().unwrap(), SecurityProfile::Plain);
    cfg.call_timeout = Duration::from_secs(20);
    let (session, _closed) = Session::<Ocpp16>::connect(cfg, handler.clone())
        .await
        .unwrap();

    cp_initiated_interactions(&session).await;
    csms_initiated_interactions(handler.as_ref()).await;
}

async fn cp_initiated_interactions(session: &Session<Ocpp16>) {
    let boot = session
        .call(BootNotificationRequest {
            charge_point_vendor: "Codex".into(),
            charge_point_model: "E2E-Simulator".into(),
            charge_point_serial_number: Some(CP_ID.into()),
            charge_box_serial_number: Some(CP_ID.into()),
            firmware_version: Some("e2e".into()),
            iccid: None,
            imsi: None,
            meter_type: Some("simulated".into()),
            meter_serial_number: Some("meter-e2e".into()),
        })
        .await
        .unwrap();
    assert!(matches!(boot.status, RegistrationStatus::Accepted));

    session.call(HeartbeatRequest {}).await.unwrap();

    session
        .call(StatusNotificationRequest {
            connector_id: 1,
            error_code: ChargePointErrorCode::NoError,
            info: Some("e2e online".into()),
            status: ChargePointStatus::Available,
            timestamp: Some(now()),
            vendor_id: None,
            vendor_error_code: None,
        })
        .await
        .unwrap();

    let auth = session
        .call(AuthorizeRequest {
            id_tag: ID_TAG.into(),
        })
        .await
        .unwrap();
    assert!(matches!(
        auth.id_tag_info.status,
        AuthorizationStatus::Accepted
    ));

    let start = session
        .call(StartTransactionRequest {
            connector_id: 1,
            id_tag: ID_TAG.into(),
            meter_start: 100,
            reservation_id: None,
            timestamp: now(),
        })
        .await
        .unwrap();
    assert!(matches!(
        start.id_tag_info.status,
        AuthorizationStatus::Accepted
    ));

    session
        .call(MeterValuesRequest {
            connector_id: 1,
            transaction_id: Some(start.transaction_id),
            meter_value: vec![meter_value("125")],
        })
        .await
        .unwrap();

    session
        .call(DataTransferRequest {
            vendor_id: "Codex".into(),
            message_id: Some("e2e".into()),
            data: Some("hello steve".into()),
        })
        .await
        .unwrap();

    session
        .call(DiagnosticsStatusNotificationRequest {
            status: DiagnosticsStatus::Uploaded,
        })
        .await
        .unwrap();

    session
        .call(FirmwareStatusNotificationRequest {
            status: FirmwareStatus::Installed,
        })
        .await
        .unwrap();

    steve_operation(
        "RemoteStopTransaction",
        json!({
            "chargeBoxIdList": [CP_ID],
            "transactionId": start.transaction_id
        }),
    )
    .await;

    session
        .call(StopTransactionRequest {
            id_tag: Some(ID_TAG.into()),
            meter_stop: 150,
            timestamp: now(),
            transaction_id: start.transaction_id,
            reason: Some(StopReason::Local),
            transaction_data: Some(vec![meter_value("150")]),
        })
        .await
        .unwrap();
}

async fn csms_initiated_interactions(handler: &E2eHandler) {
    steve_operation(
        "ChangeAvailability",
        json!({
            "chargeBoxIdList": [CP_ID],
            "connectorId": 1,
            "availType": "Operative"
        }),
    )
    .await;

    steve_operation(
        "ChangeConfiguration",
        json!({
            "chargeBoxIdList": [CP_ID],
            "keyType": "CUSTOM",
            "customConfKey": "E2EKey",
            "value": "E2EValue"
        }),
    )
    .await;

    steve_operation("ClearCache", json!({ "chargeBoxIdList": [CP_ID] })).await;

    steve_operation(
        "GetDiagnostics",
        json!({
            "chargeBoxIdList": [CP_ID],
            "location": "https://example.com/diagnostics",
            "retries": 1,
            "retryInterval": 1
        }),
    )
    .await;

    steve_operation(
        "RemoteStartTransaction",
        json!({
            "chargeBoxIdList": [CP_ID],
            "connectorId": 1,
            "idTag": ID_TAG
        }),
    )
    .await;

    steve_operation(
        "Reset",
        json!({
            "chargeBoxIdList": [CP_ID],
            "resetType": "Soft"
        }),
    )
    .await;

    steve_operation(
        "UnlockConnector",
        json!({
            "chargeBoxIdList": [CP_ID],
            "connectorId": 1
        }),
    )
    .await;

    steve_operation(
        "UpdateFirmware",
        json!({
            "chargeBoxIdList": [CP_ID],
            "location": "https://example.com/firmware.bin",
            "retrieveDateTime": "2037-01-01T00:00:00.000Z",
            "retries": 1,
            "retryInterval": 1
        }),
    )
    .await;

    steve_operation(
        "ReserveNow",
        json!({
            "chargeBoxIdList": [CP_ID],
            "connectorId": 1,
            "expiry": "2037-01-01T00:00:00.000Z",
            "idTag": ID_TAG
        }),
    )
    .await;

    steve_operation(
        "CancelReservation",
        json!({
            "chargeBoxIdList": [CP_ID],
            "reservationId": 1
        }),
    )
    .await;

    steve_operation(
        "DataTransfer",
        json!({
            "chargeBoxIdList": [CP_ID],
            "vendorId": "Codex",
            "messageId": "e2e",
            "data": "from steve"
        }),
    )
    .await;

    steve_operation(
        "GetConfiguration",
        json!({
            "chargeBoxIdList": [CP_ID],
            "commaSeparatedCustomConfKeys": "E2EKey"
        }),
    )
    .await;

    steve_operation("GetLocalListVersion", json!({ "chargeBoxIdList": [CP_ID] })).await;

    steve_operation(
        "SendLocalList",
        json!({
            "chargeBoxIdList": [CP_ID],
            "listVersion": 1,
            "updateType": "Full",
            "sendEmptyListWhenFull": true
        }),
    )
    .await;

    steve_operation(
        "TriggerMessage",
        json!({
            "chargeBoxIdList": [CP_ID],
            "triggerMessage": "Heartbeat",
            "connectorId": 1
        }),
    )
    .await;

    steve_operation(
        "GetCompositeSchedule",
        json!({
            "chargeBoxIdList": [CP_ID],
            "connectorId": 1,
            "durationInSeconds": 300,
            "chargingRateUnit": "A"
        }),
    )
    .await;

    steve_operation(
        "ClearChargingProfile",
        json!({
            "chargeBoxIdList": [CP_ID],
            "filterType": "OtherParameters",
            "connectorId": 1,
            "chargingProfilePurpose": "TxDefaultProfile",
            "stackLevel": 1
        }),
    )
    .await;

    steve_operation(
        "SetChargingProfile",
        json!({
            "chargeBoxIdList": [CP_ID],
            "connectorId": 1,
            "chargingProfilePk": 1
        }),
    )
    .await;

    for action in [
        "RemoteStopTransaction",
        "ChangeAvailability",
        "ChangeConfiguration",
        "ClearCache",
        "GetDiagnostics",
        "RemoteStartTransaction",
        "Reset",
        "UnlockConnector",
        "UpdateFirmware",
        "ReserveNow",
        "CancelReservation",
        "DataTransfer",
        "GetConfiguration",
        "GetLocalListVersion",
        "SendLocalList",
        "TriggerMessage",
        "GetCompositeSchedule",
        "ClearChargingProfile",
        "SetChargingProfile",
    ] {
        handler.assert_seen(action).await;
    }
}

async fn steve_operation(action: &str, payload: serde_json::Value) {
    let base =
        std::env::var("STEVE_HTTP_URL").unwrap_or_else(|_| "http://localhost:18180/steve".into());
    let key_value = std::env::var("STEVE_API_KEY_VALUE").unwrap_or_else(|_| "E2E-SECRET".into());
    let url = format!("{base}/api/v1/operations/{action}");
    let body = payload.to_string();

    let output = tokio::task::spawn_blocking(move || {
        Command::new("curl")
            .args([
                "-sS",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-u",
                &format!("admin:{key_value}"),
                "--data",
                &body,
                "-w",
                "\n%{http_code}",
                &url,
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let (body, status) = stdout
        .rsplit_once('\n')
        .unwrap_or_else(|| panic!("SteVe operation {action} returned malformed curl output"));

    assert!(
        output.status.success() && status == "200",
        "SteVe operation {action} failed with HTTP {status}\nbody:\n{body}\nstderr:\n{stderr}"
    );
    let response: serde_json::Value = serde_json::from_str(body)
        .unwrap_or_else(|e| panic!("SteVe operation {action} returned non-JSON body: {e}\n{body}"));
    let errors = response
        .get("errorResponses")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| {
            panic!("SteVe operation {action} response has no errorResponses array\n{body}")
        });
    let exceptions = response
        .get("exceptions")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| {
            panic!("SteVe operation {action} response has no exceptions array\n{body}")
        });
    assert!(
        errors.is_empty() && exceptions.is_empty(),
        "SteVe operation {action} reported a failed OCPP task\nbody:\n{body}"
    );
}

#[derive(Default)]
struct E2eHandler {
    seen: Mutex<Vec<&'static str>>,
}

impl E2eHandler {
    async fn mark(&self, action: &'static str) {
        self.seen.lock().await.push(action);
    }

    async fn assert_seen(&self, action: &str) {
        let seen = self.seen.lock().await;
        assert!(
            seen.iter().any(|a| *a == action),
            "SteVe never sent {action}"
        );
    }
}

#[async_trait]
impl CsmsHandler for E2eHandler {
    async fn remote_start_transaction(
        &self,
        _req: RemoteStartTransactionRequest,
    ) -> HandlerResult<RemoteStartTransactionResponse> {
        self.mark("RemoteStartTransaction").await;
        Ok(RemoteStartTransactionResponse {
            status: RemoteStartStopStatus::Accepted,
        })
    }

    async fn remote_stop_transaction(
        &self,
        _req: RemoteStopTransactionRequest,
    ) -> HandlerResult<RemoteStopTransactionResponse> {
        self.mark("RemoteStopTransaction").await;
        Ok(RemoteStopTransactionResponse {
            status: RemoteStartStopStatus::Accepted,
        })
    }

    async fn trigger_message(
        &self,
        _req: TriggerMessageRequest,
    ) -> HandlerResult<TriggerMessageResponse> {
        self.mark("TriggerMessage").await;
        Ok(TriggerMessageResponse {
            status: TriggerMessageStatus::Accepted,
        })
    }

    async fn reset(&self, _req: ResetRequest) -> HandlerResult<ResetResponse> {
        self.mark("Reset").await;
        Ok(ResetResponse {
            status: ResetStatus::Accepted,
        })
    }

    async fn change_configuration(
        &self,
        _req: ChangeConfigurationRequest,
    ) -> HandlerResult<ChangeConfigurationResponse> {
        self.mark("ChangeConfiguration").await;
        Ok(ChangeConfigurationResponse {
            status: ConfigurationStatus::Accepted,
        })
    }

    async fn get_configuration(
        &self,
        _req: GetConfigurationRequest,
    ) -> HandlerResult<GetConfigurationResponse> {
        self.mark("GetConfiguration").await;
        Ok(GetConfigurationResponse {
            configuration_key: Some(vec![ConfigurationKey {
                key: "E2EKey".into(),
                readonly: false,
                value: Some("E2EValue".into()),
            }]),
            unknown_key: None,
        })
    }

    async fn unlock_connector(
        &self,
        _req: UnlockConnectorRequest,
    ) -> HandlerResult<UnlockConnectorResponse> {
        self.mark("UnlockConnector").await;
        Ok(UnlockConnectorResponse {
            status: UnlockStatus::Unlocked,
        })
    }

    async fn change_availability(
        &self,
        _req: ChangeAvailabilityRequest,
    ) -> HandlerResult<ChangeAvailabilityResponse> {
        self.mark("ChangeAvailability").await;
        Ok(ChangeAvailabilityResponse {
            status: AvailabilityStatus::Accepted,
        })
    }

    async fn update_firmware(
        &self,
        _req: UpdateFirmwareRequest,
    ) -> HandlerResult<UpdateFirmwareResponse> {
        self.mark("UpdateFirmware").await;
        Ok(UpdateFirmwareResponse {})
    }

    async fn get_diagnostics(
        &self,
        _req: GetDiagnosticsRequest,
    ) -> HandlerResult<GetDiagnosticsResponse> {
        self.mark("GetDiagnostics").await;
        Ok(GetDiagnosticsResponse {
            file_name: Some("diagnostics-e2e.log".into()),
        })
    }

    async fn clear_cache(&self, _req: ClearCacheRequest) -> HandlerResult<ClearCacheResponse> {
        self.mark("ClearCache").await;
        Ok(ClearCacheResponse {
            status: ClearCacheStatus::Accepted,
        })
    }

    async fn get_local_list_version(
        &self,
        _req: GetLocalListVersionRequest,
    ) -> HandlerResult<GetLocalListVersionResponse> {
        self.mark("GetLocalListVersion").await;
        Ok(GetLocalListVersionResponse { list_version: 1 })
    }

    async fn send_local_list(
        &self,
        _req: SendLocalListRequest,
    ) -> HandlerResult<SendLocalListResponse> {
        self.mark("SendLocalList").await;
        Ok(SendLocalListResponse {
            status: UpdateStatus::Accepted,
        })
    }

    async fn reserve_now(&self, _req: ReserveNowRequest) -> HandlerResult<ReserveNowResponse> {
        self.mark("ReserveNow").await;
        Ok(ReserveNowResponse {
            status: ReservationStatus::Accepted,
        })
    }

    async fn cancel_reservation(
        &self,
        _req: CancelReservationRequest,
    ) -> HandlerResult<CancelReservationResponse> {
        self.mark("CancelReservation").await;
        Ok(CancelReservationResponse {
            status: CancelReservationStatus::Accepted,
        })
    }

    async fn set_charging_profile(
        &self,
        _req: SetChargingProfileRequest,
    ) -> HandlerResult<SetChargingProfileResponse> {
        self.mark("SetChargingProfile").await;
        Ok(SetChargingProfileResponse {
            status: ChargingProfileStatus::Accepted,
        })
    }

    async fn clear_charging_profile(
        &self,
        _req: ClearChargingProfileRequest,
    ) -> HandlerResult<ClearChargingProfileResponse> {
        self.mark("ClearChargingProfile").await;
        Ok(ClearChargingProfileResponse {
            status: ClearChargingProfileStatus::Accepted,
        })
    }

    async fn get_composite_schedule(
        &self,
        _req: GetCompositeScheduleRequest,
    ) -> HandlerResult<GetCompositeScheduleResponse> {
        self.mark("GetCompositeSchedule").await;
        Ok(GetCompositeScheduleResponse {
            status: GetCompositeScheduleStatus::Accepted,
            connector_id: Some(1),
            schedule_start: Some(now()),
            charging_schedule: Some(charging_schedule()),
        })
    }

    async fn data_transfer(
        &self,
        _req: DataTransferRequest,
    ) -> HandlerResult<DataTransferResponse> {
        self.mark("DataTransfer").await;
        Ok(DataTransferResponse {
            status: DataTransferStatus::Accepted,
            data: Some("ok".into()),
        })
    }
}

fn meter_value(value: &str) -> MeterValue {
    MeterValue {
        timestamp: now(),
        sampled_value: vec![SampledValue {
            value: value.into(),
            context: Some(ReadingContext::SamplePeriodic),
            format: Some(ValueFormat::Raw),
            measurand: Some(Measurand::EnergyActiveImportRegister),
            phase: None,
            location: Some(Location::Outlet),
            unit: Some(UnitOfMeasure::Wh),
        }],
    }
}

fn charging_schedule() -> ChargingSchedule {
    ChargingSchedule {
        duration: Some(300),
        start_schedule: Some(now()),
        charging_rate_unit: ChargingRateUnit::A,
        charging_schedule_period: vec![ChargingSchedulePeriod {
            start_period: 0,
            limit: 16.0,
            number_phases: Some(3),
        }],
        min_charging_rate: Some(6.0),
    }
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 28, 0, 0, 0).single().unwrap()
}
