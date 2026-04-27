//! `Device` implementation backed by MQTT (`rumqttc`).
//!
//! Topic scheme (configurable):
//!   - subscribe: `<prefix>/<battery_id>/events`   (DeviceEvent JSON)
//!   - publish:   `<prefix>/<battery_id>/commands` (DeviceCommand JSON)

use async_trait::async_trait;
use ocpp_adapter::{Device, DeviceCommand, DeviceError, DeviceEvent};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, warn};

#[derive(Debug, Clone)]
pub struct MqttDeviceConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub battery_id: String,
    pub topic_prefix: String,
}

pub struct MqttDevice {
    client: AsyncClient,
    cfg: MqttDeviceConfig,
    /// Broadcast so multiple `events()` calls share the inbound stream.
    inbound: broadcast::Sender<DeviceEvent>,
}

impl MqttDevice {
    /// Connect and start the network task. Spawns a background task that
    /// republishes incoming events on the broadcast channel.
    pub async fn connect(cfg: MqttDeviceConfig) -> Result<Self, DeviceError> {
        let mut opts = MqttOptions::new(&cfg.client_id, &cfg.broker_host, cfg.broker_port);
        opts.set_keep_alive(Duration::from_secs(30));
        if let (Some(u), Some(p)) = (cfg.username.clone(), cfg.password.clone()) {
            opts.set_credentials(u, p);
        }
        let (client, eventloop) = AsyncClient::new(opts, 32);

        let event_topic = format!("{}/{}/events", cfg.topic_prefix, cfg.battery_id);

        let (tx, _) = broadcast::channel(64);
        tokio::spawn(run_eventloop(
            eventloop,
            client.clone(),
            event_topic,
            tx.clone(),
        ));

        Ok(Self {
            client,
            cfg,
            inbound: tx,
        })
    }

    fn cmd_topic(&self) -> String {
        format!("{}/{}/commands", self.cfg.topic_prefix, self.cfg.battery_id)
    }
}

async fn run_eventloop(
    mut el: EventLoop,
    client: AsyncClient,
    event_topic: String,
    tx: broadcast::Sender<DeviceEvent>,
) {
    loop {
        match el.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                // (Re-)subscribe on every successful (re)connect since
                // we use clean_session=true (rumqttc default).
                if let Err(e) = client
                    .subscribe(&event_topic, QoS::AtLeastOnce)
                    .await
                {
                    error!(error=%e, topic=%event_topic, "mqtt re-subscribe failed");
                } else {
                    debug!(topic=%event_topic, "mqtt subscribed");
                }
            }
            Ok(Event::Incoming(Packet::Publish(p))) => {
                match serde_json::from_slice::<DeviceEvent>(&p.payload) {
                    Ok(ev) => {
                        let _ = tx.send(ev);
                    }
                    Err(e) => warn!(error=%e, topic=%p.topic, "ignoring non-DeviceEvent payload"),
                }
            }
            Ok(_other) => {}
            Err(e) => {
                error!(error=%e, "mqtt eventloop error; reconnecting after 1s");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[async_trait]
impl Device for MqttDevice {
    async fn events(&self) -> Result<mpsc::Receiver<DeviceEvent>, DeviceError> {
        let mut rx = self.inbound.subscribe();
        let (tx, out) = mpsc::channel(64);
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        warn!("mqtt subscriber lagged");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            debug!("mqtt subscriber task ending");
        });
        Ok(out)
    }

    async fn send(&self, cmd: DeviceCommand) -> Result<(), DeviceError> {
        let bytes = serde_json::to_vec(&cmd)
            .map_err(|e| DeviceError::Backend(format!("encode: {e}")))?;
        self.client
            .publish(self.cmd_topic(), QoS::AtLeastOnce, false, bytes)
            .await
            .map_err(|e| DeviceError::Backend(format!("publish: {e}")))
    }
}
