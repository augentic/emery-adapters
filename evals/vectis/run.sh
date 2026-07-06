#!/bin/sh
# Drive one vectis-adapter eval scenario against the live cursor backend.
#
#   evals/vectis/run.sh <single-screen>
#
# Builds the guests, seeds a scratch project tree from the scenario's
# seed/ + inputs/, writes the deployment manifest, and runs one
# command-mode eval. The report JSON line and full log land under
# evals/vectis/runs/<scenario>/.
#
# DRY_RUN=1 smoke-checks the wiring without a model: it builds the
# guests, seeds the scratch tree, and writes the manifest, then exits
# before spawning the driver (no cursor-agent required).
set -eu

scenario="${1:?usage: run.sh <single-screen>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
scenario_dir="$root/evals/vectis/scenarios/$scenario"
[ -d "$scenario_dir" ] || { echo "unknown scenario: $scenario" >&2; exit 2; }

# The slice each scenario builds.
case "$scenario" in
  single-screen) slice="daily-quote" ;;
esac

[ "${DRY_RUN:-0}" = "1" ] || command -v cursor-agent >/dev/null || {
  echo "cursor-agent not found on PATH; see evals/vectis/README.md" >&2
  exit 2
}

cd "$root"
cargo build -p specify-vectis -p specify-eval-guest --target wasm32-wasip2

# Scratch project tree: scenario seed files (project.yaml platform set,
# operator-curated design-system manifests) plus the slice inputs the
# eval guest reads from the shared mount.
scratch="$(mktemp -d "${TMPDIR:-/tmp}/specify-eval-$scenario.XXXXXX")"
[ -d "$scenario_dir/seed" ] && cp -R "$scenario_dir/seed/." "$scratch/"
mkdir -p "$scratch/.eval/inputs"
cp "$scenario_dir/inputs/"*.md "$scratch/.eval/inputs/"

# The deployment: the eval guest (the wasi:cli/run exporter) linked to the
# vectis adapter guest, sharing the scratch mount; the HTTP trigger
# serves the adapter's MCP reference route for the spawned cursor-agent.
addr="${HTTP_ADDR:-127.0.0.1:8094}"
wasm="$root/target/wasm32-wasip2/debug"
cat > "$scratch/omnia.toml" <<MANIFEST
[[guest]]
id = "eval"
source.path = "$wasm/specify_eval_guest.wasm"
link = ["specify:adapter/source@0.1.0", "specify:adapter/target@0.1.0"]

[[guest]]
id = "target:vectis"
source.path = "$wasm/specify_vectis.wasm"

[[mount]]
name = "."
path = "$scratch"
writable = true

[[route.http]]
prefix = "/mcp/vectis"
guest = "target:vectis"

[transport]
default = "in-process"
MANIFEST

if [ "${DRY_RUN:-0}" = "1" ]; then
  echo "dry-run $scenario: slice=$slice scratch=$scratch"
  echo "dry-run: manifest written to $scratch/omnia.toml; skipping the live driver"
  exit 0
fi

runs="$root/evals/vectis/runs/$scenario"
mkdir -p "$runs"
log="$runs/run-$(date -u +%Y%m%dT%H%M%SZ).log"
echo "eval $scenario: slice=$slice scratch=$scratch log=$log"

# The runtime supplies argv[0] (the deployment name); ours start at the
# adapter id. Capture the exit status directly — a pipe to tee would
# report tee's instead.
status=0
HTTP_ADDR="$addr" \
SPECIFY_VECTIS_MCP_URL="http://$addr/mcp/vectis" \
cargo run -q -p specify-eval-driver -- \
  run --config "$scratch/omnia.toml" -- target:vectis "$slice" .eval/inputs \
  > "$log" 2>&1 || status=$?
cat "$log"

echo "eval $scenario: exit=$status; outputs under $scratch (composition at .specify/slices/$slice/composition.yaml)"
exit "$status"
