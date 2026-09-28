# Source Adapter Examples

Live `specify` journeys via [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor). 

`emery` loads the specified adapter(s) to extract claims from one or more sources. The claims are synthesised into `spec.md` / `design.md`, the specification is sliced into `plan.md`, and the three are committed to the revision store (`.omnia/storage/blobstore/`).

## Prerequisites

1. The [emery](https://github.com/augentic/emery) repository as a sibling checkout
2. [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge). See [below](#installing-cursor-sdk-bridge) for installation.
3. `CURSOR_API_KEY`

## Build and run

Run from the repository root: `emery` mounts the invocation directory as the project, and every path an `emery.toml` names must sit inside it.

```bash
# build source adapters
cargo build --workspace --target wasm32-wasip2 --release

# run the example
set -a; source .env; set +a
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --config examples/typescript/emery.toml

# review the committed revision and its build plan
cargo run --manifest-path ../emery/Cargo.toml -- show spec
cargo run --manifest-path ../emery/Cargo.toml -- show design
cargo run --manifest-path ../emery/Cargo.toml -- show plan
```

Without `.env`:

```bash
export CURSOR_API_KEY=<Cursor API key>
```

Swap the config for [intent](intent/emery.toml), [typescript](typescript/emery.toml), or the combined [emery.toml](emery.toml).

### Using `RUST_LOG` to tune output

RUST_LOG can be composed with the `-v[v]`/`[-q[q]` flags to fine-tune tracing. For example,

```bash
export RUST_LOG="omnia_core=off,emery_sdk=trace"
export CURSOR_SDK_BRIDGE_LOG=1
```

## What to expect

A run is a few model completions with long silences between log lines. The `in progress` heartbeat every 15 s is how you tell a working run from a stuck one, and each `completion` line carries what the completion cost and where its time went. The token counts are the bridge's: `input_tokens` is the uncached context, `cache_read_tokens` the context served from the provider's prompt cache — the two together are what the model read — `output_tokens` what it wrote, `reasoning_tokens` the part of that spent thinking, and `total_tokens` the bridge's own sum of input, output, and cache reads. The wall time splits into `opening_ms` (the agent's creation and the bridge before each round's first frame), `tool_ms` (tool calls outstanding), `model_ms` (the model composing), and `check_ms` (the adapter's gate or the engine's `verify` on each candidate).

With `path = "src/"` the typescript example is three completions, no survey turn, and no slicing turn — `src/` is one production module, mined whole, and its requirements fall under the one stem `orders`, so the build plan is one slice with no model turn spent. The combined [emery.toml](emery.toml) adds a `grouping` completion for the three sources and, when their requirements fall under more than one stem, a `slicing` completion. Two runs under `auto` give the shape and the spread:

- `evidence` (one seam) — 155–195 s, of which the model composing is about 95%, tool calls a few seconds, and the bridge opening about 2 s. Context 140–200 K tokens, about 40% of it cache reads; output 17–22 K tokens, about 90% of it reasoning. Eight to eleven tool calls — one `glob`, one `read` of the module, `list_docs`, then `read_doc`s — all in the first half-minute, then one reasoning block of two minutes before the answer.
- `spec-draft` — 40–50 s; 10–12 K context, 4–5 K output of which 80–90% is reasoning; no tool calls.
- `design-draft` — 60–110 s; 10–20 K context, 6–12 K output of which about 90% is reasoning; no tool calls.

With `path` at a tree of several production modules — [typescript/emery.toml](typescript/emery.toml) ships pointed at a gitignored Node service, a start script and a Kafka consumer under `r9k/legacy/at_r9k_position_adapter/` — the run is a `survey` completion, one `evidence` completion per surface the survey names, a `grouping` completion over the stems, the two drafts, and a `slicing` completion. Two runs under `auto`, before the extract prompt carried its references inline, took 21 and 46 minutes of wall time; the one that committed read 950 K tokens and wrote 220 K, 88% of them reasoning. The start-script seam ran 8–9 minutes; the consumer seam ran 9–20 minutes per attempt and hit the cap in three attempts of four, each a minute of reads and then reasoning blocks of five to eight minutes. That seam, not the cap, is what a change to the extract prompt is measured on.

What holds run to run is the shape — one completion per seam, no shell tool, no `read_doc` for the claim rules, which ride the system prompt, and none for `survey.md`, which `list_docs` never offers — and that reasoning dominates both the wall time and the bill. What does not hold is the size, or the survey: the two one-module runs differed by a third in the evidence completion's wall time and reasoning over identical input, and the two multi-module runs named three surfaces and then two over one tree, sharing eight requirement ids of about forty. A change to a prompt is not measured by one run each side. To compare, pin `CURSOR_MODEL` — `auto` is a different model run to run — leave the cap where it is, and take several runs per side before reading a difference into the numbers.

Four optional environment knobs bound the wait and choose the model:

- `CURSOR_TIMEOUT_SECS` (600) — wall-clock cap per completion; a check's correction gets a fresh cap. A seam that needs more than the cap is a seam to cut finer or ask less of, not a reason to raise the cap: raising it to 1200 s bought one more attempt that also timed out.
- `CURSOR_INACTIVITY_SECS` (120) — cancels a run whose stream has gone silent while waiting on the bridge (the opening frame, a tool call). It does not apply once the model is composing its answer.
- `CURSOR_MAX_AGENTS` (4) — bridge agents live at once; further completions queue.
- `CURSOR_MODEL` (`auto`) — the model every completion of the run is put to, extraction and synthesis alike; `auto` is Cursor's server-side selection, and a model that reasons less shortens the synthesis completions most, since they spend their time composing rather than reading.



## Host-to-guest tool calls

The only tools an adapter's completion session declares are the reference tools — `list_docs` and `read_doc` — over its embedded prose corpus. `wasi-model` delivers them as two streams rather than direct callbacks: the host writes each `ToolCall` to the session's `calls` stream, and the guest answers with a `ToolResult` on a second stream it created and passed to `create`, carrying the same correlation ID so the host can resume the completion.

Every answer is served in-process by the SDK from the adapter's listed `PROSE`: `list_docs` returns the adapter's reference paths and Emery's `reconciliation.md` — never a system document (`extract.md`, `survey.md`, `claims.md`), which a turn either carries already or has nothing to learn from — `read_doc` returns one document body by adapter-relative path, and anything else — an unknown tool, malformed arguments, an unembedded path — comes back as a repairable error. No HTTP shelf, no MCP callback, and no access to the source input or the revision store crosses this boundary; the model reaches nothing but the adapter's own reference documents.

## Installing cursor-sdk-bridge

```bash
# download and install
curl -fsSL -o /tmp/cursor-sdk-bridge.tar.gz \
  https://github.com/cursor/sdk-bridge/releases/latest/download/cursor-sdk-bridge-standalone-darwin-arm64.tar.gz \
  && tar -xzf /tmp/cursor-sdk-bridge.tar.gz -C /tmp \
  && install /tmp/bin/cursor-sdk-bridge ~/.local/bin/cursor-sdk-bridge

# verify
cursor-sdk-bridge --help
```

See [bridge docs](https://cursor.com/docs/sdk/bridge) for more information.