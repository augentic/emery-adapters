# Authoring a source adapter

How to create an Emery source adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest adapter. Extract assembles a prompt over the inline brief and delegates to the model.
- [`sources/documentation/`](../sources/documentation/) — the whole-tree shape: one extraction pass over a bound directory, with the claim-kind table and id-derivation rules in its prompt.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

You never touch WIT directly. The `emery-sdk` SDK hides the bindings behind the `emery_sdk::SourceAdapter` trait, implemented on a unit struct; a one-line macro (`emery_sdk::source!`) wires that implementor into the component exports.

What the engine calls:

| Operation | Engine passes | You return | The engine does with it |
| --------- | ------------- | ---------- | ----------------------- |
| `metadata` | — | `AdapterMetadata` | resolve-time record (`emery-version` gate) |
| `extract` | `Context`, typed `SourceInput` (`key`, workspace-or-value) | `Evidence` | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises the typed spec and design (`emery show` projects them as `spec.md` / `design.md`) |

Three ideas carry the operation:

- **The model is a parameter.** `extract` is generic over `emery_sdk::Model`. On wasm the macro binds `WasiModel`; native tests bind `omnia_test::guest::Scripted` with scripted answers. Your code never constructs a backend.
- **Prose is embedded at build time.** `build.rs` calls `emery_prose::emit("prose")` (the `emit` feature, enabled on the build-dependency only), which walks the adapter's `prose/` tree into a sorted `DOCS` table; `emery_prose::registry!()` exposes it as `registry::docs()`, and the SDK's `SourceAdapter::prompt` reads `prompts/extract.md` from it. A dangling relative link in any prose document fails the build. The engine repository's mock adapter ([`examples/adapter/lib.rs`](https://github.com/augentic/emery/blob/main/examples/adapter/lib.rs)) embeds its prose the same way and is the smallest complete example of the shape. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are steered by schema and judged by the gate.** The provided `Self::evidence(model, ctx, material)` asks one `omnia_guest::model::Question<Evidence>`: the embedded `prompts/extract.md` is the system prompt, the derived `Evidence` schema rides the request as a steering hint (the claim-id grammar as its `pattern`), and the claim gate is the request's `check` — run over every candidate the backend proposes, with a miss handed back as the correction (`## Previous answer (rejected)` / `## Findings`) so the backend asks again within its own round budget. The adapter never sees the reply text; it gets the accepted `Evidence`, or a `bad_request` carrying the last findings once the rounds are spent. `Material::Bound` renders the ordinary workspace or inline value (a workspace as "the `SOURCE` source tree"); `Material::Prepared(note)` admits a source-specific material note after the adapter validates or reads its input.
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
tokio.workspace = true

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
```

`emery-prose` appears twice on purpose: the runtime dependency carries only the `Doc` registry, the build dependency turns on the `emit` walker. `cdylib` is the Wasm component; `rlib` is what native tests link. The one native-only dev-dependency is `omnia-test`'s `guest` feature, the scripted model the native suite binds; the runtime the built component runs under is the root package's, not the adapter's. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

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
use emery_sdk::{Context, Error, Evidence, Material, Model, SourceAdapter};
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

`tests/extract.rs` drives the trait with a scripted model — no wasm, no network — and asserts only what the adapter itself decides. For an adapter that hands `Material::Bound` straight to the SDK, that is one test: the `SOURCE` noun names the bound tree in the turn (`turn.contains("the changelog source tree")`). An adapter that validates or prepares its input adds its fail-closed cases (a source it cannot accept is a `bad_request`, never empty success — match on `Error::BadRequest { .. }` with `Scripted::default()`, since no model turn runs) and its `Prepared` note reading as intended — `intent`'s suite is the model. Do not pin prompt phrases or the SDK's brief text: a wording edit is not a behavior change, and what every adapter shares — the declared tools, the schema format, the `check` flag, the lend, the gate's repair loop — the SDK's own suite asserts once.

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)).

### 7. Name the adapter in the root suites

Two root tests complete the component rung ([testing.md § Seam suites](testing.md#2-seam-suites--cratestest-programs)); each `test_programs::foreach_adapter!()` fails to compile until a `#[tokio::test]` / `#[test]` `fn changelog()` exists:

- `tests/source.rs` — stage the minimal fixture tree on a `scratch()` project, run `extract(test_programs::ADAPTER_CHANGELOG, Authority::<Class>, &project)` (the constant is generated from the `sources/` directory; the authority is the class your prompt declares), and pass the record to `prompted(&model, include_str!("../sources/changelog/prose/prompts/extract.md"))`. Add only what your component alone shows the host (intent asserts the brief it read through the mount is the turn's material); the adapter's own behaviour stays in `tests/extract.rs`, and the SDK's side of the seam is proved over the `gated` probe.
- `tests/prose.rs` — call `corpus(changelog::Adapter::docs(), Authority::<Class>)` with the authority your prompt's `## Worked example` declares. Nothing more: reference presence is the embed-time walker's, and the prompt your component embeds is proved by `tests/source.rs`.

## Build the component and use it in a project

```bash
make adapter changelog     # fast dev build → target/wasm32-wasip2/release/changelog.wasm
```

Bind it in any Emery project by local path — every `specify` naming it loads the file fresh (nothing is cached), so a rebuild is picked up by the next run:

```bash
emery specify path/to/changelog.wasm
```

Publishing a pinned version to GHCR (`emery:changelog@<version>`) is the operator flow in [CONTRIBUTING.md § Publishing](../CONTRIBUTING.md#publishing); a project then names it by package reference (`emery:changelog@<version>`, or the first-party shorthand `changelog@<version>`) and `emery` fetches it fresh on every run that names it. Those are the two ways in: the shipped `emery` deployment declares no adapter guests, so a bare name dispatches nothing — emery's own journey host ([`examples/runtime.rs`](https://github.com/augentic/emery/blob/main/examples/runtime.rs) in the engine repository) loads its mock adapter by path the same way.

To watch it become a specification before wiring it into a project, give it an example: `examples/changelog/emery.toml` (copy a sibling's config; one `[[source]]` naming the built component by path relative to the file and the input it reads — a `path` to a fixture tree beside the config, or a `description`), the fixture if it lends one, and a row in [`examples/README.md`](../examples/README.md); then, from the repository root, `emery specify --config examples/changelog/emery.toml` and `emery show spec`. Nothing is compiled for an example — the config is data the shipped `emery` binary runs.

## Definition of done

- [ ] `src/lib.rs` carries no logic beyond the export macro, `registry!`, and re-exports; reusable logic is wasm-free library code.
- [ ] The extraction prompt is embedded under `prose/`, stays under the 800 non-blank-line cap, and its `## Worked example` JSON fence passes the claim gate under the adapter's authority (the root `tests/prose.rs`); no survey prose.
- [ ] Required per-kind extras are demanded by the prompt — the worked example carries them.
- [ ] `tests/extract.rs` covers what the adapter itself decides with a scripted model (`omnia_test::guest::Scripted`): the `SOURCE` noun landing and, where the adapter prepares its input, its fail-closed paths; `cargo nextest run -p <name>` is green.
- [ ] The root `tests/source.rs` and `tests/prose.rs` name the adapter; `cargo nextest run -p emery-adapters` is green.
- [ ] `make adapter <name>` builds the component; no `.wasm` artifacts committed.
- [ ] `make ci` is green.
