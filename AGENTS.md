# Emery Adapters — Agent Instructions

Emery's first-party **source adapters**. Each `sources/<name>` is one crate shipping as one WebAssembly component that exports the `source-adapter` world of the `emery:adapter` WIT package: `metadata`, and `extract(SourceInput) -> Evidence` — a typed `SourceInput` (a key and a workspace or inline value) in, one Evidence document of typed claims out. The contract, vocabulary, and coding standards are the engine repository's ([`augentic/emery`](https://github.com/augentic/emery/blob/main/AGENTS.md), [`docs/standards/`](https://github.com/augentic/emery/tree/main/docs/standards)); this repository owns extraction behaviour and prose. The v1 tree is archived at git tag `v1`.

## Map

| Path | Role |
| --- | --- |
| `sources/<name>/` | One adapter: `Cargo.toml` (`cdylib` + `rlib`; `[dependencies]` is `emery-sdk` alone, no build dependencies); `build.rs` = `println!("cargo::rerun-if-changed=prose")`; `src/lib.rs` = the `wasm32`-only `mod guest { static DOCS: &[Doc] = emery_sdk::include_prose!("../prose"); struct Provider; impl Model for Provider {} impl Guest }` and `pub mod survey`; `src/survey.rs` = `pub fn survey`; `prose/{prompts/extract.md, references/, rules/}`; `tests/survey.rs` |
| `codex/references/runtime/` | Shared references, reached through each adapter's `prose/references/emery-runtime` symlink |
| `crates/test-programs/` | omnia's `test-programs` pattern: guest programs (`programs/source/extract.rs` drives the component boundary; `programs/probe/*` are fixture adapters) and the generated table of every built component |
| `tests/` | Root component suites over the built components under the omnia runtime: `source.rs`, `probe.rs`, `prose.rs`; the runner in `support/mod.rs` |
| `examples/` | The `emery.toml` configs the shipped `emery` binary runs over the release-built components, and the fixtures they lend ([examples/README.md](examples/README.md)) |

The root `emery-adapters` package is tests only. The one engine crate this workspace depends on, `emery-sdk`, is git-pinned in `[patch.crates-io]` beside `emery-prose`, which arrives beneath it and is patched only so both resolve from the same engine revision; the contract crate `emery-adapter` arrives beneath `emery-sdk` too, and neither is named by an adapter. Uncomment the path patches for sibling co-development and never commit them.

## Invariants

- An adapter is a guest of the `source-adapter` world, written the way every omnia guest is: a `mod guest` at the top of `src/lib.rs` implements `emery_sdk::export::Guest` on a unit type for `wasm32` alone. Its `metadata` answers `emery_sdk::metadata(SourceKind::..)` — the kind of source the adapter reads, which the engine ranks the source by before any extract, and no answer carries it; its `extract` is one call, `emery_sdk::extract(&Provider, id, input, DOCS, async |model, ctx| ..)`, whose closure calls the crate's `survey` for the seams — the SDK lifts the input, builds the `Context`, lends `model` to the survey, and hands the seams to `emery_sdk::mine` over it — nothing else, so every model call goes through the SDK: one per seam, joined into the source's one document. `Provider` is the guest's capabilities on the WASI defaults — a unit struct with an empty `impl Model`, as every omnia guest declares — and the one place an adapter names a backend; everything beneath is generic over `P: Model`, so nothing in `src/survey.rs` is `cfg`-gated. A survey lists or reads its input (`emery_sdk::survey` walks a tree with the engine's skip roots pruned), cuts it mechanically by top-level directory under a grain floor (`survey::by_directory`) or asks the model once, under the adapter's `prompts/survey.md`, to group the files by what they serve (`survey::by_model`) — never more, and never to mine; chooses each seam (`Seam::Whole`, `Files(files)` lending one directory, or `Note(note)` lending the root); and refuses unusable input with `bad_request!` before the model is reached. An inline value, and a tree that cuts no finer than itself, are one `Whole` seam with no survey turn spent. A survey that asks nothing is a plain fn over the `Context`, passed as `async |_, ctx| survey::survey(ctx)`; one that asks the model is `async fn survey<P: Model>(model, ctx, docs)` on every target, passed as `async |model, ctx| survey::survey(model, ctx, DOCS).await`, so tests bind `Scripted` in the slot the guest binds `Provider`. The guest embeds the whole `prose/` tree as `static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");` — named relative to `src/lib.rs`, the way `include_str!` names a file — and hands it to `mine` and to a survey by model; nothing native reads the table, so it lives inside `mod guest`. There is no adapter error type: `emery_sdk::Error` is omnia's, and the WIT `error` variant is lowered and lifted inside the contract crate alone.
- Identity is the crate's version and its `emery:<name>@<semver>` package; resolve-time metadata is the component's `metadata` export. There is no manifest file.
- Adapters never read the revision store or take lifecycle authority. Preserve gaps as `[unknown]`; keep claims platform-neutral.
- `prose/prompts/extract.md` is the one extraction pass: at most 800 non-blank lines, with a `## Worked example` JSON fence that parses as the SDK's `Evidence` (claims alone — the kind of source is the adapter's metadata, never the answer's) and passes the claim gate — `tests/prose.rs` enforces both. An adapter that surveys by model embeds `prose/prompts/survey.md` too, under the same cap, with a `## Worked example` that parses as `survey::Partition`. References are linked, never inlined; a dangling relative link fails the build at the `include_prose!` line. Contributor guidance never goes in the embedded corpus.
- Do not commit built `.wasm` artifacts.

## Code style

The engine repository's [style.md](https://github.com/augentic/emery/blob/main/docs/standards/style.md) and [coding-standards.md](https://github.com/augentic/emery/blob/main/docs/standards/coding-standards.md), under the shared `[workspace.lints]` in `Cargo.toml` and the guest deny-list in `clippy.toml`. Short names that lean on the module path. Doc comments are written for the crate's user and state the contract, never the mechanics: one summary sentence of about fifteen words (a verb sentence for a fn, a noun phrase for a type, what the module provides for a `//!`), a blank line, then short plain sentences and bullet lists, with `# Examples` / `# Errors` / `# Panics` as the canonical sections, `# Errors` naming each linked class the caller matches on, and every mentioned item an intra-doc link — the engine's [coding-standards.md § Comments](https://github.com/augentic/emery/blob/main/docs/standards/coding-standards.md#comments) is the full rule. A test fn names the scenario (`one_file`, `empty_brief`), never the outcome, and the `//` comment above it carries the why. Inside a fn body, a `//` comment is either a section header — a lowercase fragment with no full stop above each blank-line-separated block, naming what the block achieves so the headers read as the fn's outline (`// load source adapters`) — or a why: a capitalised sentence beside the one line that would otherwise surprise. Neither restates the line beneath it, and a fn readable at a glance carries no headers. A fn's result comes back through its return value, never through a `&mut` argument it fills: a recursive walk returns its part and the caller `extend`s. Formatting is nightly rustfmt (`make fmt`).

## Testing

- `sources/<name>/tests/survey.rs`: what the adapter itself decides — its survey (how a tree cuts, what it leaves out, when it stays whole) and its refusals, asserted on `<name>::survey` natively: as plain `#[test]`s with no model for a mechanical survey; over `omnia_test::guest::Scripted` for one that asks the model — a scripted partition, with the candidates it offered and the notes it cut, and `Scripted::default()` where no turn is spent. Never pin prompt phrases; prompt quality is the live eval's.
- Root `tests/`: the component boundary only, for every shipped component. `foreach_adapter!` and `foreach_probe!` make a new adapter or probe a compile error until `source.rs`, `prose.rs`, or `probe.rs` names it; such a test carries its adapter's or program's name (`intent`, `probe_echo`).
- What the SDK does for every adapter (the request shape, the reference tools, the lend, the claim gate's repair and spent-rounds refusal) is asserted once in the SDK's own suite and once under the runtime over the `gated` probe — never per adapter. The host property the SDK's fan-out rests on — the completions one guest issues together are pending together — is guarded once, over the `fanout` probe behind the support's `Barrier` model.
- Always `cargo nextest`, and always `--workspace` from the root: a bare root run selects the root package alone and skips every adapter's suite.
- The guest side (`crates/test-programs/programs/`, the adapters' `guest` modules) is `cfg(target_arch = "wasm32")`, so `make lint` does not see it; lint it with the clippy command below.

Placement rules: [docs/testing.md](docs/testing.md). Creating an adapter: [docs/authoring.md](docs/authoring.md). Toolchain and publishing: [CONTRIBUTING.md](CONTRIBUTING.md).

## Commands

All from the repository root through `make` ([`Makefile`](Makefile) → mise):

```bash
make ci                              # check + vet + deny — run before committing
make check                           # fmt + lint + test + test-docs + doc
make test                            # cargo nextest run --locked --workspace --all-features
cargo nextest run -p <name>          # one adapter's survey suite
cargo nextest run -p emery-adapters  # the root component suites
cargo clippy --workspace --exclude emery-adapters --lib --examples --target wasm32-wasip2 -- -D warnings   # the guest side
cargo build -p <name> --target wasm32-wasip2 --release   # one component
cargo build --workspace --target wasm32-wasip2 --release   # every component
make publish <name>                  # push one built component to its GHCR tag
make sweep                           # drop target/ artifacts untouched for a week
```

If `make ci` cannot run, say exactly which narrower checks ran and why.
