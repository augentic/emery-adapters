# Emery Adapters — Agent Instructions

Emery's first-party adapters. Each `adapters/<kind>` is one crate, `emery-<kind>`, shipping as one WebAssembly component published as `emery:<kind>@<semver>`; its tracing target and artifact are `emery_<kind>`.

- **Source adapters** (`documentation`, `intent`, `python`, `typescript`) export the `source-adapter` world: `metadata`, and `extract(SourceInput) -> Evidence`.
- **Target adapters** (`node`, `omnia`) export the `target-adapter` world: `metadata`, and `run` over the tasks `init`, `build(slice)`, and `verify`.

The contract, vocabulary, and code style are the engine's: read its [`AGENTS.md`](https://github.com/augentic/emery/blob/main/AGENTS.md) (`../emery/AGENTS.md` in a sibling checkout) before writing Rust here. This repository owns extraction and build behaviour and the prose. Toolchain, layout, prompt authoring, the engine pin, and publishing are in [CONTRIBUTING.md](CONTRIBUTING.md). The v1 tree is archived at tag `v1`.

## Git

Never `git commit`, `git push`, open or close a pull request, or delete a branch — in this repository or in any sibling checkout — unless the maintainer lifts this for the session, explicitly and for named work. Leave every change uncommitted in the working tree; the maintainer reviews and commits. No plan or to-do list carries a commit, push, or PR step, and an instruction to complete every step does not override this.

## Map

| Path | Role |
| --- | --- |
| `adapters/<kind>/` (source) | `src/lib.rs`: `mod survey;`, `pub struct Adapter;` with `impl SourceAdapter`, `pub static PROSE: &[Doc] = emery_sdk::prose![..]` listing every document under `prose/`, and last, for `wasm32` alone, `emery_sdk::export_source!(Adapter);`. `src/survey.rs` and its children divide the source into seams. Prompts: `prose/extract.md`, plus `prose/survey.md` where the model names the surfaces |
| `adapters/<kind>/` (target) | `src/lib.rs`: the merge `RULES`, one `Grant` per program the prompts run, `pub struct Adapter;` with `impl TargetAdapter`, `PROSE`, and last `emery_sdk::export_target!(Adapter);`. Prompts: `prose/build.md` and `prose/verify.md` |
| `adapters/node/` | Strict TypeScript on Node in the shape `prose/references/layout.md` spells, seeded from `seed/` at init |
| `adapters/omnia/` | An omnia guest in Rust in the shape `prose/references/layout.md` spells. Init scaffolds the omnia-exemplar template (the `omnia-template` git dependency). `src/scaffold.rs` lays each slice's typed skeleton before its turn. `src/tree.rs` holds the tree to the layout at build and verify, as findings the turn repairs. `src/layout.rs` is the layout as data both read, and `src/syntax.rs` is the one reader of Rust (`syn`), so a check reads the syntax tree, never a line's text |
| `crates/test-programs/` | omnia's test-programs pattern: the boundary drivers (`programs/source/`, `programs/builder/`), the fixture adapters (`programs/probe/`), and the generated table of every built component |
| `tests/` | Root suites: `source.rs` and `target.rs` (each adapter natively through its `Adapter`), `prose.rs` (every corpus), and `component.rs` and `probe.rs` (built components under the omnia runtime, over `support/mod.rs`) |
| `examples/` | The `emery.toml` configs for live walks with the shipped `emery` binary ([examples/README.md](examples/README.md)) |
| `evals/` | The graded live eval, `cargo run -p evals -- [case..]`, operator-invoked and never CI: cases under `evals/cases/<name>/`, scorecards under `target/eval/`, the protocol and rounds under `evals/cards/` ([evals/README.md](evals/README.md)) |

The root `emery-adapters` package is tests only: it names each adapter as a native dev-dependency so `tests/prose.rs` can read its `PROSE`. `emery-sdk`, with `emery-prose` and `emery-lexicon` beneath it, is git-pinned in `[patch.crates-io]`. The commented `path` patches point at `../emery` for sibling co-development: uncomment them locally and never commit them.

## Invariants

