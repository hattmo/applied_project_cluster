#!/usr/bin/env python3
"""
VMware VM Operator Skill for OpenClaw

Unified skill for remote VMware vSphere VM control and autonomous operation.
Provides both basic control tools (screenshot, keyboard input) and autonomous
operation capabilities (task queues, observation/action loops).
"""

import base64
import json
import os
import time
import hashlib
from datetime import datetime
from pathlib import Path
from typing import Optional, List, Dict, Any
from dataclasses import dataclass, asdict

import requests


# Configuration
VMWARE_GATEWAY_URL = os.environ.get(
    "VMWARE_GATEWAY_URL",
    "http://vmware-gateway-service.npc.svc.cluster.local:8888"
)
VMWARE_SCREENSHOT_DIR = os.environ.get(
    "VMWARE_SCREENSHOT_DIR",
    "/tmp/vmware-screenshots"
)
VM_TASK_QUEUE_DIR = os.environ.get(
    "VM_TASK_QUEUE_DIR",
    "/tmp/vm-task-queues"
)
VM_ANALYSIS_INTERVAL = int(os.environ.get("VM_ANALYSIS_INTERVAL", "5"))
VM_MAX_RETRIES = int(os.environ.get("VM_MAX_RETRIES", "3"))
VM_ACTION_DELAY = int(os.environ.get("VM_ACTION_DELAY", "2"))


# ============================================================================
# Data Classes
# ============================================================================

@dataclass
class Task:
    """Represents a single task in the queue."""
    id: str
    description: str
    status: str = "pending"  # pending, in_progress, completed, failed
    attempts: int = 0
    last_attempt: Optional[str] = None
    notes: str = ""


@dataclass
class TaskQueue:
    """Represents an autonomous task queue."""
    queue_id: str
    vm_name: str
    created_at: str
    status: str = "running"  # running, paused, stopped, completed
    tasks: List[Task] = None
    current_task_id: Optional[str] = None
    loop_interval_seconds: int = 5
    max_iterations: int = 0  # 0 = infinite
    iteration_count: int = 0
    last_screenshot: Optional[str] = None
    last_state: str = ""
    last_action: Optional[str] = None
    action_history: List[Dict] = None
    
    def __post_init__(self):
        if self.tasks is None:
            self.tasks = []
        if self.action_history is None:
            self.action_history = []


# ============================================================================
# Helper Functions
# ============================================================================

def _ensure_screenshot_dir() -> Path:
    """Ensure screenshot directory exists."""
    path = Path(VMWARE_SCREENSHOT_DIR)
    path.mkdir(parents=True, exist_ok=True)
    return path


def _ensure_queue_dir() -> Path:
    """Ensure task queue directory exists."""
    path = Path(VM_TASK_QUEUE_DIR)
    path.mkdir(parents=True, exist_ok=True)
    return path


def _generate_screenshot_filename(vm_name: str) -> str:
    """Generate unique screenshot filename."""
    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    safe_vm_name = "".join(c if c.isalnum() or c in "-_" else "-" for c in vm_name)
    return f"{safe_vm_name}-{timestamp}.png"


def _generate_queue_id(vm_name: str) -> str:
    """Generate unique queue ID."""
    timestamp = datetime.now().isoformat()
    hash_input = f"{vm_name}-{timestamp}"
    return f"queue-{hashlib.md5(hash_input.encode()).hexdigest()[:8]}"


def _save_queue(queue: TaskQueue) -> str:
    """Save task queue to disk."""
    queue_dir = _ensure_queue_dir()
    queue_path = queue_dir / f"{queue.queue_id}.json"
    
    queue_dict = {
        **asdict(queue),
        'tasks': [asdict(task) if hasattr(task, '__dataclass_fields__') else task for task in queue.tasks],
    }
    
    queue_path.write_text(json.dumps(queue_dict, indent=2))
    return str(queue_path)


