#!/usr/bin/env bash
# Host verify backstop for `specify slice build --phase finalize`.
# Invoked by the Specify CLI when `adapter.yaml` declares `finalize_verify`.
set -euo pipefail

: "${SPECIFY_PROJECT_DIR:?SPECIFY_PROJECT_DIR must be set}"
: "${SPECIFY_SLICE_DIR:?SPECIFY_SLICE_DIR must be set}"

PROJECT_DIR="$SPECIFY_PROJECT_DIR"
SLICE_DIR="$SPECIFY_SLICE_DIR"
PROJECT_YAML="${PROJECT_DIR}/.specify/project.yaml"

mapfile -t platforms < <(grep -E '^\s*-\s+(core|ios|android|web|desktop)\s*$' "$PROJECT_YAML" | sed -E 's/^[[:space:]]*-[[:space:]]*//')

platform_enabled() {
  local want="$1"
  local p
  for p in "${platforms[@]}"; do
    [[ "$p" == "$want" ]] && return 0
  done
  return 1
}

resolve_ios_app_name() {
  if [[ -f "${SLICE_DIR}/design.md" ]]; then
    local from_design
    from_design="$(grep -E '^- `App` struct: `' "${SLICE_DIR}/design.md" | head -n1 | sed -E 's/^- `App` struct: `([^`]+)`.*/\1/' || true)"
    if [[ -n "$from_design" ]]; then
      echo "$from_design"
      return 0
    fi
  fi

  local project_yml="${PROJECT_DIR}/iOS/project.yml"
  if [[ -f "$project_yml" ]]; then
    local from_yml
    from_yml="$(grep -E '^[[:space:]]*name:' "$project_yml" | head -n1 | sed -E 's/^[[:space:]]*name:[[:space:]]*"?([^"#]+)"?.*/\1/' | tr -d "' " || true)"
    if [[ -n "$from_yml" ]]; then
      echo "$from_yml"
      return 0
    fi
  fi

  local entry name
  for entry in "${PROJECT_DIR}/iOS"/*; do
    [[ -d "$entry" ]] || continue
    name="$(basename "$entry")"
    [[ "$name" == .* || "$name" == generated ]] && continue
    if find "$entry" -name '*.swift' -print -quit | grep -q .; then
      echo "$name"
      return 0
    fi
  done

  echo "cannot resolve iOS app name for swiftformat / make targets" >&2
  return 1
}

if platform_enabled ios && [[ -d "${PROJECT_DIR}/iOS" ]]; then
  specify extension run vectis -- sync ios-scaffold
  app_name="$(resolve_ios_app_name)"
  swiftformat "iOS/${app_name}/"
  (cd "${PROJECT_DIR}/iOS" && make build && make sim-build)
fi

if platform_enabled android && [[ -d "${PROJECT_DIR}/Android" ]]; then
  (cd "${PROJECT_DIR}/Android" && make verify)
fi

specify extension run vectis -- verify --mode verify "$PROJECT_DIR"
