#!/usr/bin/env bash
# Host toolchain gate for `specify slice build --phase prepare`.
# Invoked by the Specify CLI when `adapter.yaml` declares `host_prereq`.
set -euo pipefail

: "${SPECIFY_PROJECT_DIR:?SPECIFY_PROJECT_DIR must be set}"

PROJECT_YAML="${SPECIFY_PROJECT_DIR}/.specify/project.yaml"
if [[ ! -f "$PROJECT_YAML" ]]; then
  echo "missing ${PROJECT_YAML}" >&2
  exit 1
fi

platforms=()
while IFS= read -r platform; do
  [[ -n "$platform" ]] && platforms+=("$platform")
done < <(grep -E '^\s*-\s+(core|ios|android|web|desktop)\s*$' "$PROJECT_YAML" | sed -E 's/^[[:space:]]*-[[:space:]]*//')

missing=()

platform_enabled() {
  local want="$1"
  local p
  for p in "${platforms[@]}"; do
    [[ "$p" == "$want" ]] && return 0
  done
  return 1
}

android_rust_targets_installed() {
  local rustup_home="${RUSTUP_HOME:-}"
  if [[ -z "$rustup_home" && -n "${HOME:-}" ]]; then
    rustup_home="${HOME}/.rustup"
  fi
  [[ -d "$rustup_home/toolchains" ]] || return 1
  local toolchain
  for toolchain in "$rustup_home/toolchains"/*; do
    [[ -d "$toolchain/lib/rustlib/aarch64-linux-android" ]] \
      && [[ -d "$toolchain/lib/rustlib/armv7-linux-androideabi" ]] \
      && return 0
  done
  return 1
}

xcodebuild_available() {
  if [[ -n "${DEVELOPER_DIR:-}" && -x "${DEVELOPER_DIR}/usr/bin/xcodebuild" ]]; then
    return 0
  fi
  command -v xcodebuild >/dev/null 2>&1 \
    || [[ -x /Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild ]]
}

if platform_enabled android; then
  if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
    missing+=("ANDROID_HOME (or ANDROID_SDK_ROOT) must be set when android is in project platforms")
  fi
  if ! android_rust_targets_installed; then
    missing+=("Rust Android targets not installed; run \`rustup target add aarch64-linux-android armv7-linux-androideabi\`")
  fi
fi

if platform_enabled ios && [[ "$(uname -s)" == "Darwin" ]]; then
  if ! xcodebuild_available; then
    missing+=("xcodebuild not found; install Xcode command-line tools when ios is in project platforms")
  fi
fi

if ((${#missing[@]} > 0)); then
  printf '%s\n' "${missing[@]}" >&2
  exit 1
fi
