#!/usr/bin/env bash
# OCPP gateway end-to-end demo against SteVe.
#
# Usage:
#   ./scripts/demo.sh run        Drive a full charge transaction (plug -> stop)
#   ./scripts/demo.sh verify     Show the latest transaction and its meter values
#   ./scripts/demo.sh reset      Force-close any orphan (unstopped) transactions in SteVe
#   ./scripts/demo.sh all        reset + run + verify
#
# Requirements:
#   - docker compose stack up: SteVe (steve-app-1, steve-db-1) + Mosquitto (mosquitto)
#   - Gateway running and connected (BootNotification accepted)
#   - OCPP tag TAG-001 registered in SteVe and not blocked

set -euo pipefail

CMD="${1:-all}"
BATTERY_TOPIC="batteries/battery-1/events"
ID_TAG="TAG-001"

pub() {
  docker exec mosquitto mosquitto_pub -t "$BATTERY_TOPIC" -m "$1"
  echo ">> $1"
  sleep "${2:-1}"
}

db() {
  docker exec steve-db-1 mysql -usteve -pchangeme stevedb "$@"
}

cmd_run() {
  echo "=== 1) Plug in ==="
  pub '{"type":"plugged","connector_id":1}' 2

  echo "=== 2) Authorize + StartTransaction ==="
  pub "{\"type\":\"authorize_request\",\"connector_id\":1,\"id_tag\":\"$ID_TAG\",\"meter_start\":0}" 3

  echo "=== 3) Three meter samples ==="
  for i in 1 2 3; do
    case $i in
      1) soc=42; energy=1500; temp=28 ;;
      2) soc=55; energy=3000; temp=29 ;;
      3) soc=80; energy=4800; temp=31 ;;
    esac
    ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    pub "{\"type\":\"meter\",\"connector_id\":1,\"sample\":{\"timestamp\":\"$ts\",\"soc\":$soc,\"voltage\":400.0,\"current\":50.0,\"power_w\":20000.0,\"energy_wh\":$energy,\"temperature_c\":$temp}}" 2
  done

  echo "=== 4) Stop transaction (resolves real tx_id from SteVe) ==="
  tx_id=$(db -N -e "SELECT transaction_pk FROM transaction WHERE id_tag='$ID_TAG' AND stop_timestamp IS NULL ORDER BY transaction_pk DESC LIMIT 1;" | tr -d '\r')
  if [ -z "$tx_id" ]; then
    echo "   no active transaction found; defaulting to 1"
    tx_id=1
  fi
  echo "   transaction_id=$tx_id"
  pub "{\"type\":\"session_stopped\",\"transaction_id\":$tx_id,\"meter_stop\":4800,\"reason\":\"Local\"}" 2

  echo "=== 5) Unplug ==="
  pub '{"type":"unplugged","connector_id":1}' 1

  echo "=== Done ==="
}

cmd_verify() {
  echo "=== Latest transaction ==="
  db -e "SELECT transaction_pk, id_tag, start_value, stop_value, stop_reason, start_timestamp, stop_timestamp
         FROM transaction ORDER BY transaction_pk DESC LIMIT 1;"

  tx_id=$(db -N -e "SELECT MAX(transaction_pk) FROM transaction;" | tr -d '\r')
  echo "=== Meter values for tx $tx_id ==="
  db -e "SELECT value_timestamp, measurand, value, unit
         FROM connector_meter_value WHERE transaction_pk=$tx_id
         ORDER BY value_timestamp, measurand;"

  echo "=== Charge box ==="
  db -e "SELECT charge_box_id, registration_status, last_heartbeat_timestamp FROM charge_box;"
}

cmd_reset() {
  echo "=== Closing orphan transactions ==="
  db <<SQL
UPDATE transaction
   SET stop_timestamp = NOW(6), stop_value = 0, stop_reason = 'Other'
 WHERE stop_timestamp IS NULL;
SQL
  echo "Done."
}

case "$CMD" in
  run)    cmd_run ;;
  verify) cmd_verify ;;
  reset)  cmd_reset ;;
  all)    cmd_reset; cmd_run; sleep 3; cmd_verify ;;
  *)
    echo "Usage: $0 {run|verify|reset|all}" >&2
    exit 2
    ;;
esac
