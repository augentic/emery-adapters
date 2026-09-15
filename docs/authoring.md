# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Its survey is one material: the inline brief, or the one file it reads from a tree.
- [`sources/documentation/`](../sources/documentation/) — the tree shape: a survey that cuts a bound directory into one material per top-level directory, with the claim-kind table and id-derivation rules in its prompt.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

You never touch WIT directly. The `emery-sdk` SDK hides the bindings behind the `emery_sdk::SourceAdapter` trait, implemented on a unit struct; a one-line macro (`emery_sdk::source!`) wires that implementor into the component exports.

What the engine calls:

| Operation | Engine passes | You return | The engine does with it |
| --------- | ------------- | ---------- | ----------------------- |
| `metadata` | — | `AdapterMetadata` | resolve-time record (`emery-version` gate) |
| `extract` | `Context`, typed `SourceInput` (`key`, workspace-or-value) | `Evidence` | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

`extract` is the SDK's: it asks your `survey` for the materials to mine, runs one model call per material — at most four pending at once, in material order — and joins the answers into the one `Evidence` document, re-rooting each material's `path` anchors under the source root. You implement `survey`; you never override `extract`.

Four ideas carry the operation:

- **The model is a parameter.** `extract` is generic over `emery_sdk::Model`. On wasm the macro binds `WasiModel`; native tests bind `omnia_test::guest::Scripted` with scripted answers. Your code never constructs a backend.
- **The survey is mechanical.** `survey(ctx) -> Result<Vec<Material>, Error>` is synchronous and spends no model call: it lists or reads the input and states the materials. A tree adapter lists its files with `emery_sdk::survey::files(root, keep)` — the engine's own `spec.md`, `design.md`, and `.omnia/` are pruned for it, everything else is its `keep` predicate's — and cuts them with `survey::by_directory(files, floor)`: one group per top-level directory holding at least `floor` files, the root's own files and every smaller directory folded into one remainder. Each group becomes the material its source kind calls for: `Material::Within(files)` lends the group's directory alone, so a documentation directory is mined under a grant no wider than itself; `Material::Prepared(note)` lends the whole root and tells the model which files are its own, which code needs — a handler's behaviour runs through its imports. A tree that cuts into fewer than two groups, and an inline value, are one `Material::Bound`: today's single call, unchanged. The default `survey` is that one `Bound` material.
- **Prose is embedded at build time.** `build.rs` calls `emery_prose::emit("prose")` (the `emit` feature, enabled on the build-dependency only), which walks the adapter's `prose/` tree into a sorted `DOCS` table; `emery_prose::registry!()` exposes it as `registry::docs()`, and the SDK's `SourceAdapter::prompt` reads `prompts/extract.md` from it. A dangling relative link in any prose document fails the build. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) embeds its prose the same way and is the smallest complete example of the shape. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are steered by schema and judged by the gate.** For each material, the SDK's `Self::evidence(model, ctx, material)` asks one `omnia_guest::model::Question<Answer>`: the embedded `prompts/extract.md` is the system prompt, the derived claims-only schema rides the request as a steering hint (the claim-id grammar as its `pattern`), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; the SDK gets the accepted claims, or a `bad_request` carrying the last findings once the rounds are spent. `Material::Bound` renders the ordinary workspace or inline value (a workspace as "the `SOURCE` source tree"); `Material::Within(files)` renders the lent directory and the files to mine beneath it; `Material::Prepared(note)` admits a source-specific material note after the adapter validates or reads its input.
- **Failures are Omnia errors.** Every operation fails with `emery_sdk::Error` (omnia's `omnia_guest::Error`), built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros — there is no adapter error type. Classify by who acts: a source the adapter cannot accept (an empty brief, a tree that is not the expected shape) is `bad_request!`; an unreadable file or a failed upstream is `server_error!` / `bad_gateway!`. The SDK lowers the class to the WIT `error` variant at the export, the engine lifts it back, and it surfaces to the operator as the matching exit code.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-sdk --open`. The WIT bindings types are defined in the `emery-adapter` contract crate (its `source` axis module) and re-exported at the SDK's root, so an adapter names `emery_sdk::{Context, Evidence, SourceContent, …}` and never depends on `emery-adapter` directly.

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
emery-prose.workspace = true
emery-sdk.workspace = true

[build-dependencies]
emery-prose = { workspace = true, features = ["emit"] }

[dev-dependencies]
tempfile.workspace = true
tokio.workspace = true

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
```

`emery-prose` appears twice on purpose: the runtime dependency carries only the `Doc` registry, the build dependency turns on the `emit` walker. `cdylib` is the Wasm component; `rlib` is what native tests link. `tempfile` stages the trees the survey tests cut. The one native-only dev-dependency is `omnia-test`'s `guest` feature, the scripted model the native suite binds; the runtime the built component runs under is the root package's, not the adapter's. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

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
//! Changelog source adapter.

emery_sdk::source!(crate::Adapter);

mod operations;
mod registry {
    emery_prose::registry!();
}

pub use operations::Adapter;
```

### 4. Implement the operations trait

`src/operations.rs` implements `emery_sdk::SourceAdapter` on a unit struct. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
use std::path::Path;

use emery_prose::registry::Doc;
use emery_sdk::survey;
use emery_sdk::{Context, Error, Material, SourceAdapter, SourceContent, SourceKind};

use crate::registry;

/// Extracts a bound changelog tree into structured claims.
#[derive(Debug)]
pub struct Adapter;

// Entries a directory holds before it is mined on its own.
const FLOOR: usize = 2;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "changelog";
    const KIND: SourceKind = SourceKind::Documentation;

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // One material per top-level directory of entries; a tree that cuts no
    // finer than itself, or an inline value, is the bound input whole.
    fn survey(ctx: &Context<'_>) -> Result<Vec<Material>, Error> {
        let SourceContent::Workspace(root) = &ctx.input.content else {
            return Ok(vec![Material::Bound]);
        };
        let files = survey::files(Path::new(root), |path, _| {
            path.extension().is_some_and(|extension| extension == "md")
        })?;
        let groups = survey::by_directory(files, FLOOR);
        if groups.len() < 2 {
            return Ok(vec![Material::Bound]);
        }
        Ok(groups.into_iter().map(Material::Within).collect())
    }
}
```

`SOURCE` is the noun the user turn calls the source ("the changelog source tree"). `KIND` is the kind of source the adapter reads (`intent`, `documentation`, or `behaviour` — the precedence a cross-source disagreement resolves under), stamped by the SDK on every document this adapter returns — a fact about its input, never answered by the model. `survey` is the one method with logic of its own; an adapter whose source is never worth splitting leaves the default (one `Material::Bound`) in place and implements nothing beyond `docs`. `metadata` and `prompt` are inherited: the SDK's default `metadata` reports its own version as the exact `emery-version` pin, so an adapter overrides it only to loosen or tighten that pin, and `prompt` reads `prompts/extract.md` from `docs()`.

Points that generalize:

- **The SDK owns the user turn.** `Material::Bound` selects the ordinary source-input rendering over `ctx.input`; `Material::Within(files)` renders the lent directory and lists the files beneath it. Use `Material::Prepared(note)` when a source needs validation or preparation first, or must be lent whole while the call is told which part is its own — `intent` reads its one-file tree into the material note; `typescript` lends the root and names each directory's modules, so imports resolve. The envelope always names the adapter and source key, offers the reference tools, and closes with the claims-only request. The SDK stamps `KIND`.
- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. The fixed turn closing says so ("the caller persists…; do not write it yourself") because the model has workspace access.
- **The survey is the only cut.** The source arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline source as `SourceContent::Value` — and `survey` decides how many calls mine it; each call mines its material completely. Choose a grain floor: a model call costs an agent start, so a directory of one file is folded into the root's material rather than given its own. A survey of one material is the whole source in one call, exactly as an adapter with no `survey` behaves. There is no lead focus and no model-guided survey.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`, an `example` without `replay-digest`) fails the SDK's `check`, so the backend corrects the candidate in place; an answer still missing one when the backend's rounds are spent fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **`evidence` owns the question.** An adapter selects the materials; the system prompt, the user-turn envelope, schema, gate, and correction text are the SDK's and omnia's, and the round budget is the backend's. The prompt should say that a call may be given part of the tree — the files the message lists — and must claim those and no others, with ids led by the domain noun of that part, so two calls over one tree do not name one requirement twice.