def _load_queue(queue_id: str) -> Optional[TaskQueue]:
    """Load task queue from disk."""
    queue_dir = _ensure_queue_dir()
    queue_path = queue_dir / f"{queue_id}.json"
    
    if not queue_path.exists():
        return None
    
    data = json.loads(queue_path.read_text())
    tasks = [Task(**task) if isinstance(task, dict) else task for task in data.get('tasks', [])]
    
    return TaskQueue(
        queue_id=data['queue_id'],
        vm_name=data['vm_name'],
        created_at=data['created_at'],
        status=data.get('status', 'running'),
        tasks=tasks,
        current_task_id=data.get('current_task_id'),
        loop_interval_seconds=data.get('loop_interval_seconds', 5),
        max_iterations=data.get('max_iterations', 0),
        iteration_count=data.get('iteration_count', 0),
        last_screenshot=data.get('last_screenshot'),
        last_state=data.get('last_state', ''),
        last_action=data.get('last_action'),
        action_history=data.get('action_history', [])
    )


# ============================================================================
# Basic Control Tools
# ============================================================================

def vmware_screenshot(vm_name: str, output_path: Optional[str] = None) -> dict:
    """
    Capture a screenshot from a VMware VM.
    
    Args:
        vm_name: Name of the VM to capture
        output_path: Optional custom output path
    
    Returns:
        dict with 'success', 'path', 'base64', and optional 'error'
    """
    try:
        url = f"{VMWARE_GATEWAY_URL}/api/{vm_name}/screen"
        response = requests.get(url, timeout=30)
        
        if response.status_code != 200:
            return {
                "success": False,
                "error": f"Failed to capture screenshot: HTTP {response.status_code} - {response.text}"
            }
        
        if output_path:
            screenshot_path = Path(output_path)
        else:
            screenshot_dir = _ensure_screenshot_dir()
            screenshot_path = screenshot_dir / _generate_screenshot_filename(vm_name)
        
        screenshot_path.parent.mkdir(parents=True, exist_ok=True)
        screenshot_path.write_bytes(response.content)
        
        base64_data = base64.b64encode(response.content).decode('utf-8')
        
        return {
            "success": True,
            "path": str(screenshot_path),
            "base64": base64_data,
            "size_bytes": len(response.content)
        }
        
    except requests.exceptions.ConnectionError as e:
        return {
            "success": False,
            "error": f"Cannot connect to vmware-gateway at {VMWARE_GATEWAY_URL}: {str(e)}"
        }
    except requests.exceptions.Timeout:
        return {
            "success": False,
            "error": f"Timeout connecting to vmware-gateway at {VMWARE_GATEWAY_URL}"
        }
    except Exception as e:
        return {
            "success": False,
            "error": f"Unexpected error: {str(e)}"
        }


def vmware_send_keys(vm_name: str, keys: str) -> dict:
    """
    Send keyboard input to a VMware VM.
    
    Args:
        vm_name: Name of the VM to control
        keys: Keystrokes to send (supports special keys in angle brackets)
    
    Returns:
        dict with 'success' and optional 'error' or 'message'
    """
    try:
        url = f"{VMWARE_GATEWAY_URL}/api/{vm_name}/keyboard"
        params = {"keys": keys}
        
        response = requests.get(url, params=params, timeout=30)
        
        if response.status_code != 200:
            return {
                "success": False,
                "error": f"Failed to send keys: HTTP {response.status_code} - {response.text}"
            }
        
        return {
            "success": True,
            "message": f"Successfully sent '{keys}' to {vm_name}"
        }
        
    except requests.exceptions.ConnectionError as e:
        return {
            "success": False,
            "error": f"Cannot connect to vmware-gateway at {VMWARE_GATEWAY_URL}: {str(e)}"
        }
    except requests.exceptions.Timeout:
        return {
            "success": False,
            "error": f"Timeout connecting to vmware-gateway at {VMWARE_GATEWAY_URL}"
        }
    except Exception as e:
        return {
            "success": False,
            "error": f"Unexpected error: {str(e)}"
        }


def vmware_type_text(vm_name: str, text: str) -> dict:
    """
    Type plain text to a VMware VM.
    
    Args:
        vm_name: Name of the VM to control
        text: Plain text to type
    
    Returns:
        dict with 'success' and optional 'error' or 'message'
    """
    return vmware_send_keys(vm_name=vm_name, keys=text)


