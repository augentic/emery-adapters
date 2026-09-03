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
| `metadata` | — | `SourceMetadata` | resolve-time record (compatibility floor) |
| `extract` | `Context`, typed `SourceInput` (`key`, workspace-or-value) | `Evidence` | validates fail-closed (id grammar, required per-kind extras — A8), reconciles across sources, synthesises `spec.md` / `design.md` |

Three ideas carry the operation:

- **The model is a parameter.** `extract` is generic over `emery_adapter::Model`. On wasm the macro binds `WasiModel`; native tests bind `omnia_test::guest::Scripted` with scripted answers. Your code never constructs a backend.
- **Prose is embedded at build time.** `build.rs` calls `emery_prose::emit("prose")`, which walks the adapter's `prose/` tree into a sorted `DOCS` table; `emery_adapter::registry!()` exposes it as `registry::docs()` / `registry::body("prompts/extract.md")`. A dangling relative link in any prose document fails the build. Each judgment declares `list_docs` / `read_doc` function tools and answers the model's calls in-process from that embedded corpus, so prompts cite references by relative link instead of inlining them.
- **Answers are schema-gated and repaired.** `emery_adapter::repaired(model, ctx, system, user, kind, SCHEMA, tail)` sends the prompt, parses the reply against a generated JSON schema, and re-prompts with the parse error up to `emery_adapter::MAX_REPAIRS` times before failing.

For the type-level contract — `Context`, `SourceInput`, `Evidence`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-adapter --open`.

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

[build-dependencies]
emery-prose.workspace = true

[dev-dependencies]
tokio.workspace = true

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
omnia-test = { workspace = true, features = ["guest"] }
```

`cdylib` is the Wasm component; `rlib` is what native tests link. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

### 2. Embed the prose

`build.rs` is two lines and identical in every adapter:

```rust
fn main() {
    emery_prose::emit("prose");
}
```

### 3. The library skeleton

`src/lib.rs` is the whole wasm story — the guest shim is one macro invocation, gated to `wasm32`, and carries no logic:

```rust
//! Changelog source adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    emery_adapter::source!(crate::Adapter);
}

mod operations;
mod registry {
    emery_adapter::registry!();
}

pub use operations::Adapter;
```

### 4. Implement the operations trait

`src/operations.rs` implements `emery_adapter::SourceAdapter` on a unit struct. Condensed — the real `intent` and `documentation` files are worth reading in full:

```rust
use emery_adapter::answers::{evidence_schema, evidence_tail};
use emery_adapter::registry::Doc;
use emery_adapter::types::{Context, Error, Evidence, SourceContent, SourceInput, SourceMetadata};
use emery_adapter::{Model, SourceAdapter, repaired};

use crate::registry;

/// Extracts a bound changelog tree into structured claims.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    const IDENTITY: &str = concat!("changelog@", env!("CARGO_PKG_VERSION"));

    fn metadata() -> SourceMetadata {
        SourceMetadata { emery_floor: Some("0.38.0".to_string()) }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<Evidence, Error> {
        let system = registry::body("prompts/extract.md").to_string();
        let content = match &input.content {
            SourceContent::Workspace(view) => format!(
                "`$SOURCE_DIR` is the read-only view at `{}`; nothing outside it is reachable.",
                view.root
            ),
            SourceContent::Value(value) => format!(
                "The bound material is this inline value; no `$SOURCE_DIR` is lent:\n\n{value}"
            ),
        };
        let user = format!(
            "Extract the claim set of the changelog source bound to adapter `{id}` \
             (source key `{key}`).\n\n{content}\n\n\
             Answer with one JSON object matching the gated schema: the Evidence body \
             (`authority`, `claims`). The caller persists the document; do not write it \
             yourself.",
            id = ctx.adapter_id,
            key = input.key,
        );
        let schema = evidence_schema();
        repaired(model, ctx, system, user, "evidence", &schema, evidence_tail).await
    }
}
```

Points that generalize:

- **Extract writes no artifacts.** The engine persists the Evidence; your job is to return a well-formed value. Say so explicitly in the prompt ("the caller persists…; do not write it yourself") because the model has workspace access.
- **One pass, whole source.** There is no survey step and no lead focus: extract mines the whole bound source in one call. The binding arrives prepared — a tree as `SourceContent::Workspace` (lent as `$SOURCE_DIR`), an inline binding as `SourceContent::Value`.
- **Required extras are fail-closed.** A `requirement` claim without a `statement` extra (or a `criterion` without `criterion`) fails the whole run engine-side closed (typed `bad_request`) — never a synopsis fallback. Put the per-kind table and the id-derivation rules in the prompt; reconciliation joins claims across sources by their dotted-kebab ids.
- **`repaired` owns the parse-and-retry loop.** Pick the schema constant and tail matching the operation.

### 5. Author the prose

One prompt: `prose/prompts/extract.md` — the claim-kind table with each kind's required body field, the id-derivation rules, the JSON output contract, and a worked example. Shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring); depth goes in `prose/references/`, cited by relative link.

Add the shared runtime references symlink so your prompt can cite the cross-adapter corpus (reconciliation, authority precedence):

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 6. Test natively

`tests/operations.rs` drives the trait with a scripted model — no wasm, no network. The assertions worth making: "did my prompt content land in the assembled request", "does the parsed answer round-trip", and "do required extras arrive verbatim in `Evidence`". Mirror the existing adapters' suites, including their fail-closed cases (an unreadable binding is a typed error, never empty success). Also add a `tests/registry.rs` pinning that every prompt path your operations load is actually embedded — and that no survey prose exists.

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)).

### 7. Add the conformance test

`examples/conformance/tests/conformance.rs` runs every `sources/*` component under the omnia runtime; its `foreach_source!()` is generated from the `sources/` directory, so the workspace fails to compile until a `#[tokio::test] async fn changelog()` exists there. Add a `Case` (the routed id `source:changelog`, the generated `SOURCE_CHANGELOG` constant, a minimal fixture tree, and `include_str!` of your `prose/prompts/extract.md`) and a test calling the shared `conforms` body — see [testing.md § Component conformance](testing.md#2-component-conformance).

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
