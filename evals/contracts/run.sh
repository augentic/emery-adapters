#!/bin/sh
# Drive one contracts-adapter eval scenario against the live cursor backend.
#
#   evals/contracts/run.sh <describe|design|import|source|update>
#
# Builds the guests, seeds a scratch project tree from the scenario's
# seed/ + inputs/, writes the deployment manifest, and runs one
# command-mode eval. The report JSON line and full log land under
# evals/contracts/runs/<scenario>/.
set -eu

scenario="${1:?usage: run.sh <describe|design|import|source|update>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
scenario_dir="$root/evals/contracts/scenarios/$scenario"
[ -d "$scenario_dir" ] || { echo "unknown scenario: $scenario" >&2; exit 2; }

# The slice each scenario builds (mirrors targets/contracts/tests/).
case "$scenario" in
  describe) slice="user-adapter-api" ;;
  design)   slice="returns-api" ;;
  import)   slice="import-ticket-api-contract" ;;
  source)   slice="orders-api-contract" ;;
  update)   slice="loyalty-api-contract" ;;
esac

command -v cursor-agent >/dev/null || {
  echo "cursor-agent not found on PATH; see evals/contracts/README.md" >&2
  exit 2
}

cd "$root"
cargo build -p contracts -p eval-guest --target wasm32-wasip2

# Scratch project tree: scenario seed files plus the slice inputs the
# eval guest reads from the shared mount.
scratch="$(mktemp -d "${TMPDIR:-/tmp}/specify-eval-$scenario.XXXXXX")"
[ -d "$scenario_dir/seed" ] && cp -R "$scenario_dir/seed/." "$scratch/"
mkdir -p "$scratch/.eval/inputs"
cp "$scenario_dir/inputs/"*.md "$scratch/.eval/inputs/"

# The deployment: the eval guest (the wasi:cli/run exporter) linked to the
# contracts adapter guest, sharing the scratch mount; the HTTP trigger
# serves the adapter's MCP reference route for the spawned cursor-agent.
addr="${HTTP_ADDR:-127.0.0.1:8093}"
wasm="$root/target/wasm32-wasip2/debug"
cat > "$scratch/omnia.toml" <<MANIFEST
[[guest]]
id = "eval"
source.path = "$wasm/eval_guest.wasm"
link = ["specify:adapter/source@0.1.0", "specify:adapter/target@0.1.0"]

[[guest]]
id = "target:contracts"
source.path = "$wasm/contracts.wasm"

[[mount]]
name = "."
path = "$scratch"
writable = true

[[route.http]]
prefix = "/mcp/contracts"
guest = "target:contracts"

[transport]
default = "in-process"
MANIFEST

runs="$root/evals/contracts/runs/$scenario"
mkdir -p "$runs"
log="$runs/run-$(date -u +%Y%m%dT%H%M%SZ).log"
echo "eval $scenario: slice=$slice scratch=$scratch log=$log"

# The runtime supplies argv[0] (the deployment name); ours start at the
# adapter id. Capture the exit status directly — a pipe to tee would
# report tee's instead.
status=0
HTTP_ADDR="$addr" \
SPECIFY_CONTRACTS_MCP_URL="http://$addr/mcp/contracts" \
cargo run -q -p eval-driver -- \
  run --config "$scratch/omnia.toml" -- target:contracts "$slice" .eval/inputs \
  > "$log" 2>&1 || status=$?
cat "$log"

echo "eval $scenario: exit=$status; delta under $scratch/.specify/slices/$slice/contracts"
exit "$status"
