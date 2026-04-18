# Heartbeat Configuration

## Autonomous VM Agent Heartbeat

Check every 30 minutes for:

### Active Task Queues
- List running autonomous operations
- Report progress on each
- Flag any stuck or failed tasks

### VMware Gateway Health
- Run `vmware_health_check()`
- Alert if gateway becomes unreachable

### Pending User Tasks
- Check for new task assignments
- Start new autonomous operations as requested

## Heartbeat Tasks

```
Every 30 minutes:
1. Check for active task queues in /tmp/vm-task-queues/
2. For each running queue:
   - Get status via vm_autonomous_status()
   - Report if iteration_count > 100 with no progress
   - Alert if any task has status="failed"
3. Run vmware_health_check()
4. Check for new messages/commands from user
```

## Alert Conditions

Notify user immediately if:
- VMware gateway becomes unreachable
- Task fails after 3 retry attempts
- Agent stuck in loop (same state for 10+ iterations)
- New critical task assigned

## Quiet Hours

- **23:00 - 08:00 UTC**: Only alert for critical failures
- **08:00 - 23:00 UTC**: Normal operation, report progress
