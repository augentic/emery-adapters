# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Its survey is one seam: the inline brief, or the one file it reads from a tree.
- [`sources/documentation/`](../sources/documentation/) — the tree shape: a survey that cuts a bound directory into one seam per top-level directory, with the claim-kind table and id-derivation rules in its prompt.
- [`sources/typescript/`](../sources/typescript/) — the model-surveyed shape: a second prompt, `prompts/survey.md`, under which the model groups a code tree's modules by the surface they serve before any is mined.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

You never touch WIT directly. The `emery-sdk` SDK hides the bindings behind the `emery_sdk::SourceAdapter` trait, implemented on a unit struct; a one-line macro (`emery_sdk::source!`) wires that implementor into the component exports.

What the engine calls:

| Operation | Engine passes | You return | The engine does with it |
| --------- | ------------- | ---------- | ----------------------- |
| `metadata` | — | `AdapterMetadata` | resolve-time record: the `emery-version` gate, and the `kind` of source the engine ranks this adapter's evidence by — fixed before any extract |
| `extract` | `Context`, typed `SourceInput` (`key`, workspace-or-value) | `Evidence` | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

`extract` is the SDK's: it asks your `survey` for the seams to mine, runs one model call per seam — at most four pending at once, in seam order — and joins the answers into the one `Evidence` document, re-rooting each seam's `path` anchors under the source root. You implement `survey`; you never override `extract`.

Four ideas carry the operation:

