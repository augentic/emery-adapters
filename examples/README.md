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

A run is a few model completions with long silences between log lines. The `in progress` heartbeat every 15 s tells a working run from a stuck one. Each `completion` line reports what the model read (`input_tokens` uncached, `cache_read_tokens` from the prompt cache), what it wrote (`output_tokens`, of which `reasoning_tokens` was thinking), and where the time went (`opening_ms` bridge start-up, `tool_ms` tool calls, `model_ms` composing, `check_ms` the gate or `verify`).

The shape follows the config:

- `path = "src/"` — one module under one stem, so three completions and no survey or slicing turn: `evidence` 155–195 s over 140–200 K context, `spec-draft` 40–50 s, `design-draft` 60–110 s. Reasoning is about 90% of the output and 95% of the wall time.
- The combined [emery.toml](emery.toml) adds a `grouping` completion and, across more than one stem, a `slicing` one.
- [typescript/emery.toml](typescript/emery.toml) points at a multi-module tree (a gitignored Node service under `r9k/legacy/at_r9k_position_adapter/`), so a `survey` completion precedes one `evidence` per surface. Two runs took 21 and 46 minutes; the one that committed read 950 K tokens and wrote 220 K. The Kafka-consumer seam ran 9–20 minutes and hit the cap in three attempts of four; it is the seam an extract-prompt change is measured on.

The shape holds run to run; the size does not. Identical input varied by a third in wall time, and one tree surveyed to three surfaces and then two. To compare prompts, pin `CURSOR_MODEL` (`auto` is a different model run to run), leave the cap alone, and take several runs per side.

Environment knobs:

- `CURSOR_TIMEOUT_SECS` (600) — cap per completion; a correction gets a fresh one. A seam that overruns it is a seam to cut finer, not a reason to raise it: 1200 s bought one more attempt that also timed out.
- `CURSOR_INACTIVITY_SECS` (120) — cancels a stream gone silent while waiting on the bridge; not while the model composes.
- `CURSOR_MAX_AGENTS` (4) — bridge agents live at once; further completions queue.
- `CURSOR_MODEL` (`auto`) — the model every completion is put to; one that reasons less shortens synthesis most.



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