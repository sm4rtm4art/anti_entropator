#!/usr/bin/env bash
# Delivery simulation helper for S5-C Slice D.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
DELIVERY_DIR="${REPO_ROOT}/.delivery"
SLOTS_DIR="${DELIVERY_DIR}/slots"
ACTIVE_SLOT_FILE="${DELIVERY_DIR}/active-slot"
PREVIOUS_SLOT_FILE="${DELIVERY_DIR}/active-slot.previous"
COMPOSE_FILES=(
    -f "${REPO_ROOT}/docker-compose.yml"
    -f "${REPO_ROOT}/docker-compose.delivery.yml"
)

usage() {
    cat <<'EOF'
Usage:
  scripts/delivery-sim.sh deploy <blue|green> <image> [--ephemeral-env]
  scripts/delivery-sim.sh promote <blue|green>
  scripts/delivery-sim.sh rollback
  scripts/delivery-sim.sh status
  scripts/delivery-sim.sh down <blue|green> [--destroy-data]

Examples:
  scripts/delivery-sim.sh deploy green anti_entropator:s5-d-candidate
  scripts/delivery-sim.sh deploy green anti_entropator:s5-d-candidate --ephemeral-env
  scripts/delivery-sim.sh promote green
  scripts/delivery-sim.sh rollback
  scripts/delivery-sim.sh down green --destroy-data
EOF
}

ensure_slot() {
    local slot="${1:-}"
    case "$slot" in
    blue | green) ;;
    *)
        echo "Invalid slot '${slot}'. Expected 'blue' or 'green'." >&2
        exit 1
        ;;
    esac
}

project_name_for_slot() {
    local slot="$1"
    echo "anti_entropator_${slot}"
}

set_slot_ports() {
    local slot="$1"
    export ANTI_BIND_HOST="127.0.0.1"
    if [[ "$slot" == "blue" ]]; then
        export RUSTFS_API_PORT="8200"
        export RUSTFS_CONSOLE_PORT="8210"
        export POSTGRES_PORT="8300"
        export LAKEKEEPER_PORT="8100"
    else
        export RUSTFS_API_PORT="9200"
        export RUSTFS_CONSOLE_PORT="9210"
        export POSTGRES_PORT="9300"
        export LAKEKEEPER_PORT="9100"
    fi
}

set_runtime_env_defaults() {
    export ANTI_ENTROPATOR_S3_ENDPOINT="http://rustfs:9000"
    export ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL="http://rustfs:9000"
    export ANTI_ENTROPATOR_CATALOG_ENDPOINT="http://lakekeeper:8181"
    export ANTI_ENTROPATOR_S3_REGION="${ANTI_ENTROPATOR_S3_REGION:-eu-central-1}"
    export ANTI_ENTROPATOR_BUCKET="${ANTI_ENTROPATOR_BUCKET:-anti-entropator}"
    export ANTI_ENTROPATOR_WAREHOUSE="${ANTI_ENTROPATOR_WAREHOUSE:-anti-entropator}"
}

# Placeholders are acceptable only for non-deploy paths (down) where compose
# must parse the config but no service handles real data with these values.
set_compose_required_defaults() {
    export RUSTFS_ACCESS_KEY="${RUSTFS_ACCESS_KEY:-compose-delivery-access}"
    export RUSTFS_SECRET_KEY="${RUSTFS_SECRET_KEY:-compose-delivery-secret}"
    export POSTGRES_USER="${POSTGRES_USER:-lakekeeper}"
    export POSTGRES_DB="${POSTGRES_DB:-lakekeeper}"
    export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-compose-delivery-postgres}"
    export LAKEKEEPER_PG_ENCRYPTION_KEY="${LAKEKEEPER_PG_ENCRYPTION_KEY:-compose-delivery-lakekeeper-key}"
    export LAKEKEEPER_AUTHZ_BACKEND="${LAKEKEEPER_AUTHZ_BACKEND:-allowall}"
}

