# Applied Project Cluster

NPC infrastructure for VMware vSphere automation with Matrix integration.

## Quick Start

### 1. Apply System Resources

Create an ArgoCD Application to manage all system manifests:

```yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: npc-system
  namespace: argocd
spec:
  project: default
  source:
    repoURL: https://github.com/hattmo/applied_project_cluster.git
    targetRevision: HEAD
    path: system
  destination:
    server: https://kubernetes.default.svc
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
```

This applies:
- MetalLB IP pools (`metallb-dev.yaml`, `metallb-prod.yaml`)
- LoadBalancer services (`loadbalancer-dev.yaml`, `loadbalancer-prod.yaml`)
- ArgoCD applications for dev/prod environments (`dev-npc.yaml`, `prod-npc.yaml`)

### 2. Create Credentials Secret

Create the `creds` secret in each namespace before deploying:

```bash
# Dev namespace
kubectl create secret generic creds -n dev-npc \
  --from-literal=ollama-api-key="<key>" \
  --from-literal=agent-matrix-user="@npc:matrix.npc.svc.cluster.local" \
  --from-literal=agent-matrix-password="<password>" \
  --from-file=ssh-keys=./ssh-keys/ \
  --from-literal=VMWARE_HOST="<host>" \
  --from-literal=VMWARE_USER="<user>" \
  --from-literal=VMWARE_PASSWORD="<pass>"

# Prod namespace
kubectl create secret generic creds -n prod-npc \
  --from-literal=ollama-api-key="<key>" \
  --from-literal=agent-matrix-user="@npc:matrix.npc.svc.cluster.local" \
  --from-literal=agent-matrix-password="<password>" \
  --from-file=ssh-keys=./ssh-keys/ \
  --from-literal=VMWARE_HOST="<host>" \
  --from-literal=VMWARE_USER="<user>" \
  --from-literal=VMWARE_PASSWORD="<pass>"
```

### 3. Deploy Environments

The ArgoCD applications from step 1 will automatically deploy:
- `dev-npc` → tracks `dev` branch → deploys to `dev-npc` namespace
- `prod-npc` → tracks `main` branch → deploys to `prod-npc` namespace

## Repository Structure

```
applied_project_cluster/
├── chart/                          # Helm chart for NPC components
│   ├── Chart.yaml
│   ├── values.yaml                 # Image versions (single source of truth)
│   ├── templates/
│   │   ├── controller.yaml         # Controller deployment
│   │   ├── agent.yaml              # Agent (OpenClaw + Ollama)
│   │   ├── vmware-gateway.yaml     # VMware Gateway
│   │   └── matrix.yaml             # Matrix Synapse
├── system/                         # ArgoCD and infrastructure manifests
│   ├── dev-npc.yaml                # ArgoCD app for dev
│   ├── prod-npc.yaml               # ArgoCD app for prod
│   ├── metallb-dev.yaml            # MetalLB IP pool for dev
│   ├── metallb-prod.yaml           # MetalLB IP pool for prod
│   ├── loadbalancer-dev.yaml       # LoadBalancer service for dev
│   └── loadbalancer-prod.yaml      # LoadBalancer service for prod
└── .github/workflows/
    └── build-images.yml            # CI/CD builds images from values.yaml
```

## Version Management

Edit `chart/values.yaml` to change image versions:

```yaml
images:
  controller:
    tag: "0.2.0"
  openclaw:
    tag: "0.2.0"
  vmwareGateway:
    tag: "0.2.0"
```

CI automatically builds and pushes images with version tags on push to `main`.

## Components

- **Controller** - Matrix integration and task management
- **Agent** - OpenClaw + Ollama for autonomous VM operation
- **VMware Gateway** - vSphere API proxy
- **Matrix** - Synapse server for communication

## Secrets

### `creds` (user-provided)
- `ollama-api-key`
- `agent-matrix-user`
- `agent-matrix-password`
- `ssh-keys/` (directory with `id_ed25519`, `id_ed25519.pub`)
- `VMWARE_HOST`, `VMWARE_USER`, `VMWARE_PASSWORD`

### `matrix-shared-secret` (auto-created)
- Created by Matrix deployment on first run
- Contains Synapse registration shared secret
