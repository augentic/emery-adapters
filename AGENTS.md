# Emery Adapters — Agent Instructions

Emery's first-party **source adapters**. Each `sources/<name>` is one crate shipping as one WebAssembly component that exports the `source-adapter` world of the `emery:adapter` WIT package: `metadata`, and `extract(SourceInput) -> Evidence` — a typed `SourceInput` (a key and a workspace or inline value) in, one Evidence document of typed claims out. The contract, vocabulary, and coding standards are the engine repository's ([`augentic/emery`](https://github.com/augentic/emery/blob/main/AGENTS.md), [`docs/standards/`](https://github.com/augentic/emery/tree/main/docs/standards)); this repository owns extraction behaviour and prose. The v1 tree is archived at git tag `v1`.

## Map

| Path | Role |
| --- | --- |
| `sources/<name>/` | One adapter: `Cargo.toml` (`cdylib` + `rlib`); `build.rs` = `emery_prose::emit("prose")`; `src/lib.rs` = `emery_sdk::source!(crate::Adapter)` + `registry!`; `src/operations.rs` = `impl SourceAdapter`; `prose/{prompts/extract.md, references/, rules/}`; `tests/extract.rs` |
| `codex/references/runtime/` | Shared references, reached through each adapter's `prose/references/emery-runtime` symlink |
| `crates/test-programs/` | omnia's `test-programs` pattern: guest programs (`programs/source/extract.rs` drives the seam; `programs/probe/*` are fixture adapters) and the generated table of every built component |
| `tests/` | Root seam suites over the built components under the omnia runtime: `source.rs`, `probe.rs`, `prose.rs`; the runner in `support/mod.rs` |
| `examples/` | Root-package `[[example]]` cdylibs (`documentation`, `intent`, `typescript`) plus the `emery.toml` configs the shipped `emery` binary runs ([examples/README.md](examples/README.md)) |

The root `emery-adapters` package is tests only. Engine crates (`emery-adapter`, `emery-prose`, `emery-sdk`) are git-pinned in `[patch.crates-io]`; uncomment the path patches for sibling co-development and never commit them.

## Invariants

- An adapter implements `emery_sdk::SourceAdapter` on a unit type and makes exactly one model call, through `Self::evidence(model, ctx, material)`. It chooses the material (`Material::Bound`, or `Prepared(note)` after reading or validating its input) and refuses unusable input with `bad_request!` before the model is reached. There is no adapter error type: `emery_sdk::Error` is omnia's, and the WIT `error` variant is lowered and lifted inside the contract crate alone.
- Identity is the crate's version and its `emery:<name>@<semver>` package; resolve-time metadata is the component's `metadata` export. There is no manifest file.
- Adapters never read the revision store or take lifecycle authority. Preserve gaps as `[unknown]`; keep claims platform-neutral.
- `prose/prompts/extract.md` is the one extraction pass: at most 800 non-blank lines, with a `## Worked example` JSON fence that passes the claim gate under the adapter's authority — `tests/prose.rs` enforces both. References are linked, never inlined; a dangling relative link fails the build. Contributor guidance never goes in the embedded corpus.
- Do not commit built `.wasm` artifacts.

## Code style

The engine repository's [style.md](https://github.com/augentic/emery/blob/main/docs/standards/style.md) and [coding-standards.md](https://github.com/augentic/emery/blob/main/docs/standards/coding-standards.md), under the shared `[workspace.lints]` in `Cargo.toml` and the guest deny-list in `clippy.toml`. Short names that lean on the module path; comments say what and why, never how; a test fn names the scenario (`one_file`, `empty_brief`), never the outcome, and the `//` comment above it carries the why. Formatting is nightly rustfmt (`make fmt`).

## Testing

- `sources/<name>/tests/extract.rs`: what the adapter itself decides — its material, its refusals — natively over `omnia_test::guest::Scripted`. Never pin prompt phrases; prompt quality is the live eval's.
- Root `tests/`: the component boundary only, for every shipped component. `foreach_adapter!` and `foreach_probe!` make a new adapter or probe a compile error until `source.rs`, `prose.rs`, or `probe.rs` names it; such a test carries its adapter's or program's name (`intent`, `probe_echo`).
- What the SDK does for every adapter (the request shape, the reference tools, the lend, the claim gate's repair and spent-rounds refusal) is asserted once in the SDK's own suite and once under the runtime over the `gated` probe — never per adapter.
- Always `cargo nextest`, and always `--workspace` from the root: a bare root run selects the root package alone and skips every adapter's suite.
- The guest side (`crates/test-programs/programs/`, the adapters' export shims) is `cfg(target_arch = "wasm32")`, so `make lint` does not see it; lint it with the clippy command below.

Placement rules: [docs/testing.md](docs/testing.md). Creating an adapter: [docs/authoring.md](docs/authoring.md). Toolchain and publishing: [CONTRIBUTING.md](CONTRIBUTING.md).

## Commands

All from the repository root through `make` ([`Makefile`](Makefile) → mise):

```bash
make ci                              # check + vet + deny — run before committing
make check                           # fmt + lint + test + test-docs + doc
make test                            # cargo nextest run --locked --workspace --all-features
cargo nextest run -p <name>          # one adapter's extract suite
cargo nextest run -p emery-adapters  # the root seam suites
cargo clippy --workspace --exclude emery-adapters --lib --examples --target wasm32-wasip2 -- -D warnings   # the guest side
cargo build -p <name> --target wasm32-wasip2 --release   # one component
make release                         # every component → target/wasm32-wasip2/release/
make publish <name>                  # push one built component to its GHCR tag
make sweep                           # drop target/ artifacts untouched for a week
```

If `make ci` cannot run, say exactly which narrower checks ran and why.
