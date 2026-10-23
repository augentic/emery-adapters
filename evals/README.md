# Graded live eval

`cargo run -p evals -- [case|adapter..]` runs the shipped components end to end against the live model and grades what they extract. For each case it stages a project, runs `emery specify` three times, reads the staged candidate back through `emery show <artifact> --staged --format json`, and grades it against a reviewer's `expected.toml`. With `--build` it also seals each staged candidate with `emery apply` and builds its plan through a target adapter's component — `node`, or the one `--target` names — into a greenfield repository of the run's own and reads what landed. It drives the shipped `emery` binary alone.

Operator-invoked, never CI. The component suites in [tests/source.rs](../tests/source.rs) are the regression guard; the eval measures prompt quality, where the model's judgement is the thing under test.

## Running

The prerequisites are the live examples' ([examples/README.md](../examples/README.md)): a sibling [emery](https://github.com/augentic/emery) checkout, `cursor-sdk-bridge`, and `CURSOR_API_KEY`; an `omnia` build card's second arm needs `emery-dev` and the pod's vLLM server or a local Ollama server besides ([The small-model arm](#the-small-model-arm)). Build both binaries, then run from the repository root; every path the runner resolves is relative to it.

```bash
cargo build --manifest-path ../emery/Cargo.toml --release
cargo build -p emery-typescript -p emery-python --target wasm32-wasip2 --release
set -a; source .env; set +a

cargo run -p evals -- orders express-orders    # two cases
cargo run -p evals -- python                   # every case with a python source
cargo run -p evals -- --facts fastapi-routers  # survey facts alone, no model turn
cargo run -p evals -- --build express-orders   # specify, then build the plan
cargo run -p evals -- --build --target omnia express-orders  # the same, through omnia
cargo run -p evals -- --rebuild 2026-10-09T18-15-59Z express-orders  # build that card's plan again
cargo run -p evals -- --resume 2026-10-21T06-28-53Z --at 3 --target omnia express-orders  # build on from wave 3 of that card's chain
cargo run -p evals -- --oracle 2026-10-15T17-41-37Z train  # the reference oracle over that card's omnia build, no turn spent
```

A positional argument names a case or an adapter; with none, every case runs. A case whose fixture is absent (the vendored ones, until fetched) is skipped; naming it is an error.

