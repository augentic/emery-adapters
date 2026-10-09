# Graded live eval

`cargo run -p evals -- [case|adapter..]` runs the shipped components end to end against the live model and grades what they extract. For each case it stages a project, runs `emery specify` three times, reads the committed documents back through `emery show --format json`, and grades them against a reviewer's `expected.toml`. With `--build` it also builds each committed plan through the `typescript-target` component into a greenfield repository and reads what landed. It drives the shipped `emery` binary alone.

Operator-invoked, never CI. The component suites in [tests/source.rs](../tests/source.rs) are the regression guard; the eval measures prompt quality, where the model's judgement is the thing under test.

## Running

The prerequisites are the live examples' ([examples/README.md](../examples/README.md)): a sibling [emery](https://github.com/augentic/emery) checkout, `cursor-sdk-bridge`, and `CURSOR_API_KEY`. Build both binaries, then run from the repository root; every path the runner resolves is relative to it.

```bash
cargo build --manifest-path ../emery/Cargo.toml --release
cargo build -p typescript -p python --target wasm32-wasip2 --release
set -a; source .env; set +a

cargo run -p evals -- orders express-orders    # two cases
cargo run -p evals -- python                   # every case with a python source
cargo run -p evals -- --facts fastapi-routers  # survey facts alone, no model turn
cargo run -p evals -- --build express-orders   # specify, then build the plan
```

A positional argument names a case or an adapter; with none, every case runs. A case whose fixture is absent (the vendored ones, until fetched) is skipped; naming it is an error.

| Variable | Default | Role |
| --- | --- | --- |
| `EMERY_BIN` | `../emery/target/release/emery` | The binary under test |
| `TYPESCRIPT_WASM`, `PYTHON_WASM`, `DOCUMENTATION_WASM`, `INTENT_WASM` | `target/wasm32-wasip2/release/<adapter>.wasm` | One per adapter a selected case uses |
| `TYPESCRIPT_TARGET_WASM` | `target/wasm32-wasip2/release/typescript_target.wasm` | The target `--build` builds through; cargo spells the hyphenated crate's artifact with an underscore |
| `EVAL_RUNS` | `3` | Runs per case. Three tells a stable id from a lucky one |
| `EVAL_JOBS` | unset | Passed to `emery build` as `--jobs`, the width cap; unset leaves the engine's default |
| `EVAL_LADDER` | `600/120,1200/240,2400/480` | Budget rungs, each `CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS` |
| `CURSOR_MODEL` | `auto` | Recorded in the card. Pin it before comparing two: `auto` differs run to run |
| `RUST_LOG` | set by the runner | Every adapter at `trace`; the card reads the trace lines. A caller's value is kept |

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

`--facts [case|adapter..]` stages the named cases and runs each once with `CURSOR_API_KEY` stripped. The adapter logs the facts it would lay before the model (bootstrap, registrations, decorated definitions, exports, packages) and the first turn fails before any is spent, in seconds. The text is printed and written to `target/eval/facts/<case>/facts.md`; a graded run writes the same beside its log as `run-N.facts.md`.