# Local (non-ephemeral) deploys must use real .env credentials; fail fast
# instead of falling back to static placeholders.
require_local_credentials() {
    local missing=()
    local var
    for var in RUSTFS_ACCESS_KEY RUSTFS_SECRET_KEY POSTGRES_PASSWORD LAKEKEEPER_PG_ENCRYPTION_KEY; do
        if [[ -z "${!var:-}" ]]; then
            missing+=("$var")
        fi
    done
    if [[ ${#missing[@]} -gt 0 ]]; then
        echo "Missing required credentials after loading .env: ${missing[*]}" >&2
        echo "Set them in .env or use --ephemeral-env." >&2
        exit 1
    fi
}

set_ephemeral_credentials() {
    local rand
    local rustfs_secret
    local postgres_password
    local lakekeeper_key
    rand="$(openssl rand -hex 16)"
    rustfs_secret="$(openssl rand -hex 32)"
    postgres_password="$(openssl rand -hex 24)"
    lakekeeper_key="$(openssl rand -hex 32)"
    export RUSTFS_ACCESS_KEY="delivery-access-${rand}"
    export RUSTFS_SECRET_KEY="${rustfs_secret}"
    export POSTGRES_USER="lakekeeper"
    export POSTGRES_DB="lakekeeper"
    export POSTGRES_PASSWORD="${postgres_password}"
    export LAKEKEEPER_PG_ENCRYPTION_KEY="${lakekeeper_key}"
    export LAKEKEEPER_AUTHZ_BACKEND="allowall"
}

load_env_file() {
    if [[ ! -f "${REPO_ROOT}/.env" ]]; then
        echo "Missing .env file. Either create one or use --ephemeral-env." >&2
        exit 1
    fi
    # shellcheck disable=SC1091
    set -a && source "${REPO_ROOT}/.env" && set +a
}

ensure_delivery_dirs() {
    mkdir -p "${DELIVERY_DIR}" "${SLOTS_DIR}"
}

slot_data_root() {
    local slot="$1"
    echo "${REPO_ROOT}/data/${slot}"
}

slot_log_root() {
    local slot="$1"
    echo "${REPO_ROOT}/logs/${slot}"
}

prepare_slot_directories() {
    local slot="$1"
    local rustfs_data_dir
    local rustfs_log_dir
    local postgres_dir

    rustfs_data_dir="$(slot_data_root "$slot")/rustfs"
    rustfs_log_dir="$(slot_log_root "$slot")/rustfs"
    postgres_dir="$(slot_data_root "$slot")/postgres"

    mkdir -p "$rustfs_data_dir" "$rustfs_log_dir" "$postgres_dir"

    # RustFS runs as UID 10001 in the container. sudo -n avoids hanging on an
    # interactive password prompt (e.g. local macOS shells).
    if command -v sudo >/dev/null 2>&1; then
        sudo -n chown -R 10001:10001 "$rustfs_data_dir" "$rustfs_log_dir" 2>/dev/null || true
    fi
    chown -R 10001:10001 "$rustfs_data_dir" "$rustfs_log_dir" 2>/dev/null || true
}

remove_slot_tree() {
    # Slot trees can contain container-UID-owned files (Postgres internal
    # user, RustFS 10001) that a non-root user cannot delete on Linux, so
    # fall back to sudo when plain rm fails (e.g. GitHub-hosted runners).
    local dir="$1"
    [[ -e "$dir" ]] || return 0
    if rm -rf "$dir" 2>/dev/null; then
        return 0
    fi
    if command -v sudo >/dev/null 2>&1; then
        sudo -n rm -rf "$dir"
    else
        rm -rf "$dir"
    fi
}

compose_cmd() {
    local project_name="$1"
    shift
    docker compose -p "${project_name}" "${COMPOSE_FILES[@]}" "$@"
}

# Uses the compose project label directly so the check needs no compose
# config parsing (which would require the slot env to be populated).
slot_has_running_containers() {
    local slot="$1"
    local project_name
    project_name="$(project_name_for_slot "$slot")"
    [[ -n "$(docker ps --filter "label=com.docker.compose.project=${project_name}" --filter status=running --quiet)" ]]
}

image_identity() {
    local image="$1"
    docker image inspect --format '{{if .RepoDigests}}{{index .RepoDigests 0}}{{else}}{{.Id}}{{end}}' "$image"
}

write_slot_record() {
    local slot="$1"
    local image="$2"
    local digest="$3"
    local project_name="$4"
    local record_file="${SLOTS_DIR}/${slot}.env"
    local now
    now="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    cat >"${record_file}" <<EOF
SLOT=${slot}
IMAGE=${image}
DIGEST=${digest}
PROJECT_NAME=${project_name}
UPDATED_AT=${now}
EOF
}

run_smoke() {
    local slot="$1"
    local image="$2"
    local project_name="$3"
    local marker
    local smoke_log="${DELIVERY_DIR}/last-smoke-${slot}.log"

    marker="$(date +%s)-${slot}"
    # Intentionally global: the EXIT trap must still resolve the path after
    # set -e aborts this function (a RETURN trap would not fire then).
    smoke_fixtures_dir="$(mktemp -d)"
    trap 'rm -rf "${smoke_fixtures_dir}"' EXIT
    printf 'hello-%s\n' "$marker" >"${smoke_fixtures_dir}/s5d_${marker}_a.txt"
    printf 'world-%s\n' "$marker" >"${smoke_fixtures_dir}/s5d_${marker}_b.txt"
    # mktemp -d yields mode 0700; the container user (UID 1000) must be able
    # to traverse and read the bind-mounted fixtures on Linux hosts.
    chmod 755 "${smoke_fixtures_dir}"
    chmod 644 "${smoke_fixtures_dir}"/s5d_*.txt

    # Gates run in one container so init state survives across commands.
    # doctor is intentionally absent: it requires a Docker daemon and cannot
    # pass inside the runtime image; compose healthchecks cover readiness.
    # Gates use redirection plus an explicit exit (never pipes) so failures
    # keep their exit codes under POSIX sh without pipefail.
    docker run --rm \
        --network "${project_name}_default" \
        --entrypoint /bin/sh \
        -v "${smoke_fixtures_dir}:/fixtures:ro" \
        -e ANTI_ENTROPATOR_S3_ENDPOINT="http://rustfs:9000" \
        -e ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL="http://rustfs:9000" \
        -e ANTI_ENTROPATOR_CATALOG_ENDPOINT="http://lakekeeper:8181" \
        -e ANTI_ENTROPATOR_S3_REGION="${ANTI_ENTROPATOR_S3_REGION}" \
        -e ANTI_ENTROPATOR_BUCKET="${ANTI_ENTROPATOR_BUCKET}" \
        -e ANTI_ENTROPATOR_WAREHOUSE="${ANTI_ENTROPATOR_WAREHOUSE}" \
        -e RUSTFS_ACCESS_KEY="${RUSTFS_ACCESS_KEY}" \
        -e RUSTFS_SECRET_KEY="${RUSTFS_SECRET_KEY}" \
        -e SMOKE_MARKER="${marker}" \
        "$image" \
        -ceu '
run_gate() {
    name="$1"
    shift
    if "$@" >"/tmp/${name}.out" 2>&1; then
        cat "/tmp/${name}.out"
    else
        cat "/tmp/${name}.out"
        echo "Gate failed: ${name}" >&2
        exit 1
    fi
}
run_gate help anti_entropator --help
run_gate init anti_entropator init
run_gate ingest anti_entropator ingest /fixtures
grep -Eq "Uploaded:[[:space:]]+2" /tmp/ingest.out
query="SELECT count(*) FROM files WHERE filename LIKE '\''s5d_${SMOKE_MARKER}%'\''"
run_gate query anti_entropator query "$query"
grep -F "| 2        |" /tmp/query.out
echo "Smoke marker: ${SMOKE_MARKER}"
' | tee "$smoke_log"
}

cmd_deploy() {
    local slot="$1"
    local image="$2"
    local ephemeral="${3:-false}"
    local project_name
    local digest

    ensure_slot "$slot"
    ensure_delivery_dirs
    export DELIVERY_SLOT="$slot"
    if [[ "$ephemeral" == "true" ]]; then
        set_ephemeral_credentials
    else
        load_env_file
        require_local_credentials
    fi
    set_slot_ports "$slot"
    set_runtime_env_defaults
    prepare_slot_directories "$slot"

    project_name="$(project_name_for_slot "$slot")"
    compose_cmd "$project_name" up -d --wait
    run_smoke "$slot" "$image" "$project_name"
    digest="$(image_identity "$image")"
    write_slot_record "$slot" "$image" "$digest" "$project_name"
    echo "Deploy + smoke passed for slot '${slot}'."
    echo "Image identity: ${digest}"
    echo "Slot record: ${SLOTS_DIR}/${slot}.env"
}

cmd_promote() {
    local slot="$1"
    local record_file="${SLOTS_DIR}/${slot}.env"

    ensure_slot "$slot"
    ensure_delivery_dirs
    if [[ ! -f "$record_file" ]]; then
        echo "Slot record not found for '${slot}'. Run deploy first." >&2
        exit 1
    fi
    if ! slot_has_running_containers "$slot"; then
        echo "Slot '${slot}' has no running containers; refusing to promote." >&2
        echo "Redeploy it first: scripts/delivery-sim.sh deploy ${slot} <image>" >&2
        exit 1
    fi
    if [[ -f "$ACTIVE_SLOT_FILE" ]]; then
        cp "$ACTIVE_SLOT_FILE" "$PREVIOUS_SLOT_FILE"
    fi
    cp "$record_file" "$ACTIVE_SLOT_FILE"
    echo "Promoted slot '${slot}' to active marker."
    echo "Active marker: ${ACTIVE_SLOT_FILE}"
}

cmd_rollback() {
    ensure_delivery_dirs
    if [[ ! -f "$PREVIOUS_SLOT_FILE" ]]; then
        echo "No previous active marker found at ${PREVIOUS_SLOT_FILE}." >&2
        exit 1
    fi
    if [[ -f "$ACTIVE_SLOT_FILE" ]]; then
        cp "$ACTIVE_SLOT_FILE" "${ACTIVE_SLOT_FILE}.failed.$(date +%s)"
    fi
    cp "$PREVIOUS_SLOT_FILE" "$ACTIVE_SLOT_FILE"
    echo "Rollback marker restored from previous active slot."
    echo "Active marker: ${ACTIVE_SLOT_FILE}"
}

show_marker() {
    local file="$1"
    local label="$2"
    if [[ -f "$file" ]]; then
        echo "${label}:"
        cat "$file"
    else
        echo "${label}: (not set)"
    fi
}

cmd_status() {
    ensure_delivery_dirs
    show_marker "$ACTIVE_SLOT_FILE" "Active slot marker"
    echo
    show_marker "$PREVIOUS_SLOT_FILE" "Previous slot marker"
    echo
    for slot in blue green; do
        if [[ -f "${SLOTS_DIR}/${slot}.env" ]]; then
            echo "Slot record (${slot}):"
            cat "${SLOTS_DIR}/${slot}.env"
            echo
        fi
    done
}

cmd_down() {
    local slot="$1"
    local destroy_data="${2:-false}"
    local project_name

    ensure_slot "$slot"
    export DELIVERY_SLOT="$slot"
    set_slot_ports "$slot"
    set_compose_required_defaults
    project_name="$(project_name_for_slot "$slot")"
    compose_cmd "$project_name" down --remove-orphans
    if [[ "$destroy_data" == "true" ]]; then
        remove_slot_tree "$(slot_data_root "$slot")"
        remove_slot_tree "$(slot_log_root "$slot")"
        # A destroyed slot must not stay promotable: drop its record so
        # promote fails with "Slot record not found" instead of marking a
        # gone slot active.
        rm -f "${SLOTS_DIR}/${slot}.env"
        echo "Removed data/log directories and slot record for slot '${slot}'."
    fi
}

main() {
    cd "$REPO_ROOT"
    if [[ $# -lt 1 ]]; then
        usage
        exit 1
    fi

    local command="$1"
    shift

    case "$command" in
    deploy)
        if [[ $# -lt 2 ]]; then
            usage
            exit 1
        fi
        local slot="$1"
        local image="$2"
        local ephemeral="false"
        shift 2
        while [[ $# -gt 0 ]]; do
            case "$1" in
            --ephemeral-env)
                ephemeral="true"
                ;;
            *)
                echo "Unknown deploy option: $1" >&2
                exit 1
                ;;
            esac
            shift
        done
        cmd_deploy "$slot" "$image" "$ephemeral"
        ;;
    promote)
        if [[ $# -ne 1 ]]; then
            usage
            exit 1
        fi
        cmd_promote "$1"
        ;;
    rollback)
        if [[ $# -ne 0 ]]; then
            usage
            exit 1
        fi
        cmd_rollback
        ;;
    status)
        if [[ $# -ne 0 ]]; then
            usage
            exit 1
        fi
        cmd_status
        ;;
    down)
        if [[ $# -lt 1 || $# -gt 2 ]]; then
            usage
            exit 1
        fi
        local slot="$1"
        local destroy_data="false"
        if [[ $# -eq 2 ]]; then
            if [[ "$2" != "--destroy-data" ]]; then
                echo "Unknown down option: $2" >&2
                exit 1
            fi
            destroy_data="true"
        fi
        cmd_down "$slot" "$destroy_data"
        ;;
    *)
        usage
        exit 1
        ;;
    esac
}

main "$@"
