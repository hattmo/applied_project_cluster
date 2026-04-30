from pyVim.connect import SmartConnect, Disconnect
from pyVmomi import vim
import ssl
import os
import atexit
import time
import io
import re
from urllib.parse import urlencode
from flask import Flask, request, send_file
import urllib.request

# VM whitelist regex - set in main()

app = Flask(__name__)

hid_table: dict[str, int] = {
    "KEY_MOD_LCTRL": 0x01,
    "KEY_MOD_LSHIFT": 0x02,
    "KEY_MOD_LALT": 0x04,
    "KEY_MOD_LMETA": 0x08,
    "KEY_MOD_RCTRL": 0x10,
    "KEY_MOD_RSHIFT": 0x20,
    "KEY_MOD_RALT": 0x40,
    "KEY_MOD_RMETA": 0x80,
    "a": 0x04,
    "b": 0x05,
    "c": 0x06,
    "d": 0x07,
    "e": 0x08,
    "f": 0x09,
    "g": 0x0A,
    "h": 0x0B,
    "i": 0x0C,
    "j": 0x0D,
    "k": 0x0E,
    "l": 0x0F,
    "m": 0x10,
    "n": 0x11,
    "o": 0x12,
    "p": 0x13,
    "q": 0x14,
    "r": 0x15,
    "s": 0x16,
    "t": 0x17,
    "u": 0x18,
    "v": 0x19,
    "w": 0x1A,
    "x": 0x1B,
    "y": 0x1C,
    "z": 0x1D,
    "1": 0x1E,
    "2": 0x1F,
    "3": 0x20,
    "4": 0x21,
    "5": 0x22,
    "6": 0x23,
    "7": 0x24,
    "8": 0x25,
    "9": 0x26,
    "0": 0x27,
    "KEY_ENTER": 0x28,
    "KEY_ESC": 0x29,
    "KEY_BACKSPACE": 0x2A,
    "KEY_TAB": 0x2B,
    " ": 0x2C,
    "-": 0x2D,
    "=": 0x2E,
    "[": 0x2F,
    "]": 0x30,
    "\\": 0x31,
    "KEY_HASHTILDE": 0x32,
    ";": 0x33,
    "'": 0x34,
    "`": 0x35,
    ",": 0x36,
    ".": 0x37,
    "/": 0x38,
    "KEY_CAPSLOCK": 0x39,
    "KEY_F1": 0x3A,
    "KEY_F2": 0x3B,
    "KEY_F3": 0x3C,
    "KEY_F4": 0x3D,
    "KEY_F5": 0x3E,
    "KEY_F6": 0x3F,
    "KEY_F7": 0x40,
    "KEY_F8": 0x41,
    "KEY_F9": 0x42,
    "KEY_F10": 0x43,
    "KEY_F11": 0x44,
    "KEY_F12": 0x45,
    "KEY_SYSRQ": 0x46,
    "KEY_SCROLLLOCK": 0x47,
    "KEY_PAUSE": 0x48,
    "KEY_INSERT": 0x49,
    "KEY_HOME": 0x4A,
    "KEY_PAGEUP": 0x4B,
    "KEY_DELETE": 0x4C,
    "KEY_END": 0x4D,
    "KEY_PAGEDOWN": 0x4E,
    "KEY_RIGHT": 0x4F,
    "KEY_LEFT": 0x50,
    "KEY_DOWN": 0x51,
    "KEY_UP": 0x52,
    "KEY_NUMLOCK": 0x53,
    "KEY_F13": 0x68,
    "KEY_F14": 0x69,
    "KEY_F15": 0x6A,
    "KEY_F16": 0x6B,
    "KEY_F17": 0x6C,
    "KEY_F18": 0x6D,
    "KEY_F19": 0x6E,
    "KEY_F20": 0x6F,
    "KEY_F21": 0x70,
    "KEY_F22": 0x71,
    "KEY_F23": 0x72,
    "KEY_F24": 0x73,
    # Super/Windows/Meta keys
    "KEY_SUPER_L": 0xE3,
    "KEY_SUPER_R": 0xE7,
    "KEY_META": 0xE3,
    "KEY_WIN": 0xE3,
}

