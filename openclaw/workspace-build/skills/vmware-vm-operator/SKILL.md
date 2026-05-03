# VMware VM Operator Skill - Autonomous Edition

Remotely operate VMware vSphere virtual machines through the vmware-gateway API with full autonomous operation powered by computer vision.

## Location

`/root/.openclaw/workspace/skills/vmware-vm-operator`

## Description

This skill enables OpenClaw to autonomously operate VMs by:

- **Polling the controller** for VM assignments and task queues
- **Capturing VM screenshots** via vmware-gateway API
- **Analyzing screens with Ollama vision models** (llava, bakllava, moondream)
- **Deciding keystrokes** based on visual analysis and task context
- **Sending keyboard input** via vmware-gateway API
- **Running continuous observation/action loops** indefinitely

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Controller    │────▶│   OpenClaw      │────▶│  VMware Gateway │
│  (task queues)  │     │   Agent          │     │  (screen/keys)  │
└─────────────────┘     └────────┬─────────┘     └─────────────────┘
                                 │
                                 ▼
                        ┌──────────────────┐
                        │   Ollama Vision  │
                        │   (llava, etc.)  │
                        └──────────────────┘
```

## Core Loop

```
┌─────────────────────────┐
│ 1. Poll Controller      │
│    - Get assignment     │
│    - Get task queue     │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ 2. Capture Screenshot   │ ←─── vmware_screenshot()
│    /api/<vm>/screen     │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ 3. Vision Analysis      │ ←─── ollama_analyze_screenshot()
│    What do I see?       │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ 4. Decide Keystrokes    │ ←─── decide_keystrokes()
│    What keys to send?   │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ 5. Send Keys            │ ←─── vmware_send_keys()
│    /api/<vm>/keyboard   │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ 6. Wait & Repeat        │
└─────────────────────────┘
```

## Tools

### Controller Integration

#### `controller_get_assignment`

Get the VM assignment for this agent from the controller API.

**Returns:** `AgentAssignment` object or `None`

**Example:**
```python
assignment = controller_get_assignment()
if assignment:
    print(f"Assigned to VM: {assignment.vm_name}")
```

#### `controller_get_task_queue`

Get the enabled task queue for a specific VM.

**Parameters:**
- `vm_name` (string, required) - The VM to get tasks for

**Returns:** `ControllerTaskQueue` object or `None`

**Example:**
```python
queue = controller_get_task_queue(vm_name="prod-web-01")
for task in queue.tasks:
    print(f"Task: {task.description}")
```

### VMware Gateway Control

#### `vmware_screenshot`

Capture a screenshot from a VMware VM.

**Parameters:**
- `vm_name` (string, required) - VM name
- `output_path` (string, optional) - Custom output path

**Returns:** dict with `success`, `path`, `base64`, `size_bytes`

**Example:**
```python
result = vmware_screenshot(vm_name="prod-web-01")
if result['success']:
    print(f"Screenshot saved to: {result['path']}")
```

#### `vmware_send_keys`

Send keyboard input to a VMware VM.

**Parameters:**
- `vm_name` (string, required) - VM name
- `keys` (string, required) - Keystrokes (supports special keys)

**Special Key Syntax:**
- Regular text: `hello world`
- Enter: `<enter>`
- Tab: `<tab>`
- Escape: `<esc>`
- Arrows: `<up>`, `<down>`, `<left>`, `<right>`
- Function keys: `<F1>` - `<F12>`
- Ctrl: `<ctrl_on>c<ctrl_off>` for Ctrl+C
- Alt: `<alt_on><F4><alt_off>` for Alt+F4
- Super: `<super>`

**Returns:** dict with `success`, `message` or `error`

**Example:**
```python
vmware_send_keys(vm_name="prod-web-01", keys="ls -la<enter>")
vmware_send_keys(vm_name="prod-web-01", keys="<ctrl_on>c<ctrl_off>")
```

#### `vmware_health_check`

Check if vmware-gateway is accessible.

**Returns:** dict with `success`, `gateway_url`, `status` or `error`

### Vision Analysis (Ollama)

#### `ollama_analyze_screenshot`

Send a screenshot to Ollama vision model for analysis.

**Parameters:**
- `base64_image` (string, required) - Base64-encoded PNG
- `prompt` (string, required) - Analysis prompt
- `task_context` (string, optional) - Context about current task

**Returns:** dict with `success`, `analysis`, `model`

**Example:**
```python
result = ollama_analyze_screenshot(
    base64_image=screenshot['base64'],
    prompt="What is on this screen?",
    task_context="Trying to login to the system"
)
print(result['analysis'])
```

#### `analyze_vm_state`

High-level VM state analysis using vision model.

**Parameters:**
- `base64_image` (string, required) - Screenshot
- `current_task` (string, optional) - Current task description

**Returns:** dict with `success`, `analysis`, `detected_elements`, `suggested_actions`

#### `decide_keystrokes`

Decide what keystrokes to send based on vision analysis.

**Parameters:**
- `base64_image` (string, required) - Current screenshot
- `analysis` (string, required) - Vision model analysis
- `current_task` (string, required) - Task description
- `suggested_keystrokes` (string, optional) - Hint from task
- `recent_history` (list, optional) - Recent actions

**Returns:** dict with `success`, `keystrokes`, `confidence`, `reasoning`

### Autonomous Operation

#### `agent_loop_iteration`

Execute one complete iteration of the autonomous loop.

**Returns:** dict with iteration results and step-by-step status

**Example:**
```python
result = agent_loop_iteration(state)
for step in result['steps']:
    print(f"{step['step']}: {step['status']}")
