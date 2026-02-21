#!/bin/bash
# Build and push container image to GitHub Container Registry
#
# Prerequisites:
#   export GITHUB_TOKEN=<github_token>  (needs write:packages scope)
#
# Usage:
#   ./scripts/build-and-push.sh [tag]
#
# Examples:
#   ./scripts/build-and-push.sh          # pushes :latest
#   ./scripts/build-and-push.sh v0.1.0   # pushes :v0.1.0 and :latest

set -e

REGISTRY="ghcr.io"
IMAGE="sm4rtm4art/anti_entropator"
TAG="${1:-latest}"

echo "🔨 Building anti_entropator container..."
docker build -t "${REGISTRY}/${IMAGE}:${TAG}" .

if [ "$TAG" != "latest" ]; then
    docker tag "${REGISTRY}/${IMAGE}:${TAG}" "${REGISTRY}/${IMAGE}:latest"
fi

echo ""
echo "🔐 Logging into GitHub Container Registry..."
if [ -z "$GITHUB_TOKEN" ]; then
    echo "❌ GITHUB_TOKEN not set. Please export it first:"
    echo "   export GITHUB_TOKEN=<github_token>"
    echo ""
    echo "   Create a token at: https://github.com/settings/tokens"
    echo "   Required scope: write:packages"
    exit 1
fi

echo "$GITHUB_TOKEN" | docker login ghcr.io -u sm4rtm4art --password-stdin

echo ""
echo "🚀 Pushing ${REGISTRY}/${IMAGE}:${TAG}..."
docker push "${REGISTRY}/${IMAGE}:${TAG}"

if [ "$TAG" != "latest" ]; then
    echo "🚀 Pushing ${REGISTRY}/${IMAGE}:latest..."
    docker push "${REGISTRY}/${IMAGE}:latest"
fi

echo ""
echo "✅ Done! Image available at:"
echo "   ${REGISTRY}/${IMAGE}:${TAG}"
echo "   ${REGISTRY}/${IMAGE}:latest"
