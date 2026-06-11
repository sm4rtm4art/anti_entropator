#!/usr/bin/env bash
# Validate that the default Compose config keeps published ports local-only.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

# docker compose config resolves required variables, so provide non-secret
# placeholders for validation instead of requiring a local .env file.
export RUSTFS_ACCESS_KEY="${RUSTFS_ACCESS_KEY:-compose-local-check-access}"
export RUSTFS_SECRET_KEY="${RUSTFS_SECRET_KEY:-compose-local-check-secret}"
export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-compose-local-check-postgres}"
export LAKEKEEPER_PG_ENCRYPTION_KEY="${LAKEKEEPER_PG_ENCRYPTION_KEY:-compose-local-check-lakekeeper}"

export ANTI_BIND_HOST="${ANTI_BIND_HOST:-127.0.0.1}"
export RUSTFS_API_PORT="${RUSTFS_API_PORT:-8200}"
export RUSTFS_CONSOLE_PORT="${RUSTFS_CONSOLE_PORT:-8210}"
export POSTGRES_PORT="${POSTGRES_PORT:-8300}"
export LAKEKEEPER_PORT="${LAKEKEEPER_PORT:-8100}"

default_config_json="$(docker compose config --format json)"

# Validate the delivery override path as well (green slot port offsets).
export DELIVERY_SLOT="${DELIVERY_SLOT:-green}"
export RUSTFS_API_PORT="${RUSTFS_API_PORT_GREEN:-9200}"
export RUSTFS_CONSOLE_PORT="${RUSTFS_CONSOLE_PORT_GREEN:-9210}"
export POSTGRES_PORT="${POSTGRES_PORT_GREEN:-9300}"
export LAKEKEEPER_PORT="${LAKEKEEPER_PORT_GREEN:-9100}"
delivery_config_json="$(docker compose -f docker-compose.yml -f docker-compose.delivery.yml config --format json)"

DEFAULT_CONFIG_JSON="$default_config_json" DELIVERY_CONFIG_JSON="$delivery_config_json" python3 - <<'PY'
import json
import os
import sys

def collect_violations(config_name, config):
    violations = []
    for service_name, service in config.get("services", {}).items():
        for port in service.get("ports", []) or []:
            host_ip = port.get("host_ip")
            published = port.get("published")
            target = port.get("target")
            if host_ip != "127.0.0.1":
                violations.append(
                    f"{config_name}::{service_name}: {host_ip or '<all interfaces>'}:{published}->{target}"
                )
    return violations

default_config = json.loads(os.environ["DEFAULT_CONFIG_JSON"])
delivery_config = json.loads(os.environ["DELIVERY_CONFIG_JSON"])
violations = []
violations.extend(collect_violations("default", default_config))
violations.extend(collect_violations("delivery-override", delivery_config))

if violations:
    print("Compose publishes non-local ports:", file=sys.stderr)
    for violation in violations:
        print(f"  - {violation}", file=sys.stderr)
    print(
        "Default/local and delivery-override compose configs must bind "
        "published ports to 127.0.0.1. "
        "Use a reviewed deployment profile before exposing services.",
        file=sys.stderr,
    )
    sys.exit(1)

print("Compose published ports are bound to 127.0.0.1 (default + delivery override)")
PY