# Mapping of shifted characters to (base_key, modifier_bit)
# These need shift modifier applied
shifted_chars = {
    "!": ("1", 2),
    "@": ("2", 2),
    "#": ("3", 2),
    "$": ("4", 2),
    "%": ("5", 2),
    "^": ("6", 2),
    "&": ("7", 2),
    "*": ("8", 2),
    "(": ("9", 2),
    ")": ("0", 2),
    "_": ("-", 2),
    "+": ("=", 2),
    "{": ("[", 2),
    "}": ("]", 2),
    "|": ("\\", 2),
    ":": (";", 2),
    '"': ("'", 2),
    "<": (",", 2),
    ">": (".", 2),
    "?": ("/", 2),
    "~": ("`", 2),
}


class VM_Connection:
    def __init__(self, vm_name: str):
        host = os.environ.get("VC_HOST")
        un = os.environ.get("VC_USER")
        pw = os.environ.get("VC_PASS")
        if not host or not un or not pw:
            raise Exception("VC_HOST, VC_USER, and VC_PASS not set")
        try:
            port = int(os.environ.get("VC_PORT", "443"))
        except ValueError:
            raise Exception("VC_PORT not set")
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        connection = SmartConnect(
            host=host,
            user=un,
            pwd=pw,
            port=port,
            sslContext=ctx,
        )
        atexit.register(Disconnect, connection)
        content = connection.RetrieveContent()
        if content.viewManager:
            container = content.viewManager.CreateContainerView(
                content.rootFolder, [vim.VirtualMachine], True
            )
        else:
            raise Exception("Cannot create view")
        vms = container.view
        target_vm = next((vm for vm in vms if vm.name == vm_name), None)
        if not target_vm:
            raise Exception("VM not found")
        # container.Destroy()
        self.vm = target_vm
        self.connection = connection

    def send_keys(self, keys: str):
        if not keys or len(keys.strip()) == 0:
            raise ValueError("No keys provided")

        # Parse special key sequences
        parsed_keys = self._parse_special_keys(keys)

        if len(parsed_keys) == 0:
            raise ValueError("No valid keys parsed")

        spec = vim.vm.UsbScanCodeSpec()

        for item in parsed_keys:
            # Handle tuple format (type, key_name, modifier_bits)
            key_type = item[0] if isinstance(item, tuple) else "REGULAR"
            key_name = item[1] if isinstance(item, tuple) else item
            modifier_bits = item[2] if isinstance(item, tuple) else 0

            # Apply shift to uppercase if not already handled by modifier
            if key_type == "REGULAR" and key_name.isupper() and key_name.isalpha():
                modifier_bits |= 2

            # Handle shifted characters (!, @, #, etc.)
            if key_type == "REGULAR" and key_name in shifted_chars:
                base_key, shift_bit = shifted_chars[key_name]
                key_name = base_key
                modifier_bits |= shift_bit

            event = vim.vm.UsbScanCodeSpec.KeyEvent()

            # Build ModifierType if needed
            if modifier_bits > 0:
                modifier = vim.vm.UsbScanCodeSpec.ModifierType()
                if modifier_bits & 1:  # ctrl
                    modifier.leftCtrl = True
                if modifier_bits & 2:  # shift
                    modifier.leftShift = True
                if modifier_bits & 4:  # alt
                    modifier.leftAlt = True
                if modifier_bits & 8:  # super/meta (Windows key)
                    modifier.leftMeta = True
                event.modifiers = modifier

            # Send the key (use lowercase for letters)
            lookup_key = key_name.lower() if key_name.isalpha() else key_name
            hid = hid_table.get(lookup_key)
            if hid:
                event.usbHidCode = (hid << 16) | 7
            else:
                continue
            spec.keyEvents.append(event)

            # Release key
            release_event = vim.vm.UsbScanCodeSpec.KeyEvent()
            release_event.usbHidCode = (hid << 16) | 0
            spec.keyEvents.append(release_event)
        self.vm.PutUsbScanCodes(spec)

    def _parse_special_keys(self, keys: str) -> list:
        """Parse special key sequences like <esc>, <F1>, <tab>, etc."""
        import re

        result = []

        # First, handle escape sequences: \\ -> literal \, \< -> literal <
        escaped_keys = ""
        i = 0
        while i < len(keys):
            if keys[i] == "\\" and i + 1 < len(keys):
                if keys[i + 1] == "\\":
                    escaped_keys += "\\"
                    i += 2
                elif keys[i + 1] == "<":
                    escaped_keys += "<"
                    i += 2
                else:
                    escaped_keys += keys[i]
                    i += 1
            else:
                escaped_keys += keys[i]
                i += 1

        keys = escaped_keys

        # Track modifier state
        shift_on = False
        alt_on = False
        ctrl_on = False
        super_on = False

        # Find all special key patterns
        pattern = r"<([^>]+)>"
        last_end = 0

        for match in re.finditer(pattern, keys):
            if match.start() > last_end:
                # Get text between last special key and this one
                text_between = keys[last_end : match.start()]
                # Add each character with current modifier state
                for char in text_between:
                    modifier_bits = 0
                    if shift_on:
                        modifier_bits |= 2
                    if alt_on:
                        modifier_bits |= 4
                    if ctrl_on:
                        modifier_bits |= 1
                    if super_on:
                        modifier_bits |= 8
                    result.append(("REGULAR", char, modifier_bits))

            special_key = match.group(1).upper()

            # Handle modifier toggle keys
            if special_key == "SHIFT_ON":
                shift_on = True
                last_end = match.end()
                continue
            elif special_key == "SHIFT_OFF":
                shift_on = False
                last_end = match.end()
                continue
            elif special_key == "ALT_ON":
                alt_on = True
                last_end = match.end()
                continue
            elif special_key == "ALT_OFF":
                alt_on = False
                last_end = match.end()
                continue
            elif special_key == "CTRL_ON":
                ctrl_on = True
                last_end = match.end()
                continue
            elif special_key == "CTRL_OFF":
                ctrl_on = False
                last_end = match.end()
                continue
            elif special_key in ("SUPER_ON", "WIN_ON", "META_ON"):
                super_on = True
                last_end = match.end()
                continue
            elif special_key in ("SUPER_OFF", "WIN_OFF", "META_OFF"):
                super_on = False
                last_end = match.end()
                continue

            # Build modifier bits (1=ctrl, 2=shift, 4=alt, 8=super/meta)
            modifier_bits = 0
            if shift_on:
                modifier_bits |= 2
            if alt_on:
                modifier_bits |= 4
            if ctrl_on:
                modifier_bits |= 1
            if super_on:
                modifier_bits |= 8

            # Normalize key names
            if special_key == "ESC":
                special_key = "KEY_ESC"
            elif special_key in (
                "F1",
                "F2",
                "F3",
                "F4",
                "F5",
                "F6",
                "F7",
                "F8",
                "F9",
                "F10",
                "F11",
                "F12",
            ):
                special_key = f"KEY_{special_key}"
            elif special_key in (
                "TAB",
                "ENTER",
                "SPACE",
                "BACKSPACE",
                "DELETE",
                "INSERT",
                "HOME",
                "END",
            ):
                special_key = f"KEY_{special_key}"
            elif special_key in ("UP", "DOWN", "LEFT", "RIGHT"):
                special_key = f"KEY_{special_key}"
            elif special_key in ("PAGEUP", "PAGEDOWN"):
                special_key = f"KEY_{special_key}"
            elif special_key in ("PRINTSCREEN", "SYSRQ"):
                special_key = "KEY_SYSRQ"
            elif special_key == "SCROLLLOCK":
                special_key = "KEY_SCROLLLOCK"
            elif special_key in ("SUPER", "WIN", "META", "WINDOWS"):
                special_key = "KEY_SUPER_L"

            # Store as tuple with modifier bits
            result.append(("SPECIAL", special_key, modifier_bits))
            last_end = match.end()

        if last_end < len(keys):
            # Add remaining text with current modifier state
            text_remaining = keys[last_end:]
            for char in text_remaining:
                modifier_bits = 0
                if shift_on:
                    modifier_bits |= 2
                if alt_on:
                    modifier_bits |= 4
                if ctrl_on:
                    modifier_bits |= 1
                if super_on:
                    modifier_bits |= 8
                result.append(("REGULAR", char, modifier_bits))

        return result

    def get_screen(self) -> bytes:
        # Create screenshot task
        task = self.vm.CreateScreenshot_Task()
        while task.info.state in (
            vim.TaskInfo.State.queued,
            vim.TaskInfo.State.running,
        ):
            time.sleep(1)

        if task.info.state != vim.TaskInfo.State.success:
            raise RuntimeError(task.info.error.msg)

        datastore_path = task.info.result
        # datastore_path: "[ds1] vm/foo.png"

        # Get the VM's runtime environment
        vm_folder = self.vm.parent
        while vm_folder and not isinstance(vm_folder, vim.Datacenter):
            vm_folder = vm_folder.parent

        if not isinstance(vm_folder, vim.Datacenter):
            raise RuntimeError("Cannot find datacenter")

        datacenter = vm_folder
        ds_name = datastore_path.strip("[]").split("]")[0]
        rel_path = datastore_path.split("] ")[1].lstrip("/")

        # Build download URL
        host = os.environ.get("VC_HOST")
        resource = f"/folder/{rel_path}"
        params = {"dsName": ds_name, "dcPath": datacenter.name}
        base = f"https://{host}:443"
        url = base + resource + "?" + urlencode(params)

        # Get cookie for authentication
        connection = self.connection
        client_cookie = connection._stub.cookie
        cookie_name = client_cookie.split("=", 1)[0]
        cookie_value = client_cookie.split("=", 1)[1].split(";", 1)[0]
        cookie_path = (
            client_cookie.split("=", 1)[1].split(";", 1)[1].split(";", 1)[0].lstrip()
        )
        cookie_text = f"{cookie_name}={cookie_value}; ${cookie_path}"

        # Download the screenshot
        req = urllib.request.Request(url, headers={"Cookie": cookie_text})
        # Disable SSL verification for download too
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE

        with urllib.request.urlopen(req, context=ctx) as response:
            image_data = response.read()

        # Delete the screenshot from the datastore
        try:
            file_manager = connection.content.fileManager
            file_manager.DeleteDatastoreFile(datastore_path, datacenter)
        except Exception as e:
            print(f"Warning: Could not delete screenshot: {e}")

        return image_data