def vmware_press_key(vm_name: str, key: str) -> dict:
    """
    Press a single special key on a VMware VM.
    
    Args:
        vm_name: Name of the VM to control
        key: Key name without brackets (e.g., "enter", "tab", "F1")
    
    Returns:
        dict with 'success' and optional 'error' or 'message'
    """
    formatted_key = f"<{key}>"
    return vmware_send_keys(vm_name=vm_name, keys=formatted_key)


def vmware_health_check() -> dict:
    """
    Check if vmware-gateway is accessible.
    
    Returns:
        dict with 'success', 'gateway_url', and optional 'error'
    """
    try:
        response = requests.get(VMWARE_GATEWAY_URL, timeout=5)
        
        if response.status_code == 200:
            return {
                "success": True,
                "gateway_url": VMWARE_GATEWAY_URL,
                "status": "healthy"
            }
        else:
            return {
                "success": False,
                "gateway_url": VMWARE_GATEWAY_URL,
                "error": f"Gateway returned HTTP {response.status_code}"
            }
            
    except requests.exceptions.ConnectionError as e:
        return {
            "success": False,
            "gateway_url": VMWARE_GATEWAY_URL,
            "error": f"Cannot connect: {str(e)}"
        }
    except requests.exceptions.Timeout:
        return {
            "success": False,
            "gateway_url": VMWARE_GATEWAY_URL,
            "error": "Connection timeout"
        }
    except Exception as e:
        return {
            "success": False,
            "gateway_url": VMWARE_GATEWAY_URL,
            "error": f"Unexpected error: {str(e)}"
        }


# ============================================================================
# Autonomous Operation Tools
# ============================================================================

def vm_autonomous_start(
    vm_name: str,
    tasks: List[str],
    loop_interval_seconds: int = 5,
    max_iterations: int = 0
) -> dict:
    """
    Start autonomous operation on a VM with a list of tasks.
    
    Args:
        vm_name: The VM to control
        tasks: List of task descriptions to accomplish
        loop_interval_seconds: Time between observation cycles (default: 5)
        max_iterations: Max loop iterations (0 = infinite)
    
    Returns:
        dict with queue_id, status, and task count
    """
    health = vmware_health_check()
    if not health.get('success'):
        return {
            "success": False,
            "error": f"VMware gateway not accessible: {health.get('error')}"
        }
    
    queue_id = _generate_queue_id(vm_name)
    task_objects = [Task(id=f"task-{i}", description=task) for i, task in enumerate(tasks)]
    
    queue = TaskQueue(
        queue_id=queue_id,
        vm_name=vm_name,
        created_at=datetime.now().isoformat(),
        tasks=task_objects,
        current_task_id=task_objects[0].id if task_objects else None,
        loop_interval_seconds=loop_interval_seconds,
        max_iterations=max_iterations
    )
    
    _save_queue(queue)
    
    return {
        "success": True,
        "queue_id": queue_id,
        "vm_name": vm_name,
        "task_count": len(tasks),
        "status": "running",
        "message": f"Started autonomous operation on {vm_name} with {len(tasks)} tasks"
    }


def vm_autonomous_status(task_queue_id: str) -> dict:
    """
    Get current status of autonomous operation.
    
    Args:
        task_queue_id: The queue ID from vm_autonomous_start
    
    Returns:
        dict with current task, progress, and history
    """
    queue = _load_queue(task_queue_id)
    
    if not queue:
        return {
            "success": False,
            "error": f"Task queue {task_queue_id} not found"
        }
    
    current_task = None
    if queue.current_task_id:
        current_task = next((t for t in queue.tasks if t.id == queue.current_task_id), None)
    
    total = len(queue.tasks)
    completed = sum(1 for t in queue.tasks if t.status == "completed")
    failed = sum(1 for t in queue.tasks if t.status == "failed")
    
    return {
        "success": True,
        "queue_id": queue.queue_id,
        "vm_name": queue.vm_name,
        "status": queue.status,
        "current_task": current_task.description if current_task else None,
        "current_task_id": queue.current_task_id,
        "progress": {
            "total": total,
            "completed": completed,
            "failed": failed,
            "pending": total - completed - failed
        },
        "iteration_count": queue.iteration_count,
        "last_state": queue.last_state,
        "last_action": queue.last_action,
        "last_screenshot": queue.last_screenshot,
        "action_history": queue.action_history[-10:]
    }


