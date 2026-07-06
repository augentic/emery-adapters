#!/usr/bin/env bash
# Inject -suppress-warnings into generated SPM Package.swift targets (UniFFI /
# facet output). Shell Swift under iOS/<APP_NAME>/ keeps SWIFT_TREAT_WARNINGS_AS_ERRORS.
set -eu

patch_package() {
  local pkg="$1"
  [[ -f "$pkg" ]] || return 0
  python3 - "$pkg" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()

def inject(match: re.Match[str]) -> str:
    body = match.group(0)
    if "swiftSettings" in body:
        return body
    return body[:-1] + ",\n            swiftSettings: [.unsafeFlags([\"-suppress-warnings\"])]\n        )"

patched, count = re.subn(
    r"\.target\(\s*name: \"[^\"]+\",\s*dependencies: \[[^\]]*\]\s*\)",
    inject,
    text,
    flags=re.DOTALL,
)
if count:
    path.write_text(patched)
PY
}

for pkg in "$@"; do
  patch_package "$pkg"
done
