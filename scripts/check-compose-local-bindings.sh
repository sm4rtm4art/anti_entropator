#!/usr/bin/env bash
# Validate that the default and delivery-override Compose configs keep
# published ports local-only and map the expected published->target ports.
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

DEFAULT_RUSTFS_API_PORT="$RUSTFS_API_PORT"
DEFAULT_RUSTFS_CONSOLE_PORT="$RUSTFS_CONSOLE_PORT"
DEFAULT_POSTGRES_PORT="$POSTGRES_PORT"
DEFAULT_LAKEKEEPER_PORT="$LAKEKEEPER_PORT"

default_config_json="$(docker compose config --format json)"

# Validate the delivery override path as well (green slot port offsets).
export DELIVERY_SLOT="${DELIVERY_SLOT:-green}"
export RUSTFS_API_PORT="${RUSTFS_API_PORT_GREEN:-9200}"
export RUSTFS_CONSOLE_PORT="${RUSTFS_CONSOLE_PORT_GREEN:-9210}"
export POSTGRES_PORT="${POSTGRES_PORT_GREEN:-9300}"
export LAKEKEEPER_PORT="${LAKEKEEPER_PORT_GREEN:-9100}"
delivery_config_json="$(docker compose -f docker-compose.yml -f docker-compose.delivery.yml config --format json)"

DEFAULT_CONFIG_JSON="$default_config_json" \
    DELIVERY_CONFIG_JSON="$delivery_config_json" \
    DEFAULT_RUSTFS_API_PORT="$DEFAULT_RUSTFS_API_PORT" \
    DEFAULT_RUSTFS_CONSOLE_PORT="$DEFAULT_RUSTFS_CONSOLE_PORT" \
    DEFAULT_POSTGRES_PORT="$DEFAULT_POSTGRES_PORT" \
    DEFAULT_LAKEKEEPER_PORT="$DEFAULT_LAKEKEEPER_PORT" \
    python3 - <<'PY'
import json
import os
import sys

def expected_ports(rustfs_api, rustfs_console, postgres, lakekeeper):
    """Expected published->target mappings per service."""
    return {
        "rustfs": {9000: rustfs_api, 9001: rustfs_console},
        "postgres": {5432: postgres},
        "lakekeeper": {8181: lakekeeper},
    }

def collect_violations(config_name, config, expected):
    violations = []
    services = config.get("services", {})
    for service_name, service in services.items():
        for port in service.get("ports", []) or []:
            host_ip = port.get("host_ip")
            published = port.get("published")
            target = port.get("target")
            if host_ip != "127.0.0.1":
                violations.append(
                    f"{config_name}::{service_name}: {host_ip or '<all interfaces>'}:{published}->{target}"
                )
    for service_name, mappings in expected.items():
        actual = {
            port.get("target"): str(port.get("published"))
            for port in services.get(service_name, {}).get("ports", []) or []
        }
        for target, published in mappings.items():
            if actual.get(target) != str(published):
                violations.append(
                    f"{config_name}::{service_name}: expected published port "
                    f"{published} for target {target}, got {actual.get(target)}"
                )
    return violations

default_expected = expected_ports(
    os.environ["DEFAULT_RUSTFS_API_PORT"],
    os.environ["DEFAULT_RUSTFS_CONSOLE_PORT"],
    os.environ["DEFAULT_POSTGRES_PORT"],
    os.environ["DEFAULT_LAKEKEEPER_PORT"],
)
delivery_expected = expected_ports(
    os.environ["RUSTFS_API_PORT"],
    os.environ["RUSTFS_CONSOLE_PORT"],
    os.environ["POSTGRES_PORT"],
    os.environ["LAKEKEEPER_PORT"],
)

default_config = json.loads(os.environ["DEFAULT_CONFIG_JSON"])
delivery_config = json.loads(os.environ["DELIVERY_CONFIG_JSON"])
violations = []
violations.extend(collect_violations("default", default_config, default_expected))
violations.extend(
    collect_violations("delivery-override", delivery_config, delivery_expected)
)

if violations:
    print("Compose port bindings violate the local-only contract:", file=sys.stderr)
    for violation in violations:
        print(f"  - {violation}", file=sys.stderr)
    print(
        "Default/local and delivery-override compose configs must bind "
        "published ports to 127.0.0.1 and keep the expected service port "
        "mappings. Use a reviewed deployment profile before exposing services.",
        file=sys.stderr,
    )
    sys.exit(1)

print(
    "Compose published ports are bound to 127.0.0.1 with expected mappings "
    "(default + delivery override)"
)
PY
