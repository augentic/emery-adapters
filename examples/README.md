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

A rejected candidate shows at each level by what that level is for. Bare, only the `completion` line's `attempts` says a correction round ran. At `-v`, the SDK's gate logs `candidate rejected` with the `findings` it minted — each `path` outside the seam's anchors with the anchors it accepts named, each id off its stems — and the backend logs `check rejected the candidate` with the `round` and the candidate's size, so a run's corrections read as a list of findings rather than a transcript. At `-vv`, the backend's `correction turn` carries the correction whole — the rejected answer echoed with the findings after it, as the model reads it — and the SDK's `accepted` line carries each seam's accepted evidence as JSON. `RUST_LOG=emery_sdk=debug` alone reads the findings without the backend's lines.

## What to expect

A run is a few model completions with long silences between log lines. The `in progress` heartbeat every 15 s tells a working run from a stuck one. Each `completion` line reports what the model read (`input_tokens` uncached, `cache_read_tokens` from the prompt cache), what it wrote (`output_tokens`, of which `reasoning_tokens` was thinking), and where the time went (`opening_ms` bridge start-up, `tool_ms` tool calls, `model_ms` composing, `check_ms` the gate or `verify`).

The shape follows the config:

- `path = "src/"` — one module under one stem, so three completions and no slicing turn: `evidence` 155–195 s over 140–200 K context, `spec-draft` 40–50 s, `design-draft` 60–110 s. Reasoning is about 90% of the output and 95% of the wall time.
- The combined [emery.toml](emery.toml) adds a `grouping` completion and, across more than one stem, a `slicing` one.
- [typescript/emery.toml](typescript/emery.toml) points at a multi-module tree (a gitignored Node service under `at_r9k_position_adapter/`), so the adapter parses it and decides its seams from the code — `start` for the bootstrap and everything it reaches, `kafka-consumer` for the one message handler — with no `survey` completion: the surfaces, their stems, the id each surface's requirements lead with, the modules each seam lays, and the lines a `requirement` may anchor at are decided by code and are the same on every run. Each seam's message lists what its modules spell — the boundaries a criterion cites, the packages they import and the calls made through them, the decision points, the behaviours the tree's tests state — before the modules themselves. Under `composer-2.5` the tree fits one message, so both stems share one `evidence` seam and a run is six completions in 125–165 s over 230–340 K input tokens: `evidence` 85–120 s, nearly always with one correction round in which the gate sends back the two to five requirements anchored at a line that only wires — a bare `await main.start()`, a call into the tree's own module — with the anchors it accepts in that file named, then `grouping`, one `spec-draft` per stem, `design-draft`, and `slicing` at 15–25 s each. Under the survey by model that preceded the parser, two runs took 21 and 46 minutes, the one that committed read 950 K tokens and wrote 220 K, and the Kafka-consumer seam ran 9–20 minutes and hit the cap in three attempts of four; the eval below is where the current numbers live.

The shape holds run to run; the size does not. Identical input varied by a third in wall time. To compare prompts, pin `CURSOR_MODEL` (`auto` is a different model run to run), leave the cap alone, and take several runs per side — or run the eval, which does.

Environment knobs:

- `CURSOR_TIMEOUT_SECS` (600) — cap per completion; a correction gets a fresh one. A seam that overruns it is a seam to cut finer, not a reason to raise it: 1200 s bought one more attempt that also timed out.
- `CURSOR_INACTIVITY_SECS` (120) — cancels a stream gone silent while waiting on the bridge; not while the model composes.
- `CURSOR_MAX_AGENTS` (4) — bridge agents live at once; further completions queue.
- `CURSOR_MODEL` (`auto`) — the model every completion is put to; one that reasons less shortens synthesis most.

## The graded eval