### 5. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

Add the shared runtime references symlink so your prompt can cite the cross-adapter corpus ([reconciliation.md](../codex/references/runtime/reconciliation.md) for the pipeline, [claims.md](../codex/references/runtime/claims.md) for the id grammar, `path` anchors, and the fail-closed gate — link it rather than restating those rules in your prompt):

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 6. Test natively

`tests/extract.rs` asserts only what the adapter itself decides. Its survey needs no model at all: call `Adapter::survey(&ctx)` over a `tempfile` tree and compare the `Vec<Material>` — two directories that meet the floor are two materials with the expected files, a tree of one directory is `[Material::Bound]`, an inline value is `[Material::Bound]`, and the entries the adapter leaves out are named in none (`documentation`'s and `typescript`'s suites are the model). One test drives `extract` over a one-directory tree with `omnia_test::guest::Scripted` — no wasm, no network — to see the `SOURCE` noun name the bound tree in the turn (`turn.contains("the changelog source tree")`). An adapter that validates or prepares its input adds its fail-closed cases (a source it cannot accept is a `bad_request`, never empty success — match on `Error::BadRequest { .. }` with `Scripted::default()`, since no model turn runs) and its `Prepared` note reading as intended — `intent`'s suite is the model. Do not pin prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and what every adapter shares — the declared tools, the schema format, the `check` flag, the lend, the gate's repair loop, the fan-out and the join — the SDK's own suite asserts once.

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)).

