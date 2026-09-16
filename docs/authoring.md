# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Its survey is one seam: the inline brief, or the one file it reads from a tree.
- [`sources/documentation/`](../sources/documentation/) — the tree shape: a survey that cuts a bound directory into one seam per top-level directory, with the claim-kind table and id-derivation rules in its prompt.
- [`sources/typescript/`](../sources/typescript/) — the model-surveyed shape: a second prompt, `prompts/survey.md`, under which the model groups a code tree's modules by the surface they serve before any is mined.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

An adapter is a guest of that world, the way every omnia guest is: it implements the world's `Guest` — two methods, `metadata` and `extract` — on a unit struct in a `wasm32`-only module, over the bindings `emery-sdk` re-exports as `emery_sdk::export`. Everything the two methods need is the SDK's: `export::metadata(SourceKind::..)` answers the first, and `emery_sdk::mine` does the whole of the second once the adapter has chosen its seams.

What the engine calls:

| Operation  | Engine passes                                                                             | You return        | The engine does with it                                                                                                                                                                     |
| ---------- | ----------------------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `metadata` | the adapter id                                                                            | `AdapterMetadata` | resolve-time record: the `emery-version` gate, and the `kind` of source the engine ranks this adapter's evidence by — fixed before any extract                                              |
| `extract`  | the adapter id, the WIT `input` (`key`, workspace-or-value; `SourceInput::from` lifts it) | `Evidence`        | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

Your `extract` is three lines: lift the input, ask your `survey` for the seams to mine, and hand them to `emery_sdk::mine`, which runs one model call per seam — at most four pending at once, in seam order — and joins the answers into the one `Evidence` document, re-rooting each seam's `path` anchors under the source root. You write `survey`; `mine` is the one way a model is asked to extract.

Four ideas carry the operation:

- **The model is a parameter.** `mine`, and a survey that asks the model, are generic over `emery_sdk::Model`. The guest binds `WasiModel`; native tests bind `omnia_test::guest::Scripted` with scripted answers. Your code never constructs a backend.
- **The survey chooses the cut, and never mines.** `survey(content: &SourceContent) -> Result<Vec<Seam>, Error>` — or `async fn survey<P: Model>(model: &P, ctx: &Context<'_>, docs: &'static [Doc]) -> Result<Vec<Seam>, Error>` when it asks the model, `docs` being the guest's embedded corpus — lists or reads the input and states the seams; it spends at most one model call, and only to decide the cut. A tree adapter lists its files with `emery_sdk::survey::files(root, keep)` — the engine's own `spec.md`, `design.md`, and `.omnia/` are pruned for it, everything else is its `keep` predicate's — and cuts them one of two ways. `survey::by_directory(files, floor)` is mechanical: one group per top-level directory holding at least `floor` files, the root's own files and every smaller directory folded into one remainder. `survey::by_model(model, ctx, docs, &files, floor).await` asks the model once, under the adapter's embedded `prompts/survey.md`, to group the files by what they serve — a route, a command, an exported API — which no directory layout states; the SDK lists the candidates in the turn, lends the root, checks the answer (a file never offered, a file in two groups, or an empty group goes back as findings), and folds every group under the floor with every file the model left out into one remainder, so coverage is total however the model grouped. Each group becomes the seam its source kind calls for: `Seam::Files(files)` lends the group's directory alone, so a documentation directory is mined under a grant no wider than itself; `Seam::Note(note)` lends the whole root and tells the model which files are its own, which code needs — a handler's behaviour runs through its imports. A tree that cuts into fewer than two groups, and an inline value, are one `Seam::Whole` with no survey turn spent: a single call. A survey that asks nothing is a plain fn over the `SourceContent`, tested with no model at all.
- **Prose is embedded at compile time.** `static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");` in the guest embeds the adapter's `prose/` tree — named relative to `src/lib.rs`, the way `include_str!` names one file — as a sorted table of `Doc`s; the guest hands it to `mine`, and the SDK reads `prompts/extract.md` from it for each seam's turn. A dangling relative link in any prose document fails the build, at the `include_prose!` line. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) embeds its prose the same way and is the smallest complete example of the shape. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are steered by schema and judged by the gate.** For each seam, `mine` asks one `omnia_sdk::model::Question<Evidence>` — the turn is the SDK's, not the adapter's to shape beyond its seam: the embedded `prompts/extract.md` is the system prompt, the contract's claims-only `Evidence` schema rides the request as a steering hint (the claim-id grammar as its `pattern`; a document-level `kind` is an unknown field the backend corrects), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; the SDK gets the accepted claims, or a `bad_request` carrying the last findings once the rounds are spent. `Seam::Whole` renders the ordinary workspace or inline value (a workspace as "the source tree the prompt walks"); `Seam::Files(files)` renders the lent directory and the files to mine beneath it; `Seam::Note(note)` admits a source-specific seam note after the adapter validates or reads its input.
- **Failures are Omnia errors.** Every operation fails with `emery_sdk::Error` (omnia's `omnia_sdk::Error`), built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros — there is no adapter error type. Classify by who acts: a source the adapter cannot accept (an empty brief, a tree that is not the expected shape) is `bad_request!`; an unreadable file or a failed upstream is `server_error!` / `bad_gateway!`. The contract lowers the class onto the WIT `error` variant when the guest's `?` returns it, the engine lifts it back, and it surfaces to the operator as the matching exit code.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-sdk --open` (`cargo doc -p emery-sdk --target wasm32-wasip2` for the `export` module). The contract types are defined in the `emery-adapter` contract crate (its `source` axis module) and re-exported at the SDK's root, and the world's bindings as `emery_sdk::export`, so an adapter names `emery_sdk::{Context, Evidence, SourceContent, …}` and `emery_sdk::export::{Guest, …}` and never depends on `emery-adapter` directly. The embedded prose is re-exported the same way — `emery_sdk::Doc`, `emery_sdk::include_prose!`, the `emery_sdk::prose` lookups — so an adapter's `[dependencies]` is `emery-sdk` alone, and it has no `[build-dependencies]`.

## Walkthrough

The steps below scaffold a source called `changelog` (extracts a bound directory of changelog entries). Substitute your own name — it must be unique across the first-party set.

### 1. Scaffold the crate

```text
sources/changelog/
  Cargo.toml
  build.rs
  src/
    lib.rs
    survey.rs
  prose/
    prompts/
      extract.md
    references/
      emery-runtime -> ../../../../codex/references/runtime
  tests/
    survey.rs
```

The root workspace globs `sources/*`, so the directory joins the workspace with no manifest edit — and `crates/test-programs` compiles it as a component on the next build, so the root `tests/source.rs` and `tests/prose.rs` fail to compile until they name the adapter (step 8). The minimal `Cargo.toml`:

```toml
[package]
name = "changelog"
description = "Changelog source-adapter"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
rust-version.workspace = true
publish.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[lints]
workspace = true

[dependencies]
emery-sdk.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`emery-sdk` is the one dependency: the prose is embedded by its `include_prose!` and named as its `Doc`, so there is no build dependency. `cdylib` is the Wasm component; `rlib` is what native tests link. `tempfile` stages the trees the survey tests cut. A survey that asks the model adds `tokio` and — natively alone, under `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` — `omnia-test` with its `guest` feature, the scripted model its suite binds, as `typescript`'s manifest does; the runtime the built component runs under is the root package's, not the adapter's. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

### 2. Watch the prose tree

The prose itself is embedded by the guest (step 5). `build.rs` is one line, identical in every adapter, and exists for the one thing `include_prose!` cannot track — a document added to or removed from the tree; an edit to an embedded document rebuilds on its own:

```rust
fn main() {
    println!("cargo::rerun-if-changed=prose");
}
```

### 3. The library skeleton

`src/lib.rs` is the component's guest and the survey. The guest module is written inline at the top and declared for `wasm32` alone, so the crate builds natively for its tests; the survey is wasm-free and public, since `tests/survey.rs` calls it. The embedded prose is the guest's own, declared inside it (step 5) — nothing outside the guest reads the table; the root suites read the prompts from the `prose/` tree:

```rust
//! Extracts claims from a tree of changelog entries.

#[cfg(target_arch = "wasm32")]
mod guest {
    // step 5
}

pub mod survey;
```

### 4. Write the survey

`src/survey.rs` is the one fn with logic of its own. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
//! The survey of a changelog tree: one seam per top-level directory.

use std::path::Path;

use emery_sdk::{Error, Seam, SourceContent};

// Entries a directory holds before it is mined on its own.
const FLOOR: usize = 2;

/// Returns the seams to mine: the tree cut by directory, or the input whole.
///
/// One [`Seam::Files`] per top-level directory of at least two entries and
/// one for the rest of the tree. A tree that cuts no finer than itself, or
/// an inline value, is the bound input whole. The cut is by directory
/// alone, so the model is never asked.
///
/// # Errors
///
/// Returns [`Error::ServerError`] when a directory cannot be read, and
/// [`Error::BadRequest`] for an entry whose name is not UTF-8.
pub fn survey(content: &SourceContent) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = content else {
        return Ok(vec![Seam::Whole]);
    };
    let files = emery_sdk::survey::files(Path::new(root), |path, _| {
        path.extension().is_some_and(|extension| extension == "md")
    })?;
    let groups = emery_sdk::survey::by_directory(files, FLOOR);
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }
    Ok(groups.into_iter().map(Seam::Files).collect())
}
```

An adapter whose source is never worth splitting returns `vec![Seam::Whole]` — `intent` does, after refusing an empty brief. A survey that asks the model is an `async fn survey<P: Model>(model: &P, ctx: &Context<'_>, docs: &'static [Doc])` instead — `typescript`'s lists its production modules as above, keeps a tree no directory cut would split as `Seam::Whole` with no turn spent, and otherwise hands the candidates to `emery_sdk::survey::by_model(model, ctx, docs, &files, FLOOR).await?`, turning each group it returns into one `Seam::Note`; the guest passes its `DOCS`, and `tests/survey.rs` embeds the same tree with `emery_sdk::include_prose!("../prose")` of its own. The one thing that survey adds to the crate is its prompt, `prose/prompts/survey.md` (step 6).

### 5. Export the world

The `mod guest` in `src/lib.rs` is the component, and identical in every adapter but for the source kind and the survey call — read it once and never again:

```rust
#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
    use emery_sdk::model::WasiModel;
    use emery_sdk::{Context, Doc, SourceInput, SourceKind};

    use crate::survey;

    // The extraction prompt and its references, from the tree beside `src/`.
    static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

    struct Adapter;
    export::export!(Adapter with_types_in export);

    impl Guest for Adapter {
        fn metadata(_id: AdapterId) -> AdapterMetadata {
            export::metadata(SourceKind::Documentation)
        }

        async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
            let input = SourceInput::from(input);
            let ctx = Context {
                adapter_id: &id,
                input: &input,
            };
            let seams = survey::survey(&input.content)?;
            Ok(emery_sdk::mine(&WasiModel, &ctx, DOCS, &seams).await?.into())
        }
    }
}
```

The `SourceKind` is the kind of source the adapter reads (`Intent`, `Documentation`, or `Behaviour` — the precedence a cross-source disagreement resolves under), reported here so the engine ranks every document this adapter returns before any is asked for — a fact about its input, never answered by the model and never carried in an answer. `export::metadata` reports the SDK's own version as the exact `emery-version` pin beside it; an adapter builds the `AdapterMetadata` itself only to loosen or tighten that pin. `extract` lifts the WIT input, surveys it, and hands the seams to `mine`; `?` lowers an `emery_sdk::Error` onto the WIT `error` and `.into()` lowers the `Evidence`. A survey that asks the model is awaited over the same `WasiModel` and handed the same table: `survey::survey(&WasiModel, &ctx, DOCS).await?`. `DOCS` is the whole `prose/` tree — `include_prose!` names it relative to this file, so `"../prose"` from `src/lib.rs` — sorted by tree-relative path, every relative link checked at compile time; it lives inside the guest because nothing native reads it.

Points that generalize:

- **The SDK owns the user turn.** `Seam::Whole` selects the ordinary source-input rendering over `ctx.input`; `Seam::Files(files)` renders the lent directory and lists the files beneath it. Use `Seam::Note(note)` when a source needs validation or preparation first, or must be lent whole while the call is told which part is its own — `intent` reads its one-file tree into the seam note; `typescript` lends the root and names each directory's modules, so imports resolve. The envelope always names the adapter and source key, offers the reference tools, and closes with the claims-only request; the answer is claims alone, since the kind of source is already the engine's from `metadata`.
- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. The fixed turn closing says so ("the caller persists…; do not write it yourself") because the model has workspace access.
- **The survey is the only cut.** The source arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline source as `SourceContent::Value` — and `survey` decides how many calls mine it; each call mines its seam completely. Choose a grain floor: a model call costs an agent start, so a directory of one file is folded into the root's seam rather than given its own. A survey of one seam is the whole source in one call. There is no lead focus; a survey that asks the model asks it once, through `survey::by_model`, and only how to group — what exists is the walk's, and what is left out is the remainder's, never omitted.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`, an `example` without `replay-digest`) fails the SDK's `check`, so the backend corrects the candidate in place; an answer still missing one when the backend's rounds are spent fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **The SDK owns the question.** An adapter selects the seams; the system prompt, the user-turn envelope, schema, gate, and correction text are the SDK's and omnia's, and the round budget is the backend's. The prompt should say that a call may be given part of the tree — the files the message lists — and must claim those and no others, with ids led by the domain noun of that part, so two calls over one tree do not name one requirement twice.

### 6. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

An adapter that surveys by model embeds a second: `prose/prompts/survey.md`, the system prompt of its one survey call. It says what one group is for this source (`typescript`'s: the modules that serve one externally visible surface — a route, a command, a job, an exported API), how to place a module several groups reach (once, or in none — the remainder is the SDK's), and what the answer is: `groups`, each a `name` and its `files`, spelled exactly as the turn listed them. The same cap applies, and its `## Worked example` JSON fence must parse as `emery_sdk::survey::Partition` (the root `tests/prose.rs` checks both whenever the document is embedded). The SDK writes the turn — the source, the lent root, the candidate files, the floor — so the prompt describes the grouping and never the listing.

Add the shared runtime references symlink so your prompt can cite the cross-adapter corpus ([reconciliation.md](../codex/references/runtime/reconciliation.md) for the pipeline, [claims.md](../codex/references/runtime/claims.md) for the id grammar, `path` anchors, and the fail-closed gate — link it rather than restating those rules in your prompt):

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 7. Test natively

`tests/survey.rs` asserts only what the adapter itself decides — natively: no wasm, no network. A mechanical survey is a plain fn, so its tests are plain `#[test]`s with no model in sight: call `changelog::survey::survey(&content)` over a `tempfile` tree and compare the `Vec<Seam>` — two directories that meet the floor are two seams with the expected files, a tree of one directory is `[Seam::Whole]`, an inline value is `[Seam::Whole]`, and the entries the adapter leaves out are named in none (`documentation`'s suite is the model). A survey that asks the model takes `omnia_test::guest::Scripted`: script one partition and assert what the adapter put into the turn and made of the answer — the candidates offered are exactly its production files, the root is lent, the notes follow the answer's order with the remainder last, and a group under the floor folds — and, over `Scripted::default()`, that its bound tree and inline value spend no survey turn (`typescript`'s suite is the model). An adapter that validates or prepares its input adds its fail-closed cases (a source it cannot accept is a `bad_request`, never empty success — match on `Error::BadRequest { .. }`) and its `Note` reading as intended — `intent`'s suite is the model. Do not pin prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and what every adapter shares — the declared tools, the schema format, the `check` flag, the lend, the gate's repair loop, the survey check and fold, the fan-out and the join — the SDK's own suite asserts once.

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)).

### 8. Name the adapter in the root suites

Two root tests complete the component rung ([testing.md § Component suites](testing.md#2-component-suites--cratestest-programs)); each `test_programs::foreach_adapter!()` fails to compile until a `#[tokio::test]` / `#[test]` `fn changelog()` exists:

- `tests/source.rs` — stage the minimal fixture tree on a `scratch()` project, run `extract(test_programs::ADAPTER_CHANGELOG, &project)` (the constant is generated from the `sources/` directory), and pass the record to `prompted(&model, include_str!("../sources/changelog/prose/prompts/extract.md"))`. Add only what your component alone shows the host (intent asserts the brief it read through the mount is the turn's seam); the adapter's own behaviour stays in `tests/survey.rs`, and the SDK's side of the boundary is proved over the `gated` probe.
- `tests/prose.rs` — call `corpus(include_str!("../sources/changelog/prose/prompts/extract.md"))`; an adapter that surveys by model also calls `survey(include_str!("../sources/changelog/prose/prompts/survey.md"))`, as `typescript` does, so a missing survey prompt fails the build. Nothing more: reference presence is the embed-time walker's, and the prompt your component embeds is proved by `tests/source.rs`.

## Build the component and use it in a project

```bash
cargo build -p changelog --target wasm32-wasip2 --release   # → target/wasm32-wasip2/release/changelog.wasm
```

Bind it in any Emery project by local path — every `specify` naming it loads the file fresh (nothing is cached), so a rebuild is picked up by the next run:

```bash
emery specify path/to/changelog.wasm
```

Publishing a pinned version to GHCR (`emery:changelog@<version>`) is the operator flow in [CONTRIBUTING.md § Publishing](../CONTRIBUTING.md#publishing); a project then names it by package reference (`emery:changelog@<version>`, or the first-party shorthand `changelog@<version>`) and `emery` fetches it fresh on every run that names it. Those are the two ways in: the shipped `emery` deployment declares no adapter guests, so a bare name dispatches nothing — emery's own journey host ([`examples/runtime.rs`](https://github.com/augentic/emery/blob/main/examples/runtime.rs) in the engine repository) loads its mock adapter by path the same way.

To watch it become a specification before wiring it into a project, give it an example: `examples/changelog/emery.toml` (copy a sibling's config; one `[[source]]` naming the built component, `../../target/wasm32-wasip2/release/changelog.wasm` relative to the file, and the input it reads — a `path` to a fixture tree beside the config, or a `description`), the fixture if it lends one, and a row in [`examples/README.md`](../examples/README.md); then, from the repository root, the build above, `emery specify --config examples/changelog/emery.toml` and `emery show spec`.

## Definition of done

- [ ] `src/lib.rs` carries no logic beyond the `wasm32`-only `mod guest` — its `static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");` and the `Guest` impl — and `pub mod survey`; reusable logic is wasm-free library code.
- [ ] The guest's `extract` lifts the input, calls `survey`, and hands the seams to `emery_sdk::mine` — nothing else: the adapter states its seams through `survey` — mechanically, or with one model call through `survey::by_model` — and every model call goes through the SDK.
- [ ] The extraction prompt is embedded under `prose/`, stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence parses as the SDK's `Evidence` — claims alone, no document-level `kind` — and passes the claim gate (the root `tests/prose.rs`). It tells the model that a call may be given part of the tree and must claim that part alone. A survey by model embeds `prose/prompts/survey.md` under the same cap, its worked example parsing as `survey::Partition`.
- [ ] Required per-kind extras are demanded by the prompt — the worked example carries them.
- [ ] `tests/survey.rs` covers what the adapter itself decides: its survey (how a tree cuts, what it leaves out, when it stays whole; for a survey by model, over `omnia_test::guest::Scripted`, the candidates offered, the notes cut from a scripted partition, and the bound tree that spends no turn) and, where the adapter prepares its input, its fail-closed paths; `cargo nextest run -p <name>` is green.
- [ ] The root `tests/source.rs` and `tests/prose.rs` name the adapter; `cargo nextest run -p emery-adapters` is green.
- [ ] `cargo build -p <name> --target wasm32-wasip2 --release` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
