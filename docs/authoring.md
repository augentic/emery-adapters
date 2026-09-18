# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Its survey is one seam: the inline brief, or the one file it reads from a tree.
- [`sources/documentation/`](../sources/documentation/) — the tree shape: a survey that cuts a bound directory into one seam per top-level directory, with the claim-kind table and id-derivation rules in its prompt.
- [`sources/typescript/`](../sources/typescript/) — the model-surveyed shape: a second prompt, `prompts/survey.md`, under which the model finds the surfaces a code tree exposes — each with the module a caller enters it at — before any is mined, one seam per surface.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

An adapter is a guest of that world, the way every omnia guest is: a `wasm32`-only module invokes `emery_sdk::source_adapter!(metadata, extract)` — one macro, in the shape of omnia's `command!(entry)` — over two plain fns it writes: `fn metadata() -> AdapterMetadata`, answered with `emery_sdk::metadata(SourceKind::..)`, and `async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error>`, the adapter's survey then `emery_sdk::extract`. The macro implements the world's `Guest` on a private type over the bindings `emery-sdk` re-exports as `emery_sdk::export`, lifts the WIT input and the host's model onto the `Context` before `extract` is called, and lowers its outcome onto the WIT `evidence` and `error` after, so an adapter never names a binding or a backend.

What the engine calls:

| Operation  | Engine passes                                                                             | You return        | The engine does with it                                                                                                                                                                     |
| ---------- | ----------------------------------------------------------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `metadata` | the adapter id                                                                            | `AdapterMetadata` | resolve-time record: the `emery-version` gate, and the `kind` of source the engine ranks this adapter's evidence by — fixed before any extract                                              |
| `extract`  | the adapter id, the WIT `input` (`key`, workspace-or-value; `SourceInput::from` lifts it) | `Evidence`        | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

Your `extract` is two lines: ask your `survey` for the seams to mine, and hand them to `emery_sdk::extract`, which runs one model call per seam — at most four pending at once, in seam order — and joins the answers into the one `Evidence` document in seam order; every seam is lent the source root, so every `path` anchor is root-relative as answered. You write `survey`; `mine` is the one way a model is asked to extract.

Four ideas carry the operation:

- **The call carries the model.** Every `extract` is given a `Context<'_, P>` — the adapter addressed, the input, and the model every turn is put to — and `mine`, like a survey that asks the model, is generic over that `P: Model`. The guest's lift fills the field with the host's model, `emery_sdk::Provider` (the unit struct whose empty `impl Model` picks up omnia's WASI-backed body, the provider every omnia guest would otherwise declare); under the component suite, that model is the scripted host model the runtime is given. Nothing in the adapter names a backend: the survey is generic over the model the call carries.
- **The survey chooses the cut, and never mines.** `fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error>` — so the guest's `extract` is `let seams = survey::survey(ctx.input)?;` then `emery_sdk::extract(ctx, PROSE, &seams).await` — inspects the source in whatever way its adapter requires and states the seams; it spends at most one model call, and only to decide the cut. A survey that asks the model is `async fn survey<P: Model>(ctx: &Context<'_, P>, docs: &'static [Doc])`, so that guest's `extract` calls `survey::survey(ctx, PROSE).await?`. `emery_sdk::workspace::list(root, keep)` is a physical workspace helper, not a survey shape: it returns regular files sorted and named relative to the root, pruning the engine's own `spec.md`, `design.md`, and `.omnia/`, and asks `keep` about each `workspace::Entry` (`Entry::Dir(path)` or `Entry::File(path)`, read through `entry.name()`, `entry.extension()`, and `entry.hidden()`). `documentation` cuts that listing by top-level directory under its own grain floor; `intent` uses it only to validate and read its one-file carrier. `emery_sdk::survey::surfaces(ctx, docs, keep).await` is an optional model-assisted helper for an adapter whose boundary is semantic: it asks once, under the embedded `prompts/survey.md`, for the surfaces the source exposes — a route, a command, a job, an exported API — each with the module a caller enters it at, lends the root, lists nothing, and holds every answer to the workspace under the same `workspace::Entry` policy. Each surface becomes a `Seam::Note(note)` that lends the whole root and tells the model its surface and entry, which code needs — a surface's behaviour runs from its entry through its imports, and the extract call follows it; the model groups nothing, and there is no floor, no fold, and no remainder. Neither helper defines how an adapter surveys its source. A tree a mechanical cut leaves no finer than itself, and an inline value, are one `Seam::Whole` with no survey turn spent: a single call. A tree the model finds no surface in is refused with `bad_request!`, never mined whole: a source no caller reaches is incomplete. Every seam the survey chooses is one completion the host sees, which is how the component suite reads the survey back (step 7).
- **Prose is listed, and embedded at compile time.** `pub static PROSE: &[Doc] = emery_sdk::prose!["prompts/extract.md", ..];` at the crate root lists every document of the adapter's `prose/` tree, beside its `Cargo.toml`, each by its tree-relative path, as a table of `Doc`s, each body the `include_str!` of its file; the guest hands it to `extract`, and the SDK reads `prompts/extract.md` from it for each seam's turn. A listed document the tree lacks fails the build at the list; a document the list leaves out, a relative link no listed document answers, or a listed document no prompt reaches through those links fails the root `tests/prose.rs`, which holds the list to the tree, and to the prompts the SDK puts to the model, with `emery_sdk::prose::check`. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) is the smallest complete example of the anatomy; being a root-package example with no `prose/` beside its `Cargo.toml`, it alone names its tree, `prose!("examples/adapter/prose", [..])`. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus and then from the SDK's runtime references (`emery_sdk::prose::RUNTIME` — `emery/claims.md`, `emery/reconciliation.md`), so prompts cite references by relative link instead of inlining them, and cite the shared rules as `../emery/claims.md` without listing them.
- **Answers are steered by schema and judged by the gate.** For each seam, `mine` asks one `omnia_sdk::model::Question<Evidence>` — the turn is the SDK's, not the adapter's to shape beyond its seam: the embedded `prompts/extract.md` is the system prompt, the contract's claims-only `Evidence` schema rides the request as a steering hint (the claim-id grammar as its `pattern`; a document-level `kind` is an unknown field the backend corrects), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; the SDK gets the accepted claims, or a `bad_request` carrying the last findings once the rounds are spent. `Seam::Whole` renders the ordinary workspace or inline value (a workspace as "the source tree the prompt walks"); `Seam::Files(files)` renders the lent root and the files to mine beneath it; `Seam::Note(note)` admits a source-specific seam note after the adapter validates or reads its input.
- **Failures are Omnia errors.** Every operation fails with `emery_sdk::Error` (omnia's `omnia_sdk::Error`), built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros — there is no adapter error type. Classify by who acts: a source the adapter cannot accept (an empty brief, a tree that is not the expected shape) is `bad_request!`; an unreadable file or a failed upstream is `server_error!` / `bad_gateway!`. The contract lowers the class onto the WIT `error` variant when the guest's `?` returns it, the engine lifts it back, and it surfaces to the operator as the matching exit code.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-sdk --open` (`cargo doc -p emery-sdk --target wasm32-wasip2` for the `export` module). The contract types are defined in the `emery-adapter` contract crate (its `source` axis module) and re-exported at the SDK's root, and the world's bindings as `emery_sdk::export`, so an adapter names `emery_sdk::{Context, Evidence, SourceContent, …}` and never depends on `emery-adapter` directly; `emery_sdk::export::{Guest, …}` is named only by a guest written by hand instead of through `source_adapter!`. The embedded prose is re-exported the same way — `emery_sdk::Doc`, `emery_sdk::prose!`, the `emery_sdk::prose` lookups and `emery_sdk::prose::check` — so an adapter's `[dependencies]` is `emery-sdk` alone, and it has no `[build-dependencies]`.

## Walkthrough

The steps below scaffold a source called `changelog` (extracts a bound directory of changelog entries). Substitute your own name — it must be unique across the first-party set.

### 1. Scaffold the crate

```text
sources/changelog/
  Cargo.toml
  src/
    lib.rs
    survey.rs
  prose/
    prompts/
      extract.md
```

The root workspace globs `sources/*`, so the directory joins the workspace with no workspace edit — and `crates/test-programs` compiles it as a component on the next build, so the root `tests/source.rs` and `tests/prose.rs` fail to compile until they name the adapter (steps 7 and 8). The minimal `Cargo.toml`:

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
```

`emery-sdk` is the one dependency: the prose is embedded by its `prose!` and named as its `Doc`, so there is no build dependency and no `build.rs`. `cdylib` is the Wasm component; `rlib` is what the root `tests/prose.rs` links to read `PROSE`. There are no dev-dependencies: the runtime the built component is tested under, and the scripted model it is tested over, are the root package's, not the adapter's. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

### 2. List the prose

`src/lib.rs` lists the documents the component embeds, at the crate root so the root `tests/prose.rs` can hold the list to the tree (step 8). `prose!` reads the crate's `prose/` tree, beside its `Cargo.toml`, and names each document by its tree-relative path; every body is the `include_str!` of its file, so an edit rebuilds the crate and a listed document the tree lacks fails the build here. Nothing walks the tree at build time — a document you add to `prose/` is embedded once you list it, and the root suite tells you when you forget, or when you list one no prompt links:

```rust
use emery_sdk::Doc;

/// The prompt embedded in the adapter.
pub static PROSE: &[Doc] = emery_sdk::prose!["prompts/extract.md"];
```

The references every adapter shares are not listed: the SDK embeds them as `emery_sdk::prose::RUNTIME`, and step 6 links them.

### 3. The library skeleton

`src/lib.rs` is the prose list, the survey's declaration, and the component's guest. `PROSE` (step 2) comes first — the one item the crate exports, which the root suite reads natively — then `mod survey;` and the guest module written inline beneath it, both declared for `wasm32` alone: the survey is guest code, and natively the crate builds as `PROSE` and nothing else:

```rust
//! Extracts claims from a tree of changelog entries.
//!
//! Large workspaces are divided by top-level directory. Inline inputs and
//! workspaces that cannot be divided further are mined as a whole.

use emery_sdk::Doc;

/// The prompt embedded in the adapter.
pub static PROSE: &[Doc] = emery_sdk::prose![/* step 2 */];

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    // step 5
}
```

### 4. Write the survey

`src/survey.rs` is the one fn with logic of its own. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
//! Divides a changelog tree into independently mined groups.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

// Entries a directory holds before it is mined on its own.
const FLOOR: usize = 2;

/// Returns the groups of changelog entries to mine.
///
/// Workspace files are grouped by top-level directory. Groups containing
/// fewer than two entries are combined. When at least two groups remain, each
/// becomes a [`Seam::Files`]; otherwise the input becomes one
/// [`Seam::Whole`].
///
/// Inline input is always returned as one whole seam. No model call is
/// required.
///
/// # Errors
///
/// - Returns [`Error::BadRequest`] when an entry name is not UTF-8.
/// - Returns [`Error::ServerError`] when a directory cannot be read.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Ok(vec![Seam::Whole]);
    };
    let files = emery_sdk::workspace::list(root, |entry| entry.extension() == Some("md"))?;
    let mut dir_files: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for file in &files {
        let dir = file.split_once('/').map_or(".", |(dir, _)| dir);
        dir_files.entry(dir).or_default().push(file.clone());
    }
    let small: Vec<&str> = dir_files
        .iter()
        .filter(|(dir, files)| **dir != "." && files.len() < FLOOR)
        .map(|(dir, _)| *dir)
        .collect();
    for dir in small {
        if let Some(files) = dir_files.remove(dir) {
            dir_files.entry(".").or_default().extend(files);
        }
    }
    let groups: Vec<_> = dir_files.into_values().collect();
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }
    Ok(groups.into_iter().map(Seam::Files).collect())
}
```

