# Graded live eval

`cargo run -p evals -- [case|adapter..]` runs the shipped components end to end against the live model and grades what they extract. For each case it stages a project, runs `emery specify` three times, reads the committed documents back through `emery show --format json`, and grades them against a reviewer's `expected.toml`. With `--build` it also builds each committed plan through a target adapter's component — `node`, or the one `--target` names — into a greenfield repository and reads what landed. It drives the shipped `emery` binary alone.

Operator-invoked, never CI. The component suites in [tests/source.rs](../tests/source.rs) are the regression guard; the eval measures prompt quality, where the model's judgement is the thing under test.

## Running

The prerequisites are the live examples' ([examples/README.md](../examples/README.md)): a sibling [emery](https://github.com/augentic/emery) checkout, `cursor-sdk-bridge`, and `CURSOR_API_KEY`. Build both binaries, then run from the repository root; every path the runner resolves is relative to it.

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
cargo run -p evals -- --oracle 2026-10-15T17-41-37Z train  # the reference oracle over that card's omnia build, no turn spent
```

A positional argument names a case or an adapter; with none, every case runs. A case whose fixture is absent (the vendored ones, until fetched) is skipped; naming it is an error.

| Variable | Default | Role |
| --- | --- | --- |
| `EMERY_BIN` | `../emery/target/release/emery` | The binary under test |
| `EMERY_TYPESCRIPT_WASM`, `EMERY_PYTHON_WASM`, `EMERY_DOCUMENTATION_WASM`, `EMERY_INTENT_WASM` | `target/wasm32-wasip2/release/emery_<kind>.wasm` | One per adapter a selected case uses |
| `EMERY_NODE_WASM`, `EMERY_OMNIA_WASM` | `target/wasm32-wasip2/release/emery_<kind>.wasm` | The target `--build` builds through, the one `--target` names alone |
| `EVAL_RUNS` | `3` | Runs per case. Three tells a stable id from a lucky one |
| `EVAL_JOBS` | unset | Passed to `emery build` as `--jobs`, the width cap; unset leaves the engine's default, which the backend's agent pool (`CURSOR_MAX_AGENTS`, 8) bounds in practice |
| `EVAL_LADDER` | `600/120,1200/240,2400/480` | Budget rungs, each `CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS`, which a specify climbs |
| `EVAL_BUILD_RUNG` | `1200/240` | The one rung every `emery build` runs at, in the same shape; a slice's turn writes a tree and repairs its findings, and the engine resumes a build but climbs nothing, so it starts a rung up |
| `CURSOR_MODEL` | `auto` | Recorded in the card. Pin it before comparing two: `auto` differs run to run |
| `CURSOR_MAX_TOOL_CALLS` | unset | The backend's tool calls a turn may make (128 unless set), the budget a build turn's `write_files` calls spend; the runner inherits it. Named in the card's header when set |
| `RUST_LOG` | set by the runner | The SDK and every adapter the case runs at `trace`, the backend's tool-call arguments beside them (`omnia_cursor::tools=trace`); the card reads the trace lines. A caller's value is kept |

The defaults suit a sibling release build. Set the paths when `CARGO_TARGET_DIR` puts the artifacts elsewhere, or to point at an experiment's build.

### What a run does

For each case the runner stages a project under `target/eval/<card>/<case>/`, runs `emery specify` over it `EVAL_RUNS` times, and reads each revision back through `emery show --format json`. `<card>` is the run's UTC start and the scorecard's name, so invocations never overwrite each other and a card can be read back to its `run-N.evidence-<source>-*.json` and `run-N.facts.md` later.

Staging copies each adapter's component into the binary's store, `~/.emery/adapters`, as the package `eval:<adapter>@<version>` (file `eval_<adapter>@<version>.wasm`, replaced every time), so a card never stands over a published release or a developer's `-dev` copy; `rm ~/.emery/adapters/eval_*` cleans up. Each fixture is copied in under its source's `name`, minus a Python tree's `__pycache__`, virtual environments, tool caches, and `*.egg-info`, and an `emery.toml` names each source by `name`, `eval:` reference, and `path` or `description`.

A failed run is handled by what the failure says about the arm:

- **The backend's cap ended it** (`timeout` or `inactive`): put again one rung up `EVAL_LADDER`. The card's `Budget ladder` says where each run landed; the comparison columns stay the first rung's.
- **The backend refused it before any claim was accepted** (`bad_gateway` at a turn's opening): the host's failure. Put again once at the same rung, the dead log kept as `run-N.dead.stderr`; a second refusal stands.
- **It failed after its claims were accepted** (the engine's `spec-draft`, `design-draft`, or `slicing`): graded on those claims, since the extraction being measured had finished. The card names the turn and the seams accepted.

The smallest useful pass is one small case per stem shape: `orders`, `cli-jobs`, `hono-mounted` for `typescript`; `fastapi-orders`, `click-jobs`, `flask-blueprints` for `python`. A change to an adapter's `survey.md` is judged by all eight `typescript` cases with the vendored fixtures fetched, or the six authored `python` cases.

### Survey facts without a model turn

`--facts [case|adapter..]` stages the named cases and runs each once with `CURSOR_API_KEY` stripped. The adapter logs the facts it would lay before the model (bootstrap, registrations, decorated definitions, exports, packages; for `documentation`, each document's outline) and the first turn fails before any is spent, in seconds. The text is printed and written to `target/eval/facts/<case>/facts.md`; a graded run writes the same beside its log as `run-N.facts.md`.

This is how an expectation is written, since the facts say which modules locate a surface and how each export is read, and how a change to the facts or the keep policy is checked before a live run. A line the model should not see (a hook's decorator, a data-only class) is the adapter's defect, visible here.

### Building the plan

`--build [--target <name>] [case|adapter..]` builds each plan a run commits, through a target adapter's component into a repository the engine lays, and reads what landed. `--target` names the wasm package (`node` unless told otherwise, `omnia` the other); the crates are [`emery-node`](../adapters/node/) and [`emery-omnia`](../adapters/omnia/). Staging gives the project's `emery.toml` a `[target]` table naming the component as `eval:<target>@<version>` under `name = "target"` and no repository: both targets declare the base they build over as `fresh`, so the engine lays the repository itself at `.emery/vcs/repos/target` beneath the project, its one root commit, `Initialise`, holding what the adapter's init laid — `node`'s three root manifests, `omnia`'s guest template — the engine's own `.gitignore` of `.emery/` beside them only where the init laid none. After a specify that landed and its `show`s, `emery build --config emery.toml` runs over the project (`--jobs` from `EVAL_JOBS` when set), so the engine builds the waves in working copies under `.emery/vcs/` and labels the repository's head `emery/<revision>`; nothing is pushed. The build's log and envelope are kept as `run-N.build.stderr` and `run-N.build.stdout`. A specify that failed builds nothing, and the build cells read `—`.

The build needs what the target's prompts run, on `PATH` where `emery` runs, and the release `emery` at `EMERY_BIN`. For `node`: `node` and `npm` of Node 22.18 or later (native type stripping; the layout's `@types/node` is `^24`). For `omnia`: a stable Rust toolchain with `clippy` and the `wasm32-wasip2` target installed (`rustup target add wasm32-wasip2`), since the worker cannot install one. Every build and verify turn runs inside the worker boundary the `emery` binary compiles in ([Deployment profiles](https://github.com/augentic/emery/blob/main/docs/reference/deployment-profiles.md#what-a-builds-worker-may-reach)): the worker reads the system, the working copy, a private `HOME`, and the `~/` roots the target's worker grant names, carries nothing of the caller's environment but `PATH` and the grant's variables, and reaches the network through the backend alone — its relay for Cursor's API, its proxy for the grant's hosts. The grant is the adapter's constant (`TargetAdapter::WORKER`), put on every build and verify turn, so nothing is exported for a toolchain installed where the grant looks; a root the home lacks is logged and granted nothing. Build the component first:

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

Both targets build into a fresh tree, so the repository the engine lays is the base they accept; a build pointed at a repository whose root holds a directory is refused as `base-not-fresh` before any slice builds. A build turn runs the toolchain as well as composing, and repairs the findings the adapter's gates answer its report with, so every build runs at `EVAL_BUILD_RUNG` rather than the specify's rung: `CURSOR_TIMEOUT_SECS` is process-wide and caps every turn alike, and the engine resumes a failed build from its label but climbs no ladder. A slice that outlasts the build rung is sized wrongly before it is budgeted wrongly — the engine floors a stem past twelve requirements at its sub-stems, and a plan whose slices still run long is the slicing prompt's to cut finer.

The runner reads the build three ways: the envelope on success (each slice's wave, merge commit, covered and uncovered ids, the paths its commit changed, and the paths its merge conflicted at and the model resolved; the verified heads and the waves the verify repaired; the label and where it stands) or the failure envelope the binary prints among its log lines (`slice-conflict`, `verify-failed`, the wave named in the message); the backend's `completion` lines, labelled `build-<slice>` and `verify`, for the turns' durations and tokens; and the host's `git log --first-parent` over the chain the build left — the label in `.emery/vcs/repos/target` when the run landed, the integration copy `.emery/vcs/integration` a failed run leaves for inspection — each merge commit read for its `Slice:`, `Wave:`, `Requirements:`, and `Covered:` trailers and the files it brought in. Where the envelope and the chain disagree, the chain is what the repository holds.

After each build the runner reads the tree itself, since `covered` and a verdict are the model's claims: the tip the chain stands at is exported to `run-N.fidelity/` (removed once read), and the host runs the target's checks in it. For `node`: `npm install`, `npx tsc --noEmit`, and `node --test --test-force-exit --test-timeout=5000 --test-reporter=tap 'test/**/*.test.ts'`, a check left unrun once the install fails. For `omnia`: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo build --target wasm32-wasip2`, each left unrun once an earlier one fails, the tests counted from the `test result:` line of every test binary, and an empty `clippy.toml` laid at the export's root for the checks where the tree holds none (a tree built before the layout laid its own), since clippy walks up from the crate to the first one it finds and the export sits beneath this repository's, which the worker under the fence never reads. It then matches the acceptance scenarios of the merged slices' requirements against the test names the tree's test files declare — the `test(` and `it(` strings, or the `#[test]` and `#[tokio::test]` fns under `tests/` and `crates/*/tests/` with their underscores read as spaces — exactly, or one name containing the other; lists the tracked files outside the target's layout (for `node` the root manifests, `src/index.ts`, `src/main.ts`, a stem's directory, a stem's test file or directory; for `omnia` the adapter's seed — the root package's files, `crates/common/`, and the operator's tooling — `Cargo.lock`, and `crates/<stem>/`); lists the modules the layout's lists do not reach, which a module's own tests cannot show since each drives its module alone (for `node` a stem's `index.ts` the entry does not re-export, or a part's its stem's list does not; for `omnia` a handler module `crates/<stem>/src/<part>.rs` no statement of `src/routes.rs` or `src/messaging.rs` names as `<stem>::<part>::`, a stem crate `crates/<stem>/` no Rust file outside it names as `<stem>::` — a crate the root manifest lists and nothing reaches into, which only the whole tree can show once every slice has merged — and a constant of `crates/common/src/routes/http.rs` or `routes/topic.rs` its list does not register, listed as `<table>#<NAME>`); lists the routes the tree registers per module (for `omnia`, each `.route(<path>, <method>(<handler>))` statement of `src/routes.rs`, attributed to the handler's module `crates/<stem>/src/<part>.rs`, one entry per method its second argument spells through a route fn or a `MethodFilter`, the path read from the constant of `crates/common/src/routes/http.rs` it names or a literal; the TypeScript layout registers routes on an app no one file spells, so none), marking as a **probe** a route whose module's slice has no requirement stating a method and a path, `GET /orders/{id}`, in its body or its scenarios, since such a route is the layout's shape and not the specification's; and what the checks wrote besides the lockfile (`package-lock.json`; `Cargo.lock`). The pass is written to `run-N.fidelity.md` beside the log and read into the last two build columns; a host without the toolchain leaves them empty and the card whole. Each wave's line in the run's notes also says what its verify sealed as `Wave <k> verified` — read from the envelope's `sealed` where the engine reports it, else from the chain — `(lock only)` for the lock rewrite alone, else `repaired` with the files, since an engine before `sealed` counted the lock rewrite under `repaired`; and the verify's shell part, `(shell 72s over 7 calls, 37 KB back)`: the wall time of its shell calls summed from each call to its completion, so two calls running at once count twice, and the bytes their results carried back, read from the backend's `tool call` lines under the completion's span, which is how a verify is read while the backend logs its subjects cut at eighty characters behind a `cd … &&` prefix. Each build and verify completion's line carries what its turn wrote through the SDK's tools, read from the SDK's `responded` lines: `write_files 26 calls, 6 rewrites, 1 empty, 2 shrinking; edit_files 3 calls; 1 refused` — the calls answered, the files written that the turn had written before, the files written empty, the files rewritten at under half their last size (per file where the SDK logs each file's bytes, from a one-file call's total before it did), the `edit_files` calls answered, and the calls of either tool refused.

