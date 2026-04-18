# VMware VM Operator Skill

Remotely operate VMware vSphere virtual machines through the vmware-gateway API. Provides both basic control tools and autonomous operation capabilities.

## Location

`/root/.openclaw/workspace/skills/vmware-vm-operator`

## Description

This skill enables OpenClaw to:

- **Capture VM screenshots** - See what's on the VM display
- **Send keyboard input** - Type text, press special keys, send shortcuts
- **Operate autonomously** - Run continuous observation/action loops
- **Execute task lists** - Work through objectives indefinitely
- **Analyze state** - Determine VM state from screenshots
- **Make decisions** - Choose appropriate actions for each situation

## Core Loop

```
┌─────────────────┐
│  Get Task List  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Capture Screen  │ ←─── vmware_screenshot()
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Analyze State   │ ←─── What do I see?
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Decide Action   │ ←─── What keys to send?
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Send Keys       │ ←─── vmware_send_keys()
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Update Progress │
└────────┬────────┘
         │
         └──────┐
                │
                ▼
         (repeat loop)
```

## Tools

### Basic Control

#### `vmware_screenshot`

Capture a screenshot from a VMware VM.

**Parameters:**
- `vm_name` (string, required) - The name of the VM to capture
- `output_path` (string, optional) - Custom output path (default: `/tmp/vmware-screenshots/{vm_name}-{timestamp}.png`)

**Returns:** PNG image file path and base64-encoded image data

**Example:**
```
vmware_screenshot(vm_name="prod-web-01")
```

#### `vmware_send_keys`

Send keyboard input to a VMware VM.

**Parameters:**
- `vm_name` (string, required) - The name of the VM to control
- `keys` (string, required) - Keystrokes to send (supports special keys in angle brackets)

**Special Key Syntax:**
- Standard keys: Just type them (e.g., `hello`, `ls -la`)
- Special keys in angle brackets: `<enter>`, `<tab>`, `<esc>`, `<backspace>`
- Arrow keys: `<up>`, `<down>`, `<left>`, `<right>`
- Function keys: `<F1>` through `<F12>`
- Modifiers: `<ctrl_on>c<ctrl_off>`, `<alt_on><F4><alt_off>`, `<shift_on>HELLO<shift_off>`
- Other: `<home>`, `<end>`, `<pageup>`, `<pagedown>`, `<delete>`, `<printscreen>`, `<super>`

**Returns:** Success confirmation or error message

**Examples:**
```
vmware_send_keys(vm_name="prod-web-01", keys="Hello World")
vmware_send_keys(vm_name="prod-web-01", keys="<enter>")
vmware_send_keys(vm_name="prod-web-01", keys="<ctrl_on>c<ctrl_off>")
vmware_send_keys(vm_name="prod-web-01", keys="ls -la<enter>")
vmware_send_keys(vm_name="prod-web-01", keys="<alt_on><F4><alt_off>")
```

#### `vmware_type_text`

Type plain text to a VMware VM (wrapper around send_keys).

**Parameters:**
- `vm_name` (string, required) - The name of the VM to control
- `text` (string, required) - Plain text to type

**Returns:** Success confirmation

**Example:**
```
vmware_type_text(vm_name="prod-web-01", text="username<tab>password<enter>")
```

#### `vmware_press_key`

Press a single special key (wrapper for common keys).

**Parameters:**
- `vm_name` (string, required) - The name of the VM to control
- `key` (string, required) - Key name without brackets (e.g., `enter`, `tab`, `F1`)

**Returns:** Success confirmation

**Example:**
```
vmware_press_key(vm_name="prod-web-01", key="enter")
vmware_press_key(vm_name="prod-web-01", key="F1")
```

#### `vmware_health_check`

Check if vmware-gateway is accessible.

**Returns:** dict with success status and gateway info

**Example:**
```
vmware_health_check()
```

### Autonomous Operation

#### `vm_autonomous_start`

Start autonomous operation on a VM with a list of tasks.

**Parameters:**
- `vm_name` (string, required) - The VM to control
- `tasks` (array of strings, required) - List of tasks/objectives to accomplish
- `loop_interval_seconds` (number, optional) - Time between observation cycles (default: 5)
- `max_iterations` (number, optional) - Max loop iterations (default: 0 = infinite)

**Returns:** Task queue ID and status

**Example:**
```
vm_autonomous_start(
  vm_name="prod-web-01",
  tasks=[
    "Login to the system",
    "Open a web browser",
    "Navigate to https://monitoring.example.com",
    "Check system status"
  ],
  loop_interval_seconds=5
)
```

#### `vm_autonomous_status`

Get current status of autonomous operation.

**Parameters:**
- `task_queue_id` (string, required) - The task queue ID from vm_autonomous_start

**Returns:** Current task, progress, last observation, pending actions

**Example:**
```
vm_autonomous_status(task_queue_id="queue-123")
```

#### `vm_autonomous_stop`

Stop autonomous operation.

**Parameters:**
- `task_queue_id` (string, required) - The task queue ID to stop

**Returns:** Stop confirmation and final status

**Example:**
```
vm_autonomous_stop(task_queue_id="queue-123")
```

#### `vm_analyze_screenshot`

Analyze a screenshot to determine current VM state.

