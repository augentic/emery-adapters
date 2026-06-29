#!/bin/sh
# CI guardrails for Vectis native hook scripts (POSIX `sh` contract).
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
ADAPTER_ROOT="${ROOT}/targets/vectis"
HOST_PREREQ="${ADAPTER_ROOT}/scripts/build-host-prereq.sh"
FINALIZE_VERIFY="${ADAPTER_ROOT}/scripts/build-finalize-verify.sh"

fail() {
  echo "test-hook-scripts: $*" >&2
  exit 1
}

echo "syntax-check host_prereq"
sh -n "$HOST_PREREQ"

echo "syntax-check finalize_verify"
sh -n "$FINALIZE_VERIFY"

echo "anti-pattern: no hardcoded app names"
for script in "$HOST_PREREQ" "$FINALIZE_VERIFY"; do
  if grep -E 'TodoApp|echo "TodoApp"' "$script" >/dev/null 2>&1; then
    fail "literal app name found in $(basename "$script")"
  fi
done

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT INT HUP TERM

mkdir -p "${tmpdir}/.specify/slices/demo"
cat > "${tmpdir}/.specify/project.yaml" <<'EOF'
platforms:
  - core
EOF

echo "core-only host_prereq"
SPECIFY_PROJECT_DIR="$tmpdir" sh -- "$HOST_PREREQ"

mock_bin="${tmpdir}/bin"
mkdir -p "$mock_bin"
cat > "${mock_bin}/specify" <<'EOF'
#!/bin/sh
if [ "$1" = "extension" ] && [ "$2" = "run" ]; then
  exit 0
fi
exit 1
EOF
chmod +x "${mock_bin}/specify"

echo "core-only finalize_verify"
PATH="${mock_bin}:${PATH}" \
  SPECIFY_PROJECT_DIR="$tmpdir" \
  SPECIFY_SLICE_DIR="${tmpdir}/.specify/slices/demo" \
  sh -- "$FINALIZE_VERIFY"

resolve_from_design() {
  slice_dir="$1"
  grep -E '^- `App` struct: `' "${slice_dir}/design.md" 2>/dev/null \
    | head -n1 \
    | sed -E 's/^- `App` struct: `([^`]+)`.*/\1/' || true
}

assert_app_name() {
  label="$1"
  slice_dir="$2"
  want="$3"
  got=$(resolve_from_design "$slice_dir")
  if [ "$got" != "$want" ]; then
    fail "${label}: expected '${want}', got '${got}'"
  fi
}

for name in Counter TodoApp; do
  slice_dir="${tmpdir}/slice-${name}"
  mkdir -p "$slice_dir"
  printf '%s\n' "- \`App\` struct: \`${name}\`" > "${slice_dir}/design.md"
  assert_app_name "design.md App struct (${name})" "$slice_dir" "$name"
done

echo "test-hook-scripts: ok"
