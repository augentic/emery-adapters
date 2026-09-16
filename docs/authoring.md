# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Its survey is one seam: the inline brief, or the one file it reads from a tree.
- [`sources/documentation/`](../sources/documentation/) — the tree shape: a survey that cuts a bound directory into one seam per top-level directory, with the claim-kind table and id-derivation rules in its prompt.
- [`sources/typescript/`](../sources/typescript/) — the model-surveyed shape: a second prompt, `prompts/survey.md`, under which the model finds the surfaces a code tree exposes — each with the module a caller enters it at — before any is mined, one seam per surface.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

An adapter is a guest of that world, the way every omnia guest is: a `wasm32`-only module invokes `emery_sdk::source_adapter!(metadata, extract)` — one macro, in the shape of omnia's `command!(entry)` — over two plain fns it writes: `fn metadata() -> AdapterMetadata`, answered with `emery_sdk::metadata(SourceKind::..)`, and `async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error>`, the adapter's survey then `emery_sdk::mine`. The macro implements the world's `Guest` on a private type over the bindings `emery-sdk` re-exports as `emery_sdk::export`, lifts the WIT input and the host's model onto the `Context` before `extract` is called, and lowers its outcome onto the WIT `evidence` and `error` after, so an adapter never names a binding or a backend.

What the engine calls:

| Operation  | Engine passes                                                                             | You return        | The engine does with it                                                                                                                                                                     |
| ---------- | ----------------------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `metadata` | the adapter id                                                                            | `AdapterMetadata` | resolve-time record: the `emery-version` gate, and the `kind` of source the engine ranks this adapter's evidence by — fixed before any extract                                              |
| `extract`  | the adapter id, the WIT `input` (`key`, workspace-or-value; `SourceInput::from` lifts it) | `Evidence`        | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

Your `extract` is two lines: ask your `survey` for the seams to mine, and hand them to `emery_sdk::mine`, which runs one model call per seam — at most four pending at once, in seam order — and joins the answers into the one `Evidence` document in seam order; every seam is lent the source root, so every `path` anchor is root-relative as answered. You write `survey`; `mine` is the one way a model is asked to extract.

Four ideas carry the operation:

- **The call carries the model.** Every `extract` is given a `Context<'_, P>` — the adapter addressed, the input, and the model every turn is put to — and `mine`, like a survey that asks the model, is generic over that `P: Model`. The guest's lift fills the field with the host's model, `emery_sdk::Provider` (the unit struct whose empty `impl Model` picks up omnia's WASI-backed body, the provider every omnia guest would otherwise declare); native tests build the same `Context` with `omnia_test::guest::Scripted` there. Nothing in the adapter names a backend, so no survey is `cfg`-gated.
- **The survey chooses the cut, and never mines.** `fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error>` — so the guest's `extract` is `let seams = survey::survey(ctx.input)?;` then `emery_sdk::mine(ctx, DOCS, &seams).await` — lists or reads the input and states the seams; it spends at most one model call, and only to decide the cut. A survey that asks the model is `async fn survey<P: Model>(ctx: &Context<'_, P>, docs: &'static [Doc])`, one fn on every target, so that guest's `extract` calls `survey::survey(ctx, DOCS).await?` and tests hand it a `Context` over a scripted model and their own embed of the corpus. A tree adapter's policy is one `keep` predicate — asked of each `survey::Entry` (`Entry::Dir(path)` or `Entry::File(path)`, by root-relative `/`-separated path, reading itself as `entry.name()`, `entry.extension()`, and `entry.hidden()` so the adapter never unpicks a path); the engine's own `spec.md`, `design.md`, and `.omnia/` are pruned before it is asked — and it surveys one of two ways. `emery_sdk::survey::list(root, keep)` is mechanical: the files beneath the root, sorted and named relative to it, for the adapter to cut as it sees fit — `documentation` groups them by top-level directory under its own grain floor, each group a `Seam::Files(files)` that lends the root and lists the files to mine. `emery_sdk::survey::surfaces(model, ctx, docs, keep).await` asks the model once, under the adapter's embedded `prompts/survey.md`, for the surfaces the source exposes — a route, a command, a job, an exported API — each with the module a caller enters it at, which no directory layout states; the SDK lends the root, lists nothing, and holds the answer to the tree under the same `keep` (a surface entered at no file or at a module `keep` refuses, a nameless one, or a name listed twice goes back as findings). Each surface becomes a `Seam::Note(note)` that lends the whole root and tells the model its surface and entry, which code needs — a surface's behaviour runs from its entry through its imports, and the extract call follows it; the model groups nothing, and there is no floor, no fold, and no remainder. A tree the mechanical cut leaves no finer than itself, and an inline value, are one `Seam::Whole` with no survey turn spent: a single call. A tree the model finds no surface in is refused with `bad_request!`, never mined whole: a source no caller reaches is incomplete. A survey that asks nothing is tested with no model at all.
- **Prose is embedded at compile time.** `static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");` in the guest embeds the adapter's `prose/` tree — named relative to `src/lib.rs`, the way `include_str!` names one file — as a sorted table of `Doc`s; the guest hands it to `mine`, and the SDK reads `prompts/extract.md` from it for each seam's turn. A dangling relative link in any prose document fails the build, at the `include_prose!` line. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) embeds its prose the same way and is the smallest complete example of the shape. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are steered by schema and judged by the gate.** For each seam, `mine` asks one `omnia_sdk::model::Question<Evidence>` — the turn is the SDK's, not the adapter's to shape beyond its seam: the embedded `prompts/extract.md` is the system prompt, the contract's claims-only `Evidence` schema rides the request as a steering hint (the claim-id grammar as its `pattern`; a document-level `kind` is an unknown field the backend corrects), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; the SDK gets the accepted claims, or a `bad_request` carrying the last findings once the rounds are spent. `Seam::Whole` renders the ordinary workspace or inline value (a workspace as "the source tree the prompt walks"); `Seam::Files(files)` renders the lent root and the files to mine beneath it; `Seam::Note(note)` admits a source-specific seam note after the adapter validates or reads its input.
- **Failures are Omnia errors.** Every operation fails with `emery_sdk::Error` (omnia's `omnia_sdk::Error`), built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros — there is no adapter error type. Classify by who acts: a source the adapter cannot accept (an empty brief, a tree that is not the expected shape) is `bad_request!`; an unreadable file or a failed upstream is `server_error!` / `bad_gateway!`. The contract lowers the class onto the WIT `error` variant when the guest's `?` returns it, the engine lifts it back, and it surfaces to the operator as the matching exit code.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-sdk --open` (`cargo doc -p emery-sdk --target wasm32-wasip2` for the `export` module). The contract types are defined in the `emery-adapter` contract crate (its `source` axis module) and re-exported at the SDK's root, and the world's bindings as `emery_sdk::export`, so an adapter names `emery_sdk::{Context, Evidence, SourceContent, …}` and never depends on `emery-adapter` directly; `emery_sdk::export::{Guest, …}` is named only by a guest written by hand instead of through `source_adapter!`. The embedded prose is re-exported the same way — `emery_sdk::Doc`, `emery_sdk::include_prose!`, the `emery_sdk::prose` lookups — so an adapter's `[dependencies]` is `emery-sdk` alone, and it has no `[build-dependencies]`.

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

`src/lib.rs` is the survey's declaration and the component's guest. `pub mod survey;` comes first — the survey is wasm-free and public, since `tests/survey.rs` calls it — and the guest module is written inline beneath it, declared for `wasm32` alone, so the crate builds natively for its tests. The embedded prose is the guest's own, declared inside it (step 5) — nothing outside the guest reads the table; the root suites read the prompts from the `prose/` tree:

```rust
//! Extracts claims from a tree of changelog entries.

pub mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    // step 5
}
```

### 4. Write the survey

`src/survey.rs` is the one fn with logic of its own. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
//! The survey of a changelog tree: one seam per top-level directory.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

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
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Ok(vec![Seam::Whole]);
    };
    let files = emery_sdk::survey::list(root, |entry| entry.extension() == Some("md"))?;
    let mut directories: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut remainder = Vec::new();
    for file in &files {
        match file.split_once('/') {
            Some((directory, _)) => directories.entry(directory).or_default().push(file.clone()),
            None => remainder.push(file.clone()),
        }
    }
    let mut groups = Vec::new();
    for group in directories.into_values() {
        if group.len() >= FLOOR { groups.push(group) } else { remainder.extend(group) }
    }
    if !remainder.is_empty() {
        remainder.sort();
        groups.push(remainder);
    }
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }
    Ok(groups.into_iter().map(Seam::Files).collect())
}
```

An adapter whose source is never worth splitting returns `vec![Seam::Whole]` — `intent` does, after refusing an empty brief. A survey that asks the model is `async fn survey<P: Model>(ctx: &Context<'_, P>, docs: &'static [Doc])`, one fn on every target — the guest passes it the call's `Context` and its `DOCS`; tests a `Context` over a scripted model and their own embed — `typescript`'s states what a production module is as a `fn keep(entry: Entry<'_>) -> bool`, asks the model with `emery_sdk::survey::surfaces(ctx, docs, keep).await?` — the SDK lends the root and holds each answered entry to the tree under that `keep` — turns each surface it returns into one `Seam::Note` naming the surface and its entry, and refuses a tree the model finds no surface in; `tests/survey.rs` embeds the same tree with `emery_sdk::include_prose!("../prose")` of its own. The one thing that survey adds to the crate is its prompt, `prose/prompts/survey.md` (step 6).

