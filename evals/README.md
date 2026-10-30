# Graded Live Eval

`evals/` is the live benchmark harness for `emery-adapters`. It runs the compiled WebAssembly (Wasm) source and target adapters end to end through the `emery` CLI against live language models, grades extracted specifications against human-authored expectations (`expected.toml`), and optionally builds and verifies the generated code (`--build`).

> **Note:** The evaluation runner is operator-invoked and never runs in CI. Deterministic regression tests live in [`tests/source.rs`](../tests/source.rs) and [`tests/target.rs`](../tests/target.rs); `evals/` measures live model and prompt quality.

## Contents

- [Running evaluations](#running-evaluations)
  - [CLI invocations](#cli-invocations)
  - [Environment variables](#environment-variables)
  - [What a run does](#what-a-run-does)
  - [Survey facts without a model turn (`--facts`)](#survey-facts-without-a-model-turn---facts)
- [Building the plan](#building-the-plan)
  - [How `--build` stages and runs](#how---build-stages-and-runs)
  - [Toolchain requirements and worker grants](#toolchain-requirements-and-worker-grants)
  - [How the runner grades a build (`run-N.fidelity.md`)](#how-the-runner-grades-a-build-run-nfidelitymd)
- [Reference comparison](#reference-comparison)
  - [Structural measures](#structural-measures)
  - [Black-box fixture oracle](#black-box-fixture-oracle)
  - [Standalone oracle mode (`--oracle`)](#standalone-oracle-mode---oracle)
- [Building a plan again](#building-a-plan-again)
  - [Full rebuild (`--rebuild`)](#full-rebuild---rebuild)
  - [Wave-level resume (`--resume --at`)](#wave-level-resume---resume---at)
- [The small-model arm](#the-small-model-arm)
  - [Building `emery-dev` (`omnia-genai`)](#building-emery-dev-omnia-genai)
  - [Local Ollama setup (macOS fallback)](#local-ollama-setup-macos-fallback)
  - [Historical archive: Rounds G–M RunPod H200 setup](#historical-archive-rounds-gm-runpod-h200-setup)
- [Experiment arms](#experiment-arms)
- [Cases](#cases)
  - [`expected.toml` schema](#expectedtoml-schema)
  - [`typescript` cases](#typescript-cases)
  - [`python` cases](#python-cases)
  - [`documentation` cases](#documentation-cases)
  - [Multi-source cases](#multi-source-cases)
  - [Fetching vendored fixtures](#fetching-vendored-fixtures)
- [Reading a scorecard](#reading-a-scorecard)
  - [Specification metrics](#specification-metrics)
  - [Build metrics (`--build`)](#build-metrics---build)
  - [Diagnostic notes and stability](#diagnostic-notes-and-stability)
- [The record](#the-record)

---

## Running evaluations

Running `evals/` requires the same prerequisites as [`examples/README.md`](../examples/README.md): a sibling [`augentic/emery`](https://github.com/augentic/emery) checkout at `../emery`, `cursor-sdk-bridge` on `PATH`, and `CURSOR_API_KEY` (or `emery-dev` and a local/remote OpenAI-compatible or Ollama server for [The small-model arm](#the-small-model-arm)).

### CLI invocations

Build the `emery` binary and the Wasm adapters under test, then run `cargo run -p evals` from the repository root:

```bash
cargo build --manifest-path ../emery/Cargo.toml --release
cargo build -p emery-typescript -p emery-python --target wasm32-wasip2 --release
set -a; source .env; set +a

# Run specific cases (or omit arguments to run all available cases)
cargo run -p evals -- orders express-orders

# Run all cases for a specific source adapter
cargo run -p evals -- python

# Dump parsed survey facts without spending any model turns
cargo run -p evals -- --facts fastapi-routers

# Specify, apply, and build the plan through emery:node (default) or emery:omnia
cargo run -p evals -- --build express-orders
cargo run -p evals -- --build --target omnia express-orders

# Rebuild a previous scorecard's sealed revision from scratch
cargo run -p evals -- --rebuild 2026-10-09T18-15-59Z express-orders

# Resume a previous scorecard's build starting at wave 3
cargo run -p evals -- --resume 2026-10-21T06-28-53Z --at 3 --target omnia express-orders

# Run the reference fixture oracle against an existing omnia build without model turns
cargo run -p evals -- --oracle 2026-10-15T17-41-37Z train
```

### Environment variables

| Variable | Default | Description |
| --- | --- | --- |
| `EMERY_BIN` | `../emery/target/release/emery` | Path to the `emery` binary. If the filename stem is `emery-dev` (compiled with `--features genai`), the runner automatically switches to `omnia-genai` environment variables (`GENAI_*`) and records the `genai` backend in the scorecard header. |
| `EMERY_TYPESCRIPT_WASM`, `EMERY_PYTHON_WASM`, `EMERY_DOCUMENTATION_WASM`, `EMERY_INTENT_WASM` | `target/wasm32-wasip2/release/emery_<kind>.wasm` | Path to each source adapter Wasm component used by the selected cases. |
| `EMERY_NODE_WASM`, `EMERY_OMNIA_WASM` | `target/wasm32-wasip2/release/emery_<kind>.wasm` | Path to the target adapter Wasm component used when `--build` is set. |
| `EVAL_RUNS` | `3` | Number of `emery specify` runs per case (3 runs distinguish stable output from single-run variance). |
| `EVAL_JOBS` | unset | Passed to `emery build` as `--jobs <N>` to cap concurrent slice builds per wave. |
| `EVAL_LADDER` | `600/120,1200/240,2400/480` | Comma-separated timeout/inactivity rungs (`CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS`, or `GENAI_TIMEOUT_SECS/GENAI_SHELL_TIMEOUT_SECS` under `emery-dev`) that `emery specify` climbs on timeout. |
| `EVAL_BUILD_RUNG` | `1200/240` | Fixed timeout/inactivity rung used for `emery build`. Under `emery-dev`, the second number caps individual shell commands (`cargo`), so raise it (e.g., `1200/1200`) if cold builds time out. |
| `CURSOR_MODEL` / `GENAI_MODEL` | `auto` / `gpt-5.5` | Model identifier recorded in the scorecard header. Pin an explicit model before comparing two runs. |
| `CURSOR_MAX_TOOL_CALLS` | unset (`128` in backend) | Maximum tool calls (`write_files`, `edit_files`, shell, reference reads) allowed per turn. Recorded in the scorecard header when set, along with `GENAI_MAX_*` knobs under `emery-dev`. |
| `RUST_LOG` | set by runner | Defaults to `trace` for `emery_sdk`, active adapters (`emery_<kind>`), and backend tool calls (`omnia_cursor::tools=trace`, `omnia_genai::tools=trace`). Preserves caller-provided `RUST_LOG` values. |

### What a run does

1. **Staging (`target/eval/<card>/<case>/`):** `<card>` is the UTC timestamp of the invocation. For each case, the runner copies the fixture files into a clean project directory (stripping `__pycache__`, virtual environments, tool caches, and `*.egg-info`), installs the required `.wasm` adapters into `~/.emery/adapters/` as `eval:<adapter>@<version>` (`eval_<adapter>@<version>.wasm`), and writes an `emery.toml` pointing to those `eval:` references.
2. **Execution:** Runs `emery specify` `EVAL_RUNS` times and exports each staged specification via `emery show <artifact> --staged --format json`.
3. **Failure handling:**
   - **Timeout or inactivity:** Automatically retries one rung higher on `EVAL_LADDER`.
   - **Backend refusal before any claim is accepted (`bad_gateway`):** Retries once at the same rung and saves the failed log as `run-N.dead.stderr`.
   - **Failure during engine synthesis (`spec-draft`, `design-draft`, or `slicing`):** Grades the accepted source claims anyway, since adapter extraction already completed.

To clean up temporary evaluation adapters from your local store after a run:

```bash
rm -f ~/.emery/adapters/eval_*
```

Recommended quick smoke-test subsets:

- **TypeScript:** `orders`, `cli-jobs`, `hono-mounted` (or all 8 `typescript` cases when modifying `survey.md`).
- **Python:** `fastapi-orders`, `click-jobs`, `flask-blueprints` (or all 6 authored `python` cases when modifying `survey.md`).

### Survey facts without a model turn (`--facts`)

Passing `--facts [case|adapter..]` stages the selected cases and invokes `emery specify` with `CURSOR_API_KEY` removed (or `GENAI_ENDPOINT` pointed at an closed local port under `emery-dev`). The adapter parses the codebase, logs the deterministic survey facts it would send to the model (bootstrap, route registrations, decorators, exports, and manifests, or Markdown outlines for `documentation`), and exits immediately before spending any model turn.

The extracted facts are printed to stdout and saved to `target/eval/facts/<case>/facts.md` (and to `run-N.facts.md` during full evaluation runs). Use `--facts` to author new `expected.toml` files or verify parser and keep-list changes in seconds.

---

## Building the plan

Passing `--build [--target <node|omnia>] [case|adapter..]` extends each successful `emery specify` run by sealing the staged candidate with `emery apply` and building the plan through [`emery-node`](../adapters/node/) (default) or [`emery-omnia`](../adapters/omnia/).

### How `--build` stages and runs

1. **Isolated target repository per run:** Before each run `N`, the runner writes a `[target]` table in `emery.toml` referencing `eval:<target>@<version>` with `name = "run-N"` and no `repository` URL.
2. **Sealing (`emery apply`):** `emery apply --config emery.toml` initializes a fresh Git repository at `.emery/vcs/repos/run-N`, commits `spec/{spec,design,plan}.json` as the root `Revision <id>` commit, and creates branch `emery/<revision>`. Logs are saved to `run-N.apply.stdout` and `run-N.apply.stderr`.
3. **Wave build (`emery build`):** `emery build --config emery.toml` runs `Task::Init` (scaffolding root manifests or the `omnia-template` guest workspace), builds each wave of slices in temporary worktrees, verifies each merged wave, and advances `emery/<revision>`. Logs are saved to `run-N.build.stdout` and `run-N.build.stderr`.

### Toolchain requirements and worker grants

Every build and verify turn runs inside the kernel sandbox configured by `emery` ([Deployment profiles](https://github.com/augentic/emery/blob/main/docs/reference/deployment-profiles.md#what-a-builds-worker-may-reach)). The worker only accesses the system paths, network hosts, and `~/` directories declared in the target adapter's `TargetAdapter::metadata().grants`:

| Target (`--target`) | Host prerequisite | Granted network egress | Granted `~/` read paths and env vars |
| --- | --- | --- | --- |
| `node` (default) | Node.js `>= 22.18` and `npm` on `PATH` | `registry.npmjs.org` | Reads `~/.nvm/versions/node`, `~/.volta` (`VOLTA_HOME=~/.volta`), and `~/.local/share/fnm`. |
| `omnia` | Stable Rust toolchain with `clippy` and `wasm32-wasip2` target installed | `index.crates.io`, `static.crates.io` | Reads `~/.rustup` (`RUSTUP_HOME=~/.rustup`) and `~/.cargo/bin`. |

Build the target Wasm component before running `--build`:

```bash
# Build and evaluate through emery:node
cargo build -p emery-node --target wasm32-wasip2 --release
EVAL_RUNS=1 cargo run -p evals -- --build express-orders nestjs-pair

# Build and evaluate through emery:omnia
cargo build -p emery-omnia --target wasm32-wasip2 --release
EVAL_RUNS=1 cargo run -p evals -- --build --target omnia express-orders
```

### How the runner grades a build (`run-N.fidelity.md`)

Because `covered` requirement IDs and verification verdicts are self-reported by the model turn, the evaluation runner independently exports the final Git tree to `run-N.fidelity/` and inspects three sources of truth:

1. **CLI envelope and Git commit history:** Reads `run-N.build.stdout`, backend `completion` and `responded` trace lines (counting `write_files` calls, rewrites, empty writes, shrinking writes, `edit_files` calls, refused calls, and verify shell durations), and `git log --first-parent` on `.emery/vcs/repos/run-N` (or `.emery/vcs/integration` if the build failed mid-run).
2. **Independent host toolchain checks:**
   - For `node`: Runs `npm install`, `npx tsc --noEmit`, and `node --test --test-force-exit --test-timeout=5000 --test-reporter=tap 'test/**/*.test.ts'`.
   - For `omnia`: Runs `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --target wasm32-wasip2`.
3. **Layout and wiring audit:**
   - **Scenario coverage:** Matches requirement acceptance scenario names against declared test names (`test(...)` / `it(...)` in TypeScript, `#[test]` / `#[tokio::test]` in Rust).
   - **Out-of-layout files:** Lists tracked files outside the target adapter's expected directory layout (`spec/` is always permitted).
   - **Unreached modules and topics:** Flags slice modules or stem crates that are never exported or wired into `src/index.ts`, `src/routes.rs`, or `src/messaging.rs`, as well as unused topic constants in `crates/common/src/topic.rs`.
   - **Probe routes:** Lists HTTP routes registered in `src/routes.rs` and flags any route whose slice has no corresponding HTTP route requirement as a **probe**.

All host check results are written to `run-N.fidelity.md` and summarized in the scorecard's build columns.

---

## Reference comparison

A benchmark case in `evals/cases/<name>/expected.toml` can optionally specify a `reference` path pointing to a hand-written Omnia Rust guest implementing the same service (such as [`cases/train/reference`](cases/train/reference/README.md) and [`cases/train-pair/reference`](cases/train-pair/reference/README.md)). When you run `--build --target omnia` on a case with a reference, the runner adds a `## Reference comparison` section to `run-N.fidelity.md` covering two checks:

### Structural measures

Compares the generated workspace against the hand-written reference across six counts (reporting both totals and any names unique to one side):

1. Member crates under `crates/`
2. Rust source files and non-blank lines of code
3. External crate dependencies across `[dependencies]` and `[workspace.dependencies]`
4. Configuration setting constants (`const NAME: &str = "NAME";`)
5. Messaging topic suffixes in `crates/common/src/topic.rs`

### Black-box fixture oracle

Drives the reference guest's recorded replay and static fixtures (`crates/*/data/{static,replay}/`) through the generated guest:

1. **Test injection:** Generates `tests/reference_oracle.rs` inside the exported workspace using `omnia_test::guest` doubles.
2. **Execution:** Feeds each fixture's inbound XML message to `messaging_router`, scripts the recorded upstream HTTP responses (including shared cache-aside lookup tables such as stops and routes), seeds configuration keys, and re-anchors relative timestamps in `Pacific/Auckland`.
3. **Verification:** Checks that the generated guest publishes the expected outbound events (matching `eventType`, `remoteData.externalId`, Kafka `key` header, and `locationData` coordinates, speed, and accuracy) or rejects invalid inputs as recorded. Any mismatch, panic, or logged `WARN`/`ERROR` event is reported per fixture.

### Standalone oracle mode (`--oracle`)

You can re-run the reference comparison on an existing scorecard's `run-1` build without invoking any model turns:

```bash
cargo run -p evals -- --oracle 2026-10-15T17-41-37Z train
```

`--oracle` exports the built tree to `run-1.oracle/` (keeping the directory so subsequent runs reuse the compiled `target/` cache and finish in seconds) and writes `run-1.oracle.md` and `run-1.oracle.log`. You can edit files inside `run-1.oracle/` to test manual fixes against the oracle; delete `run-1.oracle/` to force a fresh export from the Git repository.

---

## Building a plan again

To compare target adapter changes or model settings without re-running `emery specify` (which would introduce specification and slicing variance), use `--rebuild` or `--resume`.

### Full rebuild (`--rebuild`)

`--rebuild <card> [--target <name>] [case|adapter..]` copies the sealed `run-1` specification from `<card>` into a new scorecard directory and runs `emery build` from scratch using your current `EMERY_BIN`, target adapter `.wasm`, `CURSOR_MODEL` / `GENAI_MODEL`, and `EVAL_JOBS`:

```bash
cargo run -p evals -- --rebuild 2026-10-09T18-15-59Z --target omnia express-orders
```

The runner clones `.emery/vcs/repos/run-1` at its initial `Revision <id>` commit (without the previous `emery/<revision>` build branch), rewrites `[target]` in `emery.toml` for the selected target adapter, and reuses the source card's specification metrics in the new scorecard.

### Wave-level resume (`--resume --at`)

`--resume <card> --at <wave> [--target <name>] [case|adapter..]` copies the repository from `<card>` and resets the `emery/<revision>` branch to the final commit of `<wave> - 1` before invoking `emery build`:

```bash
cargo run -p evals -- --resume 2026-10-21T06-28-53Z --at 3 --target omnia express-orders
```

How `--resume --at` isolates a single wave:

- All slices merged in waves `1` through `<wave> - 1` (plus any no-op `start` slices in earlier waves, which the runner seals as empty commits) are treated by the engine as already `resumed`.
- The model under test starts at `<wave>`, building on top of the exact Git tree produced by the source card's earlier waves.
- `--at` must be `>= 2` (resuming at wave `1` is identical to `--rebuild`). If the local `omnia-template` pins have drifted from the seed repository in `<card>`, pass `--drifted` to allow the resume.

---

## The small-model arm

To evaluate local models (via Ollama) or open-weight models served over an OpenAI-compatible API (such as vLLM) instead of the Cursor backend, build the engine with the `genai` feature (`emery-dev`). When `EMERY_BIN` points to a binary named `emery-dev`, the runner automatically configures `GENAI_*` environment variables.

### Building `emery-dev` (`omnia-genai`)

Compile `emery` with `--features genai` into a separate target directory and copy the binary to `../emery/target/release/emery-dev`:

```bash
cargo build --manifest-path ../emery/Cargo.toml --release --features genai --target-dir ../emery/target/genai \
  && cp ../emery/target/genai/release/emery ../emery/target/release/emery-dev
```

- Setting `GENAI_MODEL=<name>` (without a prefix) connects to an Ollama server at `GENAI_ENDPOINT`.
- Setting `GENAI_MODEL=openai::<name>` with `OPENAI_API_KEY` connects to a vLLM or OpenAI-compatible server at `GENAI_ENDPOINT` and passes the turn's JSON schema via `response_format`.

| `omnia-genai` variable | Typical small-model value | Purpose |
| --- | --- | --- |
| `EVAL_JOBS` | `1` | Runs one slice build at a time so concurrent turns do not compete for local GPU memory. |
| `EVAL_BUILD_RUNG` | `7200/600` | Sets `GENAI_TIMEOUT_SECS=7200` per turn and `GENAI_SHELL_TIMEOUT_SECS=600` per shell command. |
| `GENAI_MAX_ROUNDS` | `64` | Caps tool-use rounds per turn so stuck turns fail fast into the engine's retry loop. |
| `GENAI_MAX_CANDIDATES` | `3` | Maximum final-answer validation/correction attempts per turn. |
| `GENAI_MAX_REPEATS` | `5` | Aborts a turn if the model repeats the exact same failing or identical tool call 5 times in a row. |
| `GENAI_MAX_TOKENS` | `16384` | Maximum output tokens decoded per model round. |

### Local Ollama setup (macOS fallback)

To run `qwen3-coder:30b` locally on Apple Silicon (requires ~25 GB unified memory):

```bash
ollama pull qwen3-coder:30b
OLLAMA_CONTEXT_LENGTH=131072 OLLAMA_FLASH_ATTENTION=1 OLLAMA_KV_CACHE_TYPE=q8_0 \
  OLLAMA_KEEP_ALIVE=-1 OLLAMA_NUM_PARALLEL=1 ollama serve

# In a second shell, rebuild a seed card through emery-dev
EMERY_BIN=../emery/target/release/emery-dev GENAI_MODEL=qwen3-coder:30b GENAI_ENDPOINT=http://127.0.0.1:11434/ \
  EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 \
  cargo run -p evals -- --rebuild 2026-10-19T04-08-24Z --target omnia express-orders

pkill -f 'ollama serve'
```

### Historical archive: Rounds G–M RunPod H200 setup

> **Archival note:** From Round F through Round M ([`evals/cards/README.md`](cards/README.md#protocol)), `omnia` evaluation rounds paired `composer-2.5` against an untrained small-model arm (`Qwen/Qwen3-Coder-Next-FP8` on a single NVIDIA H200 pod via RunPod). The untrained small-model ladder concluded at Round M when its stop rule fired, and standard `omnia` rounds now run single-arm under `emery` and `composer-2.5`. The configuration below is preserved as an archival reference for reproducing Rounds G–M or benchmarking fine-tuned checkpoints.

1. **vLLM server configuration (`/workspace/start-vllm.sh`):** Served `Qwen/Qwen3-Coder-Next-FP8` (79.7B total / 3B active parameters, FP8) on an H200 (143 GB VRAM) with vLLM `0.32.0`:
   ```bash
   vllm serve /dev/shm/Qwen3-Coder-Next-FP8 \
     --served-model-name Qwen/Qwen3-Coder-Next-FP8 \
     --host 0.0.0.0 --port 11434 \
     --max-model-len 262144 \
     --gpu-memory-utilization 0.92 \
     --enable-prefix-caching \
     --enable-prompt-tokens-details \
     --enable-auto-tool-choice \
     --tool-call-parser qwen3_coder \
     --tool-strict-level parameter
   ```
   Key flags: `--tool-strict-level parameter` enforces JSON schema constraints on tool call arguments via `xgrammar`, and `--enable-prompt-tokens-details` reports `cached_tokens` on responses.
2. **SSH tunnel (`127.0.0.1:11435 -> 127.0.0.1:11434`):** Forwarded directly over SSH under `caffeinate -dims` rather than RunPod's HTTP proxy (which terminates requests at 100 seconds):
   ```bash
   caffeinate -dims zsh -c 'while true; do
     ssh -N -L 11435:127.0.0.1:11434 -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes \
       -i ~/.runpod/ssh/runpodctl-ssh-key -p <port> root@<ip>
     rc=$?; echo "$(date -u +%H:%M:%SZ) tunnel closed (exit $rc); reopening in 10 s"; sleep 10
   done' &
   ```
3. **Wave probe protocol (`--resume <seed> --at <k>`):** Evaluated individual waves (`k = 2, 4` as the gate probes for `health` and `orders-reading`, followed by `k = 3, 5, 6`) over a `composer-2.5` seed card:
   ```bash
   for k in 2 4; do
     EMERY_BIN=../emery/target/release/emery-dev OPENAI_API_KEY=local \
       GENAI_MODEL=openai::Qwen/Qwen3-Coder-Next-FP8 GENAI_ENDPOINT=http://127.0.0.1:11435/v1/ \
       EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 \
       GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 GENAI_MAX_REPEATS=5 GENAI_MAX_TOKENS=16384 \
       cargo run -p evals -- --resume <seed> --at $k --target omnia express-orders
   done
   ```

---

## Experiment arms

Never implement an experimental prompt or survey change behind a Cargo feature flag (`make test` runs `--all-features`, which would cause native tests and Wasm component builds to diverge). Instead, build experimental arms from a detached Git worktree into `target/eval/arms/<arm>/`:

```bash
mkdir -p target/eval/arms/{control,subjects}
cargo build -p emery-documentation --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/emery_documentation.wasm target/eval/arms/control/

git worktree add --detach ../emery-adapters-arm HEAD
# Make experimental edits in ../emery-adapters-arm, then build:
cargo build --manifest-path ../emery-adapters-arm/Cargo.toml -p emery-documentation --target wasm32-wasip2 --release \
  --target-dir /tmp/emery-adapters-arm-target
cp /tmp/emery-adapters-arm-target/wasm32-wasip2/release/emery_documentation.wasm target/eval/arms/subjects/

EMERY_DOCUMENTATION_WASM=target/eval/arms/subjects/emery_documentation.wasm cargo run -p evals -- orders-doc cross-cut
git worktree remove --force ../emery-adapters-arm
```

---

## Cases

Every benchmark case lives under [`evals/cases/<name>/`](cases/) with an `expected.toml` file and a `fixture/` directory.

### `expected.toml` schema

- `[[source]]`: Configures `adapter` (`typescript`, `python`, `documentation`, `intent`), `fixture` path (or inline `description`), optional `name` and authority `rank`, expected `stems`, and `[[source.surface]]` pairs (`entry`, `stem`).
- `[[requirement]]`, `[[criterion]]`, `[[decision]]`: Expected claims, each with `stem`, source `anchor` (`path#Ln` or `path#Ln-Lm`), one-line `gloss`, and `source` name.
- `[[absent]]`: Source spans (`anchor`, `gloss`) where **no** `requirement` claim should anchor (such as startup credential fetching, client construction, or signal handlers that belong in the design document rather than behavioral requirements).
- `[[behaviour]]` (multi-source cases): Expected reconciled requirements across sources, specifying contributing `claims`, expected authority `status` (`agreed`, `unknown`, `divergence`, `conflict`), and optional `loser` source name.
- `reference` (optional): Path to a hand-written Omnia reference guest for [Reference comparison](#reference-comparison).

### `typescript` cases

| Case | What it probes | Expected stems |
| --- | --- | --- |
| [`orders`](cases/orders/expected.toml) | Single module within the inline byte budget. | `order-service` |
| [`express-orders`](cases/express-orders/expected.toml) | Express service with bootstrap and three routers. | `start`, `orders`, `customers`, `health` |
| [`cli-jobs`](cases/cli-jobs/expected.toml) | Commander CLI with background queue workers. | `start`, `import`, `reconcile`, `nightly`, `serve`, `invoices` |
| [`api-routes`](cases/api-routes/expected.toml) | Next.js App Router API with file-path routing and no bootstrap. | `orders`, `customers`, `health` |
| [`hono-mounted`](cases/hono-mounted/expected.toml) | Hono sub-apps mounted via `app.route("/orders", orders)`. | `start`, `orders`, `customers`, `health` |
| [`cli-table`](cases/cli-table/expected.toml) | Three CLI commands registered from a table in a loop. | `start`, `import`, `reconcile`, `nightly` |
| [`fastify-demo`](cases/fastify-demo/expected.toml) | Vendored official Fastify demo (`@fastify/autoload` directory plugins and `db:*` scripts). | `start`, `home`, `api`, `auth`, `users`, `tasks`, `create-database`, `drop-database`, `migrate`, `seed-database` |
| [`nestjs-boilerplate`](cases/nestjs-boilerplate/expected.toml) | Vendored 177-module NestJS application (scale benchmark). | `start`, `auth`, `users`, `files`, `home`, `run-seed` |
| [`train`](cases/train/expected.toml) | Production Kafka XML consumer with `[[absent]]` bootstrap spans and a 22-fixture Omnia `reference/` guest. | `train-update` (`start` surveyed with 0 requirements) |

### `python` cases

| Case | What it probes | Expected stems |
| --- | --- | --- |
| [`fastapi-orders`](cases/fastapi-orders/expected.toml) | Single-module FastAPI app with routes and `__main__` guard. | `start`, `orders` |
| [`fastapi-routers`](cases/fastapi-routers/expected.toml) | Multi-router FastAPI app exceeding `INLINE_BYTES` (one seam per stem) with data file `rates.json`. | `start`, `orders`, `customers`, `health` |
| [`flask-blueprints`](cases/flask-blueprints/expected.toml) | Flask app factory at module load, two blueprints, `add_url_rule`, and a `.feature` test file. | `start`, `health`, `orders`, `auth` |
| [`django-shop`](cases/django-shop/expected.toml) | Django project with string-referenced URLs, DRF viewset `@action`s, and management command. | `start`, `orders`, `import-orders` |
| [`click-jobs`](cases/click-jobs/expected.toml) | Click command group, APScheduler job, and Celery tasks in a `src/` layout. | `start`, `import`, `reconcile`, `nightly`, `serve`, `invoices` |
| [`money-library`](cases/money-library/expected.toml) | Pure library with no bootstrap and surfaces re-exported from `__init__.py`. | `money`, `parse-csv`, `convert` |
| [`fastapi-template`](cases/fastapi-template/expected.toml) | Vendored FastAPI full-stack template (`backend/`, 20 modules). | `start`, `items`, `users`, `login`, `utils`, `private` |
| [`django-styleguide`](cases/django-styleguide/expected.toml) | Vendored HackSoft Django styleguide example (128 modules, one seam per stem). | `start`, `auth`, `users`, `errors`, `files`, `google-oauth2`, `email-send`, `debug-task`, `test-non-concurrent-task`, `setup-periodic-tasks` |

### `documentation` cases

| Case | What it probes | Expected stems |
| --- | --- | --- |
| [`orders-doc`](cases/orders-doc/expected.toml) | Single Markdown specification within the inline budget. | `orders` |
| [`stories`](cases/stories/expected.toml) | User stories across two directories where directory and subject align. | `wishlist`, `invoices` |
| [`adr`](cases/adr/expected.toml) | Five Architecture Decision Records in one `adr/` directory (five distinct subjects). | `entries`, `posting`, `amounts`, `daily-close`, `exports` |
| [`narrative`](cases/narrative/expected.toml) | Three headingless prose files anchored at line 1. | `bookings`, `returns`, `membership` |
| [`cross-cut`](cases/cross-cut/expected.toml) | Twenty documents exceeding `INLINE_BYTES`, organized by audience (`guides/`, `reference/`, `policies/`) rather than domain subject. | `membership`, `sign-in`, `password-reset`, `catalogue`, `loans`, `reservations`, `returns`, `damage`, `notifications` |
| [`nestjs-docs`](cases/nestjs-docs/expected.toml) | Real-world `docs/` tree from the vendored NestJS boilerplate. | `auth`, `files`, `serialization`, `translations`, `database`, `architecture` |

### Multi-source cases

| Case | What it probes | Expected reconciliation |
| --- | --- | --- |
| [`orders-triad`](cases/orders-triad/expected.toml) | Documentation (`docs`), TypeScript (`code`), and inline intent (`brief`) with differing stem names. | 5 shared behaviors, 2 doc-only behaviors. |
| [`orders-brief`](cases/orders-brief/expected.toml) | Documentation alongside an higher-ranked intent brief that overrides one rule. | `divergence` (intent outranks documentation). |
| [`orders-divergence`](cases/orders-divergence/expected.toml) | Documentation and TypeScript code disagreeing on two rules. | `divergence` on both rules. |
| [`orders-conflict`](cases/orders-conflict/expected.toml) | Two `documentation` sources (`docs` and `faq`) contradicting each other at equal rank. | `conflict`. |
| [`orders-ranked`](cases/orders-ranked/expected.toml) | Same sources as `orders-conflict`, with `faq` lowered to `rank = 4`. | `divergence` (`docs` outranks `faq`). |
| [`orders-spelled`](cases/orders-spelled/expected.toml) | Documentation rewritten to use the exact same claim IDs as the code. | Cross-source pre-merge on matching IDs. |
| [`orders-broker`](cases/orders-broker/expected.toml) | Two ADRs naming Confluent Kafka and KeyVault alongside TypeScript code with vendor-named identifiers. | `leaks = 0`; vendor choices recorded as `decision` claims; secret surface stemmed as `secrets`. |
| [`nestjs-pair`](cases/nestjs-pair/expected.toml) | `nestjs-docs` combined with the 177-module `nestjs-boilerplate` codebase. | 14 shared behaviors, 3 doc-only behaviors. |
| [`train-pair`](cases/train-pair/expected.toml) | Multi-repo pipeline combining the SOAP `connector` and Kafka `train` services into one Omnia guest. | `inbound` and `train` stems; `reference/` guest with both crates. |

### Fetching vendored fixtures

Four external open-source fixtures are gitignored and fetched at pinned commits before running scale evaluations:

```bash
mkdir -p evals/cases/{nestjs-boilerplate,fastify-demo,fastapi-template,django-styleguide}/fixture
curl -sL https://github.com/brocoders/nestjs-boilerplate/archive/9620f159eefe38f47747d02ab162852367c5472c.tar.gz \
  | tar -xz --strip-components=1 -C evals/cases/nestjs-boilerplate/fixture
curl -sL https://github.com/fastify/demo/archive/5cd560125b3c2f0d42192bc7f493e8e3b9e75e52.tar.gz \
  | tar -xz --strip-components=1 -C evals/cases/fastify-demo/fixture
curl -sL https://github.com/fastapi/full-stack-fastapi-template/archive/cb740b656d7a0a6c5e12c7bf8e50343ec94ee9c7.tar.gz \
  | tar -xz --strip-components=2 -C evals/cases/fastapi-template/fixture '*/backend'
curl -sL https://github.com/HackSoftware/Django-Styleguide-Example/archive/a70ef43d7df03706c1211d4fcfd70b4b0120ba1e.tar.gz \
  | tar -xz --strip-components=1 -C evals/cases/django-styleguide/fixture
```

---

## Reading a scorecard

Each run writes a Markdown scorecard to `target/eval/<card>.md` and per-run artifacts under `target/eval/<card>/<case>/`.

### Specification metrics

| Column | Meaning and diagnostic target |
| --- | --- |
| `recall req` / `crit` / `dec` | Fraction of expected items matched by an accepted claim overlapping the expected source anchor. Misses point to `extract.md` or the SDK's allowed anchor spans. |
| `in stem` | Fraction of matched requirements whose claim ID starts with the expected stem. |
| `stems` | `exact`, or the list of missed (`-`) and invented (`+`) stems (points to mount resolution or bootstrap rules). |
| `surfaces` | `matched/expected ~restemmed +extra`. Grades the survey turn (`survey.md` and `survey/surface.rs`). |
| `behaviours` | For multi-source cases: `met/expected`, plus `~k` split requirements and `!k` wrong merges across distinct behaviors. |
| `waves` | Shape of the staged build plan (`<wave_count> (<slice_counts_per_wave>)`, e.g., `3 (1/2/1)`). |
| `mechanism` | `named/cited`: Requirements whose body text mentions source-platform code identifiers (decorators, classes, or `mechanism` design types) rather than platform-neutral behavior. |
| `leaks` | Count of technology names from `emery_sdk::Lexicon` appearing in accepted `requirement`/`criterion` claims or specification prose/scenarios. Expected to be `0`. |
| `exit` | On a failed run, names the turn where execution stopped (`survey-*` or `evidence-*` for adapter failures; `grouping`, `spec-draft`, `design-draft`, or `slicing` for engine turns). |

### Build metrics (`--build`)

| Column | Meaning |
| --- | --- |
| `built` | `merged/slices` (plus `~k` slices requiring AI merge conflict resolution, and the failure code if the build aborted). |
| `verified` | `passed/waves` across the waves executed by `emery build`. |
| `build wall` | Total wall-clock time for the `emery build` process. |
| `speedup` | Ratio of serial turn time to critical-path wave time, alongside the theoretical plan width factor (`slices ÷ waves`). |
| `covered` | Self-reported `covered` requirement IDs across merged slices vs. total requirements assigned to those slices. |
| `host checks` | Independent host verification result (`ok <passed>/<total>` when install, typecheck/clippy, and tests all pass, or `✗` on the failing step). |
| `scenarios named` | `named/scenarios (p%) · k outside · u unreached · p probes` from `run-N.fidelity.md`. |

### Diagnostic notes and stability

Below the summary tables, the scorecard lists:

- Per-run missed anchors, `unreached` modules, `absent hits`, and behavior reconciliation breakdowns (`met`, `met by criterion`, `split`, `missed`, `wrong merge`, `across kinds`, `cross-source pre-merge`).
- **`requirement anchors`:** Distribution of requirement anchors across surface heads, interior lines, or outside surveyed surfaces.
- **`Budget ladder`:** Which timeout rung each run completed on.
- **`Stability`:** Cross-run agreement across the `EVAL_RUNS` repetitions for stem sets, surveyed `(entry, stem)` pairs, requirement IDs, and specification subjects.

---

## The record

`target/` is gitignored and cleaned up by `make sweep` after one week. Permanent experimental records live under [`evals/cards/`](cards/):

- [`cards/README.md`](cards/README.md) defines the evaluation protocol, decision rules, and an index of all completed rounds with the Git commit SHA where each scorecard was recorded.
- `cards/<programme>.md` records the hypotheses, pre-registered decision rules, and scorecard tables for each evaluation programme.