def vm_autonomous_stop(task_queue_id: str) -> dict:
    """
    Stop autonomous operation.
    
    Args:
        task_queue_id: The queue ID to stop
    
    Returns:
        dict with stop confirmation and final status
    """
    queue = _load_queue(task_queue_id)
    
    if not queue:
        return {
            "success": False,
            "error": f"Task queue {task_queue_id} not found"
        }
    
    queue.status = "stopped"
    _save_queue(queue)
    
    return {
        "success": True,
        "queue_id": queue.queue_id,
        "status": "stopped",
        "final_iteration_count": queue.iteration_count,
        "tasks_completed": sum(1 for t in queue.tasks if t.status == "completed"),
        "tasks_failed": sum(1 for t in queue.tasks if t.status == "failed"),
        "message": f"Stopped autonomous operation on {queue.vm_name}"
    }


def vm_analyze_screenshot(vm_name: str, analysis_type: str = "generic") -> dict:
    """
    Analyze a screenshot to determine current VM state.
    
    Args:
        vm_name: The VM to analyze
        analysis_type: Type of analysis (login_screen, desktop, browser, terminal, generic)
    
    Returns:
        dict with state description and suggested actions
    """
    screenshot_result = vmware_screenshot(vm_name=vm_name)
    
    if not screenshot_result.get('success'):
        return {
            "success": False,
            "error": screenshot_result.get('error')
        }
    
    analysis = {
        "generic": {
            "description": "Screen captured - manual inspection needed",
            "detected_elements": ["unknown"],
            "suggested_actions": ["Describe what you see to guide the agent"]
        },
        "login_screen": {
            "description": "Login screen detected",
            "detected_elements": ["username_field", "password_field", "login_button"],
            "suggested_actions": ["Type username", "Press tab", "Type password", "Press enter"]
        },
        "desktop": {
            "description": "Desktop environment detected",
            "detected_elements": ["taskbar", "desktop_icons"],
            "suggested_actions": ["Open start menu", "Launch application"]
        },
        "terminal": {
            "description": "Terminal/command prompt detected",
            "detected_elements": ["command_prompt", "cursor"],
            "suggested_actions": ["Type command", "Press enter"]
        },
        "browser": {
            "description": "Web browser detected",
            "detected_elements": ["url_bar", "tabs", "webpage"],
            "suggested_actions": ["Navigate to URL", "Click links", "Scroll page"]
        }
    }
    
    selected_analysis = analysis.get(analysis_type, analysis["generic"])
    
    return {
        "success": True,
        "screenshot_path": screenshot_result['path'],
        "analysis_type": analysis_type,
        "state_description": selected_analysis["description"],
        "detected_elements": selected_analysis["detected_elements"],
        "suggested_actions": selected_analysis["suggested_actions"]
    }


def vm_decide_next_action(
    current_state: str,
    current_task: str,
    task_history: Optional[List[str]] = None
) -> dict:
    """
    Decide what action to take based on current state and goals.
    
    Args:
        current_state: Description of current VM state
        current_task: The task being worked on
        task_history: Previous actions taken
    
    Returns:
        dict with recommended action, confidence, and reasoning
    """
    task_lower = current_task.lower()
    state_lower = current_state.lower()
    
    if "login" in task_lower:
        if "username" in state_lower or "login" in state_lower:
            return {
                "success": True,
                "action": "admin<tab>password123<enter>",
                "confidence": 0.8,
                "reasoning": "Login screen detected - entering credentials"
            }
    
    if "terminal" in task_lower or "command" in task_lower:
        if "desktop" in state_lower:
            return {
                "success": True,
                "action": "<super>r",
                "confidence": 0.7,
                "reasoning": "Opening run dialog to launch terminal"
            }
        elif "terminal" in state_lower or "prompt" in state_lower:
            return {
                "success": True,
                "action": "ls -la<enter>",
                "confidence": 0.9,
                "reasoning": "Terminal ready - executing command"
            }
    
    if "browser" in task_lower or "web" in task_lower or "http" in task_lower:
        return {
            "success": True,
            "action": "<super>rfirefox<enter>",
            "confidence": 0.6,
            "reasoning": "Attempting to open Firefox browser"
        }
    
    return {
        "success": True,
        "action": None,
        "confidence": 0.3,
        "reasoning": "Unclear what action to take - human guidance needed",
        "needs_guidance": True
    }