| Variable | Default | Role |
| --- | --- | --- |
| `EMERY_BIN` | `../emery/target/release/emery` | The binary under test. A file named `emery-dev` is the genai runtime (`cargo build --release --features genai` in the engine checkout, the `emery` it builds copied under that name; the `emery-dev` bin target went with the engine's #371), and the runner then sets and records that backend's variables in place of Cursor's: `GENAI_MODEL` for the model, `GENAI_TIMEOUT_SECS`/`GENAI_SHELL_TIMEOUT_SECS` for a rung, `GENAI_MAX_TOOL_CALLS`, `GENAI_MAX_ROUNDS`, `GENAI_MAX_CANDIDATES`, `GENAI_MAX_REPEATS`, and `GENAI_MAX_TOKENS` as the knobs the card names; the backend is named in the card's header |
| `EMERY_TYPESCRIPT_WASM`, `EMERY_PYTHON_WASM`, `EMERY_DOCUMENTATION_WASM`, `EMERY_INTENT_WASM` | `target/wasm32-wasip2/release/emery_<kind>.wasm` | One per adapter a selected case uses |
| `EMERY_NODE_WASM`, `EMERY_OMNIA_WASM` | `target/wasm32-wasip2/release/emery_<kind>.wasm` | The target `--build` builds through, the one `--target` names alone |
| `EVAL_RUNS` | `3` | Runs per case. Three tells a stable id from a lucky one |
| `EVAL_JOBS` | unset | Passed to `emery build` as `--jobs`, the width cap; unset leaves the engine's default, which the backend's agent pool (`CURSOR_MAX_AGENTS`, 8) bounds in practice |
| `EVAL_LADDER` | `600/120,1200/240,2400/480` | Budget rungs, each `CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS`, which a specify climbs; under `emery-dev` the pair is `GENAI_TIMEOUT_SECS/GENAI_SHELL_TIMEOUT_SECS`, the second figure the cap on one shell command, since genai reads silence no other way |
| `EVAL_BUILD_RUNG` | `1200/240` | The one rung every `emery build` runs at, in the same shape; a slice's turn writes a tree and repairs its findings, and the engine resumes a build but climbs nothing, so it starts a rung up. Under `emery-dev` its second figure bounds the longest `cargo` step of a build turn, so a cold compile that outruns it is raised here, `1200/1200`, and recorded by the header |
| `CURSOR_MODEL` | `auto` | Recorded in the card. Pin it before comparing two: `auto` differs run to run. Under `emery-dev` the model is `GENAI_MODEL` (`gpt-5.5` unless set), recorded the same way |
| `CURSOR_MAX_TOOL_CALLS` | unset | The backend's tool calls a turn may make (128 unless set), the budget a build turn's `write_files` calls spend; the runner inherits it. Named in the card's header when set, as `GENAI_MAX_TOOL_CALLS`, `GENAI_MAX_ROUNDS`, `GENAI_MAX_CANDIDATES`, `GENAI_MAX_REPEATS`, and `GENAI_MAX_TOKENS` are under `emery-dev` |
| `RUST_LOG` | set by the runner | The SDK and every adapter the case runs at `trace`, either backend's completion and tool-call lines and the tool-call arguments beside them (`omnia_cursor::tools=trace`, `omnia_genai=debug,omnia_genai::tools=trace`); the card reads the trace lines. A caller's value is kept |

The defaults suit a sibling release build. Set the paths when `CARGO_TARGET_DIR` puts the artifacts elsewhere, or to point at an experiment's build.

### What a run does

For each case the runner stages a project under `target/eval/<card>/<case>/`, runs `emery specify` over it `EVAL_RUNS` times, and reads each staged candidate back through `emery show <artifact> --staged --format json`; a bare `specify` stages and seals nothing, so a later run's candidate replaces the earlier's and the project holds no repository. `<card>` is the run's UTC start and the scorecard's name, so invocations never overwrite each other and a card can be read back to its `run-N.evidence-<source>-*.json` and `run-N.facts.md` later.

Staging copies each adapter's component into the binary's store, `~/.emery/adapters`, as the package `eval:<adapter>@<version>` (file `eval_<adapter>@<version>.wasm`, replaced every time), so a card never stands over a published release or a developer's `-dev` copy; `rm ~/.emery/adapters/eval_*` cleans up. Each fixture is copied in under its source's `name`, minus a Python tree's `__pycache__`, virtual environments, tool caches, and `*.egg-info`, and an `emery.toml` names each source by `name`, `eval:` reference, and `path` or `description`.

A failed run is handled by what the failure says about the arm:

- **The backend's cap ended it** (`timeout` or `inactive`): put again one rung up `EVAL_LADDER`. The card's `Budget ladder` says where each run landed; the comparison columns stay the first rung's.
- **The backend refused it before any claim was accepted** (`bad_gateway` at a turn's opening): the host's failure. Put again once at the same rung, the dead log kept as `run-N.dead.stderr`; a second refusal stands.
- **It failed after its claims were accepted** (the engine's `spec-draft`, `design-draft`, or `slicing`): graded on those claims, since the extraction being measured had finished. The card names the turn and the seams accepted.

The smallest useful pass is one small case per stem shape: `orders`, `cli-jobs`, `hono-mounted` for `typescript`; `fastapi-orders`, `click-jobs`, `flask-blueprints` for `python`. A change to an adapter's `survey.md` is judged by all eight `typescript` cases with the vendored fixtures fetched, or the six authored `python` cases.

### Survey facts without a model turn

`--facts [case|adapter..]` stages the named cases and runs each once with `CURSOR_API_KEY` stripped, or, under `emery-dev`, with `GENAI_ENDPOINT` pointed at a loopback port nothing answers on, since a local server needs no key. The adapter logs the facts it would put before the model (bootstrap, registrations, decorated definitions, exports, packages; for `documentation`, each document's outline) and the first turn fails before any is spent, in seconds. The text is printed and written to `target/eval/facts/<case>/facts.md`; a graded run writes the same beside its log as `run-N.facts.md`.

This is how an expectation is written, since the facts say which modules locate a surface and how each export is read, and how a change to the facts or the keep policy is checked before a live run. A line the model should not see (a hook's decorator, a data-only class) is the adapter's defect, visible here.

### Building the plan

`--build [--target <name>] [case|adapter..]` seals each candidate a run stages, builds its plan through a target adapter's component into a repository the engine creates for the run, and reads what landed. `--target` names the wasm package (`node` unless told otherwise, `omnia` the other); the crates are [`emery-node`](../adapters/node/) and [`emery-omnia`](../adapters/omnia/). Before each run the runner writes the project's `emery.toml` a `[target]` table naming the component as `eval:<target>@<version>` under `name = "run-N"` and no repository — one repository per run, since `emery apply` seals a candidate over the current revision wherever a repository holds one, so a second run sealing into the first's would build over the first's tree, and a greenfield build needs a repository of its own. After a specify that landed and its `show --staged`s, `emery apply --config emery.toml` seals the candidate: the engine creates the repository at `.emery/vcs/repos/run-N` beneath the project, its root commit `Revision <id>` holding the three documents under `spec/`, labelled `emery/<revision>`. Then `emery build --config emery.toml` runs over the project (`--jobs` from `EVAL_JOBS` when set): the engine initialises the tree with what the adapter's init scaffolded — `node`'s three root manifests, `omnia`'s guest template, the engine's own `.gitignore` of `.emery/` beside them only where the init scaffolded none — sealed as `Initialise` over the `Revision` commit, builds the waves in working copies under `.emery/vcs/`, and labels the repository's head `emery/<revision>`; nothing is pushed. The apply's log and envelope are kept as `run-N.apply.stderr` and `run-N.apply.stdout`, the build's as `run-N.build.stderr` and `run-N.build.stdout`. A specify that failed seals and builds nothing, and the build cells read `—`; an apply that fails spends no turn and ends the invocation, since it is the harness's or the engine's defect and never the arm's.

The build needs what the target's prompts run, on `PATH` where `emery` runs, and the release `emery` at `EMERY_BIN`. For `node`: `node` and `npm` of Node 22.18 or later (native type stripping; the layout's `@types/node` is `^24`). For `omnia`: a stable Rust toolchain with `clippy` and the `wasm32-wasip2` target installed (`rustup target add wasm32-wasip2`), since the worker cannot install one. Every build and verify turn runs inside the worker boundary the `emery` binary compiles in ([Deployment profiles](https://github.com/augentic/emery/blob/main/docs/reference/deployment-profiles.md#what-a-builds-worker-may-reach)): the worker reads the system, the working copy, a private `HOME`, and the `~/` roots the target's worker grant names, carries nothing of the caller's environment but `PATH` and the grant's variables, and reaches the network through the backend alone — its relay for Cursor's API, its proxy for the grant's hosts. The grant is the adapter's constants (the `grants` of `TargetAdapter::metadata()`, united), put on every build and verify turn, so nothing is exported for a toolchain installed where the grant looks; a root the home lacks is logged and granted nothing. Build the component first:

```bash
cargo build -p emery-node --target wasm32-wasip2 --release
EVAL_RUNS=1 cargo run -p evals -- --build express-orders nestjs-pair
```

`node`'s grant reaches `registry.npmjs.org` and a Node under `~/.nvm/versions/node`, `~/.volta`, or `~/.local/share/fnm`, with `VOLTA_HOME` set to `~/.volta`. A tree whose install compiles a native module asks `nodejs.org` for Node's headers, which the grant does not name; the install completes and the backend's log names the one refused tunnel.

`omnia`'s grant reaches `index.crates.io` and `static.crates.io` — every host `cargo` fetches from; `crates.io` is `cargo search` alone, a lookup a turn makes by hand and goes on without — and the toolchain under `~/.rustup` and `~/.cargo/bin`, with `RUSTUP_HOME` set to `~/.rustup` so rustup's `cargo` finds it from the private `HOME`; `CARGO_HOME` is left unset, so each worker fetches the crates it needs into its scratch through the proxy:

```bash
cargo build -p emery-omnia --target wasm32-wasip2 --release
EVAL_RUNS=1 cargo run -p evals -- --build --target omnia express-orders
```

An `omnia` build card is run twice, the same command under `emery-dev` and `openai::Qwen/Qwen3-Coder-Next-FP8` on a RunPod H200 after it ([The small-model arm](#the-small-model-arm)), so every change the small model reads is measured under it.

Both targets scaffold a whole tree at the init, so the repository the engine creates is the base the eval builds over, with nothing of a project beside it but the sealed specification under `spec/`, which the engine's own guard keeps every slice out of. A build turn runs the toolchain as well as composing, and repairs the findings the adapter's gates answer its report with, so every build runs at `EVAL_BUILD_RUNG` rather than the specify's rung: `CURSOR_TIMEOUT_SECS` is process-wide and caps every turn alike, and the engine resumes a failed build from its label but climbs no ladder. A slice that outlasts the build rung is sized wrongly before it is budgeted wrongly — the engine floors a stem past twelve requirements at its sub-stems, and a plan whose slices still run long is the slicing prompt's to cut finer.

The runner reads the build three ways: the envelope on success (each slice's wave, merge commit, covered and uncovered ids, the paths its commit changed, and the paths its merge conflicted at and the model resolved; the verified heads and the waves the verify repaired; the label and where it stands) or the failure envelope the binary prints among its log lines (`slice-conflict`, `verify-failed`, the wave named in the message); the backend's `completion` lines, labelled `build-<slice>` and `verify`, for the turns' durations and tokens; and the host's `git log --first-parent` over the chain the build left — the label in `.emery/vcs/repos/run-N` when the run landed, the integration copy `.emery/vcs/integration` a failed run leaves for inspection — each merge commit read for its `Slice:`, `Wave:`, `Requirements:`, and `Covered:` trailers and the files it brought in. Where the envelope and the chain disagree, the chain is what the repository holds.

After each build the runner reads the tree itself, since `covered` and a verdict are the model's claims: the tip the chain stands at is exported to `run-N.fidelity/` (removed once read), and the host runs the target's checks in it. For `node`: `npm install`, `npx tsc --noEmit`, and `node --test --test-force-exit --test-timeout=5000 --test-reporter=tap 'test/**/*.test.ts'`, a check left unrun once the install fails. For `omnia`: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --target wasm32-wasip2`, each left unrun once an earlier one fails, the tests counted from the `test result:` line of every test binary, and an empty `clippy.toml` written at the export's root for the checks where the tree holds none (a tree built before the layout scaffolded its own), since clippy walks up from the crate to the first one it finds and the export sits beneath this repository's, which the worker under the fence never reads. It then matches the acceptance scenarios of the merged slices' requirements against the test names the tree's test files declare — the `test(` and `it(` strings, or the `#[test]` and `#[tokio::test]` fns under `tests/` and `crates/*/tests/` with their underscores read as spaces — exactly, or one name containing the other; lists the tracked files outside the target's layout (for `node` the root manifests, `src/index.ts`, `src/main.ts`, a stem's directory, a stem's test file or directory; for `omnia` the adapter's seed — the root package's files, `crates/common/`, and the operator's tooling — `Cargo.lock`, and `crates/<stem>/`; the sealed specification under `spec/` is the engine's in every tree and never outside); lists the modules the layout's lists do not reach, which a module's own tests cannot show since each drives its module alone (for `node` a stem's `index.ts` the entry does not re-export, or a part's its stem's list does not; for `omnia` a handler module `crates/<stem>/src/<part>.rs` no statement of `src/routes.rs` or `src/messaging.rs` names as `<stem>::<part>::`, a stem crate `crates/<stem>/` no Rust file outside it names as `<stem>::` — a crate the root manifest lists and nothing reaches into, which only the whole tree can show once every slice has merged — and a constant of `crates/common/src/routes/http.rs` or `routes/topic.rs` its list does not register, listed as `<table>#<NAME>`); lists the routes the tree registers per module (for `omnia`, each `.route(<path>, <method>(<handler>))` statement of `src/routes.rs`, attributed to the handler's module `crates/<stem>/src/<part>.rs`, one entry per method its second argument spells through a route fn or a `MethodFilter`, the path read from the constant of `crates/common/src/routes/http.rs` it names or a literal; the TypeScript layout registers routes on an app no one file spells, so none), marking as a **probe** a route whose module's slice has no requirement stating a method and a path, `GET /orders/{id}`, in its body or its scenarios, since such a route is the layout's shape and not the specification's; and what the checks wrote besides the lockfile (`package-lock.json`; `Cargo.lock`). The pass is written to `run-N.fidelity.md` beside the log and read into the last two build columns; a host without the toolchain leaves them empty and the card whole. Each wave's line in the run's notes also says what its verify sealed as `Wave <k> verified` — read from the envelope's `sealed` where the engine reports it, else from the chain — `(lock only)` for the lock rewrite alone, else `repaired` with the files, since an engine before `sealed` counted the lock rewrite under `repaired`; and the verify's shell part, `(shell 72s over 7 calls, 37 KB back)`: the wall time of its shell calls summed from each call to its completion, so two calls running at once count twice, and the bytes their results carried back, read from the backend's `tool call` lines under the completion's span, which is how a verify is read while the backend logs its subjects cut at eighty characters behind a `cd … &&` prefix. Each build and verify completion's line carries what its turn wrote through the SDK's tools, read from the SDK's `responded` lines: `write_files 26 calls, 6 rewrites, 1 empty, 2 shrinking; edit_files 3 calls; 1 refused` — the calls answered, the files written that the turn had written before, the files written empty, the files rewritten at under half their last size (per file where the SDK logs each file's bytes, from a one-file call's total before it did), the `edit_files` calls answered, and the calls of either tool refused.

### Reference comparison

A case may name a `reference`: a hand-written omnia guest of the behaviour its fixture states, vendored beside the fixture with its own recorded fixtures. For an `omnia` build of such a case the runner reads the export twice more before removing it, and writes both readings under `## Reference comparison` in `run-N.fidelity.md`:

- **Measures**: the built tree beside the reference on six counts — the crates under `crates/`, the Rust files and their non-blank lines, the external crates every manifest's `[dependencies]` and `[workspace.dependencies]` name (less the tree's own packages), the settings — the keys of `crates/common/src/config.rs` where a tree keeps a catalog, and every `const NAME: &str = "NAME";` a part declares under `crates/*/src`, as the omnia layout has them — and the topic suffixes of `crates/common/src/topic.rs` (or an older tree's `routes/topic.rs`, or the `topic` module of its `routes.rs`) — each a count on both sides and, for a measure of names, the names one side alone spells. A guest twice the reference's size, or one depending on a crate the reference did without, is read there before any line of it.
- **Oracle**: every fixture under the reference's `crates/*/data/{static,replay}/` driven through the built guest as a black box. The runner writes `tests/reference_oracle.rs` into the export, a native test over `omnia_test::guest` doubles that calls the guest's `messaging_router` and hands it each fixture's XML as a delivery; the fixtures' recorded HTTP answers are scripted at their URLs (and, for a trip allocation, at the URL and in the `{ all: [{ vehicleLabel }] }` shape the TypeScript source reads), the configuration is seeded under the reference's keys, the source's, and every key of the built tree's catalog a pattern reads a value into (an endpoint, an identity, the station filter, the delay window), and a fixture recorded with a `delay` has its creation date and first change's actual time re-anchored to now in `Pacific/Auckland`, as the reference's own tests re-anchor them. The topic the guest consumes under is probed from its topic table and the source's default, `dev-realtime-train.v1`. A fixture passes when the guest publishes the recorded events — compared by `eventType`, `remoteData.externalId`, and `locationData`'s coordinates, speed, and accuracy, camelCase or snake_case, with the Kafka `key` header the external id — or, for a recorded refusal, refuses or publishes nothing; every difference is a reason in the fixture's row, a panic (an upstream request the fixture does not answer) among them, and, for a fixture that fails, the last `warn` or `error` event the guest logged as it ran, `logged: WARN message::validation: invalid inbound message: xml parse: duplicate field …`, since a handler that logs a failure and answers `Ok(())` publishes nothing and says why there alone. A GET exchange two or more fixtures record at one path and query is a table — the stops, the routes — which a guest reading it cache-aside fetches on its first miss whatever fixture it is driven by first; such an exchange answers any fixture whose own recording lacks the path, the fixture's own answers first, and the paths answered that way are listed beneath the oracle's summary. The harness needs `chrono`, `chrono-tz`, `http`, `bytes`, `serde_json`, and `tracing` beside the layout's `omnia-test` and `tokio`; what the root package depends on under no table is appended as a native dev-dependency for the run, the workspace's pin where it carries one. The oracle is `not run` with the cause where the tree declares no root package, lacks the layout's dev-dependencies, or consumes none of the candidate topics.

The build findings carry one line, `reference oracle 18/22; crates 3 vs 2, Rust lines 1420 vs 1580, dependencies 27 vs 25`.

`--oracle <card> [case|adapter..]` runs the comparison alone over the tree `run-1` of each named case left under an earlier card — the label its build set, or the integration copy a failed build left — with no binary, component, or model: the tip is exported once to `run-1.oracle/` beside the run's files and kept, so the second invocation reuses its build directory and answers in seconds, and an operator may patch the export by hand to see what a claim's gap would cost before a build is spent on it; the comparison is written to `run-1.oracle.md` and printed, the harness's own output to `run-1.oracle.log`, and the export's build directory, the harness, and its fixtures are left out of the measures. Removing `run-1.oracle/` exports the tip again. Two cases carry a reference today: [cases/train/reference](cases/train/reference/README.md) is the `train` repository trimmed to its `position-adapter` crate and the `common` it reads, with every fixture's `output` made explicit, and [cases/train-pair/reference](cases/train-pair/reference/README.md) is the same repository trimmed to the `soap-connector` and `position-adapter` crates, the connector's `POST /inbound/xml` route wired to the topic the adapter consumes. `cargo test --manifest-path evals/cases/train/reference/Cargo.toml --workspace` runs a reference's own tests; nothing in the eval does.

### Building a plan again

`--rebuild <card> [--target <name>] [case|adapter..]` builds the plan `run-1` of each named case sealed under an earlier card again, under this invocation's `EMERY_BIN`, target, `CURSOR_MODEL`, and `EVAL_JOBS`, so one revision is built under several arms — a plan is target-neutral, so the arms may build it through different targets — and the plan's own variance is out of the comparison. The staged project is copied whole under the new card, less the engine's state, its working copies, the repository the earlier run sealed and built into, and the earlier build's files; that repository is cloned at its head alone — the engine creates it with the `Revision` commit as its root and never moves its head off it, so the clone holds the sealed revision under `spec/` and no label — into `.emery/vcs/repos/run-1` of the new project, so `emery build` reads the revision from the head's tree, finds no label, and builds every slice over the root; the `[target]` table of `emery.toml` is rewritten for this invocation's target; and the run's specify is read back from the files the earlier run kept, so the card's specify columns and completions repeat the source card's, which its header says. The card the rebuild stands on must still be under `target/eval/` and be a `--build` card of this harness: one whose `run-1` sealed its revision into `.emery/vcs/repos/run-1`, which a card of an earlier harness, holding its revision in a scratch store under an earlier grammar, never did.

`--resume <card> --at <wave> [--target <name>] [case|adapter..]` builds the same plan on from one wave of the chain the earlier card's build left, so one slice's turn is measured over the tree the earlier arm built up to it rather than over a tree of this arm's own. The staged project is copied as `--rebuild` copies it, with the repository the earlier build created (`.emery/vcs/repos/target`) and never its integration copy or worktrees, and the label `emery/<revision>` in it is moved to the last commit of wave `<wave> - 1` — a slice merged in it or its `Wave <k> verified` seal — so the engine's own resume does the rest: the slices the chain still names are `resumed` and not built again, the plan's other slices are built in waves over that tree, and the first slice this invocation's model builds is one of wave `<wave>`'s. A slice the earlier build merged as nothing — a `start` slice, `commit: null` in its `run-1.build.stdout` — stands in no commit of the chain, so the engine would build it again, a wave of its own verified over the seed before the probe's slice; the runner seals every such slice of the waves before `<wave>` but the first over the moved label, each an empty commit under the engine's own message, so one seed verify stands before the slice at any `<wave>` and the sealed slices are `resumed` beside the chain's. The build runs on to the end or its failure, so every later slice gets an attempt over the earlier arm's tree too. `--at` names a wave past the first; a resume at wave 1 is a `--rebuild`. The card's header names the source card, the wave, and the commit the label was moved to, and the build line and the build notes count the slices `resumed` and the ones `built` this run apart; the specify columns repeat the source card's, as under `--rebuild`.

### The small-model arm

Every `omnia` build card is one command run twice over one component: under the release `emery` and `composer-2.5`, then under `emery-dev` and a small open-weights model ([the protocol](cards/README.md#protocol), "An omnia card is a pair"). The model is `Qwen/Qwen3-Coder-Next-FP8` (79.7B total / 3B active, fine-grained 128×128 block-scaled FP8 safetensors, 74.9 GiB) served by vLLM on a RunPod pod with one NVIDIA H200 and reached through an SSH tunnel as `GENAI_MODEL=openai::Qwen/Qwen3-Coder-Next-FP8`; the local `qwen3-coder:30b` under Ollama on the Mac is the fallback where no pod is up. Whoever runs the round runs both arms, an agent asked to run the eval included: it brings the server up where none answers, runs the small-model arm after the large-model one, and closes the tunnel or stops the local server before it reads the card. The arm needs `emery-dev`, the same runtime over [omnia-genai](https://github.com/augentic/omnia-backends/tree/main/crates/genai) built in the engine checkout under the `genai` feature — the runner reads the backend from the binary's name — and a server with its context length set (`--max-model-len 262144` under vLLM, `OLLAMA_CONTEXT_LENGTH=131072` under the local fallback).

```bash
cargo build --manifest-path ../emery/Cargo.toml --release --features genai --target-dir ../emery/target/genai \
  && cp ../emery/target/genai/release/emery ../emery/target/release/emery-dev
```

The feature builds the one `emery` binary over `omnia-genai` in place of Cursor, so it is built into a target directory of its own and copied under the name the runner reads the backend from, leaving the release `emery` in place.

**On the pod.** `runpodctl` 3.0.1 with the account's API key in `~/.runpod/config.toml`, and the key pair it generated at `~/.runpod/ssh/runpodctl-ssh-key`. The pod is `kfon5fxhta2bcr` (`qwen3-coder-next`, `runpod/pytorch:1.0.2-cu1281-torch280-ubuntu2404`, 24 vCPU, 251 GB, one H200 of 143 GB, 30 GB of container disk, and the 120 GB network volume `cxsks2la1u` mounted at `/workspace`, $5.29 an hour while it runs); its SSH port and, on Community Cloud, its address change when it resets, so read them again before each run.

```bash
runpodctl pod list                                  # the pod and whether it runs
runpodctl ssh info kfon5fxhta2bcr                   # the ssh line: root@<ip> -p <port> with the key
ssh -i ~/.runpod/ssh/runpodctl-ssh-key -p <port> root@<ip> 'curl -s http://127.0.0.1:11434/v1/models'   # the model loaded, max_model_len 262144
```

The server runs through `/workspace/start-vllm.sh`, its log at `/workspace/vllm.log`: the script restores `/opt/vllm-venv` (vLLM 0.32.0 with `nvidia-cuda-nvcc==13.0.88` for Hopper `sm_90a` DeepGEMM FP8 kernels) from `/workspace/vllm-venv.tar.zst` when a container reset cleared the root disk, stages `/workspace/models/Qwen3-Coder-Next-FP8` into `/dev/shm/Qwen3-Coder-Next-FP8` when RAM was cleared, sets `VLLM_CACHE_ROOT=/workspace/.cache/vllm` so the warmed `torch.compile` and 1,253 DeepGEMM cubins survive resets, and runs `vllm serve /dev/shm/Qwen3-Coder-Next-FP8 --served-model-name Qwen/Qwen3-Coder-Next-FP8 --host 0.0.0.0 --port 11434 --max-model-len 262144 --gpu-memory-utilization 0.92 --enable-prefix-caching --enable-prompt-tokens-details --enable-auto-tool-choice --tool-call-parser qwen3_coder --tool-strict-level parameter`. (`--tool-strict-level parameter` puts `xgrammar` into `OrFormat([tools_structural_tag, output_format])` so each round constrains tool calls to `<tool_call>\n<function=…>` with every call's arguments held to the tool's parameter schema, and constrains any non-tool reply to the request's `response_format` JSON schema; `function`, the level through Round J, held the tool's name alone and let five `write_files` calls of the wrong shape through to the SDK's refusal. `--enable-prompt-tokens-details` has the server report `prompt_tokens_details.cached_tokens` on every reply, which the client reads as the round's `cache_read_tokens`; without it the field is `null` and the card's `cached` reads 0 against a server-side hit rate above 0.95, as every line of Round H did. A rung of the ladder that sets the sampling adds `--override-generation-config '{"temperature":0}'` to the same line, since nothing in the engine's or the SDK's requests sets one and the model's own `generation_config.json` applies otherwise.)

```bash
ssh -i ~/.runpod/ssh/runpodctl-ssh-key -p <port> root@<ip> 'nohup /workspace/start-vllm.sh > /workspace/vllm.log 2>&1 < /dev/null &'
```

The run owns a tunnel of its own, `127.0.0.1:11435` to the pod's `11434`, kept in a shell of its own for the arm; never the pod's proxy URL (`https://<pod>-11434.proxy.runpod.net`), whose edge cuts a request at 100 s while a completion's round is one request. A dropped tunnel is one `BadGateway` retry per slice and then the run's failure, so a long run wraps the tunnel in a reconnect loop that reads the pod's address and port from `runpodctl ssh info` again on each turn of the loop, since both change when the pod resets, and runs the loop under `caffeinate -dims` so the Mac never sleeps under it.

```bash
caffeinate -dims zsh -c 'while true; do
  ssh -N -L 11435:127.0.0.1:11434 -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes \
    -i ~/.runpod/ssh/runpodctl-ssh-key -p <port> root@<ip>
  rc=$?; echo "$(date -u +%H:%M:%SZ) tunnel closed (exit $rc); reopening in 10 s"; sleep 10
done' &
curl -s http://127.0.0.1:11435/v1/models      # the server answers through the tunnel with Qwen/Qwen3-Coder-Next-FP8

EMERY_BIN=../emery/target/release/emery-dev OPENAI_API_KEY=local \
  GENAI_MODEL=openai::Qwen/Qwen3-Coder-Next-FP8 GENAI_ENDPOINT=http://127.0.0.1:11435/v1/ \
  EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 \
  GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 GENAI_MAX_REPEATS=5 GENAI_MAX_TOKENS=16384 \
  cargo run -p evals -- --rebuild 2026-10-19T04-08-24Z --target omnia express-orders
kill %1                                       # the tunnel; the pod and its server stay up, and are the operator's to stop
```

**The arm as probes.** Where a round changes what one slice's turn reads rather than the plan, the arm is run as probes instead of one `--rebuild`: one `--resume` per wave of the large-model arm's chain, each over the tree the large model built up to that wave, so every slice is measured at least once over a tree the small model did not have to build first and a probe that builds on yields whole-build evidence too. Round H's five probes are waves 2 through 6 of `2026-10-21T06-28-53Z` (`health`, `customers`, `orders-reading`, `orders-placement`, `orders-mutations`), run one at a time under the same knobs; a control under the earlier round's artifacts (`EMERY_BIN` and `EMERY_OMNIA_WASM` pointed at copies kept outside `target/`) runs first on the same command, so what the server or the knobs alone move is read before the round's items are. From Round I the arm climbs a ladder, one rung a round, with a gate before the rest of the ladder ([the protocol](cards/README.md#protocol), "The small-model arm is a ladder"): rungs 0 through 2 were read at `k = 2` alone, one or two `--resume 2026-10-21T06-28-53Z --at 2` probes, about a quarter of an hour of pod time together; from rung 3 the gate reads `k = 2` and `k = 4` together — `--resume <seed> --at 2` and `--at 4`, `health` and `orders-reading`, about half an hour — since `k = 2` was met three times under rung 0 and said nothing about the slices that need the store API, and the probes at `k = 3`, `5`, and `6` and the `--rebuild` run only once both gate probes build to their labels. The card resumed is the latest large-model `--build` card built under this harness: `--resume` and `--rebuild` copy `.emery/vcs/repos/run-1` of the source card and read the chain there, so a card built before `apply` (L0, `2026-10-22T05-26-59Z`, whose repository holds an `Initialise` root and no `spec/`) resumes under nothing, and Round L re-seeds with a fresh `--build` under `composer-2.5`; `06-28-53Z`'s seed holds pins the component's template has moved past besides, which the runner refuses before any request unless `--drifted` admits it.

```bash
EVAL_RUNS=1 cargo run -p evals -- --build --target omnia express-orders   # the seed, under emery and composer-2.5
for k in 2 4; do                              # the gate: health, then orders-reading
  EMERY_BIN=../emery/target/release/emery-dev OPENAI_API_KEY=local \
    GENAI_MODEL=openai::Qwen/Qwen3-Coder-Next-FP8 GENAI_ENDPOINT=http://127.0.0.1:11435/v1/ \
    EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 \
    GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 GENAI_MAX_REPEATS=5 GENAI_MAX_TOKENS=16384 \
    cargo run -p evals -- --resume <seed> --at $k --target omnia express-orders
done
for k in 3 5 6; do                            # the rest of the ladder, once both gate probes build to their labels
  EMERY_BIN=../emery/target/release/emery-dev OPENAI_API_KEY=local \
    GENAI_MODEL=openai::Qwen/Qwen3-Coder-Next-FP8 GENAI_ENDPOINT=http://127.0.0.1:11435/v1/ \
    EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 \
    GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 GENAI_MAX_REPEATS=5 GENAI_MAX_TOKENS=16384 \
    cargo run -p evals -- --resume <seed> --at $k --target omnia express-orders
done
EMERY_BIN=/tmp/roundL/control/emery-dev EMERY_OMNIA_WASM=/tmp/roundL/emery_omnia.wasm OPENAI_API_KEY=local \
  GENAI_MODEL=openai::Qwen/Qwen3-Coder-Next-FP8 GENAI_ENDPOINT=http://127.0.0.1:11435/v1/ \
  EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 \
  GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 GENAI_MAX_REPEATS=5 \
  cargo run -p evals -- --resume <seed> --at 2 --target omnia express-orders   # the control, under the earlier round's binary and knobs
```

The server's own log says what the backend's token counts cannot: `grep 'Engine 000:' /workspace/vllm.log` gives the prompt throughput, generation throughput, GPU KV cache usage, and prefix cache hit rate. On the pod, vLLM holds 74.9 GiB of weights and 50.2 GiB of hybrid attention/Mamba (`align`) KV cache (`2,165,911` tokens, `8.26×` concurrency at `262,144` max context), evaluates cold prompts at ~18,200 tokens a second, and generates a single stream at 177 tokens a second (against Ollama's 1,226–1,508 prefill and 42.3–47.0 decode on `05-21-29Z` and `07-04-07Z`).

**On the Mac, the fallback.** `qwen3-coder:30b`, 19 GB on disk and 25 GB resident with every layer on the GPU of a 36 GB Mac, so nothing else heavy runs beside the arm; the server in a shell of its own, stopped after the arm since the model stays resident until it is.

```bash
ollama pull qwen3-coder:30b
OLLAMA_CONTEXT_LENGTH=131072 OLLAMA_FLASH_ATTENTION=1 OLLAMA_KV_CACHE_TYPE=q8_0 \
  OLLAMA_KEEP_ALIVE=-1 OLLAMA_NUM_PARALLEL=1 ollama serve
curl -s http://127.0.0.1:11434/api/tags    # the server answers with the models it serves

EMERY_BIN=../emery/target/release/emery-dev GENAI_MODEL=qwen3-coder:30b GENAI_ENDPOINT=http://127.0.0.1:11434/ \
  EVAL_RUNS=1 EVAL_JOBS=1 EVAL_BUILD_RUNG=7200/600 GENAI_MAX_ROUNDS=64 GENAI_MAX_CANDIDATES=3 \
  cargo run -p evals -- --rebuild 2026-10-19T04-08-24Z --target omnia express-orders
pkill -f 'ollama serve'                    # the model stays resident until the server stops
```

The knobs are the small-model band's, and move only where a round names the move and its header records it: `EVAL_JOBS=1`, since one slice runs at a time; `EVAL_BUILD_RUNG=7200/600`, the cap on one build turn over the cap on one shell command, the longest `cargo` step a turn may run, raised (`7200/1200`) where a card's turns die in the shell at it; `GENAI_MAX_TOOL_CALLS` at the backend's default (128), `GENAI_MAX_ROUNDS=64` (halved from 128 after Round G's `07-04-07Z` so a stuck turn fails fast into the engine's retry instead of spending 64 tail rounds at peak context), `GENAI_MAX_CANDIDATES=3`, one above the backend's default, since the correction round is where the small model did its best work and the second candidate is where it ran out on `2026-10-20T21-42-12Z` (Round G names the move), and `GENAI_MAX_REPEATS=5`, the backend's default written out so the header records it: the identical tool call failing that many times in a row ends the completion as `exhausted`, where Round G's turns spent their rounds on one refused write (Round H names it), and the identical call returning the same result that many times puts the finishing round at once, where Round H's verifies ran one `stat` or `find` to the round cap. `GENAI_MAX_TOKENS=16384`, half the backend's default (32768), the most tokens one round may decode where the request names no bound: a round that runs away ends in minutes rather than at the context (Round H's k = 5 lost 23 minutes to one), and one 32,768-token reply is a lost round — Round K's `health` spent 165 s of silence on one, cut at the cap and handed to the SDK as `edit_files {}` — where under 16,384 the cut reply costs about eighty seconds, is refused by the backend with the reason, and is corrected by the next call, while a 19 KB whole write still fits (Round L names the move). A bare `GENAI_MODEL` names an Ollama model at `GENAI_ENDPOINT`; `openai::<name>` with `OPENAI_API_KEY` names a vLLM or any OpenAI-compatible server, where `omnia-genai` also sends the turn's JSON schema as `response_format`. The card's header records the backend, the model, and every knob set, and `--facts` under `emery-dev` needs no server at all. A card is read against the band of its model: under `Qwen3-Coder-Next` the latest card of the model (`2026-10-21T05-21-29Z` and `2026-10-21T07-04-07Z`) until one passes it; under the 30b, C1 (`2026-10-19T22-47-32Z`). A difference between the two arms of one round is the model's alone, since the plan, the component, and the knobs are one; a difference between two small-model cards of one model is the round's.

### Experiment arms

An arm — a rule switched off in one function, or new code — is a local edit, run and never shipped, and never a Cargo feature: `make test` runs the workspace under `--all-features`, which would turn the arm on in the native suite while the component build kept the default, so the two rungs would assert two behaviours. Build it from a detached worktree, so the tree you commit from never holds the arm and the shipped component stays at hand as the control, each into its own path under `target/eval/arms/<arm>/`, so a later build never overwrites what an earlier card ran over:

```bash
mkdir -p target/eval/arms/{control,subjects}
cargo build -p emery-documentation --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/emery_documentation.wasm target/eval/arms/control/
git worktree add --detach ../emery-adapters-arm HEAD
# edit the arm in ../emery-adapters-arm, then
cargo build --manifest-path ../emery-adapters-arm/Cargo.toml -p emery-documentation --target wasm32-wasip2 --release \
  --target-dir /tmp/emery-adapters-arm-target
cp /tmp/emery-adapters-arm-target/wasm32-wasip2/release/emery_documentation.wasm target/eval/arms/subjects/
EMERY_DOCUMENTATION_WASM=target/eval/arms/subjects/emery_documentation.wasm cargo run -p evals -- orders-doc cross-cut
git worktree remove --force ../emery-adapters-arm
```

## Cases

Each case is a directory under [cases/](cases/) holding an `expected.toml` and, for each source bound to a tree, a `fixture/` beside it. The header comment of each `expected.toml` tells the case's story; the tables below say what each probes. The live [examples](../examples/) and the component suites read the same trees.

`expected.toml` names the run's sources, then the claims a reviewer would write from each:

- `[[source]]`: the `adapter` (`typescript`, `python`, `documentation`, `intent`); a `fixture` path from the repository root, or an inline `description`; a `name` the specification cites it by (the adapter's name when absent); a `rank`, the integer the staged `emery.toml` carries as the source's authority, `1` the highest, the adapter's kind deciding when absent; the `stems` the source's requirements lead with; and the `[[source.surface]]` pairs (`entry`, `stem`) its survey must decide.
- `[[requirement]]`, `[[criterion]]`, `[[decision]]`: each a `stem` (or a list), an `anchor` (`path#Ln` or `path#Ln-Lm`, under the source's tree), a one-line `gloss`, and a `source` where the case has several.
- `[[absent]]`: a span no accepted `requirement` claim should anchor within — an `anchor` (a file, or lines of one), a `gloss`, and a `source` where the case has several, never a `stem` — the readiness and lifecycle code (credentials, prefetch, clients, signal handlers, `listen`) whose behaviour is the design's. A requirement claim held to the span, half or more of its lines within it, is a hit; the notes print `absent hits: N` per run with each hit's span and claim, and the stability section the band across runs. Recall alone cannot show a run stating what the specification should leave out, since dropping an item only shrinks the denominator.
- `reference`, optional: the path from the repository root of a hand-written omnia guest of the same behaviour, which a `--build --target omnia` run measures the built tree against and whose recorded fixtures it drives through the built guest ([Reference comparison](#reference-comparison)).

A criterion is a boundary the code spells as a value of its own (a named constant, a default, a pattern) at the line that binds it, never a literal inside the branch that uses it. Middleware mounted for every route is expected under `start`.

### `typescript`

| Case | What it probes | Stems |
| --- | --- | --- |
| [orders](cases/orders/expected.toml) | One module, one stem: the inline-budget path | `order-service` |
| [express-orders](cases/express-orders/expected.toml) | An Express service: a bootstrap and three routers | `start`, `orders`, `customers`, `health` |
| [cli-jobs](cases/cli-jobs/expected.toml) | A commander CLI with queue workers | `start`, `import`, `reconcile`, `nightly`, `serve`, `invoices` |
| [api-routes](cases/api-routes/expected.toml) | A Next.js app-router API: handlers routed by file path, no bootstrap, nothing registered | `orders`, `customers`, `health` |
| [hono-mounted](cases/hono-mounted/expected.toml) | Sub-applications mounted with `app.route("/orders", orders)` and registering at `"/"`: the resource is in the mount | `start`, `orders`, `customers`, `health` |
| [cli-table](cases/cli-table/expected.toml) | Three commands registered from a table in one loop: one registration, three surfaces | `start`, `import`, `reconcile`, `nightly` |
| [fastify-demo](cases/fastify-demo/expected.toml) | The official Fastify demo, vendored. Routes registered on an untyped plugin parameter in modules `@fastify/autoload` loads by directory; four scripts the manifest's `db:*` scripts run | `start`, `home`, `api`, `auth`, `users`, `tasks`, `create-database`, `drop-database`, `migrate`, `seed-database` |
| [nestjs-boilerplate](cases/nestjs-boilerplate/expected.toml) | A 177-module NestJS application, vendored: the scale case. Nine controllers, two seed runners | `start`, `auth`, `users`, `files`, `home`, `run-seed` |
| [train](cases/train/expected.toml) | A production Kafka consumer, vendored whole: train-update XML read into vehicle location events through two upstream APIs, behind private wrapper packages that hide the frameworks, so the one registration is a callback handed to a consumer and led by no literal. The bootstrap's secret, clients, prefetch, status log, and exit timers are listed `[[absent]]`, so the case measures the rule that readiness is the design's. The build case for `omnia`: `reference/` beside the fixture is the hand-written guest the same service was ported to, with its 22 recorded fixtures | `train-update`; `start` surveyed, holding no requirement |

The last four were written before any run to catch a parser's conventions reading a tree wrongly.

### `python`

Six authored cases mirror `typescript` ones so both adapters are read against the same shapes; two vendored real trees were written from `--facts` and the code before any run.

| Case | What it probes | Stems |
| --- | --- | --- |
| [fastapi-orders](cases/fastapi-orders/expected.toml) | One module with the application, its routes, and its `__main__` guard: the inline-budget path and the one-module bootstrap rule | `start`, `orders` |
| [fastapi-routers](cases/fastapi-routers/expected.toml) | Mirrors `express-orders`. A console script enters `app.main`, which includes three routers; a generated table puts the tree past `INLINE_BYTES`, so one seam per stem; a `rates.json` read by a computed path is inlined as data | `start`, `orders`, `customers`, `health` |
| [flask-blueprints](cases/flask-blueprints/expected.toml) | A Flask factory constructed at load with no guard or script; two blueprints and one `add_url_rule`; a `.feature` file read as stated behaviour | `start`, `health`, `orders`, `auth` |
| [django-shop](cases/django-shop/expected.toml) | A Django project: settings and URL modules reached by the strings that name them, views under the `orders/` mount, a DRF viewset with `@action`s, a management command, a migration skipped | `start`, `orders`, `import-orders` |
| [click-jobs](cases/click-jobs/expected.toml) | Mirrors `cli-jobs`. A Click group, an APScheduler job, and two Celery tasks in a `src/` layout, mined whole | `start`, `import`, `reconcile`, `nightly`, `serve`, `invoices` |
| [money-library](cases/money-library/expected.toml) | Mirrors `orders`. A `src/` library with no bootstrap whose `__init__` re-exports the surfaces | `money`, `parse-csv`, `convert` |
| [fastapi-template](cases/fastapi-template/expected.toml) | FastAPI's full-stack template `backend/`, vendored. Construction at load, a router prefix the code does not spell, a prefix-less router stemmed by its tag. Twenty modules, one seam | `start`, `items`, `users`, `login`, `utils`, `private` |
| [django-styleguide](cases/django-styleguide/expected.toml) | HackSoft's styleguide example, vendored: the `python` scale case. Settings by star imports, namespaced `include((module, namespace))`, nested inline includes, a task registered through a wrapper. 128 modules, one seam per stem | `start`, `auth`, `users`, `errors`, `files`, `google-oauth2`, `email-send`, `debug-task`, `test-non-concurrent-task`, `setup-periodic-tasks` |

### `documentation`

Six cases read prose alone, each written before any run to a shape the shipped survey (one seam per top-level directory) reads differently. Each names its surfaces (document, stem), its requirements at the paragraphs, bullets, and sentences that state them, and its decisions where a document records one.

| Case | What it probes | Stems |
| --- | --- | --- |
| [orders-doc](cases/orders-doc/expected.toml) | The live example's one document ([examples/documentation/docs](../examples/documentation/docs)): the inline-budget path | `orders` |
| [stories](cases/stories/expected.toml) | User stories, two directories of two. The directories and the subjects agree | `wishlist`, `invoices` |
| [adr](cases/adr/expected.toml) | Five decision records in one `adr/` directory, each its own subject, which one directory lumps into one seam | `entries`, `posting`, `amounts`, `daily-close`, `exports` |
| [narrative](cases/narrative/expected.toml) | Three headless prose pieces at the root: no heading to anchor at, only a first line to name by | `bookings`, `returns`, `membership` |
| [cross-cut](cases/cross-cut/expected.toml) | Twenty documents past `INLINE_BYTES`, cut by audience into `guides/`, `reference/`, `policies/` while the subjects cut across them. Written to break the directory cut | `membership`, `sign-in`, `password-reset`, `catalogue`, `loans`, `reservations`, `returns`, `damage`, `notifications` |
| [nestjs-docs](cases/nestjs-docs/expected.toml) | The vendored NestJS boilerplate's own `docs/`: the real-tree case | `auth`, `files`, `serialization`, `translations`, `database`, `architecture` |

### Several sources

Nine cases run several sources through one `specify`; eight grade the reconciled specification by its behaviours (below), and `train-pair` grades each source's items at its own tree.

| Case | What it probes | Expects |
| --- | --- | --- |
| [orders-triad](cases/orders-triad/expected.toml) | The example document (`docs`), the `orders` module (`code`), and an inline `intent` brief (`brief`, anchoring nothing, contributing authority alone), their stems disagreeing by name | Five behaviours both state, two the document alone |
| [orders-brief](cases/orders-brief/expected.toml) | The document beside a brief that disagrees on one rule. Does a byte-equal pre-merge hide a disagreement? | `divergence`, the brief outranking the document |
| [orders-divergence](cases/orders-divergence/expected.toml) | The document rewritten so two rules differ from the code's in kind | `divergence` on both |
| [orders-conflict](cases/orders-conflict/expected.toml) | A support FAQ as a second `documentation` source (`faq`) contradicting the document: two sources of one kind disagree | `conflict` |
| [orders-ranked](cases/orders-ranked/expected.toml) | `orders-conflict`'s sources with the FAQ at `rank = 4`, beneath the code: the key alone turns the status | `divergence`, the document outranking the FAQ |
| [orders-spelled](cases/orders-spelled/expected.toml) | The document rewritten under the code's own names, so the stems agree by the byte | The one cross-source pre-merge, against `orders-triad` where none is |
| [orders-broker](cases/orders-broker/expected.toml) | Two decision records (`docs`) choosing the broker and the secret store by name, beside a small `typescript` service (`code`) whose identifiers spell them (`useConfluentKafkaConfig`, `ConfluentSecretRetriever`, `KeyVault`, `kafkajs`): the technology case, written to leak | `leaks` 0; each record's prescription a `decision`; five behaviours by role both state; the secret-store surface stemmed `secrets`, never `key-vault` |
| [nestjs-pair](cases/nestjs-pair/expected.toml) | `nestjs-docs` beside the whole `nestjs-boilerplate` tree: the cross-source scale case | Fourteen behaviours both state, three the documents alone |
| [train-pair](cases/train-pair/expected.toml) | The `train` fixture beside the SOAP connector that feeds it (`connector`), two `typescript` services of one pipeline, each bootstrapped from its own `start`: the multi-repo consolidation case. The connector's one surface is a SOAP service handed to `soap.listen` as an object, which the facts do not locate. The build case for a consolidated `omnia` guest: `reference/` is the `train` guest trimmed to both crates | `inbound` beside the `train` stem; each `start` surveyed and holding no requirement, its readiness spans `[[absent]]` per source |

A `[[behaviour]]` is one thing the specification should hold once, however many sources state it: a `gloss`, the contributing `claims` (each a `source` and an `anchor` in its tree), and, where the engine's authority rule decides it, the `status` the reconciled requirement should carry (`agreed`, `unknown`, `divergence`, `conflict`). A `divergence` or `conflict` may name the `loser`, the source whose statement the requirement's loser note is from; the status is then met only by a requirement that lost that source, so a disagreement between two other sources does not pass for the one planted.

Grading reads `emery show spec --staged --format json`. A requirement cites a behaviour when its citation shares at least as many lines with the anchor as it spends outside it, so a whole-method citation states none of the branches inside it. Anchor a behaviour at the lines that state it whole.

| Outcome | Meaning |
| --- | --- |
| met | one requirement cites it |
| met by criterion | none cites it, but a requirement `covers` an accepted criterion held there. Noted so the kind the model chose shows |
| split | two or more requirements cite it |
| missed | none holds it |
| wrong merge | one requirement cites two behaviours |
| across kinds | one requirement cites it, another holds it as a criterion; no grouping can join them |
| cross-source pre-merge | the engine joined a requirement on a byte-equal claim id from two sources. Counted apart: the one grouping the model never revisits |

A criterion counts toward neither split nor wrong merge. Prose stating two rules in one sentence would read as a split or a wrong merge; the anchors steer around it where they can, and where they cannot (the cancellation rule, one sentence in the document and two branches in the code) the case allows the one split.

### Vendored fixtures

Four real trees are not committed. Each is fetched at a pinned commit into its case's gitignored `fixture/`, and the `expected.toml` names the commit. `nestjs-docs` and `nestjs-pair` read the `nestjs-boilerplate` fixture too. The `train` and `train-pair` fixtures and their `reference/` trees are committed whole: the `train` fixture is the train position adapter service the live [typescript example](../examples/typescript/emery.toml) reads, and the `train-pair` fixture the train SOAP connector service that feeds it, each trimmed to its sources, tests, and manifest; each reference is the `train` repository trimmed to its adapter crates and `common`.

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

## Reading a card

The scorecard is printed and written to `target/eval/<card>.md`, every run's files under `target/eval/<card>/<case>/`. Compare two by hand.

In a `typescript` or `python` run the model names each source's surfaces, anchors, and stems in one survey turn, from the facts the parser read, under the adapter's `survey.md` ([emery-typescript](../adapters/typescript/prose/survey.md), [emery-python](../adapters/python/prose/survey.md)); code derives the stems it spells, the ids, the closures, and the seams from the anchors it accepts. The adapter's `surveyed` line reports those surfaces, and the `surfaces` column grades them.

Each figure points at one place in the adapter:

- **`recall req` / `crit` / `dec`**: an expected item is found when an accepted claim of its source overlaps its anchor. A miss is the extraction's: `extract.md` for a requirement the model did not state, the SDK's `skeleton` anchor and decision lists for one stated at a line the gate refused.
- **`in stem`**: a found requirement whose claim id leads with the expected stem, or any of several where the expectation lists them for shared code. The extraction's too.
- **`stems`**: `exact`, or the stems missed and invented. The mounts' and the bootstrap rule's: a stem invented is a surface the code read a stem into, a stem missed one whose surfaces were restemmed.
- **`surfaces`**: `matched/expected ~restemmed +extra`. Matched is the right entry under the right stem; restemmed the right entry under another (a note says which), telling a survey that is present and wrong from one that is absent; `no survey` is an adapter that logs no `surveyed` line, as the shipped `documentation` component does not. Anything short of `n/n` is the survey's: `survey.md` for a surface not named or named under the wrong convention; the SDK's `survey::seams` check for a finding that sent the answer back; the stem rules (the mounts and `derive` in `survey/surface.rs`, the SDK's `survey::route` under `DIALECT.route`) for a stem derived wrongly from an anchor the model got right.
- **`behaviours`**, on several sources: `met/expected`, then `~k` split and `!k` wrong merges.
- **`waves`**: the staged plan's shape as the `specify` envelope reports it — how many sets of slices are ready to build at once, then each set's width, `3 (1/2/1)`; `0` where no plan was staged. The waves are the slicing turn's `depends-on` lines read as a dependency order: their count is the longest chain, the widest the most a concurrent build could run at once, and a plan of one wave as wide as its slices declares no dependency at all.
- **`mechanism`**: `named/cited` per source — the requirements whose body names a platform's code identifier the design owns no `data` type for (a decorator, a call, a CamelCase type's name, in backticks or bare; a design type whose role is `mechanism` — a client, a retriever, a bootstrap, a settings carrier — counts as the platform's), over the requirements citing the source; the program's own vocabulary (a field, a key, a function of its own, a path, a route, a value) is left out by shape. The extraction's: a requirement stated through the platform's mechanism rather than its behaviour, which no target on another platform can cover. The notes list each with its identifiers, and the list is the figure's reading.
- **`leaks`**: the places the specification's own words name a technology, under the lexicon the engine's spec gate holds its draft to — the SDK's fixed vendor, product, protocol, framework, and host names, and the terms of the packages the run's accepted `call` claims spell — read over each accepted `requirement` and `criterion` claim's id and body, then the staged specification's preamble paragraphs and scenario lines (`name`, `given`, `when`, `then`, `and`). Expected `0`: each is a finding the SDK's extraction gate or the engine's spec-draft gate should have returned for correction, so a leak is the gate's — a name the lexicon lacks, or a seam whose own lexicon (its modules' packages) lacked a term the run's holds. The notes list each as `- leak: <where> names <names>`, and the figure stands apart from `mechanism`, which reads identifiers by shape.
- **`exit`**, on a failed run: the turn it died in. `survey-*` or `evidence-*` is the adapter's and the run is short; `spec-draft`, `design-draft`, `grouping`, or `slicing` is the engine's, after every claim was accepted, and the row's recall reads as any other's.
- **Tokens and wall clock**: from the backend's `completion` lines.

Under `--build`, seven columns follow, each the build's and none the extraction's:

- **`built`**: `merged/slices`, then `~k` for the slices whose merge the model resolved before they merged, and the failure's code where the build did not land (`2/4 ~1 `merge-unresolved``, a conflict the model could not resolve). Merged is the envelope's count on a landed run and the merge commits the integration copy holds on a failed one, so a slice that changed nothing is counted only where the run landed.
- **`verified`**: `passed/waves` — the waves whose verify passed over the waves the build ran, the latter the last wave a slice merged in on a landed run and the waves the turns fell into on a failed one. `1/2` with `verify-failed` is the second wave's tree refused.
- **`build wall`**: the `emery build` process end to end, with its exit where it failed.
- **`speedup`**: the serial walk over the critical path, then the plan's own factor. Serial is every build and verify turn in a row; the critical path takes each wave at its slowest slice — every round of its build — then the wave's verify, the waves told apart by the `verify` completions between them. The plan's factor, `slices ÷ waves`, is what [the plan widths](cards/plan-widths.md) measured with slices of equal cost and the verify free, so the gap between the two is what the width is worth once the slowest slice and the verify are paid for. A build that did not land reads how far it got instead, `1.66× (1 of 6 waves)`, since a partial path compares with nothing.
- **`covered`**: the requirement ids the merged slices' reports covered over the requirements those slices hold. An uncovered id is one the adapter left out of `covered`, never one it invented.
- **`host checks`**: what the host's own run of the layout's checks says of the tree the build left — `ok 123/123` when the install, `tsc`, and every test pass, the tests' passed over run; else each check that failed (`npx tsc ✗ · tests 118/123`) or was left unrun after a failed install. A passed verdict beside a failing cell is the verify answering what the tree does not hold; `—` where the build left no tree.
- **`scenarios named`**: `named/scenarios (p%) · k outside · u unreached · p probes` — the acceptance scenarios of the merged slices' requirements a test is named by, the tracked files outside the layout, the modules the layout's lists do not reach, and the probe routes. A scenario-named test says a test exists, not what it asserts; a file outside the layout is a slice writing where the merge rules do not reach; an unreached module is one the program never reaches — for `omnia`, a handler module its stem's entry names in neither array, a stem the root lists merge nowhere and nothing names, or a topic constant no module names — however its own tests pass; a probe is a route no requirement of its module asked for, which `run-N.fidelity.md` lists under "Routes" beside the rest.

Beneath the rows, after the completions: the build's ending (the label and where it stands; or the failure's code, exit, and first line); each wave's slices as they merged, with covered and files per slice and the wave's build durations against its verify, the verify's shell part and what it sealed; the slices whose merge the model resolved and the paths; the waves the verify repaired; the verify's refusals, quoted; the uncovered ids per slice; the host's checks, whether they agree with the verdicts, and the `run-N.fidelity.md` they are written to; for an `omnia` build of a case naming a `reference`, the oracle's passes over its fixtures and the tree's crates, lines, and dependencies against the reference's ([Reference comparison](#reference-comparison)); the chain the repository holds, newest first, each commit with its subject and the files it brought; and the build's turns with their tokens, the serial sum, and the critical path, each completion with what it wrote through the SDK's tools.

`surfaces`, `stems`, and the survey notes read per source where the case has several. Beneath the rows:

- The misses by anchor, and `unreached`, the modules the adapter placed under no surface.
- For behaviours: each miss, split, partial citation (a met behaviour one of whose claims no requirement cites), met by criterion, across kinds, status mismatch, and wrong merge; then `behaviour statuses` and `cross-source pre-merges`.
- **`requirement anchors`**: where the accepted requirement claims landed against the surveyed surfaces (the bootstrap set aside): covering a surface's head, inside it past the head (a decision, a `return`, a call), or outside every surface. An arm over the anchor set is read by this figure; the misses by anchor cannot show it.
- **`Budget ladder`**: the rung each run landed on. A run that climbs is a seam whose turn did not fit: its size, or the order it inlines its files.
- **`Stability`**: the stem sets across runs, and the `(entry, stem)` pairs, requirement ids, and spec subjects every pair of runs shares. The pairs measure the derivation's determinism: same anchors, same seams. The ids measure the model's naming, not a target the survey holds.

What the survey does fix (stems, surfaces, anchors, the files a seam inlines) is pinned by [tests/source.rs](../tests/source.rs) with the survey's answer scripted, over the `cli-jobs` and `express-orders` fixtures for `typescript` and `click-jobs` and `fastapi-routers` for `python`.

### The record

`target/` is gitignored and `make sweep` drops what sits there untouched for a week, so no card is in the tree. [cards/README.md](cards/README.md) is the protocol an arm follows, the rules adopted, and the index of the rounds with the commit each card is read from (`git show <sha>:evals/cards/<card>.md`); `cards/<programme>.md` is each round's record — its predictions and decision rule written before its arms are built, and each card's figures read against them.
