# NPC Infrastructure Helm Chart

Helm chart for deploying NPC infrastructure components to Kubernetes.

## Components

- **Controller** - Matrix integration and task management
- **Agent** - OpenClaw + Ollama for autonomous VM operation
- **VMware Gateway** - vSphere API proxy

## Version Management

Image versions are managed in `charts/values.yaml`. This is the **single source of truth** for:
- Docker image tags (CI/CD reads from here)
- Kubernetes deployments (Helm uses these values)

### Updating Versions

```bash
# 1. Edit charts/values.yaml
# Change tag values under images section:
#   images:
#     controller:
#       tag: "0.2.0"  # <- Update this

# 2. Commit and push
git add charts/values.yaml
git commit -m "bump: controller v0.2.0"
git push
```

### CI/CD Flow

1. Push to `main` triggers GitHub Actions
2. CI reads versions from `charts/npc/values.yaml`
3. Images built with version tag + `latest`
4. ArgoCD detects chart changes
5. ArgoCD deploys new versions to cluster

## Installation

### Via ArgoCD (Recommended)

Create an ArgoCD Application pointing to this chart:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: npc
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/hattmo/applied_project_cluster.git
    targetRevision: HEAD
    path: charts
  destination:
    server: https://kubernetes.default.svc
    namespace: npc
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
      - CreateNamespace=true
```

### Manual Installation

```bash
# Install with values from values.yaml
helm install npc ./charts/npc -n npc --create-namespace

# Override specific versions
helm install npc ./charts/npc -n npc --create-namespace \
  --set images.controller.tag=0.2.0 \
  --set images.openclaw.tag=0.2.0

# Dry run / template rendering
helm template npc ./charts/npc -n npc
```

## Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `images.controller.repository` | Controller image repo | `hattmo/controller` |
| `images.controller.tag` | Controller image version | `0.1.0` |
| `images.openclaw.repository` | OpenClaw image repo | `hattmo/openclaw` |
| `images.openclaw.tag` | OpenClaw image version | `0.1.0` |
| `images.vmwareGateway.repository` | VMware Gateway image repo | `hattmo/vmware-gateway` |
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
