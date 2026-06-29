#!/bin/sh
# Host toolchain gate for `specify slice build --phase prepare`.
# Invoked by the Specify CLI via `sh` when `adapter.yaml` declares `host_prereq`.
set -eu

: "${SPECIFY_PROJECT_DIR:?SPECIFY_PROJECT_DIR must be set}"

PROJECT_YAML="${SPECIFY_PROJECT_DIR}/.specify/project.yaml"
if [ ! -f "$PROJECT_YAML" ]; then
  echo "missing ${PROJECT_YAML}" >&2
  exit 1
fi

platforms=$(grep -E '^[[:space:]]*-[[:space:]]+(core|ios|android|web|desktop)[[:space:]]*$' "$PROJECT_YAML" | sed -E 's/^[[:space:]]*-[[:space:]]*//')

platform_enabled() {
  want="$1"
  echo "$platforms" | grep -qx "$want"
}

android_rust_targets_installed() {
  rustup_home="${RUSTUP_HOME:-}"
  if [ -z "$rustup_home" ] && [ -n "${HOME:-}" ]; then
    rustup_home="${HOME}/.rustup"
  fi
  [ -d "$rustup_home/toolchains" ] || return 1
  for toolchain in "$rustup_home/toolchains"/*; do
    if [ -d "$toolchain/lib/rustlib/aarch64-linux-android" ] \
      && [ -d "$toolchain/lib/rustlib/armv7-linux-androideabi" ]; then
      return 0
    fi
  done
  return 1
}

xcodebuild_available() {
  if [ -n "${DEVELOPER_DIR:-}" ] && [ -x "${DEVELOPER_DIR}/usr/bin/xcodebuild" ]; then
    return 0
  fi
  command -v xcodebuild >/dev/null 2>&1 \
    || [ -x /Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild ]
}

missing=""

if platform_enabled android; then
  if [ -z "${ANDROID_HOME:-}" ] && [ -z "${ANDROID_SDK_ROOT:-}" ]; then
    missing="${missing}ANDROID_HOME (or ANDROID_SDK_ROOT) must be set when android is in project platforms
"
  fi
  if ! android_rust_targets_installed; then
    missing="${missing}Rust Android targets not installed; run \`rustup target add aarch64-linux-android armv7-linux-androideabi\`
"
  fi
fi

if platform_enabled ios && [ "$(uname -s)" = "Darwin" ]; then
  if ! xcodebuild_available; then
    missing="${missing}xcodebuild not found; install Xcode command-line tools when ios is in project platforms
"
  fi
fi

if [ -n "$missing" ]; then
  printf '%s' "$missing" >&2
  exit 1
fi