### Reference comparison

A case may name a `reference`: a hand-written omnia guest of the behaviour its fixture states, vendored beside the fixture with its own recorded fixtures. For an `omnia` build of such a case the runner reads the export twice more before removing it, and writes both readings under `## Reference comparison` in `run-N.fidelity.md`:

- **Measures**: the built tree beside the reference on six counts — the crates under `crates/`, the Rust files and their non-blank lines, the external crates every manifest's `[dependencies]` and `[workspace.dependencies]` name (less the tree's own packages), the keys of `crates/common/src/config.rs`, and the topic suffixes of `crates/common/src/routes/topic.rs` (or the `topic` module of `routes.rs`) — each a count on both sides and, for a measure of names, the names one side alone spells. A guest twice the reference's size, or one depending on a crate the reference did without, is read there before any line of it.
- **Oracle**: every fixture under the reference's `crates/*/data/{static,replay}/` driven through the built guest as a black box. The runner lays `tests/reference_oracle.rs` into the export, a native test over `omnia_test::guest` doubles that calls the guest's `messaging_router` and hands it each fixture's XML as a delivery; the fixtures' recorded HTTP answers are scripted at their URLs (and, for a trip allocation, at the URL and in the `{ all: [{ vehicleLabel }] }` shape the TypeScript source reads), the configuration is seeded under the reference's keys, the source's, and every key of the built tree's catalog a pattern reads a value into (an endpoint, an identity, the station filter, the delay window), and a fixture recorded with a `delay` has its creation date and first change's actual time re-anchored to now in `Pacific/Auckland`, as the reference's own tests re-anchor them. The topic the guest consumes under is probed from its topic table and the source's default, `dev-realtime-train.v1`. A fixture passes when the guest publishes the recorded events — compared by `eventType`, `remoteData.externalId`, and `locationData`'s coordinates, speed, and accuracy, camelCase or snake_case, with the Kafka `key` header the external id — or, for a recorded refusal, refuses or publishes nothing; every difference is a reason in the fixture's row, a panic (an upstream request the fixture does not answer) among them, and, for a fixture that fails, the last `warn` or `error` event the guest logged as it ran, `logged: WARN message::validation: invalid inbound message: xml parse: duplicate field …`, since a handler that logs a failure and answers `Ok(())` publishes nothing and says why there alone. A GET exchange two or more fixtures record at one path and query is a table — the stops, the routes — which a guest reading it cache-aside fetches on its first miss whatever fixture it is driven by first; such an exchange answers any fixture whose own recording lacks the path, the fixture's own answers first, and the paths answered that way are listed beneath the oracle's summary. The harness needs `chrono`, `chrono-tz`, `http`, `bytes`, `serde_json`, and `tracing` beside the layout's `omnia-test` and `tokio`; what the root package depends on under no table is appended as a native dev-dependency for the run, the workspace's pin where it carries one. The oracle is `not run` with the cause where the tree declares no root package, lacks the layout's dev-dependencies, or consumes none of the candidate topics.

