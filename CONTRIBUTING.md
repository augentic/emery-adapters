# Contributing to emery-adapters

This guide covers contributor setup, repository layout, prompt authoring rules, engine version pinning, and release publishing for `emery-adapters`. For coding conventions, architectural invariants, and test ownership rules, see [`AGENTS.md`](AGENTS.md).

## Contents

- [Getting started](#getting-started)
  - [Troubleshooting first runs](#troubleshooting-first-runs)
- [Layout](#layout)
  - [Source adapters](#source-adapters)
  - [Target adapters](#target-adapters)
  - [Test harness, examples, and evals](#test-harness-examples-and-evals)
- [Prompt authoring](#prompt-authoring)
  - [Extraction and survey prompts](#extraction-and-survey-prompts)
  - [Build and verify prompts](#build-and-verify-prompts)
  - [Reference documents and worked examples](#reference-documents-and-worked-examples)
- [Engine pin and sibling co-development](#engine-pin-and-sibling-co-development)
- [Local development loops](#local-development-loops)
- [Publishing](#publishing)
- [Developer's Certificate of Origin](#developers-certificate-of-origin)
- [Pull request procedure](#pull-request-procedure)
- [Conduct](#conduct)
- [See also](#see-also)

---

## Getting started

Unless you are fixing a small, well-defined bug, please open a GitHub issue to discuss larger changes before starting work. Every commit must include a [Developer's Certificate of Origin](#developers-certificate-of-origin) sign-off (`git commit -s`).

1. **Clone the repository:** Clone `augentic/emery-adapters`. The engine dependencies (`emery-sdk` and `emery-prose`) resolve via `[patch.crates-io]` in the root `Cargo.toml` (see [Engine pin and sibling co-development](#engine-pin-and-sibling-co-development)). You only need a sibling `../emery` checkout when co-developing against uncommitted engine changes or when running live examples and evaluations.
2. **Install toolchains and `mise`:** `rustup` automatically selects the pinned stable toolchain and `wasm32-wasip2` target from `rust-toolchain.toml`. Also install a nightly rustfmt toolchain (`rustup toolchain install nightly --component rustfmt`) and [mise](https://mise.jdx.dev). The root `Makefile` delegates all tasks to [`mise.toml`](mise.toml), which installs Cargo tools (`cargo-nextest`, `cargo-hack`, `cargo-deny`, `cargo-vet`) on first use. Publishing also requires `wkg`.
3. **Run the CI suite:** Run `make ci` from the repository root before opening a pull request. Use `make check` for local advisories (`audit`, `fmt` in-place, `lint`, `outdated`, and `deps`).

To browse the adapter SDK API locally, build the Rust documentation:

```bash
cargo doc -p emery-sdk --open
cargo doc -p emery-sdk --target wasm32-wasip2 --open   # includes wasm32 guest export macros and bindings
```

### Troubleshooting first runs

- **`make fmt` fails:** The formatting task runs `cargo +nightly fmt`. Install nightly rustfmt with `rustup toolchain install nightly --component rustfmt`.
- **The first `make test` is slow:** `crates/test-programs/build.rs` compiles every adapter component and guest test program for `wasm32-wasip2` into `OUT_DIR` before running the native test suites. Subsequent runs are incremental.
- **Patch resolution errors after editing `Cargo.toml`:** The committed `[patch.crates-io]` entries fetch `augentic/emery` from Git, while the commented `path = "../emery/..."` lines require a sibling `../emery` checkout. Never commit active `path` patches, because CI runs without a sibling checkout.

---

## Layout

All adapters share a consistent crate structure and publish under the workspace's shared `[workspace.package]` version as `emery:<kind>@<version>`.

### Source adapters

Located under `adapters/{documentation,intent,typescript,python}` (crate name `emery-<kind>`):

- `src/lib.rs`: Declares `mod survey`, `pub struct Adapter` implementing `emery_sdk::SourceAdapter` (`metadata` and `extract`), `pub static PROSE` embedding all files under `prose/`, and `emery_sdk::export_source!(Adapter)` (gated on `cfg(target_arch = "wasm32")`).
- `src/survey.rs` and `src/survey/*`: Inspects the input and splits it into extraction `Seam`s. Code adapters (`typescript` and `python`) parse files into `emery_sdk::survey::code::Tree` using a pure-Rust parser (`oxc` or `ruff_python_parser`), implement the SDK's `Recogniser` trait in `survey/surface.rs`, and call `emery_sdk::survey::seams`. The `documentation` adapter parses Markdown outlines and calls `emery_sdk::survey::surfaces`.
- `prose/`: Embedded Markdown prompts (`extract.md`, optional `survey.md`, and `references/`).

### Target adapters

Located under `adapters/{node,omnia}` (crate name `emery-<kind>`):

- `src/lib.rs`: Declares merge rules (`RULES`), worker sandbox grants (`Grant`), `pub struct Adapter` implementing `emery_sdk::target::TargetAdapter` (`metadata` and `run` over `Task::Init`, `Task::Build`, and `Task::Verify`), `pub static PROSE`, and `emery_sdk::export_target!(Adapter)` on `wasm32`.
- Scaffolding and syntax checks (`omnia`): `adapters/omnia` scaffolds the workspace template from [`omnia-template`](https://github.com/augentic/omnia-exemplar) during `Task::Init`, generates per-slice Rust skeletons (`src/scaffold.rs`) before `Task::Build`, and validates the resulting syntax tree with `syn` (`src/tree.rs`) on every build and verify turn.
- `prose/`: Embedded Markdown prompts (`build.md`, `verify.md`, and `references/`).

### Test harness, examples, and evals

- `crates/test-programs/`: Builds all Wasm components and fixture probes (`programs/probe/*`, `programs/source/extract.rs`, `programs/builder/build.rs`) for component boundary tests.
- `tests/`: Root integration test suites:
  - `source.rs` and `target.rs` test each adapter's native Rust implementation over scripted model doubles.
  - `component.rs` runs every compiled `.wasm` component once inside the Omnia runtime.
  - `probe.rs` tests WIT boundary lifting, lowering, and SDK gates.
  - `prose.rs` validates every adapter's `PROSE` table, links, line caps, and JSON worked examples.
- `examples/`: Live `emery.toml` configurations and fixtures for manual end-to-end testing ([examples/README.md](examples/README.md)).
- `evals/`: Graded live evaluation harness and benchmark cases ([evals/README.md](evals/README.md)).

---

## Prompt authoring

Adapter prompts are Markdown documents compiled into the Wasm component via `emery_sdk::prose![..]` and validated automatically by `tests/prose.rs`. They do not use YAML frontmatter.

### Extraction and survey prompts

- **`prose/extract.md`:** Defines the single-pass extraction instructions, claim kinds (`requirement`, `criterion`, `decision`, `retire`, `type`, `contract`, `call`, `excerpt`), ID naming conventions, and a `## Worked example` JSON block that must parse as valid `emery_sdk::Evidence`, pass `Evidence::findings`, and contain zero technology names from `emery_sdk::Lexicon` in requirement, criterion, or retire claims.
- **`prose/survey.md`:** Included only in adapters where the model identifies surfaces from pre-parsed facts (`typescript`, `python`, and `documentation`). Must include a `## Worked example` JSON block that parses as `emery_sdk::survey::Inventory` with valid anchors and `kebab-case` stems.
- **Line budget:** Keep prompts under 500 non-blank lines where possible; `tests/prose.rs` enforces a hard maximum of 800 non-blank lines per prompt. Move detailed framework or language guidance into `prose/references/`.

### Build and verify prompts

- **`prose/build.md` and `prose/verify.md`:** Required for every target adapter (`node` and `omnia`), each capped at 800 non-blank lines.
- **Worked examples:** `build.md` must carry a `## Worked example` JSON block that parses as `emery_sdk::target::Report` (with valid root-relative `written` paths), and `verify.md` must carry one that parses as `emery_sdk::target::Verdict` (passing `Verdict::findings`).

### Reference documents and worked examples

- **Link, never inline:** Prompts reference supplementary docs using relative Markdown links (for example, `[layout.md](references/layout.md)` or `[claims.md](claims.md)`). During a turn, the model reads linked files on demand through the `list_docs` and `read_doc` tools.
- **Strict link checking:** `tests/prose.rs` verifies through `emery_sdk::check` that every file in `prose/` is listed once in `PROSE`, every relative link resolves to a listed document or an SDK runtime reference (`claims.md`, `reconciliation.md`), and every document in `PROSE` is reachable from a prompt.
- **Model-only audience:** Write reference files strictly as instructions for the model in the adapter's current contract. Never include contributor notes, historical retrospectives, or engine implementation details in `prose/`.
- **Source worked examples (`prose/references/examples/*.md`):** Every file under `prose/references/examples/` (except `README.md`) must include a `## Evidence` JSON block that parses as `Evidence` and passes the claim gate.

---

## Engine pin and sibling co-development

Compatibility with [`augentic/emery`](https://github.com/augentic/emery) is governed by two pins:

1. **WIT contract (`emery:adapter`):** Defined in `augentic/emery`'s `wit/emery.wit` and embedded in `emery-adapter` (re-exported by `emery-sdk`).
2. **Engine crate revision:** The root `Cargo.toml` pins `emery-sdk` and `emery-prose` to a specific `augentic/emery` revision (along with `Cargo.lock`). To advance the pin, update the revision or tag in `Cargo.toml`, run `cargo update -p emery-prose -p emery-sdk`, and update `supply-chain/` via `make vet-regen`.

When co-developing against uncommitted changes in a sibling `../emery` checkout, uncomment the `path = "../emery/..."` lines in `[patch.crates-io]` in the root `Cargo.toml`. Always re-comment the `path` overrides before committing.

---

## Local development loops

```bash
# Run the full CI gate (fmt-check, host + wasm32 clippy, nextest, doctests, docs, cargo-vet, cargo-deny)
make ci

# Run local advisory checks (cargo-audit, rustfmt in-place, clippy, outdated, unused deps)
make check

# Lint the wasm32 guest code alone (matches the first step of make lint-wasm)
cargo clippy -p emery-documentation -p emery-intent -p emery-typescript -p emery-python -p emery-node -p emery-omnia -p test-programs --lib --bins --examples --all-features --target wasm32-wasip2 -- -D warnings

# Build a single Wasm adapter component (outputs target/wasm32-wasip2/release/emery_<kind>.wasm)
cargo build -p emery-<kind> --target wasm32-wasip2 --release

# Build all Wasm adapter components
cargo build --workspace --target wasm32-wasip2 --release
```

Why `make lint` runs two passes:

- **`lint-host`:** Runs `cargo clippy --workspace --all-targets --all-features` and `cargo hack --each-feature` on the host target.
- **`lint-wasm`:** Because the Wasm guest entry points (`export_source!`, `export_target!`, and `crates/test-programs/programs/`) are gated on `#[cfg(target_arch = "wasm32")]`, `lint-wasm` compiles all packages in `WASM32_PACKAGES` (`mise.toml`) for `wasm32-wasip2` and enforces the guest deny-list in [`clippy.toml`](clippy.toml).

---

## Publishing

First-party adapters release from `release-X.Y.Z` branches using the GitHub Actions release workflows:

1. **Create Release:** Run on `main` to cut `release-X.Y.Z` and open a version-bump PR back to `main`.
2. **Publish Release:** Run on `release-X.Y.Z` to verify CI, create the Git tag and GitHub Release, build all Wasm components in release mode, and push each component as an OCI artifact to `ghcr.io/augentic/emery/<name>:<version>`.
3. **Create Patch:** Run on `release-X.Y.Z` to prepare `X.Y.Z+1` backports.

Pre-publish checklist:

1. `Cargo.toml` builds against a released `augentic/emery` tag with no local `path` patches active.
2. `RELEASES.md` includes the release notes and engine compatibility row (`engine X.Y.x ↔ adapters A.B.x`).
3. Every adapter's `emery-version` metadata floor matches the minimum compatible `emery` binary version.
4. The target `<version>` tag does not already exist on GHCR.

When a brand-new adapter package is published to GHCR for the first time, GitHub creates the package with **private** visibility. An organization admin must switch the package visibility to **public** at `https://github.com/orgs/augentic/packages/container/emery%2F<name>/settings` and verify anonymous pulls:

```bash
wkg oci pull ghcr.io/augentic/emery/<name>:<version> --output /tmp/<name>.wasm
mkdir -p /tmp/store && wkg get emery:<name>@<version> -o /tmp/store/
```

To publish or retry a single adapter manually from a local machine:

```bash
gh auth token | docker login ghcr.io -u <github-user> --password-stdin
cargo build --workspace --target wasm32-wasip2 --release
make publish <name>
```

---

## Developer's Certificate of Origin

All contributions must include acceptance of the [Developer Certificate of Origin (DCO)](https://developercertificate.org/):

```text
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.
660 York Street, Suite 102,
San Francisco, CA 94110 USA

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

To accept the DCO, sign off each commit (`git commit -s`) with your real name and email address:

```text
Signed-off-by: Jane Example <jane@example.com>
```

For legal reasons, anonymous or pseudonymous contributions cannot be accepted. Open a GitHub issue if you have questions.

---

## Pull request procedure

Target all pull requests at the `main` branch:

1. Create a feature branch from `main`.
2. [Rebase](https://git-scm.com/book/en/Git-Branching-Rebasing) your branch onto the latest `main`.
3. Run `make ci` and confirm that all checks pass.
4. Ensure every commit carries a `Signed-off-by:` DCO line.

Every pull request requires at least one maintainer review and passing CI. Include tests at the public boundary for any behavioral change:

- Native adapter decisions belong in `tests/source.rs` or `tests/target.rs`.
- Wasm component runtime checks belong in `tests/component.rs` or `tests/probe.rs`.
- Never assert exact wording from embedded prompts in Rust tests, never widen item visibility (`pub`) solely for tests, and never commit compiled `.wasm` files.

---

## Conduct

We are committed to providing a friendly, safe, and welcoming environment for all contributors:

- We welcome participants regardless of gender, sexual orientation, disability, ethnicity, religion, or background.
- Be kind, respectful, and constructive in discussions and code reviews.
- Harassment, insults, demeaning behavior, and trolling are not tolerated in public or private project spaces.
- If you experience or witness unacceptable behavior, please contact a member of the core team immediately.

The [Code of Conduct](CODE_OF_CONDUCT.md) applies across all project spaces.

## See also

- [`AGENTS.md`](AGENTS.md) — architecture map, invariants, code style, and test conventions
- [`augentic/emery` CONTRIBUTING.md](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md) — contributor guide for the core engine repository
