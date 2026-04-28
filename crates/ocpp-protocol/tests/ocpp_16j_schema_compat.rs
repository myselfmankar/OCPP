use std::fs;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use jsonschema::JSONSchema;
use ocpp_protocol::enums::*;
use ocpp_protocol::messages::*;
use ocpp_protocol::{Action, Call, CallResult, Frame, OcppRequest};
use serde_json::Value;

#[test]
fn all_ocpp_16j_schemas_are_covered_by_typed_samples() {
    let mut expected = schema_names();
    expected.sort();

    let mut actual = Vec::new();
    append_all_case_names(&mut actual);
    actual.sort();

    assert_eq!(actual, expected);
}

#[test]
fn all_ocpp_16j_typed_payloads_match_the_official_json_schemas() {
    request_case::<BootNotificationRequest>(BootNotificationRequest {
        charge_point_vendor: "OpenAI".into(),
        charge_point_model: "Codex-CP".into(),
        charge_point_serial_number: Some("CP-001".into()),
        charge_box_serial_number: Some("BOX-001".into()),
        firmware_version: Some("1.0.0".into()),
        iccid: None,
        imsi: None,
        meter_type: Some("AC".into()),
        meter_serial_number: Some("MTR-001".into()),
    });
    response_case::<BootNotificationRequest>(BootNotificationResponse {
        current_time: at(2026, 4, 28, 0, 0, 0),
        interval: 60,
        status: RegistrationStatus::Accepted,
    });

    request_case::<HeartbeatRequest>(HeartbeatRequest {});
    response_case::<HeartbeatRequest>(HeartbeatResponse {
        current_time: at(2026, 4, 28, 0, 1, 0),
    });

    request_case::<AuthorizeRequest>(AuthorizeRequest {
        id_tag: "VALID-TAG".into(),
    });
    response_case::<AuthorizeRequest>(AuthorizeResponse {
        id_tag_info: accepted_id_tag_info(),
    });

    request_case::<StartTransactionRequest>(StartTransactionRequest {
        connector_id: 1,
        id_tag: "VALID-TAG".into(),
        meter_start: 10,
        reservation_id: None,
        timestamp: at(2026, 4, 28, 0, 2, 0),
    });
    response_case::<StartTransactionRequest>(StartTransactionResponse {
        id_tag_info: accepted_id_tag_info(),
        transaction_id: 42,
    });

    request_case::<StopTransactionRequest>(StopTransactionRequest {
        id_tag: Some("VALID-TAG".into()),
        meter_stop: 90,
        timestamp: at(2026, 4, 28, 0, 30, 0),
        transaction_id: 42,
        reason: Some(StopReason::Local),
        transaction_data: Some(vec![meter_value()]),
    });
    response_case::<StopTransactionRequest>(StopTransactionResponse {
        id_tag_info: Some(accepted_id_tag_info()),
    });

    request_case::<MeterValuesRequest>(MeterValuesRequest {
        connector_id: 1,
        transaction_id: Some(42),
        meter_value: vec![meter_value()],
    });
    response_case::<MeterValuesRequest>(MeterValuesResponse {});

    request_case::<StatusNotificationRequest>(StatusNotificationRequest {
        connector_id: 1,
        error_code: ChargePointErrorCode::NoError,
        info: None,
        status: ChargePointStatus::Available,
        timestamp: Some(at(2026, 4, 28, 0, 3, 0)),
        vendor_id: None,
        vendor_error_code: None,
    });
    response_case::<StatusNotificationRequest>(StatusNotificationResponse {});

    request_case::<DataTransferRequest>(DataTransferRequest {
        vendor_id: "OpenAI".into(),
        message_id: Some("echo".into()),
        data: Some("payload".into()),
    });
    response_case::<DataTransferRequest>(DataTransferResponse {
        status: DataTransferStatus::Accepted,
        data: Some("ok".into()),
    });

    request_case::<RemoteStartTransactionRequest>(RemoteStartTransactionRequest {
        connector_id: Some(1),
        id_tag: "VALID-TAG".into(),
        charging_profile: Some(serde_json::to_value(charging_profile()).unwrap()),
    });
    response_case::<RemoteStartTransactionRequest>(RemoteStartTransactionResponse {
        status: RemoteStartStopStatus::Accepted,
    });

    request_case::<RemoteStopTransactionRequest>(RemoteStopTransactionRequest {
        transaction_id: 42,
    });
    response_case::<RemoteStopTransactionRequest>(RemoteStopTransactionResponse {
        status: RemoteStartStopStatus::Accepted,
    });

    request_case::<TriggerMessageRequest>(TriggerMessageRequest {
        requested_message: MessageTrigger::StatusNotification,
        connector_id: Some(1),
    });
    response_case::<TriggerMessageRequest>(TriggerMessageResponse {
        status: TriggerMessageStatus::Accepted,
    });

    request_case::<ResetRequest>(ResetRequest {
        reset_type: ResetType::Soft,
    });
    response_case::<ResetRequest>(ResetResponse {
        status: ResetStatus::Accepted,
    });

    request_case::<ChangeConfigurationRequest>(ChangeConfigurationRequest {
        key: "HeartbeatInterval".into(),
        value: "60".into(),
    });
    response_case::<ChangeConfigurationRequest>(ChangeConfigurationResponse {
        status: ConfigurationStatus::Accepted,
    });

    request_case::<GetConfigurationRequest>(GetConfigurationRequest {
        key: Some(vec!["HeartbeatInterval".into()]),
    });
    response_case::<GetConfigurationRequest>(GetConfigurationResponse {
        configuration_key: Some(vec![ConfigurationKey {
            key: "HeartbeatInterval".into(),
            readonly: false,
            value: Some("60".into()),
        }]),
        unknown_key: None,
    });

    request_case::<UnlockConnectorRequest>(UnlockConnectorRequest { connector_id: 1 });
    response_case::<UnlockConnectorRequest>(UnlockConnectorResponse {
        status: UnlockStatus::Unlocked,
    });

    request_case::<ChangeAvailabilityRequest>(ChangeAvailabilityRequest {
        connector_id: 1,
        r#type: AvailabilityType::Operative,
    });
    response_case::<ChangeAvailabilityRequest>(ChangeAvailabilityResponse {
        status: AvailabilityStatus::Accepted,
    });

    request_case::<UpdateFirmwareRequest>(UpdateFirmwareRequest {
        location: "https://example.com/firmware.bin".into(),
        retrieve_date: at(2026, 4, 28, 1, 0, 0),
        retries: Some(3),
        retry_interval: Some(60),
    });
    response_case::<UpdateFirmwareRequest>(UpdateFirmwareResponse {});

    request_case::<FirmwareStatusNotificationRequest>(FirmwareStatusNotificationRequest {
        status: FirmwareStatus::Downloaded,
    });
    response_case::<FirmwareStatusNotificationRequest>(FirmwareStatusNotificationResponse {});

    request_case::<GetDiagnosticsRequest>(GetDiagnosticsRequest {
        location: "https://example.com/diagnostics".into(),
        retries: Some(3),
        retry_interval: Some(60),
        start_time: Some(at(2026, 4, 28, 0, 0, 0)),
        stop_time: Some(at(2026, 4, 28, 1, 0, 0)),
    });
    response_case::<GetDiagnosticsRequest>(GetDiagnosticsResponse {
        file_name: Some("diagnostics.log".into()),
    });

    request_case::<DiagnosticsStatusNotificationRequest>(DiagnosticsStatusNotificationRequest {
        status: DiagnosticsStatus::Uploaded,
    });
    response_case::<DiagnosticsStatusNotificationRequest>(DiagnosticsStatusNotificationResponse {});

    request_case::<ClearCacheRequest>(ClearCacheRequest {});
    response_case::<ClearCacheRequest>(ClearCacheResponse {
        status: ClearCacheStatus::Accepted,
    });

    request_case::<GetLocalListVersionRequest>(GetLocalListVersionRequest {});
    response_case::<GetLocalListVersionRequest>(GetLocalListVersionResponse { list_version: 1 });

    request_case::<SendLocalListRequest>(SendLocalListRequest {
        list_version: 2,
        update_type: UpdateType::Full,
        local_authorization_list: vec![AuthorizationData {
            id_tag: "VALID-TAG".into(),
            id_tag_info: Some(accepted_id_tag_info()),
        }],
    });
    response_case::<SendLocalListRequest>(SendLocalListResponse {
        status: UpdateStatus::Accepted,
    });

    request_case::<ReserveNowRequest>(ReserveNowRequest {
        connector_id: 1,
        expiry_date: at(2026, 4, 28, 2, 0, 0),
        id_tag: "VALID-TAG".into(),
        parent_id_tag: None,
        reservation_id: 99,
    });
    response_case::<ReserveNowRequest>(ReserveNowResponse {
        status: ReservationStatus::Accepted,
    });

    request_case::<CancelReservationRequest>(CancelReservationRequest { reservation_id: 99 });
    response_case::<CancelReservationRequest>(CancelReservationResponse {
        status: CancelReservationStatus::Accepted,
    });

    request_case::<SetChargingProfileRequest>(SetChargingProfileRequest {
        connector_id: 1,
        cs_charging_profiles: charging_profile(),
    });
    response_case::<SetChargingProfileRequest>(SetChargingProfileResponse {
        status: ChargingProfileStatus::Accepted,
    });

    request_case::<ClearChargingProfileRequest>(ClearChargingProfileRequest {
        id: None,
        connector_id: Some(1),
        charging_profile_purpose: Some(ChargingProfilePurpose::TxDefaultProfile),
        stack_level: None,
    });
    response_case::<ClearChargingProfileRequest>(ClearChargingProfileResponse {
        status: ClearChargingProfileStatus::Accepted,
    });

    request_case::<GetCompositeScheduleRequest>(GetCompositeScheduleRequest {
        connector_id: 1,
        duration: 3600,
        charging_rate_unit: Some(ChargingRateUnit::A),
    });
    response_case::<GetCompositeScheduleRequest>(GetCompositeScheduleResponse {
        status: GetCompositeScheduleStatus::Accepted,
        connector_id: Some(1),
        schedule_start: Some(at(2026, 4, 28, 0, 0, 0)),
        charging_schedule: Some(charging_schedule()),
    });
}