The build findings carry one line, `reference oracle 18/22; crates 3 vs 2, Rust lines 1420 vs 1580, dependencies 27 vs 25`.

`--oracle <card> [case|adapter..]` runs the comparison alone over the tree `run-1` of each named case left under an earlier card — the label its build set, or the integration copy a failed build left — with no binary, component, or model: the tip is exported once to `run-1.oracle/` beside the run's files and kept, so the second invocation reuses its build directory and answers in seconds, and an operator may patch the export by hand to see what a claim's gap would cost before a build is spent on it; the comparison is written to `run-1.oracle.md` and printed, the harness's own output to `run-1.oracle.log`, and the export's build directory, the harness, and its fixtures are left out of the measures. Removing `run-1.oracle/` exports the tip again. Two cases carry a reference today: [cases/train/reference](cases/train/reference/README.md) is the `train` repository trimmed to its `position-adapter` crate and the `common` it reads, with every fixture's `output` made explicit, and [cases/train-pair/reference](cases/train-pair/reference/README.md) is the same repository trimmed to the `soap-connector` and `position-adapter` crates, the connector's `POST /inbound/xml` route wired to the topic the adapter consumes. `cargo test --manifest-path evals/cases/train/reference/Cargo.toml --workspace` runs a reference's own tests; nothing in the eval does.