This is how an expectation is written, since the facts say which modules locate a surface and how each export is read, and how a change to the facts or the keep policy is checked before a live run. A line the model should not see (a hook's decorator, a data-only class) is the adapter's defect, visible here.

### Building the plan

`--build [case|adapter..]` builds each plan a run commits, through the [`typescript-target`](../targets/typescript-target/) component into a greenfield repository, and reads what landed. Staging lays `target.git` beside the case's sources — a bare repository whose `main` is one root commit holding a `.gitignore` (`node_modules/`, `dist/`, `.emery/`) and a README naming the case, sealed through a seed clone the host's `git` pushes and the runner removes — and the project's `emery.toml` gains a `[target]` table naming the component as `eval:typescript-target@<version>`, the repository by its `file://` URL, `branch = "main"`, and `remote = "origin"`. After a specify that landed and its `show`s, `emery build --config emery.toml` runs over the project (`--jobs` from `EVAL_JOBS` when set), so the engine clones `target.git` under `.emery/vcs/repos/`, builds the waves in working copies there, and pushes the label `emery/<revision>` back; the build's log and envelope are kept as `run-N.build.stderr` and `run-N.build.stdout`. A specify that failed builds nothing, and the build cells read `—`.

The build needs what the target's prompts run: `node` and `npm` of Node 22.18 or later (native type stripping), on `PATH` where `emery` runs, and the release `emery` at `EMERY_BIN`. Build the component first:

```bash
cargo build -p typescript-target --target wasm32-wasip2 --release
EVAL_RUNS=1 cargo run -p evals -- --build express-orders nestjs-pair
```

The runner reads the build three ways: the envelope on success (each slice's wave, merge commit, covered and uncovered ids, written files, and the paths an earlier build of it conflicted at; the verified heads; the label and the remote it was pushed to) or the failure envelope the binary prints among its log lines (`slice-conflict`, `verify-failed`, the wave named in the message); the backend's `completion` lines, labelled `build-<slice>` and `verify`, for the turns' durations and tokens; and the host's `git log --first-parent` over the chain the build left — the label in `target.git` when the run landed, the integration copy `.emery/vcs/integration` a failed run leaves for inspection — each merge commit read for its `Slice:`, `Wave:`, `Requirements:`, and `Covered:` trailers and the files it brought in. Where the envelope and the chain disagree, the chain is what the repository holds.

### Experiment arms

An arm that switches a rule off is a local edit to one function, run and never shipped. Build it from a detached worktree, so the tree you commit from never holds the off state and the shipped component stays at hand as the control:

```bash
git worktree add --detach ../emery-adapters-arm HEAD
# edit the one function in ../emery-adapters-arm, then
cargo build --manifest-path ../emery-adapters-arm/Cargo.toml -p python --target wasm32-wasip2 --release \
  --target-dir /tmp/emery-adapters-arm-target
PYTHON_WASM=/tmp/emery-adapters-arm-target/wasm32-wasip2/release/python.wasm cargo run -p evals -- flask-blueprints
git worktree remove --force ../emery-adapters-arm
```

An arm that is new code is a Cargo feature of its adapter, off by default so the shipped component and the root suite stay the control. `documentation` carries `model-survey`: the model names the tree's subjects before extraction and code cuts the seams from them. Build each arm into its own path under `target/eval/arms/<arm>/`, so a later build never overwrites what an earlier card ran over:

```bash
mkdir -p target/eval/arms/{control,model-survey}
cargo build -p documentation --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/documentation.wasm target/eval/arms/control/
cargo build -p documentation --target wasm32-wasip2 --release --features model-survey
cp target/wasm32-wasip2/release/documentation.wasm target/eval/arms/model-survey/
DOCUMENTATION_WASM=target/eval/arms/model-survey/documentation.wasm cargo run -p evals -- orders-doc cross-cut
```

## Cases

Each case is a directory under [cases/](cases/) holding an `expected.toml` and, for each source bound to a tree, a `fixture/` beside it. The header comment of each `expected.toml` tells the case's story; the tables below say what each probes. The live [examples](../examples/) and the component suites read the same trees.

`expected.toml` names the run's sources, then the claims a reviewer would write from each:

- `[[source]]`: the `adapter` (`typescript`, `python`, `documentation`, `intent`); a `fixture` path from the repository root, or an inline `description`; a `name` the specification cites it by (the adapter's name when absent); a `rank`, the integer the staged `emery.toml` carries as the source's authority, `1` the highest, the adapter's kind deciding when absent; the `stems` the source's requirements lead with; and the `[[source.surface]]` pairs (`entry`, `stem`) its survey must decide.
- `[[requirement]]`, `[[criterion]]`, `[[decision]]`: each a `stem` (or a list), an `anchor` (`path#Ln` or `path#Ln-Lm`, under the source's tree), a one-line `gloss`, and a `source` where the case has several.

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
| [stories](cases/stories/expected.toml) | User stories, two directories of two. The directory cut and the subjects agree | `wishlist`, `invoices` |
| [adr](cases/adr/expected.toml) | Five decision records in one `adr/` directory, each its own subject, which one directory lumps into one seam | `entries`, `posting`, `amounts`, `daily-close`, `exports` |
| [narrative](cases/narrative/expected.toml) | Three headless prose pieces at the root: no heading to anchor at, only a first line to name by | `bookings`, `returns`, `membership` |
| [cross-cut](cases/cross-cut/expected.toml) | Twenty documents past `INLINE_BYTES`, cut by audience into `guides/`, `reference/`, `policies/` while the subjects cut across them. Written to break the directory cut | `membership`, `sign-in`, `password-reset`, `catalogue`, `loans`, `reservations`, `returns`, `damage`, `notifications` |
| [nestjs-docs](cases/nestjs-docs/expected.toml) | The vendored NestJS boilerplate's own `docs/`: the real-tree case | `auth`, `files`, `serialization`, `translations`, `database`, `architecture` |

### Several sources

Seven cases run several sources through one `specify` and grade the reconciled specification by its behaviours (below).

| Case | What it probes | Expects |
| --- | --- | --- |
| [orders-triad](cases/orders-triad/expected.toml) | The example document (`docs`), the `orders` module (`code`), and an inline `intent` brief (`brief`, anchoring nothing, contributing authority alone), their stems disagreeing by name | Five behaviours both state, two the document alone |
| [orders-brief](cases/orders-brief/expected.toml) | The document beside a brief that disagrees on one rule. Does a byte-equal pre-merge hide a disagreement? | `divergence`, the brief outranking the document |
| [orders-divergence](cases/orders-divergence/expected.toml) | The document rewritten so two rules differ from the code's in kind | `divergence` on both |
| [orders-conflict](cases/orders-conflict/expected.toml) | A support FAQ as a second `documentation` source (`faq`) contradicting the document: two sources of one kind disagree | `conflict` |
| [orders-ranked](cases/orders-ranked/expected.toml) | `orders-conflict`'s sources with the FAQ at `rank = 4`, beneath the code: the key alone turns the status | `divergence`, the document outranking the FAQ |
| [orders-spelled](cases/orders-spelled/expected.toml) | The document rewritten under the code's own names, so the stems agree by the byte | The one cross-source pre-merge, against `orders-triad` where none is |
| [nestjs-pair](cases/nestjs-pair/expected.toml) | `nestjs-docs` beside the whole `nestjs-boilerplate` tree: the cross-source scale case | Fourteen behaviours both state, three the documents alone |

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

Four real trees are not committed. Each is fetched at a pinned commit into its case's gitignored `fixture/`, and the `expected.toml` names the commit. `nestjs-docs` and `nestjs-pair` read the `nestjs-boilerplate` fixture too.

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

In a `typescript` or `python` run the model names each source's surfaces, anchors, and stems in one survey turn, from the facts the parser read, under the adapter's `survey.md` ([typescript](../sources/typescript/prose/survey.md), [python](../sources/python/prose/survey.md)); code derives the stems it spells, the ids, the closures, and the seams from the anchors it accepts. The adapter's `surveyed` line reports those surfaces, and the `surfaces` column grades them.

Each figure points at one place in the adapter:

- **`recall req` / `crit` / `dec`**: an expected item is found when an accepted claim of its source overlaps its anchor. A miss is the extraction's: `extract.md` for a requirement the model did not state, the SDK's `skeleton` anchor and decision lists for one stated at a line the gate refused.
- **`in stem`**: a found requirement whose claim id leads with the expected stem, or any of several where the expectation lists them for shared code. The extraction's too.
- **`stems`**: `exact`, or the stems missed and invented. The mounts' and the bootstrap rule's: a stem invented is a surface the code read a stem into, a stem missed one whose surfaces were restemmed.
- **`surfaces`**: `matched/expected ~restemmed +extra`. Matched is the right entry under the right stem; restemmed the right entry under another (a note says which), telling a survey that is present and wrong from one that is absent; `no survey` is an adapter that logs no `surveyed` line, as the shipped `documentation` component does not. Anything short of `n/n` is the survey's: `survey.md` for a surface not named or named under the wrong convention; the SDK's `survey::seams` check for a finding that sent the answer back; the stem rules (the mounts and `derive` in `survey/surface.rs`, the SDK's `survey::route` under `DIALECT.route`) for a stem derived wrongly from an anchor the model got right.
- **`behaviours`**, on several sources: `met/expected`, then `~k` split and `!k` wrong merges.
- **`waves`**: the committed plan's shape as the `specify` envelope reports it — how many sets of slices are ready to build at once, then each set's width, `3 (1/2/1)`; `0` where no plan was committed. The waves are the slicing turn's `depends-on` lines read as a dependency order: their count is the longest chain, the widest the most a concurrent build could run at once, and a plan of one wave as wide as its slices declares no dependency at all.
- **`exit`**, on a failed run: the turn it died in. `survey-*` or `evidence-*` is the adapter's and the run is short; `spec-draft`, `design-draft`, `grouping`, or `slicing` is the engine's, after every claim was accepted, and the row's recall reads as any other's.
- **Tokens and wall clock**: from the backend's `completion` lines.

Under `--build`, five columns follow, each the build's and none the extraction's:

- **`built`**: `merged/slices`, then `-k` for the slices that conflicted once and merged on their second build, `!1` where a second conflict ended the run, and the failure's code where the build did not land (`2/4 !1 `slice-conflict``). Merged is the envelope's count on a landed run and the merge commits the integration copy holds on a failed one, so a slice that changed nothing is counted only where the run landed.
- **`verified`**: `passed/waves` — the waves whose verify passed over the waves the build ran, the latter the last wave a slice merged in on a landed run and the waves the turns fell into on a failed one. `1/2` with `verify-failed` is the second wave's tree refused.
- **`build wall`**: the `emery build` process end to end, with its exit where it failed.
- **`speedup`**: the serial walk over the critical path, then the plan's own factor. Serial is every build and verify turn in a row; the critical path takes each wave at its slowest slice — every round of its build — then the wave's verify, the waves told apart by the `verify` completions between them. The plan's factor, `slices ÷ waves`, is what [the plan widths](cards/README.md#the-plan-widths) measured with slices of equal cost and the verify free, so the gap between the two is what the width is worth once the slowest slice and the verify are paid for. A build that did not land reads how far it got instead, `1.66× (1 of 6 waves)`, since a partial path compares with nothing.
- **`covered`**: the requirement ids the merged slices' reports covered over the requirements those slices hold. An uncovered id is one the adapter left out of `covered`, never one it invented.

Beneath the rows, after the completions: the build's ending (the label and where it stands, pushed to which remote; or the failure's code, exit, and first line); each wave's slices as they merged, with covered and files per slice and the wave's build durations against its verify; the conflicted slices and their paths; the verify's refusals, quoted; the uncovered ids per slice; the chain the repository holds, newest first, each commit with its subject and the files it brought; and the build's turns with their tokens, the serial sum, and the critical path.

`surfaces`, `stems`, and the survey notes read per source where the case has several. Beneath the rows:

- The misses by anchor, and `unreached`, the modules the adapter placed under no surface.
- For behaviours: each miss, split, partial citation (a met behaviour one of whose claims no requirement cites), met by criterion, across kinds, status mismatch, and wrong merge; then `behaviour statuses` and `cross-source pre-merges`.
- **`requirement anchors`**: where the accepted requirement claims landed against the surveyed surfaces (the bootstrap set aside): covering a surface's head, inside it past the head (a decision, a `return`, a call), or outside every surface. An arm over the anchor set is read by this figure; the misses by anchor cannot show it.
- **`Budget ladder`**: the rung each run landed on. A run that climbs is a seam whose turn did not fit: its size, or the order it lays its files.
- **`Stability`**: the stem sets across runs, and the `(entry, stem)` pairs, requirement ids, and spec subjects every pair of runs shares. The pairs measure the derivation's determinism: same anchors, same seams. The ids measure the model's naming, not a target the survey holds.

What the survey does fix (stems, surfaces, anchors, the files a seam lays) is pinned by [tests/source.rs](../tests/source.rs) with the survey's answer scripted, over the `cli-jobs` and `express-orders` fixtures for `typescript` and `click-jobs` and `fastapi-routers` for `python`.

### The record

`target/` is gitignored and `make sweep` drops what sits there untouched for a week, so no card is in the tree. [cards/README.md](cards/README.md) is the record: the protocol an arm follows, each experiment's predictions and decision rule written before its arms are built, each card's figures read against them, and the commit each card is read from (`git show <sha>:evals/cards/<card>.md`).