#[test]
fn all_actions_round_trip_in_ocpp_j_frame_shapes() {
    frame_case::<BootNotificationRequest>(
        BootNotificationRequest::default(),
        BootNotificationResponse {
            current_time: at(2026, 4, 28, 0, 0, 0),
            interval: 60,
            status: RegistrationStatus::Accepted,
        },
    );
    frame_case::<HeartbeatRequest>(
        HeartbeatRequest {},
        HeartbeatResponse {
            current_time: at(2026, 4, 28, 0, 1, 0),
        },
    );
    frame_case::<AuthorizeRequest>(
        AuthorizeRequest {
            id_tag: "VALID-TAG".into(),
        },
        AuthorizeResponse {
            id_tag_info: accepted_id_tag_info(),
        },
    );

    for action in [
        "BootNotification",
        "Heartbeat",
        "Authorize",
        "StartTransaction",
        "StopTransaction",
        "MeterValues",
        "StatusNotification",
        "DataTransfer",
        "RemoteStartTransaction",
        "RemoteStopTransaction",
        "TriggerMessage",
        "Reset",
        "ChangeConfiguration",
        "GetConfiguration",
        "UnlockConnector",
        "ChangeAvailability",
        "UpdateFirmware",
        "FirmwareStatusNotification",
        "GetDiagnostics",
        "DiagnosticsStatusNotification",
        "ClearCache",
        "GetLocalListVersion",
        "SendLocalList",
        "ReserveNow",
        "CancelReservation",
        "SetChargingProfile",
        "ClearChargingProfile",
        "GetCompositeSchedule",
    ] {
        let parsed = Action::parse(action).unwrap_or_else(|| panic!("missing action {action}"));
        assert_eq!(parsed.as_str(), action);
    }
}