### Building a plan again

`--rebuild <card> [--target <name>] [case|adapter..]` builds the plan `run-1` of each named case committed under an earlier card again, under this invocation's `EMERY_BIN`, target, `CURSOR_MODEL`, and `EVAL_JOBS`, so one revision is built under several arms — a plan is target-neutral, so the arms may build it through different targets — and the plan's own variance is out of the comparison. The staged project is copied whole under the new card, less the engine's repositories and working copies and the earlier build's files; the `[target]` table of `emery.toml` is rewritten for this invocation's target, so the engine lays a repository holding no label afresh and builds every slice over its root; and the run's specify is read back from the files the earlier run kept, so the card's specify columns and completions repeat the source card's, which its header says. The card the rebuild stands on must still be under `target/eval/`.

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
| [fastapi-routers](cases/fastapi-routers/expected.toml) | Mirrors `express-orders`. A console script enters `app.main`, which includes three routers; a generated table puts the tree past `INLINE_BYTES`, so one seam per stem; a `rates.json` read by a computed path is laid as data | `start`, `orders`, `customers`, `health` |
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

Grading reads `emery show spec --format json`. A requirement cites a behaviour when its citation shares at least as many lines with the anchor as it spends outside it, so a whole-method citation states none of the branches inside it. Anchor a behaviour at the lines that state it whole.

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
- **`waves`**: the committed plan's shape as the `specify` envelope reports it — how many sets of slices are ready to build at once, then each set's width, `3 (1/2/1)`; `0` where no plan was committed. The waves are the slicing turn's `depends-on` lines read as a dependency order: their count is the longest chain, the widest the most a concurrent build could run at once, and a plan of one wave as wide as its slices declares no dependency at all.
- **`mechanism`**: `named/cited` per source — the requirements whose body names a platform's code identifier the design owns no `data` type for (a decorator, a call, a CamelCase type's name, in backticks or bare; a design type whose role is `mechanism` — a client, a retriever, a bootstrap, a settings carrier — counts as the platform's), over the requirements citing the source; the program's own vocabulary (a field, a key, a function of its own, a path, a route, a value) is left out by shape. The extraction's: a requirement stated through the platform's mechanism rather than its behaviour, which no target on another platform can cover. The notes list each with its identifiers, and the list is the figure's reading.
- **`leaks`**: the places the specification's own words name a technology, under the lexicon the engine's spec gate holds its draft to — the SDK's fixed vendor, product, protocol, framework, and host names, and the terms of the packages the run's accepted `call` claims spell — read over each accepted `requirement` and `criterion` claim's id and body, then the committed specification's preamble paragraphs and scenario lines (`name`, `given`, `when`, `then`, `and`). Expected `0`: each is a finding the SDK's extraction gate or the engine's spec-draft gate should have returned for correction, so a leak is the gate's — a name the lexicon lacks, or a seam whose own lexicon (its modules' packages) lacked a term the run's holds. The notes list each as `- leak: <where> names <names>`, and the figure stands apart from `mechanism`, which reads identifiers by shape.
- **`exit`**, on a failed run: the turn it died in. `survey-*` or `evidence-*` is the adapter's and the run is short; `spec-draft`, `design-draft`, `grouping`, or `slicing` is the engine's, after every claim was accepted, and the row's recall reads as any other's.
- **Tokens and wall clock**: from the backend's `completion` lines.

