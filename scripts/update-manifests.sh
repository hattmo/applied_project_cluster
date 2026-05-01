#!/bin/bash
# Update Kubernetes manifests with image versions from versions.yml
# Run this after updating versions.yml to sync K8s manifests

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
VERSIONS_FILE="$ROOT_DIR/versions.yml"

if [ ! -f "$VERSIONS_FILE" ]; then
    echo "Error: versions.yml not found at $VERSIONS_FILE"
    exit 1
fi

echo "Reading versions from $VERSIONS_FILE..."

# Parse versions.yml (simple YAML parsing for our structure)
CONTROLLER_VERSION=$(grep -A1 "controller:" "$VERSIONS_FILE" | grep "version:" | awk '{print $2}' | tr -d '"')
OPENCLAW_VERSION=$(grep -A1 "openclaw:" "$VERSIONS_FILE" | grep "version:" | awk '{print $2}' | tr -d '"')
VMWARE_GATEWAY_VERSION=$(grep -A1 "vmware-gateway:" "$VERSIONS_FILE" | grep "version:" | awk '{print $2}' | tr -d '"')

echo "  controller: $CONTROLLER_VERSION"
echo "  openclaw: $OPENCLAW_VERSION"
echo "  vmware-gateway: $VMWARE_GATEWAY_VERSION"

# Update controller.yml
CONTROLLER_MANIFEST="$ROOT_DIR/system/controller.yml"
if [ -f "$CONTROLLER_MANIFEST" ]; then
    echo "Updating $CONTROLLER_MANIFEST..."
    sed -i "s|hattmo/controller:.*|hattmo/controller:$CONTROLLER_VERSION|g" "$CONTROLLER_MANIFEST"
    echo "  ✓ Set controller image to hattmo/controller:$CONTROLLER_VERSION"
fi

# Update agent.yml (openclaw image)
AGENT_MANIFEST="$ROOT_DIR/system/agent.yml"
if [ -f "$AGENT_MANIFEST" ]; then
    echo "Updating $AGENT_MANIFEST..."
    sed -i "s|hattmo/openclaw:.*|hattmo/openclaw:$OPENCLAW_VERSION|g" "$AGENT_MANIFEST"
    echo "  ✓ Set openclaw image to hattmo/openclaw:$OPENCLAW_VERSION"
fi

# Update vmware.yml (vmware-gateway image)
VMWARE_MANIFEST="$ROOT_DIR/system/vmware.yml"
if [ -f "$VMWARE_MANIFEST" ]; then
    echo "Updating $VMWARE_MANIFEST..."
    sed -i "s|hattmo/vmware-gateway:.*|hattmo/vmware-gateway:$VMWARE_GATEWAY_VERSION|g" "$VMWARE_MANIFEST"
    echo "  ✓ Set vmware-gateway image to hattmo/vmware-gateway:$VMWARE_GATEWAY_VERSION"
fi

echo ""
echo "Manifests updated successfully!"
echo ""
echo "Next steps:"
echo "  1. Commit versions.yml and updated manifests"
echo "  2. Push to trigger CI/CD build with new version tags"
echo "  3. Deploy updated manifests to cluster"