fn request_case<R>(request: R)
where
    R: OcppRequest,
{
    let payload = serde_json::to_value(&request).unwrap();
    validate(R::ACTION, &payload);

    let frame = Frame::Call(Call {
        unique_id: format!("{}-request", R::ACTION),
        action: R::ACTION.to_string(),
        payload,
    });
    assert_eq!(Frame::from_text(&frame.to_text().unwrap()).unwrap(), frame);
}

fn response_case<R>(response: R::Response)
where
    R: OcppRequest,
{
    let payload = serde_json::to_value(&response).unwrap();
    validate(&format!("{}Response", R::ACTION), &payload);

    let frame = Frame::Result(CallResult {
        unique_id: format!("{}-response", R::ACTION),
        payload,
    });
    assert_eq!(Frame::from_text(&frame.to_text().unwrap()).unwrap(), frame);
}

fn frame_case<R>(request: R, response: R::Response)
where
    R: OcppRequest,
{
    request_case::<R>(request);
    response_case::<R>(response);
}

fn validate(schema_name: &str, payload: &Value) {
    let schema_json = fs::read_to_string(schema_path(schema_name)).unwrap();
    let schema: Value = serde_json::from_str(&schema_json).unwrap();
    let compiled = JSONSchema::compile(&schema).unwrap();

    let validation = compiled.validate(payload);
    if let Err(errors) = validation {
        let details = errors
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect::<Vec<_>>()
            .join("\n");
        panic!("{schema_name} payload failed schema validation:\n{payload:#}\n{details}");
    }
}