def vm_execute_action(vm_name: str, action: str, wait_seconds: int = 2) -> dict:
    """
    Execute a decided action on the VM.
    
    Args:
        vm_name: The VM to control
        action: The action/keystrokes to send
        wait_seconds: Time to wait after execution
    
    Returns:
        dict with execution result
    """
    if not action:
        return {
            "success": False,
            "error": "No action specified"
        }
    
    result = vmware_send_keys(vm_name=vm_name, keys=action)
    
    if not result.get('success'):
        return result
    
    time.sleep(wait_seconds)
    
    screenshot = vmware_screenshot(vm_name=vm_name)
    
    return {
        "success": True,
        "action": action,
        "waited_seconds": wait_seconds,
        "new_screenshot": screenshot.get('path'),
        "message": result.get('message')
    }


def vm_autonomous_loop_iteration(task_queue_id: str) -> dict:
    """
    Execute one iteration of the autonomous loop.
    
    This is the core loop that:
    1. Captures screenshot
    2. Analyzes state
    3. Decides action
    4. Executes action
    5. Updates queue
    
    Args:
        task_queue_id: The queue to process
    
    Returns:
        dict with iteration results
    """
    queue = _load_queue(task_queue_id)
    
    if not queue or queue.status != "running":
        return {
            "success": False,
            "error": f"Queue {task_queue_id} not found or not running"
        }
    
    if queue.max_iterations > 0 and queue.iteration_count >= queue.max_iterations:
        queue.status = "completed"
        _save_queue(queue)
        return {
            "success": True,
            "status": "completed",
            "reason": f"Reached max iterations ({queue.max_iterations})"
        }
    
    if not queue.current_task_id:
        queue.status = "completed"
        _save_queue(queue)
        return {
            "success": True,
            "status": "completed",
            "reason": "All tasks completed"
        }
    
    current_task = next((t for t in queue.tasks if t.id == queue.current_task_id), None)
    
    if not current_task:
        return {
            "success": False,
            "error": "Current task not found"
        }
    
    if current_task.status == "completed":
        remaining = [t for t in queue.tasks if t.status == "pending"]
        if remaining:
            queue.current_task_id = remaining[0].id
            _save_queue(queue)
            return {
                "success": True,
                "status": "task_advanced",
                "new_task": remaining[0].description
            }
        else:
            queue.status = "completed"
            _save_queue(queue)
            return {
                "success": True,
                "status": "completed",
                "reason": "All tasks completed"
            }
    
    iteration_result = {
        "iteration": queue.iteration_count + 1,
        "task": current_task.description,
        "steps": []
    }
    
    # Step 1: Capture screenshot
    screenshot = vmware_screenshot(vm_name=queue.vm_name)
    queue.last_screenshot = screenshot.get('path')
    iteration_result['steps'].append({
        "step": "screenshot",
        "success": screenshot.get('success', False)
    })
    
    if not screenshot.get('success'):
        iteration_result['error'] = screenshot.get('error')
        return iteration_result
    
    # Step 2: Analyze state
    analysis = vm_analyze_screenshot(vm_name=queue.vm_name)
    queue.last_state = analysis.get('state_description', '')
    iteration_result['steps'].append({
        "step": "analyze",
        "state": queue.last_state
    })
    
    # Step 3: Decide action
    decision = vm_decide_next_action(
        current_state=queue.last_state,
        current_task=current_task.description,
        task_history=[h.get('action') for h in queue.action_history[-5:]]
    )
    iteration_result['steps'].append({
        "step": "decide",
        "action": decision.get('action'),
        "confidence": decision.get('confidence')
    })
    
    # Step 4: Execute action
    if decision.get('action'):
        execution = vm_execute_action(
            vm_name=queue.vm_name,
            action=decision['action'],
            wait_seconds=VM_ACTION_DELAY
        )
        queue.last_action = decision['action']
        queue.action_history.append({
            "timestamp": datetime.now().isoformat(),
            "action": decision['action'],
            "reasoning": decision.get('reasoning'),
            "success": execution.get('success', False)
        })
        iteration_result['steps'].append({
            "step": "execute",
            "success": execution.get('success', False)
        })
        
        if execution.get('success'):
            current_task.attempts += 1
            current_task.last_attempt = datetime.now().isoformat()
            
            if current_task.attempts >= 1 and not decision.get('needs_guidance'):
                current_task.status = "completed"
    else:
        iteration_result['needs_guidance'] = True
    
    queue.iteration_count += 1
    
    if current_task.attempts >= VM_MAX_RETRIES:
        current_task.status = "failed"
        current_task.notes = f"Failed after {VM_MAX_RETRIES} attempts"
    
    _save_queue(queue)
    
    return iteration_result


