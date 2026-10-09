# Source Adapter Examples

Live `specify` journeys via [omnia-cursor](https://github.com/augentic/omnia-backends/tree/main/crates/cursor). 

`emery` loads the specified adapter(s) to extract claims from one or more sources. The claims are synthesised into `spec.md` / `design.md`, the specification is sliced into `plan.md`, and the three are committed to the revision store (`.emery/storage/blobstore/`).

## Prerequisites

1. The [emery](https://github.com/augentic/emery) repository as a sibling checkout
2. [cursor-sdk-bridge](https://github.com/cursor/sdk-bridge). See [below](#installing-cursor-sdk-bridge) for installation.
3. `CURSOR_API_KEY` in a `.env` file



## Build and run

Run from the repository root: `emery` mounts the invocation directory as the project, and every path an `emery.toml` names must sit inside it.

```bash
# build source adapters
cargo build --workspace --target wasm32-wasip2 --release

# copy adpaters to the omnia-managed plugin store using fully qualifed adapter name
mkdir -p ~/.emery/adapters
cp "target/wasm32-wasip2/release/typescript.wasm" ~/.emery/adapters/emery_typescript@0.13.0.wasm

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

Swap the config for [intent](intent/emery.toml), [typescript](typescript/emery.toml), [python](python/emery.toml), or the combined [emery.toml](emery.toml). A rebuilt component is a `cp` over its `-dev` file: the store never replaces a release by itself, and the run reads whatever the file holds.

### Tuning the log output

The level is `info` bare; `-v` and `-vv` raise it a step each, `-q` and `-qq` lower it, and `RUST_LOG` refines it per crate:

```bash
export RUST_LOG="omnia_core=off,emery_sdk=trace"
export CURSOR_SDK_BRIDGE_LOG=1
```

How much of a correction round you see depends on the level:

| Level | What shows |
| --- | --- |
| bare | Only the `completion` line's `attempts` says a correction ran |
| `-v` | The SDK's gate logs `candidate rejected` with its `findings`: each `path` outside the seam's anchors, with the anchors it accepts named, and each id off its stems. The backend logs `check rejected the candidate` with the `round` and the candidate's size. Corrections read as a list of findings, not a transcript |
| `-vv` | The backend's `correction turn` carries the correction whole, as the model reads it: the rejected answer with the findings after it. The SDK's `accepted` line carries each seam's accepted evidence as JSON |

`RUST_LOG=emery_sdk=debug` alone shows the findings without the backend's lines.

## What is the adapter?

An `adapter` in these configs is a package reference, `emery:<name>@<version>`. Before fetching anything, `emery` looks in its store, `~/.emery/adapters`, for the file `emery_<name>@<version>.wasm`. Whatever sits there is that release on this machine, whoever wrote it.

The examples name the published version (`emery:typescript@0.13.0`), so the `cp` above makes your build stand in for that release: the run reads the file and never the registry. `rm` the file and a project naming the release fetches it again. To keep a build beside the release instead of in its place, name a `-dev` version in both the config and the file (`emery:typescript@0.13.0-dev`, `emery_typescript@0.13.0-dev.wasm`).

## What to expect

A run is a few model completions with long silences between log lines. The `in progress` heartbeat every 15 s tells a working run from a stuck one. Each `completion` line reports what the model read (`input_tokens` uncached, `cache_read_tokens` from the prompt cache), what it wrote (`output_tokens`, of which `reasoning_tokens` was thinking), and where the time went (`opening_ms` bridge start-up, `tool_ms` tool calls, `model_ms` composing, `check_ms` the gate or `verify`).

The shape follows the config:

- `path = "src/"` — one module under one stem, so three completions and no slicing turn: `evidence` 155–195 s over 140–200 K context, `spec-draft` 40–50 s, `design-draft` 60–110 s. Reasoning is about 90% of the output and 95% of the wall time.
- The combined [emery.toml](emery.toml) adds a `grouping` completion and, across more than one stem, a `slicing` one; its Python source — the same orders service as one FastAPI module under its `__main__` guard — is one `survey-<source>` completion and one `evidence` seam over the module, and its documentation source one `survey-<source>` completion over the document's outline, in which the model names the subjects and the stem each one's ids lead with, and one `evidence` seam over them, so the grouping reconciles two behaviour sources' claims about the same routes beside the specification's and the brief's.
- [typescript/emery.toml](typescript/emery.toml) points at a multi-module tree (a gitignored Node service under `at_r9k_position_adapter/`), so the adapter parses it and opens one `survey-<source>` completion over the facts it read — the manifest, the bootstrap, every call that hands a function to a package's receiver, the packages — in which the model names the surfaces at the lines that register them: `start` for the bootstrap and everything it reaches is the adapter's own, the one message handler the model's, whose stem the code reads from its registration where it spells one. From the accepted anchors the rest is decided by code — the id each surface's requirements lead with, the modules each seam lays, and the lines a `requirement` may anchor at — so two runs that accept the same anchors cut the same seams. Each seam's message lists what its modules spell — the boundaries a criterion cites, the packages they import and the calls made through them, the decision points, the behaviours the tree's tests state — before the modules themselves. Under `composer-2.5` the tree fits one message, so both stems share one `evidence` seam and a run is seven completions: the survey at 40–60 s, `evidence` 85–120 s, nearly always with one correction round in which the gate sends back the two to five requirements anchored at a line that only wires — a bare `await main.start()`, a call into the tree's own module — with the anchors it accepts in that file named, then `grouping`, one `spec-draft` per stem, `design-draft`, and `slicing` at 15–25 s each. The graded eval ([evals/README.md](../evals/README.md)) is where the current numbers live, and [evals/cards/README.md](../evals/cards/README.md) records the experiment that made the survey by model the default over the parser's own detection.
- [python/emery.toml](python/emery.toml) points at the eval's `fastapi-routers` fixture — a FastAPI service whose generated postcode table puts its modules past the inline budget — so the adapter parses it, opens one `survey-<source>` completion over the manifest's console script, the bootstrap, the three routers' decorated `def`s, and the packages, and then one `evidence` seam per stem (`start`, `orders`, `customers`, `health`), each laying its closure in order until the postcode table ends the laid run and listing the modules after it, the `rates.json` the pricing module reads laid as a data file. The six `python` eval cases ([evals/README.md](../evals/README.md)) are the graded walks over the same adapter.

The shape holds run to run; the size does not. Identical input varied by a third in wall time. To compare prompts, pin `CURSOR_MODEL` (`auto` is a different model run to run), leave the cap alone, and take several runs per side — or run the eval (`cargo run -p evals`), which does.

Environment knobs:

- `CURSOR_TIMEOUT_SECS` (600) — cap per completion; a correction gets a fresh one. A seam that overruns it is a seam to cut finer, not a reason to raise it: 1200 s bought one more attempt that also timed out.
- `CURSOR_INACTIVITY_SECS` (120) — cancels a stream gone silent while waiting on the bridge; not while the model composes.
- `CURSOR_MAX_AGENTS` (4) — bridge agents live at once; further completions queue.
- `CURSOR_MODEL` (`auto`) — the model every completion is put to; one that reasons less shortens synthesis most.



## Host-to-guest tool calls

The only tools an adapter's completion session declares are the reference tools — `list_docs` and `read_doc` — over its embedded prose corpus. `wasi-model` delivers them as two streams rather than direct callbacks: the host writes each `ToolCall` to the session's `calls` stream, and the guest answers with a `ToolResult` on a second stream it created and passed to `create`, carrying the same correlation ID so the host can resume the completion.

Every answer is served in-process by the SDK from the adapter's listed `PROSE`: `list_docs` returns the adapter's reference paths and Emery's `reconciliation.md` — never a system document (`extract.md`, `claims.md`), which a turn carries already — `read_doc` returns one document body by adapter-relative path, and anything else — an unknown tool, malformed arguments, an unembedded path — comes back as a repairable error. No HTTP shelf, no MCP callback, and no access to the source input or the revision store crosses this boundary; the model reaches nothing but the adapter's own reference documents.

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