fn schema_path(schema_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/OCPP_1.6/schemas/json")
        .join(format!("{schema_name}.json"))
}

fn schema_names() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/OCPP_1.6/schemas/json");
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn append_all_case_names(out: &mut Vec<String>) {
    for action in [
        "Authorize",
        "BootNotification",
        "CancelReservation",
        "ChangeAvailability",
        "ChangeConfiguration",
        "ClearCache",
        "ClearChargingProfile",
        "DataTransfer",
        "DiagnosticsStatusNotification",
        "FirmwareStatusNotification",
        "GetCompositeSchedule",
        "GetConfiguration",
        "GetDiagnostics",
        "GetLocalListVersion",
        "Heartbeat",
        "MeterValues",
        "RemoteStartTransaction",
        "RemoteStopTransaction",
        "ReserveNow",
        "Reset",
        "SendLocalList",
        "SetChargingProfile",
        "StartTransaction",
        "StatusNotification",
        "StopTransaction",
        "TriggerMessage",
        "UnlockConnector",
        "UpdateFirmware",
    ] {
        out.push(action.to_string());
        out.push(format!("{action}Response"));
    }
}

fn at(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, min, sec)
        .single()
        .unwrap()
}

fn accepted_id_tag_info() -> IdTagInfo {
    IdTagInfo {
        status: AuthorizationStatus::Accepted,
        expiry_date: Some(at(2026, 12, 31, 23, 59, 59)),
        parent_id_tag: None,
    }
}

fn meter_value() -> MeterValue {
    MeterValue {
        timestamp: at(2026, 4, 28, 0, 4, 0),
        sampled_value: vec![SampledValue {
            value: "12.3".into(),
            context: Some(ReadingContext::SamplePeriodic),
            format: Some(ValueFormat::Raw),
            measurand: Some(Measurand::EnergyActiveImportRegister),
            phase: None,
            location: Some(Location::Outlet),
            unit: Some(UnitOfMeasure::Wh),
        }],
    }
}

fn charging_profile() -> ChargingProfile {
    ChargingProfile {
        charging_profile_id: 7,
        transaction_id: None,
        stack_level: 1,
        charging_profile_purpose: ChargingProfilePurpose::TxDefaultProfile,
        charging_profile_kind: ChargingProfileKind::Absolute,
        recurrency_kind: None,
        valid_from: None,
        valid_to: None,
        charging_schedule: charging_schedule(),
    }
}

fn charging_schedule() -> ChargingSchedule {
    ChargingSchedule {
        duration: Some(3600),
        start_schedule: Some(at(2026, 4, 28, 0, 0, 0)),
        charging_rate_unit: ChargingRateUnit::A,
        charging_schedule_period: vec![ChargingSchedulePeriod {
            start_period: 0,
            limit: 16.0,
            number_phases: Some(3),
        }],
        min_charging_rate: Some(6.0),
    }
}