Under `--build`, seven columns follow, each the build's and none the extraction's:

- **`built`**: `merged/slices`, then `~k` for the slices whose merge the model resolved before they merged, and the failure's code where the build did not land (`2/4 ~1 `merge-unresolved``, a conflict the model could not resolve). Merged is the envelope's count on a landed run and the merge commits the integration copy holds on a failed one, so a slice that changed nothing is counted only where the run landed.
- **`verified`**: `passed/waves` — the waves whose verify passed over the waves the build ran, the latter the last wave a slice merged in on a landed run and the waves the turns fell into on a failed one. `1/2` with `verify-failed` is the second wave's tree refused.
- **`build wall`**: the `emery build` process end to end, with its exit where it failed.
- **`speedup`**: the serial walk over the critical path, then the plan's own factor. Serial is every build and verify turn in a row; the critical path takes each wave at its slowest slice — every round of its build — then the wave's verify, the waves told apart by the `verify` completions between them. The plan's factor, `slices ÷ waves`, is what [the plan widths](cards/plan-widths.md) measured with slices of equal cost and the verify free, so the gap between the two is what the width is worth once the slowest slice and the verify are paid for. A build that did not land reads how far it got instead, `1.66× (1 of 6 waves)`, since a partial path compares with nothing.
- **`covered`**: the requirement ids the merged slices' reports covered over the requirements those slices hold. An uncovered id is one the adapter left out of `covered`, never one it invented.
- **`host checks`**: what the host's own run of the layout's checks says of the tree the build left — `ok 123/123` when the install, `tsc`, and every test pass, the tests' passed over run; else each check that failed (`npx tsc ✗ · tests 118/123`) or was left unrun after a failed install. A passed verdict beside a failing cell is the verify answering what the tree does not hold; `—` where the build left no tree.
- **`scenarios named`**: `named/scenarios (p%) · k outside · u unreached · p probes` — the acceptance scenarios of the merged slices' requirements a test is named by, the tracked files outside the layout, the modules the layout's lists do not reach, and the probe routes. A scenario-named test says a test exists, not what it asserts; a file outside the layout is a slice writing where the merge rules do not reach; an unreached module is one the program never mounts, however its own tests pass; a probe is a route no requirement of its module asked for, which `run-N.fidelity.md` lists under "Routes" beside the rest.