An adapter whose source is never worth splitting returns `vec![Seam::Whole]` — `intent` does, after refusing an empty brief. A survey that asks the model is `async fn survey<P: Model>(ctx: &Context<'_, P>, docs: &'static [Doc])` — the guest passes it the call's `Context` and the crate's `PROSE` — and `typescript`'s states what a production module is as a `fn keep(entry: Entry<'_>) -> bool`, asks the model with `emery_sdk::survey::surfaces(ctx, docs, keep).await?` — the SDK lends the root and holds each answered entry to the tree under that `keep` — turns each surface it returns into one `Seam::Note` naming the surface and its entry, and refuses a tree the model finds no surface in; under the component suite that turn is the first completion the host sees, under the `prompts/survey.md` the component embeds. The one thing that survey adds to the crate is its prompt, `prose/prompts/survey.md` (step 6).

### 5. Export the world

The `mod guest` in `src/lib.rs` is the component, and identical in every adapter but for the source kind and the survey call — read it once and never again:

```rust
#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};

    use crate::{PROSE, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::extract(ctx, PROSE, &seams).await
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

`emery_sdk::source_adapter!(metadata, extract)` is the export, in the shape of omnia's `command!(entry)`: it implements the world's `Guest` on a private type, invokes the bindings' `export!` for it, and answers the two WIT calls with the two fns it names, in the WIT's order — so nothing in the module is a binding, and a fn of another shape is refused where the macro names it. The `SourceKind` is the kind of source the adapter reads (`Intent`, `Documentation`, or `Behaviour` — the precedence a cross-source disagreement resolves under), reported here so the engine ranks every document this adapter returns before any is asked for — a fact about its input, never answered by the model and never carried in an answer. `emery_sdk::metadata` reports the SDK's own version as the exact `emery-version` pin beside it; an adapter builds the `AdapterMetadata` itself only to loosen or tighten that pin. `extract` is given the call's `Context<'_, P>` — the adapter addressed, the input, and the model every turn is put to; the SDK's lift built it from the WIT input and the host's model, `emery_sdk::Provider` (the unit struct whose empty `impl Model` picks up omnia's `wasm32` body, the provider every omnia guest would otherwise declare), so the adapter is generic over `P: Model` and names no backend — and is the adapter's survey then `emery_sdk::extract` over that `Context` under `PROSE`, its outcome lowered onto the WIT `evidence` and `error` by the SDK; a mechanical survey is `let seams = survey::survey(ctx.input)?;`, one that asks the model is `survey::survey(ctx, PROSE).await?`. `PROSE` is the crate's list of its `prose/` tree (step 2), handed to `mine` and to a survey by model; the guest names it, never declares it, because the root suite reads the same table.

Points that generalize:

- **The SDK owns the user turn.** `Seam::Whole` selects the ordinary source-input rendering over `ctx.input`; `Seam::Files(files)` renders the lent root and lists the files to mine beneath it. Use `Seam::Note(note)` when a source needs validation or preparation first, or the call must be told which part of the lent root is its own — `intent` reads its one-file tree into the seam note; `typescript` names each call's surface and entry, and the root is lent so imports resolve. The envelope always names the adapter and source key, offers the reference tools, and closes with the claims-only request; the answer is claims alone, since the kind of source is already the engine's from `metadata`.
- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. The fixed turn closing says so ("the caller persists…; do not write it yourself") because the model has workspace access.
- **The survey is the only cut.** The source arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline source as `SourceContent::Value` — and `survey` decides how many calls mine it; each call mines its seam completely. For a mechanical cut, choose a grain floor: a model call costs an agent start, so a directory of one file is folded into the root's seam rather than given its own. A survey of one seam is the whole source in one call. There is no lead focus; a survey that asks the model asks it once, through `emery_sdk::survey::surfaces`, and only where the source's boundary lies — what exists is the tree's, and the SDK holds every answer to it; each surface is its own seam, and what lies behind a surface is the extract call's to follow, never the survey's to group.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`, an `example` without `replay-digest`) fails the SDK's `check`, so the backend corrects the candidate in place; an answer still missing one when the backend's rounds are spent fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **The SDK owns the question.** An adapter selects the seams; the system prompt, the user-turn envelope, schema, gate, and correction text are the SDK's and omnia's, and the round budget is the backend's. The prompt should say that a call may be given part of the tree — the files the message lists — and must claim those and no others, with ids led by the domain noun of that part, so two calls over one tree do not name one requirement twice.

### 6. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

An adapter that surveys by model embeds a second: `prose/prompts/survey.md`, the system prompt of its one survey call. It says what a surface is for this source (`typescript`'s: one thing a caller outside the source reaches — a route, a command, a job, an exported API) and where a caller enters it, what is not one (the modules behind a surface — a service, a repository, a logger — which the extract call reaches from the entry), and what the answer is: `surfaces`, each a `name` and its `entry`, a `/`-separated path relative to `$SOURCE_DIR` to a module the tree holds. The same cap applies, and its `## Worked example` JSON fence must parse as `emery_sdk::survey::Inventory` (the root `tests/prose.rs` checks both whenever the document is listed). The SDK writes the turn — the source, the lent root, how an entry is named — and holds every entry to the tree under the adapter's `keep`, so the prompt describes the boundary and what a module is for this source, and lists nothing.

Cite the rules every adapter shares by linking the SDK's runtime references rather than restating them: [`claims.md`](https://github.com/augentic/emery/blob/main/crates/sdk/prose/emery/claims.md) for the `id` grammar, `path` anchors, the skip roots, and the fail-closed gate, and [`reconciliation.md`](https://github.com/augentic/emery/blob/main/crates/sdk/prose/emery/reconciliation.md) for the pipeline. From `prose/prompts/extract.md` they are `../emery/claims.md` and `../emery/reconciliation.md` — `emery_sdk::prose::RUNTIME` embeds them once, in the SDK your crate compiles against, and `read_doc` answers them after your own table — so nothing is copied, symlinked, or listed, and the link is dead on disk by design. The root `tests/prose.rs` accepts those two targets and refuses a link to anything else the list does not embed — a directory, an unlisted file, a path outside the tree — as it refuses a document you forget to list, or a document of your own at `emery/claims.md`, which would shadow the SDK's; each is caught at `make test`, not when the model asks `read_doc` for it.

### 7. Test the component

The adapter has no tests of its own: its one contract is the `source-adapter` world, and what it decides is asserted through the built component in the root `tests/source.rs` ([testing.md § Component suites](testing.md#1-component-suites--cratestest-programs)), which `test_programs::foreach_adapter!()` fails to compile until a `#[tokio::test] async fn changelog()` exists there. Each scenario stages a fixture tree on a `scratch()` project, runs the component under the omnia runtime over a scripted host model, and reads what the host saw:

- The completeness test, `changelog`: stage the minimal tree — one directory, so a mechanical survey stays whole — run `extract(test_programs::ADAPTER_CHANGELOG, &project, 1)` (the constant is generated from the `sources/` directory; the count is the seams the workspace leg opens), and pass the record to `prompted(&model, prompt::CHANGELOG, 1)` — `metadata` opened no completion, each `extract` one under the compiled-in `prompts/extract.md`, which the `prompt` module `include_str!`s.
- The survey, as `changelog_<scenario>` tests: a tree that cuts is `extract(.., seams)` with the seam count — the strict script fails the run if the survey opened any other number — and `partitioned(&turns, &[..])` over the turns `prompted` returns asserts each directory's files share one turn and appear in no other, and an entry the adapter leaves out appears in none (`documentation_directories` is the model). Read a turn as data — a path, a brief, a line the adapter wrote — never as the SDK's phrasing.
- A survey by model: script the inventory as the first answer, one extract answer per surface, one for the inline value, and assert the first request's system is the compiled-in `prompts/survey.md`, each extract turn names its surface and entry, and the inline leg spends no survey turn (`typescript` is the model). An inventory the adapter's `keep` refuses shows as the first `check` exchange's correction, naming each entry, with the accepted inventory alone reaching an extract turn (`typescript_non_production`).
- Refusals: `refused(test_programs::ADAPTER_CHANGELOG, &project, None, ScriptedModel::default())` runs the driver's `refused bad_request` mode over the workspace, `Some(value)` over an inline value; assert `model.seen()` is empty, or holds the one survey turn (`intent_not_one_file`, `intent_empty_brief`, `typescript_no_surface`).

Do not pin prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and what every adapter shares — the declared tools, the schema format, the `check` flag, the lend, the gate's repair loop, the survey check against the tree, the fan-out and the join — the SDK's own suite asserts once, and the `gated` probe once more under the runtime.

Run with `cargo nextest run -p emery-adapters --test source` (never bare `cargo test` — see [testing.md](testing.md)).

### 8. Check the corpus

- `tests/prose.rs` — add the crate as a root dev-dependency (`changelog = { path = "sources/changelog" }` under the root `Cargo.toml`'s native `[dev-dependencies]`) and call `corpus(changelog::PROSE, "changelog", &["prompts/extract.md"])`, which holds the list to the tree and to the prompts named with `emery_sdk::prose::check` and then checks the extraction prompt; an adapter that surveys by model names `"prompts/survey.md"` too and also calls `survey(changelog::PROSE)`, as `typescript` does, so an unlisted survey prompt fails the suite. Nothing more: the prompt your component embeds is proved by `tests/source.rs`.

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

- [ ] `src/lib.rs` carries no logic beyond the `pub static PROSE: &[Doc] = emery_sdk::prose![..];` list and, for `wasm32` alone, `mod survey;` and the `mod guest` — `emery_sdk::source_adapter!(metadata, extract)` and the two fns it names; nothing inside `src/survey.rs` names a target or a backend.
- [ ] The guest's `extract` is its `survey` for the seams, then `emery_sdk::extract(ctx, PROSE, &seams).await` — nothing else: the adapter states its seams through `survey` — mechanically, or with one model call through `emery_sdk::survey::surfaces` — and every model call goes through the SDK.
- [ ] Every document under `prose/` is listed in `PROSE`, every relative link in the prose names a listed document, and every listed document is reached from a prompt by those links (the root `tests/prose.rs`). The extraction prompt stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence parses as the SDK's `Evidence` — claims alone, no document-level `kind` — and passes the claim gate (the root `tests/prose.rs`). It tells the model that a call may be given one seam of the tree — a directory, or a surface and its entry — and must claim that seam alone. A survey by model embeds `prose/prompts/survey.md` under the same cap, its worked example parsing as `survey::Inventory`. Any worked example under `prose/references/examples/` carries its Evidence under a `## Evidence` JSON fence that parses and passes the gate the same way (the root `tests/prose.rs`).
- [ ] Every reference is written for the model in this adapter's contract — what to read in the source, which claims to emit, in the prompt's vocabulary — with nothing addressed to a contributor and nothing the prompt would contradict.
- [ ] Required per-kind extras are demanded by the prompt — the worked example carries them.
- [ ] The root `tests/source.rs` covers what the adapter itself decides, beside its completeness test, as `<name>_<scenario>` tests over the built component: its survey (how a tree cuts, what it leaves out, when it stays whole; for a survey by model, the inventory scripted as the first turn, the entries its `keep` refuses, and the inline value that spends no turn) and, where the adapter prepares its input, its fail-closed refusals. The adapter has no `tests/` and no dev-dependencies.
- [ ] The root `tests/prose.rs` names the adapter, and the root `Cargo.toml` lists it as a native dev-dependency; `cargo nextest run -p emery-adapters` is green.
- [ ] `cargo build -p <name> --target wasm32-wasip2 --release` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
