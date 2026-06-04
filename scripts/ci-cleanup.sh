#!/usr/bin/env bash
# Best-effort cleanup for GitHub-hosted and future self-hosted runners.
set -u

workspace="${GITHUB_WORKSPACE:-$(pwd)}"
registry="${REGISTRY:-ghcr.io}"

if [[ "${GITHUB_ACTIONS:-}" != "true" && "${ANTI_ENTROPATOR_FORCE_CI_CLEANUP:-}" != "true" ]]; then
    echo "Skipping CI cleanup outside GitHub Actions"
    exit 0
fi

echo "::group::CI runner cleanup"

if [[ -d "$workspace" ]]; then
    rm -f "$workspace/.env" "$workspace"/.env.* 2>/dev/null || true
    rm -rf \
        "$workspace/trivy-results" \
        "$workspace/artifacts" \
        "$workspace"/anti_entropator-*.tar.gz \
        "$workspace"/verified-image.tar \
        "$workspace"/publish-image.tar \
        "$workspace"/publish-tags.txt \
        2>/dev/null || true
fi

if [[ -n "${HOME:-}" ]]; then
    rm -f "$HOME/.docker/config.json" 2>/dev/null || true
fi

if command -v docker >/dev/null 2>&1; then
    docker logout "$registry" >/dev/null 2>&1 || true
    docker image rm anti_entropator:local-scan >/dev/null 2>&1 || true

    # Best-effort removal of locally built/loaded release images (dynamic GHCR
    # tags from verify/prepare/push). Matters for future self-hosted runners;
    # a no-op on ephemeral GitHub-hosted runners. bash 3.2 safe (no mapfile).
    image_name="${IMAGE_NAME:-}"
    if [[ -n "$image_name" ]]; then
        docker images --format '{{.Repository}}:{{.Tag}}' "${registry}/${image_name}" 2>/dev/null |
            while IFS= read -r img; do
                [[ -z "$img" || "$img" == *:'<none>' ]] && continue
                docker image rm -f "$img" >/dev/null 2>&1 || true
            done
    fi
fi

echo "CI runner cleanup completed"
echo "::endgroup::"
