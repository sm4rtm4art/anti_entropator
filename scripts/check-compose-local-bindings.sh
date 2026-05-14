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

config_json="$(docker compose config --format json)"

CONFIG_JSON="$config_json" python3 - <<'PY'
import json
import os
import sys

config = json.loads(os.environ["CONFIG_JSON"])
violations = []

for service_name, service in config.get("services", {}).items():
    for port in service.get("ports", []) or []:
        host_ip = port.get("host_ip")
        published = port.get("published")
        target = port.get("target")
        if host_ip != "127.0.0.1":
            violations.append(
                f"{service_name}: {host_ip or '<all interfaces>'}:{published}->{target}"
            )

if violations:
    print("Compose publishes non-local ports:", file=sys.stderr)
    for violation in violations:
        print(f"  - {violation}", file=sys.stderr)
    print(
        "Default/local compose must bind published ports to 127.0.0.1. "
        "Use a reviewed deployment profile before exposing services.",
        file=sys.stderr,
    )
    sys.exit(1)

print("Compose published ports are bound to 127.0.0.1")
PY