**Parameters:**
- `vm_name` (string, required) - The VM to analyze
- `analysis_type` (string, optional) - Type of analysis: "login_screen", "desktop", "browser", "terminal", "generic" (default: "generic")

**Returns:** State description, detected elements, suggested actions

**Example:**
```
vm_analyze_screenshot(vm_name="prod-web-01", analysis_type="login_screen")
```

#### `vm_decide_next_action`

Decide what action to take based on current state and goals.

**Parameters:**
- `current_state` (string, required) - Description of current VM state
- `current_task` (string, required) - The task being worked on
- `task_history` (array, optional) - Previous actions taken

**Returns:** Recommended action (keystrokes), confidence, reasoning

**Example:**
```
vm_decide_next_action(
  current_state="Login screen with username field focused",
  current_task="Login to the system",
  task_history=[]
)
```

#### `vm_execute_action`

Execute a decided action on the VM.

**Parameters:**
- `vm_name` (string, required) - The VM to control
- `action` (string, required) - The action/keystrokes to send
- `wait_seconds` (number, optional) - Time to wait after execution (default: 2)

**Returns:** Execution result, new screenshot path

**Example:**
```
vm_execute_action(
  vm_name="prod-web-01",
  action="admin<tab>password123<enter>",
  wait_seconds=3
)
```

#### `vm_autonomous_loop_iteration`

Execute one iteration of the autonomous loop (observe → analyze → decide → act).

**Parameters:**
- `task_queue_id` (string, required) - The task queue to process

**Returns:** Iteration results with steps taken

**Example:**
```
vm_autonomous_loop_iteration(task_queue_id="queue-123")
```

## Configuration

**Hardcoded Default:**
- **Gateway URL:** `http://vmware-gateway-service.npc.svc.cluster.local:8888`

**Environment Variables (optional overrides):**
- `VMWARE_GATEWAY_URL` - Override gateway URL
- `VMWARE_SCREENSHOT_DIR` - Directory to save screenshots (default: `/tmp/vmware-screenshots`)
- `VM_TASK_QUEUE_DIR` - Task queue storage (default: `/tmp/vm-task-queues`)
- `VM_ANALYSIS_INTERVAL` - Seconds between observations (default: 5)
- `VM_MAX_RETRIES` - Max retries per task (default: 3)
- `VM_ACTION_DELAY` - Seconds to wait after actions (default: 2)

## Usage Patterns

### Basic Remote Control

```
# Capture screen
vmware_screenshot(vm_name="prod-web-01")

# Type text
vmware_type_text(vm_name="prod-web-01", text="Hello World")

# Press special keys
vmware_press_key(vm_name="prod-web-01", key="enter")

# Send complex sequence
vmware_send_keys(vm_name="prod-web-01", keys="<ctrl_on>c<ctrl_off>")
```

### Autonomous Login

```
# Start autonomous operation
vm_autonomous_start(
  vm_name="prod-web-01",
  tasks=[
    "Login with username 'admin' and password 'secret123'",
    "Open terminal",
    "Run 'systemctl status nginx'",
    "Report the output"
  ]
)

# Check progress
vm_autonomous_status(task_queue_id="queue-abc123")

# Stop when done
vm_autonomous_stop(task_queue_id="queue-abc123")
```

### Continuous Monitoring

```
# Monitor a dashboard every 30 seconds
vm_autonomous_start(
  vm_name="monitor-vm-01",
  tasks=[
    "Navigate to https://grafana.example.com",
    "Check for any red alerts",
    "If alerts found, note them and continue monitoring"
  ],
  loop_interval_seconds=30,
  max_iterations=0
)
```

### Manual Decision Loop

```
# Capture and analyze
screenshot = vmware_screenshot(vm_name="prod-web-01")
analysis = vm_analyze_screenshot(vm_name="prod-web-01", analysis_type="login_screen")

# Decide action
decision = vm_decide_next_action(
  current_state=analysis['state_description'],
  current_task="Login to the system"
)

# Execute
vm_execute_action(
  vm_name="prod-web-01",
  action=decision['action'],
  wait_seconds=3
)
```

## State Detection

The agent can detect these common states:

| State | Detection Clues | Typical Actions |
|-------|----------------|-----------------|
| Login Screen | Username/password fields, login button | Type credentials, press enter |
| Desktop | Taskbar, start menu, desktop icons | Open applications |
| Terminal | Command prompt, cursor, text output | Type commands |
| Browser | URL bar, tabs, webpage content | Navigate, click, scroll |
| Application | Window chrome, menus, content area | Interact with UI |
| Error/Dialog | Alert boxes, error messages | Acknowledge, dismiss |

## Error Handling

- **Task fails 3 times** - Mark as failed, move to next task
- **VM unresponsive** - Retry connection, alert after 5 failures
- **Unexpected state** - Log state, attempt recovery actions
- **Screenshot fails** - Retry up to 3 times, then pause
- **Gateway unreachable** - Run `vmware_health_check()` and alert user

## Security Notes

- VM names are validated against a whitelist on the gateway
- Gateway should run behind TLS in production
- Use dedicated vSphere service account with minimal permissions
- Screenshots may contain sensitive data - handle securely
- Task queues may contain credentials - store securely
- Action history logs all keystrokes - review before sharing

## Dependencies

- `requests` - HTTP requests to vmware-gateway
- `Pillow` - Image handling (optional, for advanced analysis)
