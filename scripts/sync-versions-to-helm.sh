#!/bin/bash
# Sync versions.yml to Helm values.yaml
# Run this after updating versions.yml to update Helm chart values

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
VERSIONS_FILE="$ROOT_DIR/versions.yml"
VALUES_FILE="$ROOT_DIR/charts/npc/values.yaml"

if [ ! -f "$VERSIONS_FILE" ]; then
    echo "Error: versions.yml not found at $VERSIONS_FILE"
    exit 1
fi

if [ ! -f "$VALUES_FILE" ]; then
    echo "Error: values.yaml not found at $VALUES_FILE"
    exit 1
fi

echo "Reading versions from $VERSIONS_FILE..."

# Parse versions.yml using grep/awk
get_version() {
    local component=$1
    grep -A1 "${component}:" "$VERSIONS_FILE" | grep "version:" | awk '{print $2}' | tr -d '"'
}

CONTROLLER_VERSION=$(get_version "controller")
OPENCLAW_VERSION=$(get_version "openclaw")
VMWARE_GATEWAY_VERSION=$(get_version "vmware-gateway")

echo "  controller: $CONTROLLER_VERSION"
echo "  openclaw: $OPENCLAW_VERSION"
echo "  vmware-gateway: $VMWARE_GATEWAY_VERSION"

echo "Updating $VALUES_FILE..."

# Use a temp file for safe editing
TEMP_FILE=$(mktemp)

# Read values.yaml and update tags
awk -v ctrl="$CONTROLLER_VERSION" -v open="$OPENCLAW_VERSION" -v vm="$VMWARE_GATEWAY_VERSION" '
BEGIN { in_images = 0; in_component = "" }
/^images:/ { in_images = 1 }
in_images && /^  controller:/ { in_component = "controller" }
in_images && /^  openclaw:/ { in_component = "openclaw" }
in_images && /^  vmware-gateway:/ { in_component = "vmware" }
in_images && in_component == "controller" && /tag:/ {
    sub(/tag: ".*"/, "tag: \"" ctrl "\"")
    in_component = ""
}
in_images && in_component == "openclaw" && /tag:/ {
    sub(/tag: ".*"/, "tag: \"" open "\"")
    in_component = ""
}
in_images && in_component == "vmware" && /tag:/ {
    sub(/tag: ".*"/, "tag: \"" vm "\"")
    in_component = ""
}
{ print }
' "$VALUES_FILE" > "$TEMP_FILE"

mv "$TEMP_FILE" "$VALUES_FILE"

echo "  ✓ Updated controller tag to $CONTROLLER_VERSION"
echo "  ✓ Updated openclaw tag to $OPENCLAW_VERSION"
echo "  ✓ Updated vmware-gateway tag to $VMWARE_GATEWAY_VERSION"

echo ""
echo "Helm values updated successfully!"
echo ""
echo "Next steps:"
echo "  1. Commit versions.yml and charts/npc/values.yaml"
echo "  2. Push to trigger CI/CD build with new version tags"
echo "  3. ArgoCD will automatically deploy with new versions"