### 5. Export the world

The `mod guest` in `src/lib.rs` is the component, and identical in every adapter but for the source kind and the survey call — read it once and never again:

```rust
#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, SourceKind};

    use crate::survey;

    // The extraction prompt and its references, from the tree beside `src/`.
    static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::mine(ctx, DOCS, &seams).await
    }
}
```

The macro hides one fixed expansion, not adapter policy. Written out, its boundary is:

```rust
const _: () = {
    struct Adapter;
    emery_sdk::export::export!(Adapter with_types_in emery_sdk::export);

    impl emery_sdk::export::Guest for Adapter {
        fn metadata(_id: emery_sdk::export::AdapterId) -> emery_sdk::export::AdapterMetadata {
            emery_sdk::guest::metadata(metadata)
        }

        async fn extract(
            id: emery_sdk::export::AdapterId,
            input: emery_sdk::export::Input,
        ) -> Result<emery_sdk::export::Evidence, emery_sdk::export::Error> {
            emery_sdk::guest::extract(extract, id, input).await
        }
    }
};
```

The anonymous constant keeps the generated `Adapter` out of the module's namespace. A guest that needs to control the boundary can implement `emery_sdk::export::Guest` by hand and still use the `emery_sdk::guest` lift and lower helpers.

`emery_sdk::source_adapter!(metadata, extract)` is the export, in the shape of omnia's `command!(entry)`: it implements the world's `Guest` on a private type, invokes the bindings' `export!` for it, and answers the two WIT calls with the two fns it names, in the WIT's order — so nothing in the module is a binding, and a fn of another shape is refused where the macro names it. The `SourceKind` is the kind of source the adapter reads (`Intent`, `Documentation`, or `Behaviour` — the precedence a cross-source disagreement resolves under), reported here so the engine ranks every document this adapter returns before any is asked for — a fact about its input, never answered by the model and never carried in an answer. `emery_sdk::metadata` reports the SDK's own version as the exact `emery-version` pin beside it; an adapter builds the `AdapterMetadata` itself only to loosen or tighten that pin. `extract` is given the call's `Context<'_, P>` — the adapter addressed, the input, and the model every turn is put to; the SDK's lift built it from the WIT input and the host's model, `emery_sdk::Provider` (the unit struct whose empty `impl Model` picks up omnia's `wasm32` body, the provider every omnia guest would otherwise declare), so the adapter is generic over `P: Model` and names no backend — and is the adapter's survey then `emery_sdk::mine` over that `Context` under `DOCS`, its outcome lowered onto the WIT `evidence` and `error` by the SDK; a mechanical survey is `let seams = survey::survey(ctx.input)?;`, one that asks the model is `survey::survey(ctx, DOCS).await?`. `DOCS` is the whole `prose/` tree — `include_prose!` names it relative to this file, so `"../prose"` from `src/lib.rs` — sorted by tree-relative path, every relative link checked at compile time; it lives inside the guest because nothing native reads it.

Points that generalize:

- **The SDK owns the user turn.** `Seam::Whole` selects the ordinary source-input rendering over `ctx.input`; `Seam::Files(files)` renders the lent root and lists the files to mine beneath it. Use `Seam::Note(note)` when a source needs validation or preparation first, or the call must be told which part of the lent root is its own — `intent` reads its one-file tree into the seam note; `typescript` names each call's surface and entry, and the root is lent so imports resolve. The envelope always names the adapter and source key, offers the reference tools, and closes with the claims-only request; the answer is claims alone, since the kind of source is already the engine's from `metadata`.
- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. The fixed turn closing says so ("the caller persists…; do not write it yourself") because the model has workspace access.
- **The survey is the only cut.** The source arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline source as `SourceContent::Value` — and `survey` decides how many calls mine it; each call mines its seam completely. For a mechanical cut, choose a grain floor: a model call costs an agent start, so a directory of one file is folded into the root's seam rather than given its own. A survey of one seam is the whole source in one call. There is no lead focus; a survey that asks the model asks it once, through `emery_sdk::survey::surfaces`, and only where the source's boundary lies — what exists is the tree's, and the SDK holds every answer to it; each surface is its own seam, and what lies behind a surface is the extract call's to follow, never the survey's to group.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`, an `example` without `replay-digest`) fails the SDK's `check`, so the backend corrects the candidate in place; an answer still missing one when the backend's rounds are spent fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **The SDK owns the question.** An adapter selects the seams; the system prompt, the user-turn envelope, schema, gate, and correction text are the SDK's and omnia's, and the round budget is the backend's. The prompt should say that a call may be given part of the tree — the files the message lists — and must claim those and no others, with ids led by the domain noun of that part, so two calls over one tree do not name one requirement twice.

### 6. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

An adapter that surveys by model embeds a second: `prose/prompts/survey.md`, the system prompt of its one survey call. It says what a surface is for this source (`typescript`'s: one thing a caller outside the source reaches — a route, a command, a job, an exported API) and where a caller enters it, what is not one (the modules behind a surface — a service, a repository, a logger — which the extract call reaches from the entry), and what the answer is: `surfaces`, each a `name` and its `entry`, a `/`-separated path relative to `$SOURCE_DIR` to a module the tree holds. The same cap applies, and its `## Worked example` JSON fence must parse as `emery_sdk::survey::Inventory` (the root `tests/prose.rs` checks both whenever the document is embedded). The SDK writes the turn — the source, the lent root, how an entry is named — and holds every entry to the tree under the adapter's `keep`, so the prompt describes the boundary and what a module is for this source, and lists nothing.

Add the shared runtime references symlink so your prompt can cite the cross-adapter corpus ([reconciliation.md](../codex/references/runtime/reconciliation.md) for the pipeline, [claims.md](../codex/references/runtime/claims.md) for the id grammar, `path` anchors, and the fail-closed gate — link it rather than restating those rules in your prompt):

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 7. Test natively

`tests/survey.rs` asserts only what the adapter itself decides — natively: no wasm, no network. A mechanical survey is a plain fn over the `SourceInput`, so its tests are plain `#[test]`s with no model in sight: call `changelog::survey::survey(&input)` over a `tempfile` tree and compare the `Vec<Seam>` — two directories that meet the floor are two seams with the expected files, a tree of one directory is `[Seam::Whole]`, an inline value is `[Seam::Whole]`, and the entries the adapter leaves out are named in none (`documentation`'s suite is the model). A survey that asks the model takes a `Context` built over `omnia_test::guest::Scripted`, in the field the guest's lift fills with the host's model: script one inventory over a real `tempfile` tree and assert what the adapter decided — the root is lent under its prompt, an entry at a test, a declaration file, a dependency, or build output goes back as a finding while a production module is accepted, the notes follow the answer's order and each names its surface and entry, however many surfaces enter at one module — that an empty inventory is a typed refusal, and, over `Scripted::default()`, that its inline value spends no survey turn (`typescript`'s suite is the model). An adapter that validates or prepares its input adds its fail-closed cases (a source it cannot accept is a `bad_request`, never empty success — match on `Error::BadRequest { .. }`) and its `Note` reading as intended — `intent`'s suite is the model. Do not pin prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and what every adapter shares — the declared tools, the schema format, the `check` flag, the lend, the gate's repair loop, the survey check, the fan-out and the join — the SDK's own suite asserts once.

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

- [ ] `src/lib.rs` carries no logic beyond `pub mod survey` and the `wasm32`-only `mod guest` — its `static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");`, `emery_sdk::source_adapter!(metadata, extract)`, and the two fns it names; reusable logic is wasm-free library code, and none of it is `cfg`-gated.
- [ ] The guest's `extract` is its `survey` for the seams, then `emery_sdk::mine(ctx, DOCS, &seams).await` — nothing else: the adapter states its seams through `survey` — mechanically, or with one model call through `emery_sdk::survey::surfaces` — and every model call goes through the SDK.
- [ ] The extraction prompt is embedded under `prose/`, stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence parses as the SDK's `Evidence` — claims alone, no document-level `kind` — and passes the claim gate (the root `tests/prose.rs`). It tells the model that a call may be given one seam of the tree — a directory, or a surface and its entry — and must claim that seam alone. A survey by model embeds `prose/prompts/survey.md` under the same cap, its worked example parsing as `survey::Inventory`.
- [ ] Required per-kind extras are demanded by the prompt — the worked example carries them.
- [ ] `tests/survey.rs` covers what the adapter itself decides: its survey (how a tree cuts, what it leaves out, when it stays whole; for a survey by model, over a `Context` carrying `omnia_test::guest::Scripted` and a real `tempfile` tree, the entries its `keep` refuses, the notes cut from a scripted inventory, and the inline value that spends no turn) and, where the adapter prepares its input, its fail-closed paths; `cargo nextest run -p <name>` is green.
- [ ] The root `tests/source.rs` and `tests/prose.rs` name the adapter; `cargo nextest run -p emery-adapters` is green.
- [ ] `cargo build -p <name> --target wasm32-wasip2 --release` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
