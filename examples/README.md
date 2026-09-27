# Source Adapter Examples

Live `specify` journeys via [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor). 

`emery` loads the specified adapter(s) to extract claims from one or more sources. The claims are synthesised into `spec.md` / `design.md`, which are then committed to the revision store (`.omnia/storage/blobstore/`).

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
export CURSOR_API_KEY=<Cursor API key>
cargo run --manifest-path ../emery/Cargo.toml -- specify -v --config examples/typescript/emery.toml

# review the committed revision
cargo run --manifest-path ../emery/Cargo.toml -- show spec
cargo run --manifest-path ../emery/Cargo.toml -- show design
```

Swap the config for [intent](intent/emery.toml), [typescript](typescript/emery.toml), or the combined [emery.toml](emery.toml).

### Using `RUST_LOG` to tune output

RUST_LOG can be composed with the `-v[v]`/`[-q[q]` flags to fine-tune tracing. For example,

```bash
export RUST_LOG="omnia_core=off,emery_sdk=trace"
export CURSOR_SDK_BRIDGE_LOG=1
```

## What to expect

A run is a few model completions with long silences between log lines. The `in progress` heartbeat every 15 s is how you tell a working run from a stuck one, and each `completion` line carries what the completion cost: `input_tokens` with the `cache_read_tokens` among them, `output_tokens` with the `reasoning_tokens` among them, and how the wall time split between `opening_ms` (the bridge and the first frame), `tool_ms` (tool calls outstanding), and `model_ms` (the model composing).

The typescript example is three completions, about five minutes on one observed run:

- `evidence` (one seam) — 193 s, of which 184 s was the model composing, 4 s tool calls, and 2 s the bridge opening; 120 K input tokens, 77 K of them cache reads; 22 K output tokens, 21 K of them reasoning; 8 tool calls (one `glob`, one `read`, `list_docs`, five `read_doc`) in the first 25 s, then one reasoning block of 110 s before the answer.
- `spec-draft` — 40 s; 9 K input, 4 K output (3.3 K reasoning); no tool calls.
- `design-draft` — 59 s; 8 K input, 6.5 K output (5.6 K reasoning); no tool calls.

There is no survey row: the example's `src/` is one production module, so it is mined whole with no survey turn spent. Reasoning dominates both the wall time and the bill, and it varies two- to threefold run to run for the same input; what holds is the shape — one completion per seam, no shell tool, no `read_doc` for the claim rules, which ride the system prompt, and no `read_doc` for `survey.md`, which `list_docs` keeps out of a mining turn. Cache reads are billed at a fraction of fresh input, so `input_tokens` alone overstates the cost of a completion with many tool rounds.

Four optional environment knobs bound the wait and choose the model:

- `CURSOR_TIMEOUT_SECS` (600) — wall-clock cap per completion; a check's correction gets a fresh cap.
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