`cargo run --example eval -- [case..]` grades the shipped `typescript` component end to end over the live model. Each case under [eval/cases/](eval/cases/) is a directory named for it holding one `expected.toml`: the fixture tree (relative to the repository root), the stems every accepted requirement must lead with, the `[[surface]]` pairs (`entry`, `stem`) the survey must decide, and the requirements and criteria a reviewer would write from the code, each as a `stem`, an `anchor` (`path#Ln` or `path#Ln-Lm`), and a one-line `gloss`. The runner stages every case (or the named ones) as its own project beneath `target/eval/`, runs `emery specify` over it `EVAL_RUNS` times (three), reads the committed documents back through `emery show --format json`, and grades each run: an expected item is found when an accepted claim's anchor overlaps its lines, a stem is a miss when the run's requirements do not lead with it, a surface is found when the adapter's `surveyed` line names a surface at its entry under its stem (the `surfaces` column, `matched/expected +extra`), and the token and wall-clock figures come from the backend's `completion` lines on stderr. A run the backend's cap ends — a `timeout` or `inactive` completion — is put again one rung up `EVAL_LADDER` (`600/120,1200/240,2400/480`, each rung `CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS`), so the card's `Budget ladder` table says at which budget each run landed while the comparison columns stay the first rung's. It prints and writes a dated scorecard to `target/eval/<timestamp>.md`; compare two by hand. Three runs a case is what tells a stable id from a lucky one; `EVAL_RUNS` trades time for confidence, and the `Stability` lines count the stem sets across the runs and the (entry, stem) pairs, requirement ids, and spec subjects every pair of runs shares. An expected criterion is a boundary the code spells as a value of its own — a named constant, a default, a pattern, at the line that binds it — never a literal written into the branch that uses it, and middleware mounted for every route is expected under `start`.

The five cases: [orders](eval/cases/orders/expected.toml) is one module under one stem (the inline-budget path); [express-orders](eval/cases/express-orders/expected.toml) an Express service over `typescript/express-orders/` (routes under `orders`, `customers`, `health`, and the bootstrap under `start`); [cli-jobs](eval/cases/cli-jobs/expected.toml) a commander CLI with queue workers over `typescript/cli-jobs/`; [api-routes](eval/cases/api-routes/expected.toml) a Next.js app-router API over `typescript/api-routes/` — four route modules a framework routes by file path, exporting `GET` / `POST` / `DELETE` with no bootstrap and nothing registered, the convention the parser has not been taught; [r9k](eval/cases/r9k/expected.toml) the Kafka consumer above. Build both binaries first and point the runner at them:

```bash
cargo build --manifest-path ../emery/Cargo.toml --release
cargo build -p typescript --target wasm32-wasip2 --release
set -a; source .env; set +a
EMERY_BIN=../emery/target/release/emery TYPESCRIPT_WASM=target/wasm32-wasip2/release/typescript.wasm \
  cargo run --example eval -- orders express-orders
```

Both paths are the defaults, so a sibling `../emery` release build needs neither variable — unless `CARGO_TARGET_DIR` points both builds elsewhere, when the variables name the artifacts there. `RUST_LOG` is set for the run unless the caller sets it — the scorecard reads the SDK's `accepted` trace lines, the adapter's `surveyed` line, and the backend's `completion` lines — and `CURSOR_MODEL` is recorded in the card, so pin it before comparing two.

The `typescript` adapter's `model-survey` feature builds the survey-by-model arm: the parser reads the tree as before, but the model names the surfaces, their anchors, and their stems from the facts it read, under [survey.md](../sources/typescript/prose/survey.md), and code derives the rest from those anchors as it does from its own. Build it to a path of its own and point the runner at it; its `surveyed` line is the model's surfaces, and the card notes where the parser would have named differently:

```bash
cargo build -p typescript --features model-survey --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/typescript.wasm target/eval/typescript-model.wasm
cargo build -p typescript --target wasm32-wasip2 --release
TYPESCRIPT_WASM=target/eval/typescript-model.wasm cargo run --example eval -- orders express-orders
```

A card worth keeping is copied by hand into [eval/cards/](eval/cards/) — `target/` is gitignored and `make sweep` drops what sits there untouched for a week — and its README says what each card shows; the cards there run from the survey-by-model baseline through each change to the survey. The `Stability` line a card ends with counts the requirement ids two runs share: the revision diff is positional and the model names each id's second segment, so the figure measures the model's naming run to run and is not a target the survey holds — the stems, the surfaces, the anchors, and the files a seam lays are what the survey fixes, and [tests/source.rs](../tests/source.rs) pins those over `typescript/cli-jobs/` and `typescript/express-orders/` with no model turn.

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