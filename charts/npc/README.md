# NPC Infrastructure Helm Chart

Helm chart for deploying NPC infrastructure components to Kubernetes.

## Components

- **Controller** - Matrix integration and task management
- **Agent** - OpenClaw + Ollama for autonomous VM operation
- **VMware Gateway** - vSphere API proxy

## Version Management

Image versions are managed centrally in `versions.yml` at the repo root.

### Updating Versions

```bash
# 1. Edit versions.yml
# Change version numbers in versions.yml

# 2. Sync to Helm values
./scripts/sync-versions-to-helm.sh

# 3. Commit and push
git add versions.yml charts/npc/values.yaml
git commit -m "bump: v0.2.0"
git push
```

### CI/CD Flow

1. Push to `main` triggers GitHub Actions
2. Images built with version tag + `latest`
3. ArgoCD detects chart changes
4. ArgoCD deploys new versions to cluster

## Installation

### Via ArgoCD (Recommended)

The `system/npc-app.yaml` defines an ArgoCD Application that automatically syncs this chart.

### Manual Installation

```bash
# Install with default values
helm install npc ./charts/npc -n npc --create-namespace

# Override versions
helm install npc ./charts/npc -n npc --create-namespace \
  --set images.controller.tag=0.2.0 \
  --set images.openclaw.tag=0.2.0

# Dry run
helm template npc ./charts/npc -n npc
```

## Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `images.controller.tag` | Controller image version | `0.1.0` |
| `images.openclaw.tag` | OpenClaw image version | `0.1.0` |
| `images.vmwareGateway.tag` | VMware Gateway image version | `0.1.0` |
| `namespace` | Target namespace | `npc` |
| `controller.enabled` | Deploy controller | `true` |
| `agent.enabled` | Deploy agent | `true` |
| `vmwareGateway.enabled` | Deploy vmware-gateway | `true` |

## Prerequisites

- Kubernetes 1.21+
- Helm 3.0+
- ArgoCD (for automated deployment)
- Docker Hub access (for images)

## Secrets Required

The following secrets must exist in the `npc` namespace:

- `matrix-shared-secret` - Matrix admin registration secret
- `agent-matrix-creds` - Agent Matrix login credentials
- `ollama-creds` - Ollama API key
- `ollama-ssh-keys` - SSH keys for Ollama
- `vmware-creds` - VMware vSphere credentials
