use std::sync::Arc;
use chrono::{DateTime, Utc};
use ocpp_protocol::enums::{AuthorizationStatus, ChargePointErrorCode, ChargePointStatus, Measurand, ReadingContext, UnitOfMeasure, ChargingProfilePurpose};
use ocpp_protocol::messages::{
    AuthorizeRequest, MeterValue, MeterValuesRequest, SampledValue, StartTransactionRequest, StatusNotificationRequest, StopTransactionRequest,
    ChargingProfile,
};
use ocpp_protocol::Ocpp16;
use ocpp_store::state::{ActiveTransaction, CpState, PendingStop};
use ocpp_store::profiles::ProfileStore;
use ocpp_store::reservations::{ReservationStore, Reservation};
use ocpp_store::auth::AuthStore;
use ocpp_store::config::ConfigStore;
use ocpp_transport::Session;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::events::{DeviceEvent, MeterSample};

pub struct SmartChargingEngine;

impl SmartChargingEngine {
    pub fn evaluate(profile: &ChargingProfile, now: DateTime<Utc>) -> Option<f64> {
        let schedule = &profile.charging_schedule;
        
        let start_time = schedule.start_schedule
            .or(profile.valid_from)
            .unwrap_or(now);

        if let Some(from) = profile.valid_from {
            if now < from { return None; }
        }
        if let Some(to) = profile.valid_to {
            if now > to { return None; }
        }

        let elapsed = (now - start_time).num_seconds() as i32;
        if elapsed < 0 { return None; }

        if let Some(duration) = schedule.duration {
            if elapsed >= duration { return None; }
        }

        let mut limit = None;
        let mut periods = schedule.charging_schedule_period.clone();
        periods.sort_by_key(|p| p.start_period);

        for p in periods {
            if elapsed >= p.start_period {
                limit = Some(p.limit);
            } else {
                break;
            }
        }

        limit
    }

    pub fn evaluate_combined(profiles: &[ChargingProfile], now: DateTime<Utc>) -> Option<f64> {
        let mut sorted = profiles.to_vec();
        sorted.sort_by(|a, b| {
            let a_p = Self::purpose_rank(a.charging_profile_purpose);
            let b_p = Self::purpose_rank(b.charging_profile_purpose);
            if a_p != b_p {
                return b_p.cmp(&a_p);
            }
            b.stack_level.cmp(&a.stack_level)
        });

        for p in sorted {
            if let Some(l) = Self::evaluate(&p, now) {
                return Some(l);
            }
        }
        None
    }

    fn purpose_rank(p: ChargingProfilePurpose) -> i32 {
        match p {
            ChargingProfilePurpose::ChargePointMaxProfile => 3,
            ChargingProfilePurpose::TxDefaultProfile => 2,
            ChargingProfilePurpose::TxProfile => 1,
        }
    }
}

pub struct TransactionManager {
    cp_id: String,
    state: CpState,
    pub(crate) auth: AuthStore,
    pub(crate) config: ConfigStore,
    pub(crate) profiles: ProfileStore,
    pub(crate) reservations: ReservationStore,
    active: Mutex<Vec<ActiveTransaction>>,
}

impl TransactionManager {
    pub fn new(
        cp_id: String,
        state: CpState,
        auth: AuthStore,
        config: ConfigStore,
        profiles: ProfileStore,
        reservations: ReservationStore,
        active_list: Vec<ActiveTransaction>,
    ) -> Self {
        Self {
            cp_id,
            state,
            auth,
            config,
            profiles,
            reservations,
            active: Mutex::new(active_list),
        }
    }

    fn get_config_bool(&self, key: &str, default: bool) -> bool {
        self.config.get(key).ok().flatten().map(|v| v == "true").unwrap_or(default)
    }

