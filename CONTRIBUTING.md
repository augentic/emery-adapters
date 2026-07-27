# Contributing to emery-adapters

Human-facing contributor guide (toolchain, layout, prompts, pin, publishing). Creating an adapter end-to-end is [`docs/authoring.md`](docs/authoring.md); agent and contract rules live in [`AGENTS.md`](AGENTS.md); test ownership in [`docs/testing.md`](docs/testing.md). The live eval repair loop is in the [README](README.md).

## Getting started

1. Clone this repository. The engine crates (`emery-adapter`, `emery-native`, `emery-probe`, `emery-prose`) resolve as git dependencies on [`augentic/emery`](https://github.com/augentic/emery), pinned by release tag (`tag = "vX.Y.Z"`) and the committed `Cargo.lock` — no sibling checkout is needed to build or test. A sibling checkout at `../emery` is required only for [co-development against uncommitted engine changes](#engine-pin-and-sibling-co-development) and for `cargo make wasm-run`.
2. Optional for Vectis materialize FS tests: clone [`augentic/vectis-template`](https://github.com/augentic/vectis-template) at `../vectis-template` (or set `VECTIS_TEMPLATE_DIR`). CI does **not** network-clone it; without the checkout those tests skip clearly. Live Vectis eval builds need the same prerequisite relative to the eval sandbox (see [`examples/eval/README.md`](examples/eval/README.md)).
3. `rustup` picks up the pinned **stable** toolchain from `rust-toolchain.toml` (including the `wasm32-wasip2` target); a nightly toolchain is additionally needed for the `fmt` arm (`cargo +nightly fmt`). Install `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet`. Publishing also uses `wkg`.
4. Run `cargo make check` from the repo root. Before opening a PR, run `cargo make ci`.

For the adapter SDK's type-level contract (the `Source` / `Target` traits, seam DTOs, answer schemas), generate the docs locally: `cargo doc -p emery-adapter --open`.

Unless you are fixing a known bug, discuss larger changes in a GitHub issue first. Legal / DCO expectations match the engine repo — see [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md).

### Troubleshooting first runs

- **`cargo make fmt` fails** — the fmt arm shells out to `cargo +nightly fmt`; install any nightly toolchain (`rustup toolchain install nightly --component rustfmt`).
- **Eval case commands hang or fail authenticating** — they need [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated via `cursor-agent login` or `CURSOR_API_KEY` in a repo-root `.env`.
- **`cargo make wasm-run` fails immediately** — it requires the sibling [`augentic/emery`](https://github.com/augentic/emery) checkout at `../emery` (it drives that repo's built `emery` binary).
- **Patch-resolution errors after editing the root `Cargo.toml`** — the `[patch."https://github.com/augentic/emery.git"]` block only resolves when `../emery` exists; re-comment it if you are not co-developing.

## Layout

Every adapter — the three targets and the five sources — shares the same guest anatomy:

```text
{targets,sources}/
  <name>/             # e.g. targets/{contracts,omnia,vectis}, sources/{intent,…}
    prose/            # agent-facing markdown (embedded into the component)
      prompts/        # operation system-prompt fragments
      references/     # lazy MCP reference corpus
      rules/          # engineering standards (target adapters)
    Cargo.toml        # `<name>` — adapter identity semver is its `version`
    src/              # wasm-free adapter logic + wasm32-only `guest` shim
    tests/            # native integration suite
codex/                # cross-adapter rules/ and references/runtime/
examples/
  wasm/               # component-seam example (`cargo make wasm-run`)
  eval/               # native catalog and live eval cases
Cargo.toml            # virtual workspace: `examples/eval` + `{sources,targets}/*`
```

Identity lives in the guest crate's `Cargo.toml` `version` (the shared `[workspace.package]` SemVer) and the package reference it publishes under (`emery:<name>@<semver>`). Axis is the exported world (`source` xor `target`). The compatibility floor and — for targets — the declared build `inputs[]` and platforms capability are compiled into the `metadata` operation's record.

Crux shell-detection heuristics live in `targets/vectis/src/shell.rs`.

## Prompt authoring

Adapter prompts are markdown documents compiled into the guest and driven by the engine's orchestrations. They are not skills: no YAML frontmatter, no discovery metadata.

- **Parent prompts** (`prose/prompts/{guidance,build,merge}.md` for targets, `prose/prompts/{survey,extract}.md` for sources) orchestrate — bindings, mode dispatch, phase order, the stop-hint contract — and load phase sub-prompts by relative-link instruction. Cap ~150 non-blank lines; orchestration that needs more means a sub-prompt is missing.
- **Phase sub-prompts** (`prose/prompts/build/<phase>.md`, or `build/<platform>/<phase>.md` for per-platform targets) carry one phase's operational body. Soft cap ~500 non-blank lines, hard cap 800 — above that, split into sub-phase prompts or move material to `prose/references/`.
- **References are cited via relative markdown links, never inlined** — the `prose` crate's build-time embed includes Markdown documents and follows symlinks, so keep every relative reference resolvable. Vectis keeps sample Emery artifacts (`tokens.yaml`, `assets.yaml`) under `targets/vectis/prose/references/examples/`; the live `$TEMPLATE_DIR` ([`vectis-template`](https://github.com/augentic/vectis-template)) checkout is the worked example for core + shells + DX (see that folder's README). Other adapters may still ship markdown walkthroughs under `prose/references/examples/<flavour>/` (exempt from prompt caps).

## Engine pin and sibling co-development

Two compatibility choices are independent, for first- and third-party adapter authors alike:

1. **WIT contract version** — the `emery:adapter` WIT package, embedded in the `adapter` SDK and published from `augentic/emery`'s `wit/emery.wit`.
2. **Engine revision** — the workspace resolves `emery-adapter`, `emery-native`, `emery-probe`, and `emery-prose` as git dependencies on `augentic/emery`, pinned by **release tag** (`tag = "vX.Y.Z"` in the root `Cargo.toml`; RFC-77 D13) plus the committed `Cargo.lock`. Advancing the pin is deliberate: bump the tag on all four dependencies to a released engine line, run `cargo update -p emery-adapter -p emery-native -p emery-probe -p emery-prose`, and commit both files — never resolve a floating branch.

For sibling co-development against uncommitted engine changes, uncomment the `[patch."https://github.com/augentic/emery.git"]` block at the bottom of the root `Cargo.toml` (it points at `../emery`) and work in both trees; re-comment it before committing. The patch block must never be active at publish time.

## Local development loops

```bash
cargo make check                 # fmt + clippy + nextest + doctests + doc
cargo make ci                    # full gate — adds cargo-vet + cargo-deny
cargo make adapter <name>        # fast one-component build → target/wasm32-wasip2/release/<name>.wasm
cargo make release               # release-build every adapter
cargo make lab -- ARGS       # any emery verb through the native lab shim
```

The `fmt` arm uses nightly `rustfmt`. Eval runs **natively** and proves prompt quality; WASM/WIT conformance stays with the wasm example (`cargo make wasm-run`). See [docs/testing.md](docs/testing.md) for the five-rung map and the [repo README](README.md) for the live eval repair loop.

Local no-registry loop: `cargo make adapter <name>` then `emery adapter add <path.wasm>`. The `emery` runtime installs published artifacts automatically on a cold package-pin miss (`emery:<name>@<version>`).

## Publishing

The first-party adapter train releases from durable `release-X.Y.Z` branches with the same verbs as the engine repo (RFC-77): dispatch **Create Release** on `main` to cut `release-X.Y.Z` (it also opens the bump-`main` PR), stabilize and backport on the branch, dispatch **Publish Release** on the branch (tag, GitHub Release, GHCR packages), and dispatch **Create Patch** on the same branch for `X.Y.Z → X.Y.Z+1`. The train version is the shared `[workspace.package]` SemVer; `RELEASES.md` carries the line's notes, including a compatibility row (`engine X.Y.x ↔ adapters A.B.x (WIT emery:adapter@…, floor ≥ …)`).

Before a train publishes, these gates must hold:

1. The tree builds against a **published** `emery:adapter` WIT pin.
2. CI is green against a **released (or RC)** engine revision — the engine git dependencies are tag-pinned (`tag = "vX.Y.Z"`), with no active sibling `[patch]` block.
3. Every adapter's `emery-floor` names the minimum host that can run this train.
4. Releasing a new SemVer: the GHCR version tag must not already exist for a first-time push of that train (the publish helper probes and never replaces an existing tag; a re-run skips it and continues).

**Publish Release** runs CI, tags and creates the GitHub Release, then release-builds every adapter and pushes each as a Wasm OCI artifact to `ghcr.io/augentic/emery-adapters/<name>:<version>` via the same `cargo make release` / `cargo make publish <name>` path used locally. The helper derives `<version>` from the workspace manifest and never replaces an existing version tag — released bytes are immutable by policy (GHCR has no registry-native tag immutability, so the helper probe is the compensating control). An already-published tag is a successful skip, so a partial Publish Release (or local `cargo make publish`) can be re-run safely.

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
3. Prefer integration tests in each adapter's `tests/` suite; read [docs/testing.md](docs/testing.md) before adding or relocating tests.
4. Keep adapter names unique across the source and target axes.
5. Do not commit built `.wasm` artifacts.

## See also

- [docs/authoring.md](docs/authoring.md) — creating an adapter, from empty directory to published component
- [AGENTS.md](AGENTS.md) — vocabulary, component contract, agent commands
- [docs/testing.md](docs/testing.md) — five-rung map
- [README.md](README.md) — live eval: run → debug → edit prose → re-run
- [examples/eval/](examples/eval/) — eval case catalog and depth
- [examples/wasm/README.md](examples/wasm/README.md) — component-seam example
- [codex/rules/README.md](codex/rules/README.md) — engineering-rule catalog
- [emery CONTRIBUTING](https://github.com/augentic/emery/blob/main/CONTRIBUTING.md) — DCO and org contribution norms
