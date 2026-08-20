# Contributing to emery-adapters

Human-facing contributor guide (toolchain, layout, prompts, pin, publishing). Creating an adapter end-to-end is [`docs/authoring.md`](docs/authoring.md); agent and contract rules live in [`AGENTS.md`](AGENTS.md); test ownership in [`docs/testing.md`](docs/testing.md); the graded live eval in [`examples/eval/README.md`](examples/eval/README.md).

## Getting started

1. Clone this repository. Until an engine release carries the extract-only SDK, the engine crates (`emery-adapter`, `emery-prose`) resolve through the `[patch.crates-io]` git patches in the root `Cargo.toml` (see [Engine pin and sibling co-development](#engine-pin-and-sibling-co-development)); once that release exists the pin moves to its tag (`tag = "vX.Y.Z"`, RFC-77 D13). A sibling `../emery` checkout is needed only for co-development (uncomment the path patches) and the live eval (it drives that repo's built `emery` binary).
2. `rustup` picks up the pinned **stable** toolchain from `rust-toolchain.toml` (including the `wasm32-wasip2` target); a nightly toolchain is additionally needed for the `fmt` arm (`cargo +nightly fmt`). Install `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet`. Publishing also uses `wkg`.
3. Run `cargo make check` from the repo root. Before opening a PR, run `cargo make ci`.

For the adapter SDK's type-level contract (the `Source` trait, seam DTOs, answer schemas), generate the docs locally: `cargo doc -p emery-adapter --open`.

Unless you are fixing a known bug, discuss larger changes in a GitHub issue first. Legal / DCO expectations match the engine repo — see [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md).

### Troubleshooting first runs

- **`cargo make fmt` fails** — the fmt arm shells out to `cargo +nightly fmt`; install any nightly toolchain (`rustup toolchain install nightly --component rustfmt`).
- **`cargo make eval` refuses immediately** — it needs the sibling shipped binary (`cargo build --release --bin emery` in `../emery`, or `EMERY_BIN`) and the built components (`cargo make release`); the live model backend is the binary's own (the Cursor client).
- **Patch-resolution errors after editing the root `Cargo.toml`** — the committed `[patch.crates-io]` git patches fetch `augentic/emery`; the commented path patches only resolve when `../emery` exists. Do not commit active path patches: CI has no sibling checkout.

## Layout

Every source adapter shares the same guest anatomy:

```text
sources/
  <name>/             # documentation, intent, typescript
    prose/            # agent-facing markdown (embedded into the component)
      prompts/        # extract.md — the one extraction pass
      references/     # lazy reference corpus + the emery-runtime symlink
      rules/          # adapter-local engineering rules
    Cargo.toml        # `<name>` — adapter identity semver is its `version`
    src/              # wasm-free adapter logic + wasm32-only `guest` shim
    tests/            # native integration suite
codex/references/runtime/   # shared runtime references (reconciliation, authority)
examples/eval/        # the graded live-eval runner and its cases
Cargo.toml            # virtual workspace: examples/eval + sources/*
```

Identity lives in the guest crate's `Cargo.toml` `version` (the shared `[workspace.package]` SemVer) and the package reference it publishes under (`emery:<name>@<semver>`). The compatibility floor is compiled into the `metadata` operation's record.

## Prompt authoring

Adapter prompts are markdown documents compiled into the guest and driven by the engine's `extract` dispatch. They are not skills: no YAML frontmatter, no discovery metadata.

- **`prose/prompts/extract.md`** carries the whole extraction pass: the claim-kind table with each kind's required body field (fail-closed engine-side, A8), the id-derivation rules reconciliation joins on, and the JSON output contract. Soft cap ~500 non-blank lines, hard cap 800 — above that, move material to `prose/references/`.
- **References are cited via relative markdown links, never inlined** — the `prose` crate's build-time embed includes Markdown documents and follows symlinks, so keep every relative reference resolvable.
- Survey prompts are deleted, never ported (ADR-0008).

## Engine pin and sibling co-development

Two compatibility choices are independent:

1. **WIT contract version** — the `emery:adapter` WIT package, embedded in the `emery-adapter` SDK and published from `augentic/emery`'s `wit/emery.wit`.
2. **Engine revision** — the workspace resolves `emery-adapter` and `emery-prose` on `augentic/emery`, pinned by **release tag** (`tag = "vX.Y.Z"` in the root `Cargo.toml`; RFC-77 D13) plus the committed `Cargo.lock`. Advancing the pin is deliberate: bump the tag on both dependencies, run `cargo update -p emery-adapter -p emery-prose`, and commit both files — never resolve a floating branch.

For sibling co-development against uncommitted engine changes, uncomment the path patches in the root `Cargo.toml` `[patch.crates-io]` block (they point at `../emery`); they must never be active on the committed tree or at publish time. **Current state**: git patches are active — the extract-only SDK is not yet on a tagged engine release, so the tag pin (and with it the first adapter train publish) waits on that release cut.

## Local development loops

```bash
cargo make check                 # fmt + clippy + nextest + doctests + doc
cargo make ci                    # full gate — adds cargo-vet + cargo-deny
cargo make adapter <name>        # fast one-component build → target/wasm32-wasip2/release/<name>.wasm
cargo make release               # release-build every adapter
cargo make eval [id]             # graded live eval over the public contract (operator-invoked)
```

The `fmt` arm uses nightly `rustfmt`. Native crate tests are the Rust inner loop; the live eval proves prompt quality end to end and writes the dated scorecard.

## Publishing

The first-party adapter train releases from durable `release-X.Y.Z` branches with the same verbs as the engine repo (RFC-77): dispatch **Create Release** on `main` to cut `release-X.Y.Z` (it also opens the bump-`main` PR), stabilize and backport on the branch, dispatch **Publish Release** on the branch (tag, GitHub Release, GHCR packages), and dispatch **Create Patch** on the same branch for `X.Y.Z → X.Y.Z+1`. The train version is the shared `[workspace.package]` SemVer; `RELEASES.md` carries the line's notes, including a compatibility row (`engine X.Y.x ↔ adapters A.B.x (WIT emery:adapter@…, floor ≥ …)`).

Before a train publishes, these gates must hold:

1. The tree builds against a **published** `emery:adapter` WIT pin.
2. CI is green against a **released (or RC)** engine revision — the engine dependencies are tag-pinned (`tag = "vX.Y.Z"`), with no active sibling `[patch]` block.
3. Every adapter's `emery-floor` names the minimum host that can run this train.
4. Releasing a new SemVer: the GHCR version tag must not already exist for a first-time push of that train.

**Publish Release** runs CI, tags and creates the GitHub Release, then release-builds every adapter and pushes each as a Wasm OCI artifact to `ghcr.io/augentic/emery-adapters/<name>:<version>` via the same `cargo make release` / `cargo make publish <name>` path used locally. The helper derives `<version>` from the workspace manifest.

A brand-new package is created **private**: flip it to public in the GHCR package settings (`https://github.com/orgs/augentic/packages/container/emery-adapters%2F<name>/settings`) so anonymous consumers can pull, then confirm the round-trip:

```bash
wkg oci pull ghcr.io/augentic/emery-adapters/<name>:<version> --output /tmp/<name>.wasm
```

Local breakout (retry a single adapter after GHCR login):

```bash
gh auth token | docker login ghcr.io -u <github-user> --password-stdin
cargo make release
cargo make publish <name>
```

## Before you open a PR

1. Branch off `main`.
2. Run `cargo make ci` (or say exactly which narrower checks ran and why the full gate was unavailable).
3. Read [docs/testing.md](docs/testing.md) before adding, deleting, or relocating tests. New tests default to the adapter's `tests/` suite; do not add a `src` `#[cfg(test)]` module without a one-line Keep or Collapse reason from that document, and never widen `pub` surface solely for a test. When deleting unit coverage, run the coverage brake (`CRATE=<adapter> cargo make cov`) before and after.
4. Do not commit built `.wasm` artifacts.

## See also

- [docs/authoring.md](docs/authoring.md) — creating a source adapter
- [AGENTS.md](AGENTS.md) — vocabulary, component contract, agent commands
- [docs/testing.md](docs/testing.md) — test ownership
- [examples/eval/README.md](examples/eval/README.md) — the graded live eval
- [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md) — DCO and org contribution norms