### 7. Name the adapter in the root suites

Two root tests complete the component rung ([testing.md § Seam suites](testing.md#2-seam-suites--cratestest-programs)); each `test_programs::foreach_adapter!()` fails to compile until a `#[tokio::test]` / `#[test]` `fn changelog()` exists:

- `tests/source.rs` — stage the minimal fixture tree on a `scratch()` project, run `extract(test_programs::ADAPTER_CHANGELOG, &project)` (the constant is generated from the `sources/` directory), and pass the record to `prompted(&model, include_str!("../sources/changelog/prose/prompts/extract.md"))`. Add only what your component alone shows the host (intent asserts the brief it read through the mount is the turn's material); the adapter's own behaviour stays in `tests/extract.rs`, and the SDK's side of the seam is proved over the `gated` probe.
- `tests/prose.rs` — call `corpus::<changelog::Adapter>(changelog::Adapter::docs())`. Nothing more: reference presence is the embed-time walker's, and the prompt your component embeds is proved by `tests/source.rs`.

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
- [ ] `extract` is not overridden: the adapter states its materials through `survey`, mechanically, and every model call goes through the SDK.
- [ ] The extraction prompt is embedded under `prose/`, stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence parses as the SDK's `Answer` and passes the claim gate under the adapter's constant (the root `tests/prose.rs`); no survey prose. It tells the model that a call may be given part of the tree and must claim that part alone.
- [ ] Required per-kind extras are demanded by the prompt — the worked example carries them.
- [ ] `tests/extract.rs` covers what the adapter itself decides: its survey with no model (how a tree cuts, what it leaves out, when it stays whole), and with a scripted model (`omnia_test::guest::Scripted`) the `SOURCE` noun landing and, where the adapter prepares its input, its fail-closed paths; `cargo nextest run -p <name>` is green.
- [ ] The root `tests/source.rs` and `tests/prose.rs` name the adapter; `cargo nextest run -p emery-adapters` is green.
- [ ] `cargo build -p <name> --target wasm32-wasip2 --release` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