    pub async fn handle_event(
        &self,
        session: &Arc<Session<Ocpp16>>,
        event: DeviceEvent,
    ) -> anyhow::Result<()> {
        match event {
            DeviceEvent::AuthorizeRequest {
                connector_id,
                id_tag,
                meter_start,
            } => {
                // Phase 8: Local Authorization Logic
                let mut auth_info = None;

                // 1. Check Local Authorization List if enabled
                let list_enabled = self.get_config_bool("LocalAuthListEnabled", false);
                if list_enabled {
                    if let Ok(Some(info)) = self.auth.get_id_tag(&id_tag) {
                        info!(cp=%self.cp_id, id_tag, "Local list hit");
                        auth_info = Some(info);
                    }
                }

                // 2. Check Authorization Cache if enabled and not already found
                if auth_info.is_none() && self.get_config_bool("AuthorizationCacheEnabled", true) {
                    if let Ok(Some(info)) = self.auth.get_cache(&id_tag) {
                        info!(cp=%self.cp_id, id_tag, "Local cache hit");
                        auth_info = Some(info);
                    }
                }

                // 3. Fallback to CSMS if online or not found locally
                let final_auth = if let Some(info) = auth_info {
                    info
                } else {
                    let auth = session.call(AuthorizeRequest { id_tag: id_tag.clone() }).await?;
                    // Update cache if accepted
                    if matches!(auth.id_tag_info.status, AuthorizationStatus::Accepted) {
                        let _ = self.auth.put_cache(&id_tag, &auth.id_tag_info);
                    }
                    auth.id_tag_info
                };

                if !matches!(final_auth.status, AuthorizationStatus::Accepted) {
                    info!(?final_auth.status, id_tag, "Authorization rejected");
                    return Ok(());
                }

                // Reservation check (OCPP 1.6 §5.13.1)
                let reservation = self.reservations.get(connector_id).ok().flatten()
                    .or_else(|| self.reservations.get(0).ok().flatten());
                
                let mut reservation_id = None;
                if let Some(res) = reservation {
                    let matches_tag = res.id_tag == id_tag;
                    let matches_parent = final_auth.parent_id_tag.as_ref()
                        .map_or(false, |p| p == &res.id_tag);

                    if matches_tag || matches_parent {
                        reservation_id = Some(res.reservation_id);
                        info!(cp=%self.cp_id, connector_id, id_tag, reservation_id=?reservation_id, "Using reservation");
                    } else {
                        info!(cp=%self.cp_id, connector_id, id_tag, expected=?res.id_tag, "Connector reserved for someone else; rejecting Authorize");
                        // We do not start transaction if reserved for others
                        return Ok(());
                    }
                }

                let started_at = Utc::now();
                let resp = session.call(StartTransactionRequest {
                    connector_id,
                    id_tag: id_tag.clone(),
                    meter_start,
                    reservation_id,
                    timestamp: started_at,
                }).await?;

                if !matches!(resp.id_tag_info.status, AuthorizationStatus::Accepted) {
                    warn!(cp=%self.cp_id, ?resp.id_tag_info.status, transaction_id=resp.transaction_id, "StartTransaction rejected by CSMS");
                    return Ok(());
                }

                // If a reservation was used, clear it now
                if reservation_id.is_some() {
                    let _ = self.reservations.delete(connector_id);
                    let _ = self.reservations.delete(0); // Also clear connector 0 if it was the source
                }

                let tx = ActiveTransaction {
                    transaction_id: resp.transaction_id,
                    connector_id,
                    id_tag,
                    meter_start,
                    started_at,
                    pending_stop: None,
                };
                self.state.put_tx(&tx)?;
                self.active.lock().await.push(tx);
            }
            DeviceEvent::SessionStopped {
                transaction_id,
                meter_stop,
                reason,
            } => {
                let timestamp = Utc::now();
                let req = StopTransactionRequest {
                    id_tag: None,
                    meter_stop,
                    timestamp,
                    transaction_id,
                    reason,
                    transaction_data: None,
                };
                let session_for_send = session.clone();
                self.stop_or_persist(transaction_id, meter_stop, timestamp, reason, || async move {
                    session_for_send.call(req).await.map(|_| ()).map_err(|e| e.to_string())
                }).await;
            }
            DeviceEvent::Meter { connector_id, sample } => {
                let tx_id = self.active.lock().await.iter()
                    .find(|t| t.connector_id == connector_id)
                    .map(|t| t.transaction_id);
                
                let req = MeterValuesRequest {
                    connector_id,
                    transaction_id: tx_id,
                    meter_value: vec![self.meter_value_from(sample)],
                };
                let _ = session.call(req).await;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn resume_transactions(&self, session: &Arc<Session<Ocpp16>>) {
        let active_list = self.active.lock().await;
        for tx in active_list.iter() {
            let _ = session.call(StatusNotificationRequest {
                connector_id: tx.connector_id,
                error_code: ChargePointErrorCode::NoError,
                status: ChargePointStatus::Charging,
                info: Some(format!("resumed tx {}", tx.transaction_id)),
                timestamp: Some(Utc::now()),
                vendor_id: None,
                vendor_error_code: None,
            }).await;
        }
    }

    pub async fn retry_pending_stops<F, Fut>(&self, mut send: F)
    where
        F: FnMut(i32, &PendingStop) -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        let mut txs = self.active.lock().await;
        let mut keep = Vec::with_capacity(txs.len());
        for tx in txs.drain(..) {
            if let Some(ps) = tx.pending_stop.clone() {
                match send(tx.transaction_id, &ps).await {
                    Ok(()) => {
                        let _ = self.state.remove_tx(tx.transaction_id);
                        info!(transaction_id = tx.transaction_id, "pending StopTransaction delivered on reconnect");
                    }
                    Err(e) => {
                        warn!(transaction_id = tx.transaction_id, error = %e, "pending StopTransaction retry still failing");
                        keep.push(tx);
                    }
                }
            } else {
                keep.push(tx);
            }
        }
        *txs = keep;
    }

    async fn stop_or_persist<F, Fut>(
        &self,
        transaction_id: i32,
        meter_stop: i32,
        timestamp: chrono::DateTime<Utc>,
        reason: Option<ocpp_protocol::enums::StopReason>,
        send: F,
    ) where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>>,
    {
        match send().await {
            Ok(()) => {
                let _ = self.state.remove_tx(transaction_id);
                self.active.lock().await.retain(|t| t.transaction_id != transaction_id);
            }
            Err(e) => {
                warn!(transaction_id, error = %e, "StopTransaction send failed; persisting");
                let mut act = self.active.lock().await;
                if let Some(tx) = act.iter_mut().find(|t| t.transaction_id == transaction_id) {
                    tx.pending_stop = Some(PendingStop {
                        meter_stop,
                        timestamp,
                        reason,
                    });
                    let _ = self.state.put_tx(tx);
                }
            }
        }
    }

    pub fn meter_value_from(&self, s: MeterSample) -> MeterValue {
        let mut sampled = Vec::new();
        let push = |sampled: &mut Vec<SampledValue>, measurand, unit, val: Option<f32> | {
            if let Some(v) = val {
                sampled.push(SampledValue {
                    value: format!("{v}"),
                    context: Some(ReadingContext::SamplePeriodic),
                    format: None,
                    measurand: Some(measurand),
                    phase: None,
                    location: None,
                    unit: Some(unit),
                });
            }
        };
        push(&mut sampled, Measurand::SoC, UnitOfMeasure::Percent, s.soc);
        push(&mut sampled, Measurand::Voltage, UnitOfMeasure::V, s.voltage);
        push(&mut sampled, Measurand::CurrentImport, UnitOfMeasure::A, s.current);
        push(&mut sampled, Measurand::PowerActiveImport, UnitOfMeasure::W, s.power_w);
        push(
            &mut sampled,
            Measurand::Temperature,
            UnitOfMeasure::Celsius,
            s.temperature_c,
        );
        if let Some(wh) = s.energy_wh {
            sampled.push(SampledValue {
                value: format!("{wh}"),
                context: Some(ReadingContext::SamplePeriodic),
                format: None,
                measurand: Some(Measurand::EnergyActiveImportRegister),
                phase: None,
                location: None,
                unit: Some(UnitOfMeasure::Wh),
            });
        }
        MeterValue {
            timestamp: s.timestamp,
            sampled_value: sampled,
        }
    }

    pub fn get_limit(&self, connector_id: i32) -> Option<f64> {
        let all = self.profiles.list(None).ok()?;
        let applicable: Vec<ChargingProfile> = all.into_iter()
            .filter(|(cid, _)| *cid == 0 || *cid == connector_id)
            .map(|(_, p)| p)
            .collect();
            
        SmartChargingEngine::evaluate_combined(&applicable, Utc::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use ocpp_protocol::messages::{ChargingSchedule, ChargingSchedulePeriod};

    #[test]
    fn test_smart_charging_evaluation() {
        let now = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let schedule = ChargingSchedule {
            duration: Some(3600),
            start_schedule: Some(now),
            charging_rate_unit: ocpp_protocol::enums::ChargingRateUnit::A,
            charging_schedule_period: vec![
                ChargingSchedulePeriod { start_period: 0, limit: 16.0, number_phases: None },
                ChargingSchedulePeriod { start_period: 1800, limit: 10.0, number_phases: None },
            ],
            min_charging_rate: None,
        };

        let profile = ChargingProfile {
            charging_profile_id: 1,
            transaction_id: None,
            stack_level: 0,
            charging_profile_purpose: ChargingProfilePurpose::ChargePointMaxProfile,
            charging_profile_kind: ocpp_protocol::enums::ChargingProfileKind::Absolute,
            recurrency_kind: None,
            valid_from: None,
            valid_to: None,
            charging_schedule: schedule,
        };

        // At T+0
        assert_eq!(SmartChargingEngine::evaluate(&profile, now), Some(16.0));
        // At T+1799
        assert_eq!(SmartChargingEngine::evaluate(&profile, now + chrono::Duration::seconds(1799)), Some(16.0));
        // At T+1800
        assert_eq!(SmartChargingEngine::evaluate(&profile, now + chrono::Duration::seconds(1800)), Some(10.0));
        // At T+3600 (expired)
        assert_eq!(SmartChargingEngine::evaluate(&profile, now + chrono::Duration::seconds(3601)), None);
    }
}