- **The model is a parameter.** `extract` is generic over `emery_sdk::Model`. On wasm the macro binds `WasiModel`; native tests bind `omnia_test::guest::Scripted` with scripted answers. Your code never constructs a backend.
- **The survey chooses the cut, and never mines.** `survey<P: Model>(model, ctx) -> impl Future<Output = Result<Vec<Seam>, Error>> + Send` lists or reads the input and states the seams; it spends at most one model call, and only to decide the cut. A tree adapter lists its files with `emery_sdk::survey::files(root, keep)` — the engine's own `spec.md`, `design.md`, and `.omnia/` are pruned for it, everything else is its `keep` predicate's — and cuts them one of two ways. `survey::by_directory(files, floor)` is mechanical: one group per top-level directory holding at least `floor` files, the root's own files and every smaller directory folded into one remainder. `survey::by_model(model, ctx, docs, &files, floor).await` asks the model once, under the adapter's embedded `prompts/survey.md`, to group the files by what they serve — a route, a command, an exported API — which no directory layout states; the SDK lists the candidates in the turn, lends the root, checks the answer (a file never offered, a file in two groups, or an empty group goes back as findings), and folds every group under the floor with every file the model left out into one remainder, so coverage is total however the model grouped. Each group becomes the seam its source kind calls for: `Seam::Files(files)` lends the group's directory alone, so a documentation directory is mined under a grant no wider than itself; `Seam::Note(note)` lends the whole root and tells the model which files are its own, which code needs — a handler's behaviour runs through its imports. A tree that cuts into fewer than two groups, and an inline value, are one `Seam::Whole` with no survey turn spent: today's single call, unchanged. The default `survey` is that one `Whole` seam. A survey that asks nothing is not an `async fn`; it returns `std::future::ready(..)` over the seams it computed.
- **Prose is embedded at build time.** `build.rs` calls `emery_prose::emit("prose")` (the `emit` feature, enabled on the build-dependency only), which walks the adapter's `prose/` tree into a sorted `DOCS` table; `emery_sdk::registry!()` exposes it as `registry::docs()`, and the SDK's `SourceAdapter::prompt` reads `prompts/extract.md` from it. A dangling relative link in any prose document fails the build. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) embeds its prose the same way and is the smallest complete example of the shape. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are steered by schema and judged by the gate.** For each seam, `extract` asks one `omnia_sdk::model::Question<Evidence>` — the turn is the SDK's, not a trait method an adapter calls or overrides: the embedded `prompts/extract.md` is the system prompt, the contract's claims-only `Evidence` schema rides the request as a steering hint (the claim-id grammar as its `pattern`; a document-level `kind` is an unknown field the backend corrects), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; the SDK gets the accepted claims, or a `bad_request` carrying the last findings once the rounds are spent. `Seam::Whole` renders the ordinary workspace or inline value (a workspace as "the source tree the prompt walks"); `Seam::Files(files)` renders the lent directory and the files to mine beneath it; `Seam::Note(note)` admits a source-specific seam note after the adapter validates or reads its input.
- **Failures are Omnia errors.** Every operation fails with `emery_sdk::Error` (omnia's `omnia_sdk::Error`), built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros — there is no adapter error type. Classify by who acts: a source the adapter cannot accept (an empty brief, a tree that is not the expected shape) is `bad_request!`; an unreadable file or a failed upstream is `server_error!` / `bad_gateway!`. The SDK lowers the class to the WIT `error` variant at the export, the engine lifts it back, and it surfaces to the operator as the matching exit code.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-sdk --open`. The WIT bindings types are defined in the `emery-adapter` contract crate (its `source` axis module) and re-exported at the SDK's root, so an adapter names `emery_sdk::{Context, Evidence, SourceContent, …}` and never depends on `emery-adapter` directly. The prose registry is re-exported the same way — `emery_sdk::Doc`, `emery_sdk::registry`, `emery_sdk::registry!` — so an adapter's `[dependencies]` is `emery-sdk` alone; `emery-prose` is only the build dependency that runs `emit`.

## Walkthrough

The steps below scaffold a source called `changelog` (extracts a bound directory of changelog entries). Substitute your own name — it must be unique across the first-party set.

### 1. Scaffold the crate

```text
sources/changelog/
  Cargo.toml
  build.rs
  src/
    lib.rs
    operations.rs
  prose/
    prompts/
      extract.md
    references/
      emery-runtime -> ../../../../codex/references/runtime
  tests/
    extract.rs
```

The root workspace globs `sources/*`, so the directory joins the workspace with no manifest edit — and `crates/test-programs` compiles it as a component on the next build, so the root `tests/source.rs` and `tests/prose.rs` fail to compile until they name the adapter (step 7). The minimal `Cargo.toml`:

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

[build-dependencies]
emery-prose = { workspace = true, features = ["emit"] }

[dev-dependencies]
tempfile.workspace = true
tokio.workspace = true

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
```

`emery-prose` is a build dependency alone: it turns on the `emit` walker, and the `Doc` registry over the table it writes reaches the adapter through `emery_sdk::registry!` and `emery_sdk::Doc`. `cdylib` is the Wasm component; `rlib` is what native tests link. `tempfile` stages the trees the survey tests cut. The one native-only dev-dependency is `omnia-test`'s `guest` feature, the scripted model the native suite binds; the runtime the built component runs under is the root package's, not the adapter's. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

### 2. Embed the prose

`build.rs` is two lines and identical in every adapter:

```rust
fn main() {
    emery_prose::emit("prose");
}
```

### 3. The library skeleton

`src/lib.rs` is the whole wasm story — the guest shim is one macro invocation at the crate root, which declares the `wasm32`-only guest module itself and carries no logic:

```rust
//! Extracts claims from a tree of changelog entries.

emery_sdk::source!(crate::Adapter);

mod operations;
mod registry {
    emery_sdk::registry!();
}

pub use operations::Adapter;
```

### 4. Implement the operations trait

`src/operations.rs` implements `emery_sdk::SourceAdapter` on a unit struct. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
use std::future::{Future, ready};
use std::path::Path;

use emery_sdk::survey;
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceAdapter, SourceContent, SourceKind};

use crate::registry;

/// The adapter over a tree of changelog entries.
#[derive(Debug)]
pub struct Adapter;

// Entries a directory holds before it is mined on its own.
const FLOOR: usize = 2;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // One seam per top-level directory of entries; a tree that cuts no
    // finer than itself, or an inline value, is the bound input whole. The
    // cut is by directory alone, so the model is never asked.
    fn survey<P: Model>(
        _model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Vec<Seam>, Error>> + Send {
        ready(cut(&ctx.input.content))
    }
}

fn cut(content: &SourceContent) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = content else {
        return Ok(vec![Seam::Whole]);
    };
    let files = survey::files(Path::new(root), |path, _| {
        path.extension().is_some_and(|extension| extension == "md")
    })?;
    let groups = survey::by_directory(files, FLOOR);
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }
    Ok(groups.into_iter().map(Seam::Files).collect())
}
```

`KIND` is the kind of source the adapter reads (`intent`, `documentation`, or `behaviour` — the precedence a cross-source disagreement resolves under), reported by the SDK in the component's `metadata` so the engine ranks every document this adapter returns before any is asked for — a fact about its input, never answered by the model and never carried in an answer. `survey` is the one method with logic of its own; an adapter whose source is never worth splitting leaves the default (one `Seam::Whole`) in place and implements nothing beyond `docs`. `metadata` and `prompt` are inherited: the SDK's default `metadata` reports its own version as the exact `emery-version` pin beside `KIND`, so an adapter overrides it only to loosen or tighten that pin, and `prompt` reads `prompts/extract.md` from `docs()`.

A survey that asks the model is an `async fn` instead — `typescript`'s lists its production modules as above, keeps a tree no directory cut would split as `Seam::Whole` with no turn spent, and otherwise hands the candidates to `survey::by_model(model, ctx, registry::docs(), &files, FLOOR).await?`, turning each group it returns into one `Seam::Note`. The one thing that survey adds to the crate is its prompt, `prose/prompts/survey.md` (step 5).

Points that generalize:

- **The SDK owns the user turn.** `Seam::Whole` selects the ordinary source-input rendering over `ctx.input`; `Seam::Files(files)` renders the lent directory and lists the files beneath it. Use `Seam::Note(note)` when a source needs validation or preparation first, or must be lent whole while the call is told which part is its own — `intent` reads its one-file tree into the seam note; `typescript` lends the root and names each directory's modules, so imports resolve. The envelope always names the adapter and source key, offers the reference tools, and closes with the claims-only request; the answer is claims alone, since `KIND` is already the engine's from `metadata`.
- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. The fixed turn closing says so ("the caller persists…; do not write it yourself") because the model has workspace access.
- **The survey is the only cut.** The source arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline source as `SourceContent::Value` — and `survey` decides how many calls mine it; each call mines its seam completely. Choose a grain floor: a model call costs an agent start, so a directory of one file is folded into the root's seam rather than given its own. A survey of one seam is the whole source in one call, exactly as an adapter with no `survey` behaves. There is no lead focus; a survey that asks the model asks it once, through `survey::by_model`, and only how to group — what exists is the walk's, and what is left out is the remainder's, never omitted.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`, an `example` without `replay-digest`) fails the SDK's `check`, so the backend corrects the candidate in place; an answer still missing one when the backend's rounds are spent fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **The SDK owns the question.** An adapter selects the seams; the system prompt, the user-turn envelope, schema, gate, and correction text are the SDK's and omnia's, and the round budget is the backend's. The prompt should say that a call may be given part of the tree — the files the message lists — and must claim those and no others, with ids led by the domain noun of that part, so two calls over one tree do not name one requirement twice.

### 5. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

An adapter that surveys by model embeds a second: `prose/prompts/survey.md`, the system prompt of its one survey call. It says what one group is for this source (`typescript`'s: the modules that serve one externally visible surface — a route, a command, a job, an exported API), how to place a module several groups reach (once, or in none — the remainder is the SDK's), and what the answer is: `groups`, each a `name` and its `files`, spelled exactly as the turn listed them. The same cap applies, and its `## Worked example` JSON fence must parse as `emery_sdk::survey::Partition` (the root `tests/prose.rs` checks both whenever the document is embedded). The SDK writes the turn — the source, the lent root, the candidate files, the floor — so the prompt describes the grouping and never the listing.

Add the shared runtime references symlink so your prompt can cite the cross-adapter corpus ([reconciliation.md](../codex/references/runtime/reconciliation.md) for the pipeline, [claims.md](../codex/references/runtime/claims.md) for the id grammar, `path` anchors, and the fail-closed gate — link it rather than restating those rules in your prompt):

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 6. Test natively

`tests/extract.rs` asserts only what the adapter itself decides. Its survey takes a scripted model, `omnia_test::guest::Scripted` — no wasm, no network. A mechanical survey takes `Scripted::default()`: call `Adapter::survey(&model, &ctx).await` over a `tempfile` tree, compare the `Vec<Seam>` — two directories that meet the floor are two seams with the expected files, a tree of one directory is `[Seam::Whole]`, an inline value is `[Seam::Whole]`, and the entries the adapter leaves out are named in none — and assert `model.seen().is_empty()` (`documentation`'s suite is the model). A survey that asks the model scripts one partition and asserts what the adapter put into the turn and made of the answer: the candidates offered are exactly its production files, the root is lent, the notes follow the answer's order with the remainder last, and a group under the floor folds; its bound tree and inline value spend no survey turn (`typescript`'s suite is the model). An adapter that validates or prepares its input adds its fail-closed cases (a source it cannot accept is a `bad_request`, never empty success — match on `Error::BadRequest { .. }` with `Scripted::default()`, since no model turn runs) and its `Note` reading as intended — `intent`'s suite is the model. Do not pin prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and what every adapter shares — the declared tools, the schema format, the `check` flag, the lend, the gate's repair loop, the survey check and fold, the fan-out and the join — the SDK's own suite asserts once.

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)).

### 7. Name the adapter in the root suites

Two root tests complete the component rung ([testing.md § Component suites](testing.md#2-component-suites--cratestest-programs)); each `test_programs::foreach_adapter!()` fails to compile until a `#[tokio::test]` / `#[test]` `fn changelog()` exists:

- `tests/source.rs` — stage the minimal fixture tree on a `scratch()` project, run `extract(test_programs::ADAPTER_CHANGELOG, &project)` (the constant is generated from the `sources/` directory), and pass the record to `prompted(&model, include_str!("../sources/changelog/prose/prompts/extract.md"))`. Add only what your component alone shows the host (intent asserts the brief it read through the mount is the turn's seam); the adapter's own behaviour stays in `tests/extract.rs`, and the SDK's side of the boundary is proved over the `gated` probe.
- `tests/prose.rs` — call `corpus(changelog::Adapter::docs())`; an adapter that surveys by model also asserts its `prompts/survey.md` is embedded, as `typescript` does. Nothing more: reference presence is the embed-time walker's, and the prompt your component embeds is proved by `tests/source.rs`.

## Build the component and use it in a project

```bash
cargo build -p changelog --target wasm32-wasip2 --release   # → target/wasm32-wasip2/release/changelog.wasm
```

Bind it in any Emery project by local path — every `specify` naming it loads the file fresh (nothing is cached), so a rebuild is picked up by the next run:

```bash
emery specify path/to/changelog.wasm
```

Publishing a pinned version to GHCR (`emery:changelog@<version>`) is the operator flow in [CONTRIBUTING.md § Publishing](../CONTRIBUTING.md#publishing); a project then names it by package reference (`emery:changelog@<version>`, or the first-party shorthand `changelog@<version>`) and `emery` fetches it fresh on every run that names it. Those are the two ways in: the shipped `emery` deployment declares no adapter guests, so a bare name dispatches nothing — emery's own journey host ([`examples/runtime.rs`](https://github.com/augentic/emery/blob/main/examples/runtime.rs) in the engine repository) loads its mock adapter by path the same way.

To watch it become a specification before wiring it into a project, give it an example: a root-package `[[example]]` cdylib (`examples/changelog/lib.rs` with `emery_sdk::source!(changelog::Adapter)`), a workspace path dep with `default-features = false` so the adapter rlib does not also export the world, `examples/changelog/emery.toml` (copy a sibling's config; one `[[source]]` naming `examples/changelog.wasm` relative to the file and the input it reads — a `path` to a fixture tree beside the config, or a `description`), the fixture if it lends one, and a row in [`examples/README.md`](../examples/README.md); then, from the repository root, `cargo build --example changelog --target wasm32-wasip2 --release`, `emery specify --config examples/changelog/emery.toml` and `emery show spec`.

## Definition of done

- [ ] `src/lib.rs` carries no logic beyond the export macro, `registry!`, and re-exports; reusable logic is wasm-free library code.
- [ ] `extract` is not overridden: the adapter states its seams through `survey` — mechanically, or with one model call through `survey::by_model` — and every model call goes through the SDK.
- [ ] The extraction prompt is embedded under `prose/`, stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence parses as the SDK's `Evidence` — claims alone, no document-level `kind` — and passes the claim gate (the root `tests/prose.rs`). It tells the model that a call may be given part of the tree and must claim that part alone. A survey by model embeds `prose/prompts/survey.md` under the same cap, its worked example parsing as `survey::Partition`.
- [ ] Required per-kind extras are demanded by the prompt — the worked example carries them.
- [ ] `tests/extract.rs` covers what the adapter itself decides over a scripted model (`omnia_test::guest::Scripted`): its survey (how a tree cuts, what it leaves out, when it stays whole and spends no turn; for a survey by model, the candidates offered and the notes cut from a scripted partition) and, where the adapter prepares its input, its fail-closed paths; `cargo nextest run -p <name>` is green.
- [ ] The root `tests/source.rs` and `tests/prose.rs` name the adapter; `cargo nextest run -p emery-adapters` is green.
- [ ] `cargo build -p <name> --target wasm32-wasip2 --release` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
