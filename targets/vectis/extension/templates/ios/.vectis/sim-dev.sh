#!/usr/bin/env bash
# CLI-owned simulator install/launch — local dev only (not verify).
set -euo pipefail

APP_NAME='__APP_NAME__'
APP_NAME_LOWER='__APP_NAME_LOWER__'
IOS_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_ID="com.vectis.${APP_NAME_LOWER}.debug"
DERIVED_DATA="${IOS_DIR}/DerivedData"
APP_PATH="${DERIVED_DATA}/Build/Products/Debug-iphonesimulator/${APP_NAME}.app"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

resolve_sim_udid() {
	if [[ -n "${SIM_UDID:-}" ]]; then
		echo "$SIM_UDID"
		return
	fi

	python3 <<'PY'
import json
import os
import subprocess
import sys
from typing import Dict, List, Optional

device_filter = os.environ.get("SIM_DEVICE")
os_filter = os.environ.get("SIM_OS")


def normalize_os(version: str) -> str:
    return version.replace(".", "-").lower()


def list_devices(state: Optional[str] = None) -> List[Dict]:
    cmd = ["xcrun", "simctl", "list", "devices"]
    if state == "booted":
        cmd.extend(["booted", "-j"])
    else:
        cmd.extend(["available", "-j"])
    raw = subprocess.check_output(cmd, text=True)
    data = json.loads(raw)
    devices: List[Dict] = []
    if state == "booted":
        for runtime_devices in data.get("devices", {}).values():
            for device in runtime_devices:
                if device.get("isAvailable", True):
                    devices.append(device)
        return devices

    for runtime_id, runtime_devices in data.get("devices", {}).items():
        runtime_version = runtime_id.rsplit(".", 1)[-1]
        for device in runtime_devices:
            if not device.get("isAvailable", True):
                continue
            entry = dict(device)
            entry["runtime_version"] = runtime_version
            devices.append(entry)
    return devices


if device_filter and os_filter:
    normalized_os = normalize_os(os_filter)
    for device in list_devices():
        if device.get("name") != device_filter:
            continue
        runtime_version = device.get("runtime_version", "")
        if normalized_os not in runtime_version.lower():
            continue
        print(device["udid"])
        sys.exit(0)
    print(
        f"no available simulator matches SIM_DEVICE={device_filter!r} and SIM_OS={os_filter!r}",
        file=sys.stderr,
    )
    print("run: xcrun simctl list devices available", file=sys.stderr)
    sys.exit(1)

for device in list_devices("booted"):
    print(device["udid"])
    sys.exit(0)

for device in list_devices():
    if "iPhone" in device.get("name", ""):
        print(device["udid"])
        sys.exit(0)

print("no available iPhone simulator found", file=sys.stderr)
print("run: xcrun simctl list devices available", file=sys.stderr)
sys.exit(1)
PY
}

boot_simulator() {
	local udid="$1"
	if ! xcrun simctl list devices booted | grep -q "$udid"; then
		xcrun simctl boot "$udid" 2>/dev/null || true
		open -a Simulator
	fi
}

cmd_install() {
	if [[ ! -d "$APP_PATH" ]]; then
		echo "app not found at ${APP_PATH}; run 'make sim-build' first" >&2
		exit 1
	fi
	local udid
	udid="$(resolve_sim_udid)"
	boot_simulator "$udid"
	xcrun simctl install "$udid" "$APP_PATH"
}

cmd_launch() {
	local udid
	udid="$(resolve_sim_udid)"
	boot_simulator "$udid"
	xcrun simctl launch "$udid" "$BUNDLE_ID"
}

cmd_run() {
	if [[ ! -d "$APP_PATH" ]]; then
		bash "${SCRIPT_DIR}/sim-build.sh"
	fi
	cmd_install
	cmd_launch
}

cmd_app_path() {
	echo "$APP_PATH"
}

usage() {
	echo "usage: $(basename "$0") {install|launch|run|app-path}" >&2
	exit 1
}

case "${1:-}" in
install) cmd_install ;;
launch) cmd_launch ;;
run) cmd_run ;;
app-path) cmd_app_path ;;
*) usage ;;
esac