- **Adapters are omnia guests.** `src/lib.rs` declares `pub struct Adapter;`, implements the SDK trait on it, and invokes `export_source!` or `export_target!` last, under `cfg(target_arch = "wasm32")` alone. No adapter names a binding or a model backend: every model call goes through the SDK. There is no adapter error type; `emery_sdk::Error` is omnia's.
- **Source adapters.** `metadata()` declares the `kind` the engine ranks the source by. `extract` runs the crate's `survey` for the seams, then `emery_sdk::extract(ctx, PROSE, &seams)`, then joins only what the survey read from the source itself (`typescript` and `python` through `Survey::join`). The survey inspects its source the way the source demands and owns the cut. What code can derive from the source (closures, stems, ids, `type` claims) is code's, never the model's. Refuse only unusable input, with `bad_request!`.
- **Target adapters.** `run` matches the three tasks. `Init` scaffolds the files of the layout the tree lacks with the adapter's own code: `create_new`, a held file left standing, no model. `Build` is `target::build(&ctx, PROSE, check)`, after any scaffold the adapter lays from the typed slice. `Verify` is `target::verify(&ctx, PROSE, check)`. An adapter writes before a turn, never after. One turn builds one slice whole: what a scaffold derives from the typed slice is enforced by the write guard and `check`, while what it lays by heuristic is a default the prompt leaves open. A target declares no kind and no base; where a build lands is the operator's `[target] repository`.
- **Grants.** One `Grant { egress, read, env }` per program the prompts run, spelled from that program's own documentation of its reach; none by default. Nothing in the environment widens them.
- **Claims are platform-neutral.** A `requirement` or `criterion` states behaviour by role and names no technology; the SDK's lexicon gate returns one that does. Technology reaches the design through `call` and `excerpt` from code sources and `decision` from prose sources. `retire` is for a passage that withdraws a behaviour outright, never for silence or an in-place revision. A target reads the technology from the design's `## APIs and integrations`, never from the specification.
- **Prompts.** `extract.md`, `survey.md`, `build.md`, and `verify.md` are each at most 800 non-blank lines, with a `## Worked example` JSON fence that parses as the SDK's `Evidence`, `Inventory`, `Report`, or `Verdict` and passes its gate. Every document under `prose/references/examples/` other than `README.md` has an `## Evidence` fence held the same way. `tests/prose.rs` enforces all of it.
- **References** are linked, never inlined. Every link names a document in `PROSE` or an SDK runtime reference (`claims.md`, `reconciliation.md`), and every `PROSE` document is reached from a prompt. A rule the SDK or its brief states is linked, never restated. References address the model in the current contract; contributor guidance never goes in the embedded corpus.
- **Dependencies.** `emery-sdk`, plus the pure-Rust parser the source demands, pinned exactly and confined to the one parsing module. A target whose layout another repository owns takes that repository's crate as a git dependency. Never a `build.rs`, a C dependency, or a crate that reaches past WASI.
- Adapters never read the revision store or take lifecycle authority. Preserve gaps as `[unknown]`.
- Identity is the crate's version and its `emery:<kind>@<semver>` package; there is no manifest file. Never commit built `.wasm` artifacts.

## Code style

The engine's code style applies in full. In addition:

- An adapter's public API is the crate's `//!`, `Adapter`, and `PROSE`; `survey` is private. The `//!` is one short paragraph after the summary sentence. For a source adapter: what it accepts, how it divides it, and any refusal a caller sees. For a target adapter: the layout it initialises, how a slice lands, and the checks it runs.
- The survey's data model is the SDK's (`emery_sdk::survey::code`). An adapter's `parse.rs` wraps its `Module` (`Deref` to the core), `parse/walk.rs` is the one producer, and the recogniser in `survey/surface.rs` implements `Recogniser`.
- What a lookup reads of the language is data: the adapter's one `static DIALECT: emery_sdk::survey::Dialect` in `survey/dialect.rs`. Anything pure over strings or lists that reads no `Module` is the SDK's, never an adapter's copy.
- The guest deny-list in [`clippy.toml`](clippy.toml) is part of the gate.

## Testing

- Tests drive the public boundary over strict scripted doubles, scripting exactly the exchanges a run consumes. A test fn names the scenario, never the outcome. An adapter has no tests or dev dependencies of its own.
- `tests/source.rs` calls each source's `Adapter::extract` over a `Context` with the SDK's `Scripted` model, and asserts the turns opened (`Seen`, `Exchange`) and the evidence answered. A survey turn is answered with a scripted `Inventory`; assert what code derived from it, never the answer's own words back.
- `tests/target.rs` calls each target's `Adapter::run` over `Init`, `Build`, and `Verify` on a scratch tree, scripting the tool calls. It asserts the rules, the grant union, what init scaffolds and leaves standing, the build's brief and report, and the verdict.
- `tests/component.rs` runs every built component once under the omnia runtime, for what a native call cannot show: the export, the lift and lower, the mount. `tests/probe.rs` covers the boundary through the fixture adapters.
- What the SDK does for every adapter is asserted once, in the SDK's suite, never per adapter. Never pin prompt phrases; prompt quality is the live eval's.
- `foreach_adapter!`, `foreach_target!`, and `foreach_probe!` make a new adapter a compile error until the suites name it. A new adapter is also a root dev-dependency and an entry in `WASM32_PACKAGES` in [`mise.toml`](mise.toml). Its scenarios are `<stem>_<scenario>` (`emery_intent_empty_brief`).
- An experiment arm is never a Cargo feature, since `make test` runs `--all-features`; build it from a worktree.
- Always `cargo nextest`, `--workspace` from the root.

## Commands

All from the repository root through `make` ([`Makefile`](Makefile) → mise, tasks from [`augentic/toolkit`](https://github.com/augentic/toolkit) pinned in [`mise.toml`](mise.toml)).

```bash
make ci # exactly the CI jobs — run before handing over
make check # local advisories
make test # cargo nextest run --locked --workspace --all-features
make lint # clippy natively, then the wasm32-wasip2 guest pass over WASM32_PACKAGES
make fmt # cargo +nightly fmt --all
cargo nextest run -p emery-adapters --test source --test target # what the adapters decide, natively
cargo nextest run -p emery-adapters --test component # the built components, under the runtime
cargo build -p emery-<kind> --target wasm32-wasip2 --release # one component, emery_<kind>.wasm
cargo run -p evals -- [case..] # the graded live eval; operator-invoked
make publish <name> # push one built component to its GHCR tag
```

If `make ci` cannot run, say exactly why and which checks ran instead.
