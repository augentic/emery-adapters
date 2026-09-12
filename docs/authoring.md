# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Extract assembles a prompt over the inline brief and delegates to the model.
- [`sources/documentation/`](../sources/documentation/) — the whole-tree shape: one extraction pass over a bound directory, with the claim-kind table and id-derivation rules in its prompt.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

You never touch WIT directly. The `emery-adapter` SDK hides the bindings behind the `emery_adapter::SourceAdapter` trait, implemented on a unit struct; a one-line macro (`emery_adapter::source!`) wires that implementor into the component exports.

What the engine calls:

| Operation | Engine passes | You return | The engine does with it |
| --------- | ------------- | ---------- | ----------------------- |
| `metadata` | — | `AdapterMetadata` | resolve-time record (`emery-version` gate) |
| `extract` | `Context`, typed `SourceInput` (`key`, workspace-or-value) | `Evidence` | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

Three ideas carry the operation:

- **The model is a parameter.** `extract` is generic over `emery_adapter::Model`. On wasm the macro binds `WasiModel`; native tests bind `omnia_test::guest::Scripted` with scripted answers. Your code never constructs a backend.
- **Prose is embedded at build time.** `build.rs` calls `emery_prose::emit("prose")` (the `emit` feature, enabled on the build-dependency only), which walks the adapter's `prose/` tree into a sorted `DOCS` table; `emery_prose::registry!()` exposes it as `registry::docs()`, and the SDK's `SourceAdapter::prompt` reads `prompts/extract.md` from it. A dangling relative link in any prose document fails the build. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) embeds its prose the same way and is the smallest complete example of the shape. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are steered by schema and judged by the gate.** The provided `Self::evidence(model, ctx, material)` asks one `omnia_guest::model::Question<Evidence>`: the embedded `prompts/extract.md` is the system prompt, the derived `Evidence` schema rides the request as a steering hint (the claim-id grammar as its `pattern`), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; it gets the accepted `Evidence`, or a `bad_request` carrying the last findings once the rounds are spent. `Material::Bound` renders the ordinary workspace or inline value (a workspace as "the `SOURCE` source tree"); `Material::Prepared(note)` admits a source-specific material note after the adapter validates or reads its input.
- **Failures are Omnia errors.** Every operation fails with `emery_adapter::Error` (omnia's `omnia_guest::Error`), built with the re-exported `bad_request!` / `server_error!` / `bad_gateway!` macros — there is no adapter error type. Classify by who acts: a source the adapter cannot accept (an empty brief, a tree that is not the expected shape) is `bad_request!`; an unreadable file or a failed upstream is `server_error!` / `bad_gateway!`. The SDK lowers the class to the WIT `error` variant at the export, the engine lifts it back, and it surfaces to the operator as the matching exit code.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-adapter --open`. The wire DTOs are defined in `emery-source` and re-exported at the SDK's root, so an adapter names `emery_adapter::{Context, Evidence, SourceContent, …}` and never depends on `emery-source` directly.

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
    operations.rs
    registry.rs
```

The root workspace globs `sources/*`, so the directory joins the workspace with no manifest edit. The minimal `Cargo.toml`:

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
emery-adapter.workspace = true
emery-prose.workspace = true

[build-dependencies]
emery-prose = { workspace = true, features = ["emit"] }

[dev-dependencies]
tokio.workspace = true

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
```

`emery-prose` appears twice on purpose: the runtime dependency carries only the `Doc` registry, the build dependency turns on the `emit` walker. `cdylib` is the Wasm component; `rlib` is what native tests link. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

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

emery_adapter::source!(crate::Adapter);

mod operations;
mod registry {
    emery_prose::registry!();
}

pub use operations::Adapter;
```

### 4. Implement the operations trait

`src/operations.rs` implements `emery_adapter::SourceAdapter` on a unit struct. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
use emery_adapter::{Context, Error, Evidence, Material, Model, SourceAdapter};
use emery_prose::registry::Doc;

use crate::registry;

/// Extracts a bound changelog tree into structured claims.
#[derive(Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "changelog";

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn extract<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Evidence, Error> {
        Self::evidence(model, ctx, Material::Bound).await
    }
}
```

`SOURCE` is the noun the user turn calls the source ("the changelog source tree"). `metadata` and `prompt` are inherited: the SDK's default `metadata` reports its own version as the exact `emery-version` pin, so an adapter overrides it only to loosen or tighten that pin, and `prompt` reads `prompts/extract.md` from `docs()`.

Points that generalize:

- **The SDK owns the user turn.** `Material::Bound` selects the ordinary source-input rendering over `ctx.input`. Use `Material::Prepared(note)` when a source needs validation or preparation first — `intent`, for example, reads its one-file tree into the material note. The envelope always names the adapter and source key, offers the reference tools, and closes with the Evidence request.
- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. The fixed turn closing says so ("the caller persists…; do not write it yourself") because the model has workspace access.
- **One pass, whole source.** There is no survey step and no lead focus: extract mines the whole bound source in one call. The source arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline source as `SourceContent::Value`.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`, an `example` without `replay-digest`) fails the SDK's `check`, so the backend corrects the candidate in place; an answer still missing one when the backend's rounds are spent fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **`evidence` owns the question.** An adapter selects the turn material; the system prompt, the user-turn envelope, schema, gate, and correction text are the SDK's and omnia's, and the round budget is the backend's.

### 5. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

Add the shared runtime references symlink so your prompt can cite the cross-adapter corpus ([reconciliation.md](../codex/references/runtime/reconciliation.md) for the pipeline, [claims.md](../codex/references/runtime/claims.md) for the id grammar, `path` anchors, and the fail-closed gate — link it rather than restating those rules in your prompt):

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 6. Test natively

`tests/operations.rs` drives the trait with a scripted model — no wasm, no network. The assertions worth making: "did my prompt content land in the assembled request", "does the parsed answer round-trip", and "do required extras arrive verbatim in `Evidence`". Mirror the existing adapters' suites, including their fail-closed cases (a source the adapter cannot accept is a `bad_request`, never empty success — match on `Error::BadRequest { .. }`). Also add a `tests/registry.rs` pinning that every prompt path your operations load is actually embedded — and that no survey prose exists.

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)).

### 7. Add the conformance test

`examples/conformance/tests/conformance.rs` runs every `sources/*` component under the omnia runtime; its `foreach_source!()` is generated from the `sources/` directory, so the workspace fails to compile until a `#[tokio::test] async fn changelog()` exists there. Add a `Case` (the adapter id `source:changelog`, the generated `SOURCE_CHANGELOG` constant, a minimal fixture tree, and `include_str!` of your `prose/prompts/extract.md`) and a test calling the shared `conforms` body — see [testing.md § Component conformance](testing.md#2-component-conformance).

## Build the component and use it in a project

```bash
make adapter changelog     # fast dev build → target/wasm32-wasip2/release/changelog.wasm
```

Bind it in any Emery project by local path — the first `specify` naming it seeds the project's component cache:

```bash
emery specify path/to/changelog.wasm
```

To exercise it through the graded live eval, add a case to `examples/eval/src/main.rs` (a fixture under `examples/eval/cases/<id>/fixture/` plus its graded expectations) — see [examples/eval/README.md](../examples/eval/README.md). Publishing a pinned version to GHCR (`emery:changelog@<version>`) is the operator flow in [CONTRIBUTING.md § Publishing](../CONTRIBUTING.md#publishing). To load it as a static guest, build the component and declare it in the host runtime the same way emery's journey host declares its mock source (`examples/runtime.rs`).

## Definition of done

- [ ] `src/lib.rs` carries no logic beyond the export macro, `registry!`, and re-exports; reusable logic is wasm-free library code.
- [ ] Every prompt path loaded by `operations.rs` exists under `prose/` (pinned by `tests/registry.rs`); no survey prose.
- [ ] Required per-kind extras are demanded by the prompt and asserted in the native tests.
- [ ] Native `tests/` cover extract with a scripted model (`omnia_test::guest::Scripted`), including fail-closed paths; `cargo nextest run -p <name>` is green.
- [ ] `examples/conformance/tests/conformance.rs` carries the adapter's conformance test; `cargo nextest run -p conformance` is green.
- [ ] `make adapter <name>` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
