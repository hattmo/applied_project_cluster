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

## Installation

### Via ArgoCD

Apply the ArgoCD Application manifests in `system/argocd/`:
- `dev-npc.yaml` - deploys to `dev-npc` namespace
- `prod-npc.yaml` - deploys to `prod-npc` namespace

### Via Helm CLI

```bash
# Deploy to specific namespace
helm install npc ./chart -n <namespace> --create-namespace

# Examples
helm install npc ./chart -n dev-npc --create-namespace
helm install npc ./chart -n prod-npc --create-namespace
```

The namespace is specified at install time via `-n` flag. No namespace templating in the chart.
