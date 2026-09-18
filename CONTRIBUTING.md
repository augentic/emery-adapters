# Contributing to emery-adapters

Human-facing contributor guide (toolchain, layout, prompts, pin, publishing). Creating an adapter end-to-end is [`docs/authoring.md`](docs/authoring.md); agent and contract rules live in [`AGENTS.md`](AGENTS.md); test ownership in [`docs/testing.md`](docs/testing.md).

## Getting started

1. Clone this repository. Until an engine release carries the extract-only SDK, the engine crates (`emery-sdk`, and `emery-prose` beneath it) resolve through the `[patch.crates-io]` git patches in the root `Cargo.toml` (see [Engine pin and sibling co-development](#engine-pin-and-sibling-co-development)); once that release exists the pin moves to its tag (`tag = "vX.Y.Z"`, RFC-77 D13). A sibling `../emery` checkout is needed only for co-development (uncomment the path patches) and the live eval (it drives that repo's built `emery` binary).
2. `rustup` picks up the pinned **stable** toolchain from `rust-toolchain.toml` (including the `wasm32-wasip2` target); a nightly toolchain is additionally needed for the `fmt` arm (`cargo +nightly fmt`). The first `make` installs [mise](https://mise.jdx.dev) if it is missing. Also install `cargo-nextest`, `cargo-deny`, and `cargo-vet`. Publishing also uses `wkg`.
3. Run `make check` from the repo root. Before opening a PR, run `make ci`.

For the adapter SDK's type-level contract (`mine`, the `survey` helpers, the contract types, the answer schemas), generate the docs locally: `cargo doc -p emery-sdk --open`; the `export` module — the world an adapter's guest implements — documents under `--target wasm32-wasip2`.

Unless you are fixing a known bug, discuss larger changes in a GitHub issue first. Legal / DCO expectations match the engine repo — see [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md).

### Troubleshooting first runs

- **`make fmt` fails** — the fmt arm shells out to `cargo +nightly fmt`; install any nightly toolchain (`rustup toolchain install nightly --component rustfmt`).
- **The first `make test` is slow** — `crates/test-programs/build.rs` nested-builds every adapter component and every guest program for `wasm32-wasip2` under `OUT_DIR` before the native suites compile; later builds are incremental.
- **Patch-resolution errors after editing the root `Cargo.toml`** — the committed `[patch.crates-io]` git patches fetch `augentic/emery`; the commented path patches only resolve when `../emery` exists. Do not commit active path patches: CI has no sibling checkout.

## Layout

Every source adapter shares the same guest anatomy:

```text
sources/
  <name>/             # documentation, intent, typescript
    prose/            # agent-facing markdown (listed in src/lib.rs, embedded into the component)
      extract.md      # the one extraction pass; survey.md beside it for an adapter that surveys by model
      references/     # lazy reference corpus; the shared runtime references are the SDK's, linked as <doc>.md
    Cargo.toml        # `<name>` — adapter identity semver is its `version`
    src/              # lib.rs (PROSE, then the wasm32-only `mod survey` and `mod guest`) + survey.rs, the guest's survey
crates/test-programs/ # omnia's test-programs pattern: guest programs + the nested wasm32 build of every component
  programs/<group>/   # one scenario per file: source/extract.rs drives the component boundary, probe/ are fixture adapters
  src/                # lib.rs: the generated artifact table (native) / helpers.rs (wasm32)
  build.rs            # one omnia_test::build::Components build → gen.rs (every adapter + every program)
tests/                # root component suites: source.rs (every shipped component, and what each adapter decides), probe.rs (the error arms, the lowering, the SDK's side of the boundary), prose.rs (every adapter's corpus)
  support/            # mod.rs — the one runner source.rs and probe.rs share (the deployment under the omnia runtime)
examples/             # live walks: one emery.toml per adapter (plus one over all three) the shipped `emery` binary runs, and the fixtures they lend
Cargo.toml            # the tests `emery-adapters` root package over crates/* + sources/*
```

Identity lives in the guest crate's `Cargo.toml` `version` (the shared `[workspace.package]` SemVer) and the package reference it publishes under (`emery:<name>@<semver>`). The compatibility floor is compiled into the `metadata` operation's record.

## Prompt authoring

Adapter prompts are markdown documents compiled into the guest and driven by the engine's `extract` dispatch. They are not skills: no YAML frontmatter, no discovery metadata.

- **`prose/extract.md`** carries the whole extraction pass: the claim-kind table with each kind's required body field (the `emery_sdk::Evidence::findings` gate, run as the check on each seam's turn inside the SDK's `extract` so the backend corrects a miss in place, and fail-closed engine-side, A8), the id-derivation rules reconciliation joins on, and the JSON output contract. Soft cap ~500 non-blank lines, hard cap 800 — above that, move material to `prose/references/`.
- **`prose/survey.md`**, for an adapter that surveys by model, is the system prompt of its one survey call: what a surface is for this source and where a caller enters it, what is not one — the modules behind a surface, which the extract call follows — and the `surfaces` answer, each a `name` and its `entry` — never a claim, and never a grouping. Same caps; its `## Worked example` must parse as `emery_sdk::survey::Inventory`.
- **References are cited via relative markdown links, never inlined** — the model reads a reference through `read_doc`, which answers from the adapter's `PROSE` and then from the SDK's runtime references (`emery_sdk::prose::RUNTIME`: `claims.md` for the claim `id` grammar, `path` anchors, the skip roots, and the fail-closed gate; `reconciliation.md` for the `specify` pipeline), so every relative link must name a document listed in `PROSE` or one of those two (`claims.md` from a prompt at the `prose/` root), and every listed document must be reached from a prompt by such a link (the root `tests/prose.rs` holds the list to the `prose/` tree and refuses a link to a directory, an unlisted file, or a path outside the tree, a listed document no prompt reaches, and a listed document at a runtime reference's path). Link a rule the runtime references state rather than restating it; the references ship with the SDK the adapter compiles against, so they move with the contract they describe.
- **A reference is written for the model, in the adapter's current contract.** It says what to read in the source and which claims to emit, in the prompt's vocabulary — the surface, the entry, the claim kinds and their extras — and nothing addressed to a contributor: no retrospectives, no tooling proposals, no description of what the engine renders downstream. Prose the prompt would contradict is worse than none.
- **Worked examples live under `prose/references/examples/`**, one document per scenario beside a `README.md` index: the surface as the survey names it, the source the extract call reads, and the Evidence the call answers with under a `## Evidence` JSON fence — claims alone, like the prompt's own worked example. The root `tests/prose.rs` parses every such fence as the SDK's `Evidence` and holds it to the claim gate, so an example cannot teach a shape the adapter would refuse.
- The v1 survey prompts were deleted, never ported (ADR-0008); the survey a model makes today chooses a cut and mines nothing.

## Engine pin and sibling co-development

Two compatibility choices are independent:

1. **WIT contract version** — the `emery:adapter` WIT package, embedded in the `emery-adapter` contract crate (which the `emery-sdk` SDK re-exports) and published from `augentic/emery`'s `wit/emery.wit`.
2. **Engine revision** — the workspace resolves `emery-prose` and `emery-sdk` on `augentic/emery`, pinned by **release tag** (`tag = "vX.Y.Z"` in the root `Cargo.toml`; RFC-77 D13) plus the committed `Cargo.lock`; the contract crate `emery-adapter` rides beneath `emery-sdk` at the same revision. Advancing the pin is deliberate: bump the tag on both dependencies, run `cargo update -p emery-prose -p emery-sdk`, and commit both files — never resolve a floating branch.

For sibling co-development against uncommitted engine changes, uncomment the path patches in the root `Cargo.toml` `[patch.crates-io]` block (they point at `../emery`); they must never be active on the committed tree or at publish time. **Current state**: git patches are active — the extract-only SDK is not yet on a tagged engine release, so the tag pin (and with it the first adapter train publish) waits on that release cut.

## Local development loops

```bash
make check                 # fmt + lint + nextest + doctests + doc
make ci                    # full gate — adds cargo-vet + cargo-deny
cargo clippy --workspace --exclude emery-adapters --lib --examples --target wasm32-wasip2 -- -D warnings   # the guest side alone
cargo build -p <name> --target wasm32-wasip2 --release   # one adapter → target/wasm32-wasip2/release/<name>.wasm (the path the examples bind)
cargo build --workspace --target wasm32-wasip2 --release   # every adapter
```

The `fmt` arm uses nightly `rustfmt`. `make lint` runs clippy under `-D warnings` twice: over the native side, then — since the guest side, the programs under `crates/test-programs/programs/` and the adapters' `survey` and `guest` modules, is `cfg(target_arch = "wasm32")` and native clippy compiles it to nothing — for `wasm32-wasip2`, the target it ships on, where `clippy.toml`'s guest deny-list applies (the second command above on its own). `make vet` is check-only; regenerate audit inputs with `make vetgen`. The component suites are the Rust inner loop and prove every built component under the omnia runtime, each adapter's own decisions included; `emery specify --config examples/<name>/emery.toml` walks one adapter live through the shipped `emery` binary and the Cursor backend ([examples/README.md](examples/README.md)); the graded live eval — being recreated as a root example beside the live examples — proves prompt quality end to end and writes the dated scorecard.

## Publishing

The first-party adapter train releases from durable `release-X.Y.Z` branches with the same verbs as the engine repo (RFC-77): dispatch **Create Release** on `main` to cut `release-X.Y.Z` (it also opens the bump-`main` PR), stabilize and backport on the branch, dispatch **Publish Release** on the branch (tag, GitHub Release, GHCR packages), and dispatch **Create Patch** on the same branch for `X.Y.Z → X.Y.Z+1`. The train version is the shared `[workspace.package]` SemVer; `RELEASES.md` carries the line's notes, including a compatibility row (`engine X.Y.x ↔ adapters A.B.x (WIT emery:adapter@…, floor ≥ …)`).

Before a train publishes, these gates must hold:

1. The tree builds against a **published** `emery:adapter` WIT pin.
2. CI is green against a **released (or RC)** engine revision — the engine dependencies are tag-pinned (`tag = "vX.Y.Z"`), with no active sibling `[patch]` block.
3. Every adapter's `emery-version` names the minimum host that can run this train.
4. Releasing a new SemVer: the GHCR version tag must not already exist for a first-time push of that train.

**Publish Release** runs CI, tags and creates the GitHub Release, then release-builds every adapter and pushes each as a Wasm OCI artifact to `ghcr.io/augentic/emery-adapters/<name>:<version>` via the same build and `make publish <name>` path used locally. The helper derives `<version>` from the workspace manifest.

A brand-new package is created **private**: flip it to public in the GHCR package settings (`https://github.com/orgs/augentic/packages/container/emery-adapters%2F<name>/settings`) so anonymous consumers can pull, then confirm the round-trip:

```bash
wkg oci pull ghcr.io/augentic/emery-adapters/<name>:<version> --output /tmp/<name>.wasm
```

Local breakout (retry a single adapter after GHCR login):

```bash
gh auth token | docker login ghcr.io -u <github-user> --password-stdin
cargo build --workspace --target wasm32-wasip2 --release
make publish <name>
```

## Before you open a PR

1. Branch off `main`.
2. Run `make ci` (or say exactly which narrower checks ran and why the full gate was unavailable).
3. Read [docs/testing.md](docs/testing.md) before adding, deleting, or relocating tests. A behavior the adapter itself decides goes in the root `tests/source.rs`, asserted through the built component; the component boundary is `tests/probe.rs`'s; do not add a `src` `#[cfg(test)]` module without a one-line reason from that document, never pin a prompt phrase, and never widen `pub` surface — or compile a module natively — solely for a test.
4. Do not commit built `.wasm` artifacts.

## See also

- [docs/authoring.md](docs/authoring.md) — creating a source adapter
- [AGENTS.md](AGENTS.md) — vocabulary, component contract, agent commands
- [docs/testing.md](docs/testing.md) — test ownership
- [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md) — DCO and org contribution norms
