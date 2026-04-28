#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${STEVE_E2E_WORKDIR:-$ROOT_DIR/target/steve-e2e}"
STEVE_DIR="$WORK_DIR/steve"
COMPOSE_FILE="$WORK_DIR/docker-compose.yml"
SEED_FILE="$WORK_DIR/seed.sql"

STEVE_REF="${STEVE_REF:-master}"
PROJECT="${STEVE_E2E_PROJECT:-ocpp_steve_e2e}"
HTTP_PORT="${STEVE_HTTP_PORT:-18180}"
DB_PORT="${STEVE_DB_PORT:-13306}"
API_KEY_HEADER="${STEVE_API_KEY_HEADER:-STEVE-API-KEY}"
API_KEY_VALUE="${STEVE_API_KEY_VALUE:-E2E-SECRET}"
CP_ID="${STEVE_E2E_CP_ID:-CP-0001}"
ID_TAG="${STEVE_E2E_ID_TAG:-E2E-TAG}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_cmd git
require_cmd docker
require_cmd curl
require_cmd cargo

mkdir -p "$WORK_DIR"

if [[ ! -d "$STEVE_DIR/.git" ]]; then
  rm -rf "$STEVE_DIR"
  git clone --depth 1 --branch "$STEVE_REF" https://github.com/steve-community/steve.git "$STEVE_DIR"
fi

git -C "$STEVE_DIR" fetch --depth 1 origin "$STEVE_REF" >/dev/null 2>&1 || true
git -C "$STEVE_DIR" checkout -q "$STEVE_REF" || true

python3 - "$STEVE_DIR/src/main/resources/application-docker.properties" "$API_KEY_VALUE" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
api_value = sys.argv[2]
lines = []
for line in path.read_text().splitlines():
    if line.startswith("webapi.value"):
        lines.append(f"webapi.value = {api_value}")
    else:
        lines.append(line)
path.write_text("\n".join(lines) + "\n")
PY

cat > "$COMPOSE_FILE" <<YAML
services:
  db:
    image: mariadb:10.11.16
    environment:
      MYSQL_RANDOM_ROOT_PASSWORD: "yes"
      MYSQL_DATABASE: stevedb
      MYSQL_USER: steve
      MYSQL_PASSWORD: changeme
      TZ: UTC
    ports:
      - "${DB_PORT}:3306"
  app:
    build:
      context: ./steve
    depends_on:
      - db
    links:
      - "db:mariadb"
    volumes:
      - ./steve:/code
    ports:
      - "${HTTP_PORT}:8180"
YAML

if [[ "${STEVE_E2E_CLEAN:-1}" != "0" ]]; then
  docker compose -p "$PROJECT" -f "$COMPOSE_FILE" down -v --remove-orphans >/dev/null 2>&1 || true
fi

docker compose -p "$PROJECT" -f "$COMPOSE_FILE" up -d --build db app

echo "waiting for SteVe HTTP endpoint on localhost:${HTTP_PORT}"
for _ in {1..240}; do
  if curl -fsS "http://localhost:${HTTP_PORT}/steve/manager/signin" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

curl -fsS "http://localhost:${HTTP_PORT}/steve/manager/signin" >/dev/null

echo "waiting for SteVe database migrations"
for _ in {1..120}; do
  if docker compose -p "$PROJECT" -f "$COMPOSE_FILE" exec -T db \
    mariadb -usteve -pchangeme stevedb -e "SELECT COUNT(*) FROM charge_box" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

cat > "$SEED_FILE" <<SQL
INSERT INTO charge_box (charge_box_id, registration_status, ocpp_protocol, description)
VALUES ('$CP_ID', 'Accepted', 'ocpp1.6J', 'Codex live E2E charge point')
ON DUPLICATE KEY UPDATE
  registration_status = 'Accepted',
  ocpp_protocol = 'ocpp1.6J',
  description = VALUES(description);

INSERT IGNORE INTO connector (charge_box_id, connector_id)
VALUES ('$CP_ID', 0), ('$CP_ID', 1);

INSERT INTO ocpp_tag (id_tag, expiry_date, max_active_transaction_count, note)
VALUES ('$ID_TAG', '2037-01-01 00:00:00', 5, 'Codex live E2E idTag')
ON DUPLICATE KEY UPDATE
  expiry_date = VALUES(expiry_date),
  max_active_transaction_count = VALUES(max_active_transaction_count),
  note = VALUES(note);

INSERT INTO charging_profile (
  charging_profile_pk,
  stack_level,
  charging_profile_purpose,
  charging_profile_kind,
  duration_in_seconds,
  charging_rate_unit,
  min_charging_rate,
  description
) VALUES (
  1,
  1,
  'TxDefaultProfile',
  'Absolute',
  300,
  'A',
  6.0,
  'Codex live E2E TxDefaultProfile'
) ON DUPLICATE KEY UPDATE
  stack_level = VALUES(stack_level),
  charging_profile_purpose = VALUES(charging_profile_purpose),
  charging_profile_kind = VALUES(charging_profile_kind),
  duration_in_seconds = VALUES(duration_in_seconds),
  charging_rate_unit = VALUES(charging_rate_unit),
  min_charging_rate = VALUES(min_charging_rate),
  description = VALUES(description);

INSERT INTO charging_schedule_period (charging_profile_pk, start_period_in_seconds, power_limit, number_phases)
VALUES (1, 0, 16.0, 3)
ON DUPLICATE KEY UPDATE
  power_limit = VALUES(power_limit),
  number_phases = VALUES(number_phases);
SQL

docker compose -p "$PROJECT" -f "$COMPOSE_FILE" exec -T db \
  mariadb -usteve -pchangeme stevedb < "$SEED_FILE"

echo "running Rust live E2E against SteVe"
STEVE_E2E=1 \
STEVE_WS_URL="ws://localhost:${HTTP_PORT}/steve/websocket/CentralSystemService/${CP_ID}" \
STEVE_HTTP_URL="http://localhost:${HTTP_PORT}/steve" \
STEVE_API_KEY_HEADER="$API_KEY_HEADER" \
STEVE_API_KEY_VALUE="$API_KEY_VALUE" \
cargo test -p ocpp-transport --test steve_e2e -- --nocapture