```

#### `run_autonomous_loop`

Run the autonomous loop continuously.

**Parameters:**
- `max_iterations` (int, optional) - Max iterations (0 = infinite)

**Example:**
```python
result = run_autonomous_loop(max_iterations=0)  # Run forever
```

#### `get_agent_status`

Get current agent status and recent history.

**Returns:** dict with agent state, recent actions, errors

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_NAME` | `agent_0` | This agent's name (must match controller) |
| `CONTROLLER_URL` | `http://controller.npc.svc.cluster.local:8080` | Controller API URL |
| `VMWARE_GATEWAY_URL` | `http://vmware-gateway.npc.svc.cluster.local` | VMware gateway URL |
| `OLLAMA_URL` | `http://ollama.npc.svc.cluster.local:11434` | Ollama API URL |
| `OLLAMA_VISION_MODEL` | `llava:latest` | Vision model to use |
| `POLL_INTERVAL` | `10` | Seconds between controller polls |
| `LOOP_INTERVAL` | `3` | Seconds between loop iterations |
| `VM_ACTION_DELAY` | `2` | Seconds to wait after sending keys |
| `VISION_TIMEOUT` | `30` | Timeout for vision analysis (seconds) |

### Local Paths

| Path | Purpose |
|------|---------|
| `/tmp/vmware-screenshots` | Screenshot storage |
| `/tmp/vm-task-queues` | Agent state persistence |

## Usage Patterns

### Start Autonomous Operation

```bash
# Set environment
export AGENT_NAME=agent_0
export CONTROLLER_URL=http://controller.npc.svc.cluster.local:8080
export VMWARE_GATEWAY_URL=http://vmware-gateway.npc.svc.cluster.local

# Run autonomously (infinite loop)
python vmware_operator.py run

# Run for specific iterations
python vmware_operator.py run 100
```

### Check Agent Status

```bash
python vmware_operator.py status
```

### Test Connectivity

```bash
python vmware_operator.py test
```

### Manual Control Loop

```python
from vmware_operator import *

# Get assignment
assignment = controller_get_assignment()
if not assignment:
    print("No assignment")
    exit()

print(f"Assigned to: {assignment.vm_name}")

# Get tasks
queue = controller_get_task_queue(assignment.vm_name)
print(f"Tasks: {len(queue.tasks)}")

# Run one iteration
state = _load_state()
result = agent_loop_iteration(state)
print(json.dumps(result, indent=2))
```

## Controller API Integration

### Agent Assignment

The controller assigns agents to VMs via `/api/v1/agent-assignments`:

```json
{
  "id": "uuid",
  "agent_name": "agent_0",
  "vm_name": "prod-web-01",
  "enabled": true
}
```

### Task Queues

Tasks are defined in `/api/v1/task-queues`:

```json
{
  "id": "uuid",
  "name": "Login and check nginx",
  "vm_name": "prod-web-01",
  "enabled": true,
  "tasks": [
    {
      "description": "Login to the system",
      "keystrokes": "admin<tab>password<enter>",
      "delay_ms": 2000
    },
    {
      "description": "Check nginx status",
      "keystrokes": "systemctl status nginx<enter>",
      "delay_ms": 3000
    }
  ]
}
```

The agent:
1. Polls controller every `POLL_INTERVAL` seconds
2. Finds its assignment by `agent_name`
3. Gets the enabled task queue for its assigned VM
4. Cycles through tasks continuously
5. Uses vision to adapt if suggested keystrokes don't work

## Vision Model Integration

### Supported Models

Any Ollama vision model works:

- `llava:latest` - General purpose, good accuracy
- `bakllava` - Faster, smaller
- `moondream` - Very fast, compact
- `llava-llama3` - Better reasoning

### Analysis Prompts

The skill uses two-stage analysis:

1. **State Analysis**: "What type of screen is this? What elements are visible?"
2. **Decision**: "What keystrokes should I send to progress toward this task?"

### Handling Vision Failures

If vision analysis fails:
- Falls back to suggested keystrokes from task
- Logs error and retries next iteration
- After 3 failures, marks task as failed and moves on

## State Persistence

Agent state is saved to `/tmp/vm-task-queues/{agent_name}-state.json`:

```json
{
  "agent_name": "agent_0",
  "assigned_vm": "prod-web-01",
  "current_queue_id": "uuid",
  "current_task_index": 2,
  "status": "working",
  "loop_iteration": 147,
  "last_action": "systemctl status nginx<enter>",
  "action_history": [...]
}
```

This allows:
- Surviving restarts
- Resuming where left off
- Debugging via action history

## Error Handling

| Error | Behavior |
|-------|----------|
| Controller unreachable | Retry next poll, status="error" |
| No assignment | Status="idle", wait for assignment |
| VMware gateway down | Status="error", log error |
| Vision timeout | Fall back to suggested keystrokes |
| Keys fail to send | Retry up to 3 times, then skip task |
| Stuck in loop | Human intervention needed |

## Debugging

### View Recent Actions

```bash
python vmware_operator.py status | jq .recent_actions
```

### View Last Screenshot

```bash
ls -lt /tmp/vmware-screenshots | head
```

### Check Agent State

```bash
cat /tmp/vm-task-queues/agent_0-state.json | jq
```

### Test Individual Components

```python
# Test screenshot
vmware_screenshot("prod-web-01")

# Test keys
vmware_send_keys("prod-web-01", "<enter>")

# Test vision
result = ollama_analyze_screenshot(base64_img, "What do you see?")
print(result['analysis'])
```

## Security Notes

- VM names validated by gateway whitelist
- Gateway should run behind TLS in production
- Screenshots may contain sensitive data
- Action history logs all keystrokes
- Agent credentials should be scoped minimally

## Dependencies

- `requests` - HTTP client
- Ollama with vision model installed
- Access to controller API
- Access to vmware-gateway API