Beneath the rows, after the completions: the build's ending (the label and where it stands; or the failure's code, exit, and first line); each wave's slices as they merged, with covered and files per slice and the wave's build durations against its verify, the verify's shell part and what it sealed; the slices whose merge the model resolved and the paths; the waves the verify repaired; the verify's refusals, quoted; the uncovered ids per slice; the host's checks, whether they agree with the verdicts, and the `run-N.fidelity.md` they are written to; for an `omnia` build of a case naming a `reference`, the oracle's passes over its fixtures and the tree's crates, lines, and dependencies against the reference's ([Reference comparison](#reference-comparison)); the chain the repository holds, newest first, each commit with its subject and the files it brought; and the build's turns with their tokens, the serial sum, and the critical path, each completion with what it wrote through the SDK's tools.

`surfaces`, `stems`, and the survey notes read per source where the case has several. Beneath the rows:

- The misses by anchor, and `unreached`, the modules the adapter placed under no surface.
- For behaviours: each miss, split, partial citation (a met behaviour one of whose claims no requirement cites), met by criterion, across kinds, status mismatch, and wrong merge; then `behaviour statuses` and `cross-source pre-merges`.
- **`requirement anchors`**: where the accepted requirement claims landed against the surveyed surfaces (the bootstrap set aside): covering a surface's head, inside it past the head (a decision, a `return`, a call), or outside every surface. An arm over the anchor set is read by this figure; the misses by anchor cannot show it.
- **`Budget ladder`**: the rung each run landed on. A run that climbs is a seam whose turn did not fit: its size, or the order it lays its files.
- **`Stability`**: the stem sets across runs, and the `(entry, stem)` pairs, requirement ids, and spec subjects every pair of runs shares. The pairs measure the derivation's determinism: same anchors, same seams. The ids measure the model's naming, not a target the survey holds.

What the survey does fix (stems, surfaces, anchors, the files a seam lays) is pinned by [tests/source.rs](../tests/source.rs) with the survey's answer scripted, over the `cli-jobs` and `express-orders` fixtures for `typescript` and `click-jobs` and `fastapi-routers` for `python`.

### The record

`target/` is gitignored and `make sweep` drops what sits there untouched for a week, so no card is in the tree. [cards/README.md](cards/README.md) is the protocol an arm follows, the rules adopted, and the index of the rounds with the commit each card is read from (`git show <sha>:evals/cards/<card>.md`); `cards/<programme>.md` is each round's record — its predictions and decision rule written before its arms are built, and each card's figures read against them.
