# TOOLS.md - Local Notes

Skills define _how_ tools work. This file is for _your_ specifics — the stuff that's unique to your setup.

## What Goes Here

Things like:

- Camera names and locations
- SSH hosts and aliases
- Preferred voices for TTS
- Speaker/room names
- Device nicknames
- Anything environment-specific

## Examples

```markdown
### Cameras

- living-room → Main area, 180° wide angle
- front-door → Entrance, motion-triggered

### SSH

- home-server → 192.168.1.100, user: admin

### TTS

- Preferred voice: "Nova" (warm, slightly British)
- Default speaker: Kitchen HomePod
```

## Why Separate?

Skills are shared. Your setup is yours. Keeping them apart means you can update skills without losing your notes, and share skills without leaking your infrastructure.

---

## VMware VM Control

- **Gateway URL:** `http://vmware-gateway-service.npc.svc.cluster.local:8888`
- **Screenshot Dir:** `/tmp/vmware-screenshots`
- **Default VM:** (set per-session or specify in commands)

### Available VMs

| VM Name | Purpose | OS |
|---------|---------|-----|
| (add your VMs here) | | |

### Notes

- VM names are case-sensitive
- Use `vmware_health_check()` to verify gateway connectivity
- Screenshots saved to `/tmp/vmware-screenshots/`

---

Add whatever helps you do your job. This is your cheat sheet.
