# Contributing to emery-adapters

Human-facing contributor guide (toolchain, layout, prompts, pin, publishing). The adapter shape, the contract rules, and test ownership live in [`AGENTS.md`](AGENTS.md); an existing `sources/<name>` is the template for a new one.

## Getting started

1. Clone this repository. Until an engine release carries the extract-only SDK, the engine crates (`emery-sdk`, and `emery-prose` beneath it) resolve through the `[patch.crates-io]` git patches in the root `Cargo.toml` (see [Engine pin and sibling co-development](#engine-pin-and-sibling-co-development)); once that release exists the pin moves to its tag (`tag = "vX.Y.Z"`, RFC-77 D13). A sibling `../emery` checkout is needed only for co-development (uncomment the path patches) and the live eval (it drives that repo's built `emery` binary).
2. `rustup` picks up the pinned **stable** toolchain from `rust-toolchain.toml` (including the `wasm32-wasip2` target); a nightly toolchain is additionally needed for the `fmt` arm (`cargo +nightly fmt`). Install [mise](https://mise.jdx.dev) yourself: the root `Makefile` forwards every target to [`mise.toml`](mise.toml), which includes the shared Augentic Rust tasks, and never installs mise. The cargo subcommands those tasks need (`cargo-nextest`, `cargo-hack`, `cargo-deny`, `cargo-vet`, …) are installed on first use. Publishing also uses `wkg`.
3. Run `make ci` from the repo root before opening a PR; `make check` is the local advisory set (audit, `fmt` rewriting in place, lint, outdated, udeps).

For the adapter SDK's type-level contract (`extract`, the `workspace` helpers, the contract types, the answer schemas), generate the docs locally: `cargo doc -p emery-sdk --open`; the `export` module — the world an adapter's guest implements — documents under `--target wasm32-wasip2`.

Unless you are fixing a known bug, discuss larger changes in a GitHub issue first. Every commit accepts the [Developer's Certificate of Origin](#developers-certificate-of-origin).

### Troubleshooting first runs

- **`make fmt` fails** — the fmt arm shells out to `cargo +nightly fmt`; install any nightly toolchain (`rustup toolchain install nightly --component rustfmt`).
- **The first `make test` is slow** — `crates/test-programs/build.rs` nested-builds every adapter component and every guest program for `wasm32-wasip2` under `OUT_DIR` before the native suites compile; later builds are incremental.
- **Patch-resolution errors after editing the root `Cargo.toml`** — the committed `[patch.crates-io]` git patches fetch `augentic/emery`; the commented path patches only resolve when `../emery` exists. Do not commit active path patches: CI has no sibling checkout.

## Layout

Every source adapter shares the same guest anatomy:

```text
sources/
  <name>/             # documentation, intent, typescript, python
    prose/            # agent-facing markdown (listed in src/lib.rs, embedded into the component)
      extract.md      # the one extraction pass, and the one extraction prompt
      survey.md       # the one survey prompt, where the model names the surfaces (typescript, python)
      references/     # lazy reference corpus; the shared runtime references are the SDK's, linked as <doc>.md
    Cargo.toml        # `<name>` — adapter identity semver is its `version`
    src/              # lib.rs (`mod survey`, `Adapter` and its trait impl, PROSE, then the wasm32-only export macro) + survey.rs, the adapter's survey, and whatever the survey reads its source with as its children under survey/ (typescript and python alike: parse.rs and parse/walk.rs, resolve.rs, surface.rs — the SDK's `Recogniser`, which the SDK's `survey::seams` surveys and cuts through)
crates/test-programs/ # omnia's test-programs pattern: guest programs + the nested wasm32 build of every component
  programs/<group>/   # one scenario per file: source/extract.rs drives the component boundary, probe/ are fixture adapters
  src/                # lib.rs: the generated artifact table (native) / helpers.rs (wasm32)
  build.rs            # one omnia_test::build::Components build → gen.rs (every adapter + every program)
tests/                # root component suites: source.rs (every shipped component, and what each adapter decides), probe.rs (the error arms, the lowering, the SDK's side of the boundary), prose.rs (every adapter's corpus)
  support/            # mod.rs — the one runner source.rs and probe.rs share (the deployment under the omnia runtime)
examples/             # live walks: one emery.toml per adapter (plus one over all four) the shipped `emery` binary runs, and the fixtures they lend
Cargo.toml            # the tests `emery-adapters` root package over crates/* + sources/*
```

Identity lives in the guest crate's `Cargo.toml` `version` (the shared `[workspace.package]` SemVer) and the package reference it publishes under (`emery:<name>@<semver>`). The compatibility floor is compiled into the `metadata` operation's record.

## Prompt authoring

Adapter prompts are markdown documents compiled into the guest and driven by the engine's `extract` dispatch. They are not skills: no YAML frontmatter, no discovery metadata.

- **`prose/extract.md`** carries the whole extraction pass: the claim-kind table with each kind's required body field (the `emery_sdk::Evidence::findings` gate, run as the check on each seam's turn inside the SDK's `extract` so the backend corrects a miss in place, and fail-closed engine-side, A8), the id-derivation rules reconciliation joins on, and the JSON output contract. Soft cap ~500 non-blank lines, hard cap 800 — above that, move material to `prose/references/`.
- **`prose/survey.md` is the one survey prompt, and only an adapter whose surfaces are the model's to name carries one.** `typescript` and `python` parse their trees into the SDK's `Tree` through their `Recogniser`, and the SDK's `emery_sdk::survey::seams` lays what the tree says — the manifest, the bootstrap, every registration through a package, every registering decorator (every registering `def` and class, for `python`), the entry modules' exports, the modules whole where they fit — into one `survey-<source>` turn under it, answered as the SDK's `Inventory`: each surface named at the lines that register or declare it with the stem its ids lead with, and the modules no surface reaches. The SDK holds the answer to the tree and derives the rest from the accepted anchors through the recogniser — the registration whole, the stem the code spells there, the discriminating id, a class's methods, the closure — and lays that into each seam's turn as text the extraction prompt describes under its Inputs. Both prompts tell the model how to read what code hands it, never how to find it; the same cap applies, and the root `tests/prose.rs` parses `survey.md`'s `## Worked example` fence as an `Inventory` whose anchors parse and whose stems are kebab-case.
- **References are cited via relative markdown links, never inlined** — the model reads a reference through `read_doc`, which answers from the adapter's `PROSE` and then from the SDK's runtime references (`emery_sdk::RUNTIME`: `claims.md` for the claim `id` grammar, `path` anchors, the skip roots, and the fail-closed gate; `reconciliation.md` for the `specify` pipeline), so every relative link must name a document listed in `PROSE` or one of those two (`claims.md` from a prompt at the `prose/` root), and every listed document must be reached from a prompt by such a link (the root `tests/prose.rs` holds the list to the `prose/` tree and refuses a link to a directory, an unlisted file, or a path outside the tree, a listed document no prompt reaches, and a listed document at a runtime reference's path). Link a rule the runtime references state rather than restating it; the references ship with the SDK the adapter compiles against, so they move with the contract they describe.
- **A reference is written for the model, in the adapter's current contract.** It says what to read in the source and which claims to emit, in the prompt's vocabulary — the surface, the entry, the claim kinds and their extras — and nothing addressed to a contributor: no retrospectives, no tooling proposals, no description of what the engine renders downstream. Prose the prompt would contradict is worse than none.
- **Worked examples live under `prose/references/examples/`**, one document per scenario beside a `README.md` index: the surface as the survey lays it into the turn, the source the extract call reads, and the Evidence the call answers with under a `## Evidence` JSON fence — claims alone, like the prompt's own worked example. The root `tests/prose.rs` parses every such fence as the SDK's `Evidence` and holds it to the claim gate, so an example cannot teach a shape the adapter would refuse.
- The v1 survey prompts were deleted, never ported (ADR-0008): they asked the model to find the surfaces in a tree it was handed. `typescript`'s and `python`'s `survey.md` are not that prompt — the model names surfaces among the facts the parser laid, and everything a claim id or a seam rests on is derived by code from the anchors it accepted, with the model put to nothing else before the seams' turns.

## Engine pin and sibling co-development

Two compatibility choices are independent:

1. **WIT contract version** — the `emery:adapter` WIT package, embedded in the `emery-adapter` contract crate (which the `emery-sdk` SDK re-exports) and published from `augentic/emery`'s `wit/emery.wit`.
2. **Engine revision** — the workspace resolves `emery-prose` and `emery-sdk` on `augentic/emery`, pinned by **release tag** (`tag = "vX.Y.Z"` in the root `Cargo.toml`; RFC-77 D13) plus the committed `Cargo.lock`; the contract crate `emery-adapter` rides beneath `emery-sdk` at the same revision. Advancing the pin is deliberate: bump the tag on both dependencies, run `cargo update -p emery-prose -p emery-sdk`, and commit both files — never resolve a floating branch.

For sibling co-development against uncommitted engine changes, uncomment the path patches in the root `Cargo.toml` `[patch.crates-io]` block (they point at `../emery`); they must never be active on the committed tree or at publish time. **Current state**: git patches are active — the extract-only SDK is not yet on a tagged engine release, so the tag pin (and with it the first adapter train publish) waits on that release cut.

## Local development loops

```bash
make ci                    # exactly the CI jobs: fmt-check + lint + nextest + doctests + docs + cargo-vet + cargo-deny
make check                 # local advisories: audit + fmt (rewrites) + lint + outdated + udeps
cargo clippy -p documentation -p intent -p typescript -p python -p test-programs --lib --bins --examples --all-features --target wasm32-wasip2 -- -D warnings   # the guest side alone (`WASM32_PACKAGES` in mise.toml)
cargo build -p <name> --target wasm32-wasip2 --release   # one adapter → target/wasm32-wasip2/release/<name>.wasm (the file the examples copy into the store)
cargo build --workspace --target wasm32-wasip2 --release   # every adapter
```

The tasks are the shared `mise/rust.toml` of [`augentic/toolkit`](https://github.com/augentic/toolkit), pinned in [`mise.toml`](mise.toml) to the tag every `uses:` under `.github/workflows/` names; bump both in one pull request when the pin moves. The `fmt` arm uses nightly `rustfmt`. `make lint` runs clippy under `-D warnings` natively first (`lint-host`: all targets with all features, then each feature through `cargo hack`), then — since the guest side, the programs under `crates/test-programs/programs/` and the adapters' `survey` and `guest` modules, is `cfg(target_arch = "wasm32")` and native clippy compiles it to nothing — the shared `lint-wasm` pass for `wasm32-wasip2`, the target it ships on, where `clippy.toml`'s guest deny-list applies: every workspace lib, bin and example, never tests or benches (the third command above is its first half). The root package and the `evals` harness are native, so they contribute nothing to that pass and need no exclusion. `make vet` is check-only; regenerate audit inputs with `make vet-regen`. The component suites are the Rust inner loop and prove every built component under the omnia runtime, each adapter's own decisions included; `emery specify --config examples/<name>/emery.toml` walks one adapter live through the shipped `emery` binary and the Cursor backend ([examples/README.md](examples/README.md)); the graded live eval, `cargo run -p evals` over the cases under `evals/cases/`, proves prompt quality end to end and writes the dated scorecard ([evals/README.md](evals/README.md)).

## Publishing

The first-party adapter train releases from durable `release-X.Y.Z` branches with the same verbs as the engine repo (RFC-77): dispatch **Create Release** on `main` to cut `release-X.Y.Z` (it also opens the bump-`main` PR), stabilize and backport on the branch, dispatch **Publish Release** on the branch (tag, GitHub Release, GHCR packages), and dispatch **Create Patch** on the same branch for `X.Y.Z → X.Y.Z+1`. The train version is the shared `[workspace.package]` SemVer; `RELEASES.md` carries the line's notes, including a compatibility row (`engine X.Y.x ↔ adapters A.B.x (WIT emery:adapter@…, floor ≥ …)`).

Before a train publishes, these gates must hold:

1. The tree builds against a **published** `emery:adapter` WIT pin.
2. CI is green against a **released (or RC)** engine revision — the engine dependencies are tag-pinned (`tag = "vX.Y.Z"`), with no active sibling `[patch]` block.
3. Every adapter's `emery-version` names the minimum host that can run this train.
4. Releasing a new SemVer: the GHCR version tag must not already exist for a first-time push of that train.

**Publish Release** runs CI, tags and creates the GitHub Release, then release-builds every adapter and pushes each as a Wasm OCI artifact to `ghcr.io/augentic/emery/<name>:<version>` — the tag the `augentic.io` registry resolves the package reference `emery:<name>@<version>` to, so the `emery` binary, which routes the `emery` namespace there, fetches it on the first run that names it — via the same build and `make publish <name>` path used locally. The helper derives `<version>` from the workspace manifest.

A brand-new package is created **private**: flip it to public in the GHCR package settings (`https://github.com/orgs/augentic/packages/container/emery%2F<name>/settings`) so anonymous consumers can pull, then confirm the round-trip both ways — the OCI tag, and the package reference as the binary fetches it:

```bash
wkg oci pull ghcr.io/augentic/emery/<name>:<version> --output /tmp/<name>.wasm
mkdir -p /tmp/store && wkg get emery:<name>@<version> -o /tmp/store/   # writes /tmp/store/emery_<name>@<version>.wasm, the file emery's store reads
```

Local breakout (retry a single adapter after GHCR login):

```bash
gh auth token | docker login ghcr.io -u <github-user> --password-stdin
cargo build --workspace --target wasm32-wasip2 --release
make publish <name>
```

## Developer's Certificate of Origin

All contributions must include acceptance of the [DCO](https://developercertificate.org/):

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

To accept the DCO, add this line to each commit message with your name and email address (`git commit -s` will do this for you):

```text
Signed-off-by: Jane Example <jane@example.com>
```

For legal reasons, no anonymous or pseudonymous contributions are accepted; open a GitHub issue if this is a problem for you.

## Pull request procedure

Pull requests should be targeted at the `main` branch. Before creating a pull request, go through this checklist:

1. Create a feature branch off of `main`.
2. [Rebase](https://git-scm.com/book/en/Git-Branching-Rebasing) your local changes against `main`.
3. Run `make ci` and confirm that it passes: exactly the CI jobs, in order.
4. Accept the Developer's Certificate of Origin on all commits (see above).

All contributions are made via pull request. All patches from all contributors get reviewed. At least one review from a maintainer is required for all patches (even patches from maintainers). When CI fails, authors are expected to update the pull request until it passes.

Normally, all pull requests must include tests that cover your change. Occasionally, a change will be very difficult to test for; in those cases, include a note in your commit message explaining why.

Tests here cover a change at its public boundary: a behaviour the adapter itself decides goes in the root `tests/source.rs`, asserted through the built component; the component boundary is `tests/probe.rs`'s. Never pin a prompt phrase, and never widen `pub` surface — or compile a module natively — solely for a test. Do not commit built `.wasm` artifacts.

## Conduct

Whether you are a regular contributor or a newcomer, we care about making this community a safe place for you and we've got your back.

- We are committed to providing a friendly, safe and welcoming environment for all, regardless of gender, sexual orientation, disability, ethnicity, religion, or similar personal characteristic.
- Be kind and courteous. There is no need to be mean or rude.
- We will exclude you from interaction if you insult, demean or harass anyone. In particular, we do not tolerate behavior that excludes people in socially marginalized groups.
- Private harassment is also unacceptable. If you feel you have been or are being harassed or made uncomfortable by a community member, please contact a member of the core team immediately.
- Likewise any spamming, trolling, flaming, baiting or other attention-stealing behaviour is not welcome.

We welcome discussion about creating a welcoming, safe, and productive environment for the community. If you have any questions, feedback, or concerns please let us know with a GitHub issue. The [Code of Conduct](CODE_OF_CONDUCT.md) applies throughout.

## See also

- [AGENTS.md](AGENTS.md) — vocabulary, component contract, test ownership, agent commands
- [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md) — the engine repository's guide
