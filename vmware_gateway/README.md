# vSphere Proxy

A Flask-based HTTP proxy for remote VMware vSphere VM control. Send keyboard input and capture screenshots from VMs via a simple REST API.

## Features

- **Remote Keyboard Input**: Send keystrokes to VMs using USB HID scan codes
- **Screenshot Capture**: Grab screen images from VMs
- **VM Whitelist**: Regex-based VM name validation for security
- **Special Key Support**: Handle function keys, modifiers, arrows, and more
- **Modifier State Tracking**: Support for shift/ctrl/alt/super toggle sequences

## Requirements

- Python 3.12+
- VMware vSphere/ESXi with API access
- Network connectivity to vCenter/ESXi host

## Installation

### Using uv (Recommended)

```bash
# Clone the repository
git clone <repository-url>
cd applied_project

# Install dependencies
uv sync

# Run the server
uv run gunicorn main:app -b 0.0.0.0:8888
```

### Using pip

```bash
pip install flask gunicorn pyvmomi
python main.py
```

### Docker

```bash
docker build -t vsphere-proxy .
docker run -p 8888:8888 \
  -e VC_HOST=vcenter.example.com \
  -e VC_USER=administrator@vsphere.local \
  -e VC_PASS=your_password \
  -e VC_PORT=443 \
  -e VM_WHITE_LIST=".*" \
  vsphere-proxy
```

## Configuration

Set the following environment variables:

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `VC_HOST` | Yes | - | vCenter/ESXi hostname or IP |
| `VC_USER` | Yes | - | vSphere username (e.g., `administrator@vsphere.local`) |
| `VC_PASS` | Yes | - | vSphere password |
| `VC_PORT` | No | `443` | vSphere API port |
| `VM_WHITE_LIST` | No | `.*` | Regex pattern to whitelist VM names |

### Example Environment Setup

```bash
export VC_HOST="vcenter.example.com"
export VC_USER="administrator@vsphere.local"
export VC_PASS="SuperSecretPassword"
export VC_PORT="443"
export VM_WHITE_LIST="^prod-.*"  # Only allow VMs starting with "prod-"
```

## API Endpoints

### Send Keyboard Input

```
GET /api/<vm_name>/keyboard?keys=<keys>
```

**Parameters:**
- `vm_name` - Target VM name (validated against whitelist)
- `keys` - Keystrokes to send (URL-encoded)

**Special Key Syntax:**
Wrap special keys in angle brackets:

| Key | Syntax |
|-----|--------|
| Escape | `<esc>` |
| Tab | `<tab>` |
| Enter | `<enter>` |
| Backspace | `<backspace>` |
| Delete | `<delete>` |
| Arrow Keys | `<up>`, `<down>`, `<left>`, `<right>` |
| Function Keys | `<F1>` through `<F12>` |
| Home/End | `<home>`, `<end>` |
| Page Up/Down | `<pageup>`, `<pagedown>` |
| Windows/Super | `<super>`, `<win>`, `<meta>` |
| Print Screen | `<printscreen>` |
| Scroll Lock | `<scrolllock>` |

**Modifier Toggles:**
Hold modifiers across multiple keys:

```
<shift_on>hello<shift_off>     # Types "HELLO"
<ctrl_on>c<ctrl_off>           # Sends Ctrl+C
<alt_on><F4><alt_off>          # Sends Alt+F4
```

**Examples:**

```bash
# Type "Hello World"
curl "http://localhost:8888/api/my-vm/keyboard?keys=Hello%20World"

# Press Enter
curl "http://localhost:8888/api/my-vm/keyboard?keys=%3Center%3E"

# Send Ctrl+C
curl "http://localhost:8888/api/my-vm/keyboard?keys=%3Cctrl_on%3Ec%3Cctrl_off%3E"

# Type "ls -la" then Enter
curl "http://localhost:8888/api/my-vm/keyboard?keys=ls%20-la%3Center%3E"

# Press Alt+F4
curl "http://localhost:8888/api/my-vm/keyboard?keys=%3Calt_on%3E%3CF4%3E%3Calt_off%3E"
```

### Capture Screenshot

```
GET /api/<vm_name>/screen
```

Returns a PNG image of the VM's current screen.

**Example:**

```bash
# Download screenshot
curl -o screenshot.png "http://localhost:8888/api/my-vm/screen"

# View in browser
open "http://localhost:8888/api/my-vm/screen"
```

### Health Check

```
GET /
```

Returns `OK` if the server is running.

## Security Considerations

1. **VM Whitelist**: Always set `VM_WHITE_LIST` to restrict which VMs can be controlled. Example patterns:
   - `^prod-.*` - Only VMs starting with "prod-"
   - `^(web|app)-.*` - Only VMs starting with "web-" or "app-"
   - `.*` - All VMs (not recommended for production)

2. **Network Security**: Run behind a reverse proxy with TLS. The server has no built-in authentication.

3. **vSphere Credentials**: Use a dedicated service account with minimal required permissions.

4. **SSL Verification**: The proxy disables SSL verification for vSphere connections. Ensure your network is trusted.

## Usage Examples

### Basic Remote Control Script

```python
import requests

BASE_URL = "http://localhost:8888"
VM_NAME = "my-vm"

def type_text(text):
    requests.get(f"{BASE_URL}/api/{VM_NAME}/keyboard", params={"keys": text})

def press_key(key):
    requests.get(f"{BASE_URL}/api/{VM_NAME}/keyboard", params={"keys": f"<{key}>"})

def get_screenshot():
    resp = requests.get(f"{BASE_URL}/api/{VM_NAME}/screen")
    with open("screenshot.png", "wb") as f:
        f.write(resp.content)

# Example: Open terminal and run a command
type_text("ls -la")
press_key("enter")
```

### Automation with Screenshot Verification

```bash
# Type a command
curl "http://localhost:8888/api/my-vm/keyboard?keys=uname%20-a%3Center%3E"

# Wait for execution
sleep 2

# Capture result
curl -o result.png "http://localhost:8888/api/my-vm/screen"
```

## Troubleshooting

### "VM not found"
- Verify the VM name matches exactly (case-sensitive)
- Check that the VM is registered and visible in vSphere
- Ensure your credentials have permission to view the VM

### "Connection refused" or SSL errors
- Verify `VC_HOST` is reachable
- Check `VC_PORT` (default 443)
- Ensure vSphere API is enabled on the target

### Keys not typing correctly
- Some characters require shift (e.g., `!`, `@`, `#`) - these are handled automatically
- Non-US keyboard layouts may not match the HID scan codes
- Special characters can be escaped: `\<` for literal `<`, `\\` for literal `\`

### Screenshots fail
- VM must be powered on
- VMware Tools should be installed for best results
- Check vSphere permissions for datastore access

## License

MIT - See LICENSE file

## Author

Matthew Howard <matthew@hattmo.com>
