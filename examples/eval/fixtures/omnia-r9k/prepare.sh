#!/usr/bin/env bash
# Populate the Omnia r9k migration fixture with the Propellerhead
# at_r9k_position_adapter TypeScript tree. The tree is gitignored
# (UNLICENSED upstream) — operators supply it via clone or a local path.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
DEST="$ROOT/project/legacy/at_r9k_position_adapter"
BITBUCKET_URL="${OMNIA_R9K_GIT_URL:-https://bitbucket.org/Propellerhead/at_r9k_position_adapter.git}"

if [[ -d "$DEST/src" ]]; then
  echo "omnia-r9k fixture ready: $DEST"
  exit 0
fi

stage_from() {
  local src="$1"
  if [[ ! -d "$src/src" ]]; then
    return 1
  fi
  mkdir -p "$ROOT/project/legacy"
  rm -rf "$DEST"
  # Prefer rsync when present so we can drop deps and env sidecars.
  if command -v rsync >/dev/null 2>&1; then
    mkdir -p "$DEST"
    rsync -a \
      --exclude node_modules \
      --exclude .git \
      --exclude '.github/env.*' \
      "$src"/ "$DEST"/
  else
    cp -R "$src" "$DEST"
    rm -rf "$DEST/node_modules" "$DEST/.git"
    rm -f "$DEST"/.github/env.*
  fi
  echo "omnia-r9k fixture staged from $src → $DEST"
  return 0
}

if [[ -n "${OMNIA_R9K_SOURCE:-}" ]]; then
  stage_from "$OMNIA_R9K_SOURCE" || {
    echo "error: OMNIA_R9K_SOURCE=$OMNIA_R9K_SOURCE is not an at_r9k_position_adapter tree (missing src/)" >&2
    exit 1
  }
  exit 0
fi

# Convenience: a sibling test-spec checkout next to specify-adapters.
if stage_from "$ROOT/../../../../test-spec/legacy/at_r9k_position_adapter" 2>/dev/null; then
  exit 0
fi

echo "omnia-r9k: cloning $BITBUCKET_URL …"
mkdir -p "$ROOT/project/legacy"
rm -rf "$DEST"
if git clone --depth 1 "$BITBUCKET_URL" "$DEST"; then
  rm -rf "$DEST/.git" "$DEST/node_modules"
  rm -f "$DEST"/.github/env.*
  echo "omnia-r9k fixture ready: $DEST"
  exit 0
fi

cat >&2 <<EOF
error: could not populate examples/eval/fixtures/omnia-r9k/project/legacy/at_r9k_position_adapter

Provide the Propellerhead at_r9k_position_adapter tree one of these ways:

  OMNIA_R9K_SOURCE=/path/to/at_r9k_position_adapter \\
    cargo make eval-omnia-r9k-prepare

  # or clone yourself, then re-run prepare:
  git clone $BITBUCKET_URL \\
    examples/eval/fixtures/omnia-r9k/project/legacy/at_r9k_position_adapter

Upstream: https://bitbucket.org/Propellerhead/at_r9k_position_adapter
EOF
exit 1
