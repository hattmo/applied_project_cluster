# Heartbeat Configuration - Autonomous VM Agent

## Overview

This agent runs a **continuous autonomous loop** - polling the controller, capturing screenshots, analyzing with vision, and sending keystrokes. The heartbeat is for **supervisory checks**, not the main operation loop.

## Heartbeat Interval

**Every 30 minutes** - Check supervisory status

## Heartbeat Tasks

```
Every 30 minutes:
1. Check agent state file: /tmp/vm-task-queues/{AGENT_NAME}-state.json
2. Verify loop is progressing (loop_iteration increasing)
3. Check for persistent errors
4. Run vmware_health_check()
5. Verify controller connectivity
6. Alert if stuck or error state >10 minutes
```

## State File Location

```
/tmp/vm-task-queues/{AGENT_NAME}-state.json
```

Key fields to monitor:
- `status` - Should be "working" or "running" (not "error" or "idle" for long)
- `loop_iteration` - Should be increasing over time
- `assigned_vm` - Should have a VM assigned
- `error_message` - Should be null or transient
- `last_controller_poll` - Should be recent

## Alert Conditions

Notify user immediately if:

| Condition | Threshold |
|-----------|-----------|
| VMware gateway unreachable | >3 consecutive failures |
| Controller unreachable | >5 consecutive failures |
| Vision model failing | >10 consecutive failures |
| Stuck in error state | >10 minutes |
| Loop not progressing | Same iteration for >30 minutes |
| No VM assignment | >1 hour without assignment |

## Quiet Hours

- **23:00 - 08:00 UTC**: Only critical alerts (gateway down, crash)
- **08:00 - 23:00 UTC**: Normal operation alerts

## Recovery Actions

If issues detected:

1. **Gateway down**: Run `vmware_health_check()`, alert if still failing
2. **No assignment**: Poll controller manually, check agent-assignments API
3. **Vision failing**: Test Ollama connection, try fallback to suggested keystrokes
4. **Stuck loop**: Restart agent process, preserve state file

## Manual Commands

```bash
# Check current status
python vmware_operator.py status

# Test connectivity
python vmware_operator.py test

# Restart loop (preserves state)
python vmware_operator.py run

# View recent actions
cat /tmp/vm-task-queues/{AGENT_NAME}-state.json | jq .action_history[-10:]

# View recent screenshots
ls -lt /tmp/vmware-screenshots | head
```

## State File Schema

```json
{
  "agent_name": "agent_0",
  "assigned_vm": "prod-web-01",
  "current_queue_id": "uuid",
  "current_task_index": 2,
  "last_controller_poll": "2026-05-03T05:30:00Z",
  "last_screenshot_path": "/tmp/vmware-screenshots/prod-web-01-20260503-053000.png",
  "last_analysis": "Desktop environment visible...",
  "last_action": "systemctl status nginx<enter>",
  "action_history": [...],
  "loop_iteration": 147,
  "status": "working",
  "error_message": null
}
```

## Integration with Controller

The controller sends Matrix messages every 60 seconds to assigned agents with task prompts. The agent:
- Polls controller API directly (more reliable than Matrix)
- Uses Matrix messages as backup task source if needed
- Reports status via Matrix when alerting

## Performance Expectations

| Metric | Target |
|--------|--------|
| Loop iteration time | 5-10 seconds |
| Screenshot capture | <2 seconds |
| Vision analysis | <10 seconds |
| Key send + wait | 2-5 seconds |
| Iterations per hour | 360-720 |