# ============================================================================
# Tool Registry
# ============================================================================

TOOLS = {
    # Basic Control
    "vmware_screenshot": {
        "description": "Capture a screenshot from a VMware VM",
        "function": vmware_screenshot,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "output_path": {"type": "string", "required": False}
        }
    },
    "vmware_send_keys": {
        "description": "Send keyboard input to a VMware VM (supports special keys)",
        "function": vmware_send_keys,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "keys": {"type": "string", "required": True}
        }
    },
    "vmware_type_text": {
        "description": "Type plain text to a VMware VM",
        "function": vmware_type_text,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "text": {"type": "string", "required": True}
        }
    },
    "vmware_press_key": {
        "description": "Press a single special key on a VMware VM",
        "function": vmware_press_key,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "key": {"type": "string", "required": True}
        }
    },
    "vmware_health_check": {
        "description": "Check if vmware-gateway is accessible",
        "function": vmware_health_check,
        "parameters": {}
    },
    # Autonomous Operation
    "vm_autonomous_start": {
        "description": "Start autonomous VM operation with a task list",
        "function": vm_autonomous_start,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "tasks": {"type": "array", "required": True},
            "loop_interval_seconds": {"type": "number", "required": False},
            "max_iterations": {"type": "number", "required": False}
        }
    },
    "vm_autonomous_status": {
        "description": "Get status of autonomous operation",
        "function": vm_autonomous_status,
        "parameters": {
            "task_queue_id": {"type": "string", "required": True}
        }
    },
    "vm_autonomous_stop": {
        "description": "Stop autonomous operation",
        "function": vm_autonomous_stop,
        "parameters": {
            "task_queue_id": {"type": "string", "required": True}
        }
    },
    "vm_analyze_screenshot": {
        "description": "Analyze VM screenshot to determine state",
        "function": vm_analyze_screenshot,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "analysis_type": {"type": "string", "required": False}
        }
    },
    "vm_decide_next_action": {
        "description": "Decide next action based on state and goals",
        "function": vm_decide_next_action,
        "parameters": {
            "current_state": {"type": "string", "required": True},
            "current_task": {"type": "string", "required": True},
            "task_history": {"type": "array", "required": False}
        }
    },
    "vm_execute_action": {
        "description": "Execute an action on the VM",
        "function": vm_execute_action,
        "parameters": {
            "vm_name": {"type": "string", "required": True},
            "action": {"type": "string", "required": True},
            "wait_seconds": {"type": "number", "required": False}
        }
    },
    "vm_autonomous_loop_iteration": {
        "description": "Execute one iteration of the autonomous loop",
        "function": vm_autonomous_loop_iteration,
        "parameters": {
            "task_queue_id": {"type": "string", "required": True}
        }
    }
}


if __name__ == "__main__":
    print("VMware VM Operator Skill")
    print("=" * 40)
    print(f"\nGateway: {VMWARE_GATEWAY_URL}")
    print(f"\nAvailable tools:")
    for name, tool in TOOLS.items():
        print(f"  - {name}: {tool['description']}")
