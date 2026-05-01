# NPC Infrastructure Helm Chart

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

Commit and push - CI builds images, ArgoCD deploys.

## Prerequisites

### External Secret: `creds`

The chart expects a secret named `creds` to exist in the target namespace. Create it before deploying:

```bash
kubectl create secret generic creds -n <namespace> \
  --from-literal=matrix-shared-secret="<secret>" \
  --from-literal=ollama-api-key="<key>" \
  --from-literal=agent-matrix-user="@user:server" \
  --from-literal=agent-matrix-password="<password>" \
  --from-file=ssh-keys=path/to/ssh-keys-dir/
```

**Required keys:**
- `matrix-shared-secret` - Matrix admin registration secret
- `ollama-api-key` - Ollama API key
- `agent-matrix-user` - Agent Matrix user ID
- `agent-matrix-password` - Agent Matrix password
- `ssh-keys/` - Directory containing `id_ed25519` and `id_ed25519.pub`

### VMware Credentials

The `creds` secret should also contain VMware credentials as env vars:
- `VMWARE_HOST`
- `VMWARE_USER`
- `VMWARE_PASSWORD`

## Installation

### Via ArgoCD

Apply the ArgoCD Application manifests in `system/`:
- `dev-npc.yaml` - deploys to `dev-npc` namespace (tracks `dev` branch)
- `prod-npc.yaml` - deploys to `prod-npc` namespace (tracks `main` branch)

### Via Helm CLI

```bash
# Deploy to specific namespace
helm install npc ./chart -n <namespace> --create-namespace

# Examples
helm install npc ./chart -n dev-npc --create-namespace
helm install npc ./chart -n prod-npc --create-namespace
```

The namespace is specified at install time via `-n` flag.

## External LoadBalancer

To expose the controller externally, apply the loadbalancer manifest for your environment:

```bash
# Dev
kubectl apply -f system/loadbalancer-dev.yaml

# Prod
kubectl apply -f system/loadbalancer-prod.yaml
```
