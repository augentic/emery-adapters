# Authoring an adapter

How to create a Emery adapter, from an empty directory to a published component. Written for Rust developers comfortable with async and Cargo workspaces; no prior WebAssembly-component experience is assumed.

Before starting, skim two existing adapters — they are the reference implementations this guide condenses:

- [`sources/intent/`](../sources/intent/) — the smallest **source** adapter (~75 lines of Rust). Both operations assemble a prompt and delegate to the model.
- [`targets/contracts/`](../targets/contracts/) — the smallest **target** adapter. Shows multi-phase builds, deterministic in-guest validation, and phased merge gates.

Toolchain setup, layout conventions, and publishing mechanics live in [CONTRIBUTING.md](../CONTRIBUTING.md); this guide links into them rather than repeating them.

## How an adapter executes

An adapter is one Rust crate that ships as one Wasm component exporting exactly one axis world from the `emery:adapter` WIT package (owned by [`augentic/emery`](https://github.com/augentic/emery)). There is no manifest file: identity is the crate's `name` + `version`, and resolve-time metadata comes from the component's own `metadata` export.

You never touch WIT directly. The `adapter` SDK hides the bindings behind two per-axis traits — `adapter::Source` and `adapter::Target` — that you implement on a unit struct. A one-line macro (`adapter::source!` / `adapter::target!`) wires that implementor into the component exports.

The same trait implementation runs on two hosts:

| Host | How it links | Used by |
| ---- | ------------ | ------- |
| **Native** | The crate is `rlib`; the engine's `native` catalog links it directly | `cargo nextest`, `cargo make eval`, `cargo make lab` |
| **Wasm** | The crate is `cdylib`; the `wasm32-wasip2` build exports the WIT world | The shipped `emery` CLI, `cargo make wasm-contracts` / `wasm-omnia-r9k` |

This split is why the day-to-day loop is fast: prose and Rust changes are picked up by native tests and live eval with no component build.

What the engine calls, per axis:

| Axis | Operation | Engine passes | You return | Engine persists it as |
| ---- | --------- | ------------- | ---------- | --------------------- |
| source | `metadata` | — | `SourceMetadata` | resolve-time record |
| source | `survey` | `Context` | `Vec<Lead>` | `## Lead inventory` blocks in `discovery.md` |
| source | `extract` | `Context`, one `Lead` | `Evidence` | `.emery/slices/<slice>/evidence/<source>.yaml` |
| target | `metadata` | — | `TargetMetadata` (floor, build `inputs[]`, platforms) | resolve-time record |
| target | `guidance` | `Context` | prompt `String` | read by core synthesis |
| target | `build` | `Context`, slice name, typed `inputs`, `WorkingTree` | `Report` | build report; gates the `built` transition |
| target | `merge` | `Context`, slice name, `MergePhase`, `WorkingTree` | `Report` | merge gate report (`preflight` before the commit, `postflight` after) |

Three ideas carry every operation:

- **The model is a parameter.** Judgment operations are generic over `adapter::Model`. On wasm the macro binds `WasiModel`; native tests bind `omnia_testkit::model::Harness` with scripted answers. Your code never constructs a backend.
- **Prose is embedded at build time.** `build.rs` calls `prose::emit("prose")`, which walks the adapter's `prose/` tree into a sorted `DOCS` table; `adapter::registry!()` exposes it as `registry::docs()` / `registry::body("prompts/survey.md")`. A dangling relative link in any prose document fails the build. The export macros also serve `prose/references/**` over MCP, so prompts cite references by relative link instead of inlining them.
- **Answers are schema-gated and repaired.** `adapter::repaired(model, ctx, system, user, kind, SCHEMA, tail)` sends the prompt, parses the reply against a generated JSON schema, and re-prompts with the parse error up to `adapter::MAX_REPAIRS` times before failing. Targets use the `adapter::phase` helpers (`phase::phase`, `phase::report`, `phase::enforce`) built on the same kernel.

For the type-level contract — `Context`, `Lead`, `Evidence`, `Report`, the answer schemas — generate the SDK docs locally with `cargo doc -p emery-adapter --open`. The engine-side view of the same seam is [`emery` docs/explanation/adapter-anatomy.md](https://github.com/augentic/emery/blob/main/docs/explanation/adapter-anatomy.md).

## Walkthrough: a source adapter

The steps below scaffold a source called `changelog` (surveys a bound directory of changelog entries). Substitute your own name — it must be unique across **both** axes: a name lives under `sources/<name>/` xor `targets/<name>/`, never both.

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
      survey.md
      extract.md
    references/
      emery-runtime -> ../../../../codex/references/runtime
  tests/
    operations.rs
```

The root workspace globs `sources/*` and `targets/*`, so the directory joins the workspace with no manifest edit. The minimal `Cargo.toml`:

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
omnia-testkit.workspace = true
tokio.workspace = true
```

`cdylib` is the Wasm component; `rlib` is what native tests and the eval catalog link. The identity SemVer is the shared `[workspace.package] version` — adapters version together.

### 2. Embed the prose

`build.rs` is two lines and identical in every adapter:

```rust
fn main() {
    prose::emit("prose");
}
```

### 3. The library skeleton

`src/lib.rs` is the whole wasm story — the guest shim is one macro invocation, gated to `wasm32`, and carries no logic:

```rust
//! Changelog source adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Adapter);
}

mod operations;
mod registry {
    adapter::registry!();
}

pub use operations::Adapter;
```

(For a target, the only difference is `adapter::target!(crate::Adapter)`.)

### 4. Implement the operations trait

`src/operations.rs` implements `adapter::Source` on a unit struct. Condensed from `intent` — the real file is worth reading in full:

```rust
use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceMetadata};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

/// Surveys a bound changelog tree into slice-sized leads.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Source for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "changelog",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> SourceMetadata {
        // Declare the minimum host that can run this adapter once it depends
        // on host behavior; first-party adapters set it on every train release.
        SourceMetadata { emery_floor: Some("0.34.0".to_string()) }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
        let system = registry::body("prompts/survey.md").to_string();
        let user = format!(
            "Survey the changelog source bound to adapter `{id}`. Resolve the binding \
             from `plan.yaml` under `sources.<key>`; its `path` names the bound tree. \
             Answer with one JSON object matching the gated schema: a `leads` array.",
            id = ctx.adapter_id,
        );
        repaired(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA, leads_tail).await
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, lead: &Lead,
    ) -> Result<Evidence, Error> {
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract Evidence for this lead:\n\n{lead}\n\nAnswer with one JSON object \
             matching the gated schema (Evidence body: `authority`, `claims`).",
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
```

Points that generalize:

- **Operations write no workflow artifacts.** The engine persists leads and Evidence; your job is to return well-formed values. Say so explicitly in the prompt ("the caller persists…; do not write it yourself") because the model has workspace access.
- **The binding is resolved by prompt, not by API.** The agent reads `plan.yaml` inside the lent workspace; `ctx.adapter_id` names which binding is yours.
- **`repaired` owns the parse-and-retry loop.** Pick the schema constant and tail matching the operation; the committed goldens live in the engine repo under `crates/project/answers/`.

### 5. Author the prose

Sources need two prompts: `prose/prompts/survey.md` and `prose/prompts/extract.md`. The shape rules are in [CONTRIBUTING.md § Prompt authoring](../CONTRIBUTING.md#prompt-authoring) — headline: parent prompts orchestrate and stay under ~150 non-blank lines, phase sub-prompts carry one phase (hard cap 800), and references are cited by relative link, never inlined.

Add the shared runtime references symlink so your prompts can cite the cross-adapter corpus:

```bash
ln -s ../../../../codex/references/runtime sources/changelog/prose/references/emery-runtime
```

The embed walker follows symlinks and fails the build on any dangling relative link, so broken prose is caught at `cargo build`, not at run time.

### 6. Test natively

`tests/operations.rs` drives the trait with a scripted model — no wasm, no network. The assertions worth making are "did my prompt content land in the assembled request" and "does the parsed answer round-trip":

```rust
use std::path::Path;

use adapter::Source as _;
use adapter::seam::Context;
use changelog::Adapter;
use omnia_testkit::model::Harness;

fn ctx() -> Context<'static> {
    Context {
        adapter_id: "source:changelog",
        project_root: Path::new("."),
        mcp_url: None,
    }
}

#[tokio::test]
async fn survey_prompts_and_parses() {
    let model = Harness::answering(
        [r#"{"leads":[{"lead":"release-notes","synopsis":"Publish release notes."}]}"#],
    );

    let leads = Adapter::survey(&model, &ctx()).await.unwrap();

    assert_eq!(leads.len(), 1);
    let request = &model.requests()[0];
    assert!(request.system.as_deref().unwrap().starts_with("# changelog.survey"));
}
```

Run with `cargo nextest run -p changelog` (never bare `cargo test` — see [testing.md](testing.md)). Also add a `tests/registry.rs` mirroring the existing adapters: it pins that every prompt path your operations load is actually embedded.

## Target adapters: what changes

The skeleton (steps 1–3) is identical apart from `adapter::target!`. The differences are in the trait and the prose tree:

- **`TargetMetadata` declares more.** `inputs: Vec<BuildInput>` names the working-tree paths the engine assembles into each build request (e.g. contracts declares `{ path: "contracts", required: false }`), and `platforms: Option<PlatformsCapability>` declares required/allowed/default platform sets (vectis) or `None` (contracts, omnia).
- **Three operations.** `guidance` returns a prompt string read by core synthesis. `build` receives the slice name, typed `Input` documents (proposal / design / tasks / spec), and a `WorkingTree`; resolve paths with `ctx.tree_root(tree)`. `merge` runs twice per slice: `MergePhase::Preflight` before the engine's deterministic commit (a failure blocks the merge), `Postflight` after it (a failure is reported but the merge stands).
- **You return a `Report`, and you should enforce it.** A `success` report with blocking findings is rejected engine-side, so the strong pattern (see `contracts`) is *validate-before-visible*: run your deterministic in-guest validator after the model answers and override the report with any residual blocking findings via `phase::enforce`. Model-facing phases go through `phase::phase` / `phase::report` rather than raw `repaired`.
- **Targets carry rules.** Engineering standards ship as `prose/rules/*.md` with stable rule IDs, embedded like any other prose and applied by your build review prompts. Cross-adapter rules live under `codex/rules/`; see [codex/rules/README.md](../codex/rules/README.md) for the namespace model.
- **Prompts per axis.** Targets need `prose/prompts/{guidance,build,merge}.md`; per-phase depth goes in `prose/prompts/build/<phase>.md` (or `build/<platform>/<phase>.md` for per-platform targets like vectis).

## Wire it into the dev harness

The eval composition ([`examples/eval/`](../examples/eval/)) links adapters statically, so a new adapter needs three edits before live eval can see it:

1. A workspace dependency alias in the root `Cargo.toml` (the block that already lists `intent`, `contracts`, …): `changelog = { path = "sources/changelog" }`.
2. A dependency in `examples/eval/Cargo.toml`: `changelog.workspace = true`.
3. A catalog line in `examples/eval/src/main.rs`: `.source::<changelog::Adapter>()` (or `.target::<…>()`).

How to exercise it live depends on the axis:

- **Target adapter** — add a build case: a data directory under `examples/eval/cases/<id>/` with a `case.toml` (`kind = "build"`, slice name, `expect` artifacts) and a `fixture/` carrying the exact refined state `slice build` consumes (`.emery/project.yaml`, the slice's `metadata.yaml`, proposal / design / tasks / specs, plus any source material). Anatomy and the `expect` gate: [examples/eval/README.md](../examples/eval/README.md#case-shapes). Run it with `cargo make eval <id> --restart`.
- **Source adapter** — build cases drive only the target build today, so exercise `survey` / `extract` live through a workflow case (`kind = "workflow"`) that binds your source, e.g. `notes = "changelog:notes"` under `[sources]` over a fixture tree.

Either way needs an authenticated `cursor-agent`. From here you are in the standard repair loop — edit `prose/**`, re-run, compare scratch trees — documented in the [repo README](../README.md).

## Build the component and use it in a project

```bash
cargo make adapter changelog     # fast dev build → target/wasm32-wasip2/release/changelog.wasm
```

Seed it into any Emery project (re-run after each rebuild):

```bash
emery adapter add target/wasm32-wasip2/release/changelog.wasm
```

The project then resolves the adapter by bare name (`changelog`) from its component cache. `cargo make wasm-contracts` / `cargo make wasm-omnia-r9k` exercise the real component seam end-to-end for the adapters they script. Publishing a pinned version to GHCR (`emery:changelog@<version>`) is the operator flow in [CONTRIBUTING.md § Publishing](../CONTRIBUTING.md#publishing).

## Definition of done

- [ ] Name is unique across `sources/` and `targets/`; axis matches the exported world.
- [ ] `src/lib.rs` carries no logic beyond the export macro, `registry!`, and re-exports; reusable logic is wasm-free library code.
- [ ] Every prompt path loaded by `operations.rs` exists under `prose/` (pinned by a `tests/registry.rs`), and prompt-shape caps are respected.
- [ ] Native `tests/` cover each operation with a scripted `Harness`; `cargo nextest run -p <name>` is green.
- [ ] Catalog entry in `examples/eval/src/main.rs`, exercised live at least once (target: a build case; source: a workflow case).
- [ ] `cargo make adapter <name>` builds the component; no `.wasm` artifacts committed.
- [ ] `cargo make ci` is green.