class VM_Dict:
    def __init__(self):
        self.inner: dict[str, VM_Connection] = dict()

    def __getitem__(self, key: str) -> VM_Connection:
        vm = self.inner.get(key, None)
        if not vm:
            vm = VM_Connection(key)
            self.inner[key] = vm
        return vm


vm_dict = VM_Dict()
vm_white_list = re.compile(os.environ.get("VM_WHITE_LIST", ".*"))


def validate_vm_name(vm: str) -> str:
    """Validate VM name against whitelist regex."""
    if not vm_white_list.match(vm):
        raise ValueError(f"VM name '{vm}' does not match whitelist pattern")
    return vm


@app.route("/api/<vm>/keyboard")
def send_keys(vm: str):
    validate_vm_name(vm)
    vm_conn = vm_dict[vm]
    keys = request.args.get("keys", "")

    if not keys or len(keys.strip()) == 0:
        return "No keys provided", 400

    try:
        vm_conn.send_keys(keys)
        print(vm, keys)
        return "OK"
    except ValueError as e:
        return str(e), 400
    except Exception as e:
        return f"Error: {e}", 500


@app.route("/api/<vm>/screen")
def get_screen(vm: str):
    validate_vm_name(vm)
    vm_conn = vm_dict[vm]
    image_data = vm_conn.get_screen()
    return send_file(
        io.BytesIO(image_data),
        mimetype="image/png",
        as_attachment=False,
        download_name=f"{vm}_screenshot.png",
    )


@app.route("/api/vms")
def list_vms():
    """List all VMs in vSphere that pass the whitelist regex."""
    host = os.environ.get("VC_HOST")
    un = os.environ.get("VC_USER")
    pw = os.environ.get("VC_PASS")
    if not host or not un or not pw:
        return "VMware credentials not configured", 500
    
    try:
        port = int(os.environ.get("VC_PORT", "443"))
    except ValueError:
        return "Invalid VC_PORT", 500
    
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE
    
    try:
        connection = SmartConnect(
            host=host,
            user=un,
            pwd=pw,
            port=port,
            sslContext=ctx,
        )
        atexit.register(Disconnect, connection)
        content = connection.RetrieveContent()
        
        if content.viewManager:
            container = content.viewManager.CreateContainerView(
                content.rootFolder, [vim.VirtualMachine], True
            )
        else:
            return "Cannot create view", 500
        
        vms = container.view
        # Filter VMs that match the whitelist regex
        valid_vms = [vm.name for vm in vms if vm_white_list.match(vm.name)]
        # container.Destroy()
        
        return {"vms": valid_vms, "count": len(valid_vms), "pattern": vm_white_list.pattern}
    except Exception as e:
        return {"error": str(e)}, 500


@app.route("/")
def root():
    return "OK"
