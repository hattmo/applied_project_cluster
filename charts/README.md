# NPC Infrastructure Helm Chart

## Version Management

Edit `charts/values.yaml` to change image versions:

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
