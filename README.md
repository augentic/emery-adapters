# specify-adapters

First-party Specify **adapters** — Wasm OCI artifacts published to GHCR under
one lockstep workspace SemVer and consumed by the platform `specify` binary.

Each adapter is a **guest component**: one crate (`<name>`) whose wasm-free
library modules carry the adapter logic — natively tested through the crate's
own `tests/` suite — and whose wasm32-only `guest` module is the hand-written
export shim over `adapter`'s shared WIT bindings, with `prose/` trees
(`prompts/`, `references/`, and `rules/` where declared) embedded at build
time. The deployable artifact is the built component — no manifest file, no
committed wasm. The platform resolves the published component from the
registry and reads resolve-time facts through the WIT `metadata` operation.

Repository-wide contributor guidance lives in [`AGENTS.md`](AGENTS.md); adapter-local guidance extends it from nested `AGENTS.md` files such as [`targets/vectis/AGENTS.md`](targets/vectis/AGENTS.md).

## Layout

Every adapter — the three targets and the five sources — shares the same guest anatomy:

```text
{targets,sources}/
  <name>/             # e.g. targets/{contracts,omnia,vectis}, sources/{intent,documentation,typescript,screenshots,captures}
    prose/            #   agent-facing markdown (embedded into the component)
      prompts/        #   operation system-prompt fragments
      references/     #   lazy MCP reference corpus
      rules/          #   engineering standards (target adapters)
    Cargo.toml        #   `<name>` — the adapter component; its `version` is the adapter identity semver
    src/              #   wasm-free adapter logic + the wasm32-only `guest` shim module
    tests/            #   native integration suite (one auto-discovered binary per area)
codex/                # cross-adapter prose: rules/ (UNI-* engineering rules)
                      # and references/runtime/ (the spec-runtime bundle
                      # adapters symlink into their prose/)
examples/
  wasm/               # wasm seam example: fixture tree + `cargo make wasm-run`
                      # (sibling specify binary + adapter components; see README.md)
  eval/               # native composition package: first-party catalog,
                      # `eval` trial, and prompt scenarios
Cargo.toml            # virtual workspace: `examples/eval` + `{sources,targets}/*`
```

Identity lives in the guest crate's `Cargo.toml` `version` (the shared
`[workspace.package]` SemVer) and the package reference it publishes under
(`specify:<name>@<semver>`). Axis is the exported
world (`source` xor `target`). The compatibility floor and — for targets — the
declared build `inputs[]` and platforms capability are compiled into the
`describe` operation's manifest record.

Crux shell-detection heuristics live in `targets/vectis/src/shell.rs`.

## Prompt authoring

Adapter prompts are markdown documents compiled into the guest and driven by the engine's orchestrations. They are not skills: no YAML frontmatter, no discovery metadata. Two roles, one discipline:

- **Parent prompts** (`prose/prompts/{guidance,build,merge}.md` for targets, `prose/prompts/{survey,extract}.md` for sources) orchestrate — bindings, mode dispatch, phase order, the stop-hint contract — and load phase sub-prompts by relative-link instruction. Cap ~150 non-blank lines; orchestration that needs more means a sub-prompt is missing.
- **Phase sub-prompts** (`prose/prompts/build/<phase>.md`, or `build/<platform>/<phase>.md` for per-platform targets) carry one phase's operational body. Soft cap ~500 non-blank lines, hard cap 800 — above that, split into sub-phase prompts or move material to `prose/references/`.
- **References are cited via relative markdown links, never inlined** — the `prose` crate's build-time embed includes Markdown documents and follows symlinks, so keep every relative reference resolvable. Worked examples live under `prose/references/examples/<flavour>/` (exempt from prompt caps).



The local gate mirrors CI — run it from the repo root:

```bash
cargo make check   # fmt + clippy + nextest + doctests + doc
cargo make ci      # the full gate — adds cargo-vet + cargo-deny
```

The `fmt` arm uses nightly `rustfmt`. Install a nightly toolchain plus the `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet` tools (publishing also uses `wkg`); the tasks are defined in `Makefile.toml`.

Release-build every adapter for wasm32-wasip2 (components land
at `target/wasm32-wasip2/release/<name>.wasm`):

```bash
cargo make release
```

For fast development iteration, `cargo make adapter <name>` builds one
component with fast profile settings into the same path (see
[TESTING.md](TESTING.md)).

The `eval` package at [`examples/eval/`](examples/eval/README.md) links every adapter crate in-process and owns the first-party catalog declaration (in `examples/eval/src/main.rs`) over the engine-owned `native` host, composing it with Specify's `probe` library through its `client` feature — both consumed from revision-pinned git sources like the `adapter` SDK. It carries the live trial and prompt-scenario loops without coupling the engine repository back to concrete adapters. Eval runs **natively** and proves prompt quality; WASM/WIT conformance stays with the wasm example (`cargo make wasm-run`). See [TESTING.md](TESTING.md) for the five-rung map. The development entry point:

```bash
cargo make specify -- --project-dir /path/to/project plan status
```

Two compatibility choices are independent, for first- and third-party adapter authors alike: the **WIT contract version** an adapter targets (the `specify:adapter` WIT package, embedded in the `adapter` SDK and published from `augentic/specify`'s `wit/specify.wit`), and the **engine revision** the workspace resolves for the `adapter` SDK, `guest` crate, `native` host, and `probe` library. The engine crates are declared as git dependencies on `augentic/specify`; today the committed path patch resolves them from the sibling `../specify` checkout, and once the exposing engine revision is published the manifest pins it explicitly (add `rev` values in the root `Cargo.toml`, run `cargo update`, and commit the lockfile). The pin advances deliberately, not with every engine commit.

For sibling co-development against uncommitted engine changes, the committed `[patch."https://github.com/augentic/specify.git"]` section in the root `Cargo.toml` resolves the engine crates from the sibling `../specify` working tree.

## Publishing

Publication is manual and local in this cut (GitHub Actions automation is a
later cut; `.github/workflows/release.yaml` stays dormant until then). Each
adapter publishes as a standard Wasm OCI artifact to public GHCR under
`ghcr.io/augentic/specify-adapters/<name>:<version>`, where `<version>` is the
shared `[workspace.package]` SemVer.

One-time setup: authenticate to GHCR with a token carrying `write:packages`
(the `wkg oci push` leg reads the Docker credential config):

```bash
gh auth token | docker login ghcr.io -u <github-user> --password-stdin
```

Release-build, then push one component to its exact version tag:

```bash
cargo make release
cargo make publish <name>
```

The helper derives `<version>` from the workspace manifest and refuses to
replace an existing version tag — released bytes are immutable by policy
(GHCR has no registry-native tag immutability, so the helper probe is the
compensating control). A brand-new package is created **private**: flip it to
public in the GHCR package settings
(`https://github.com/orgs/augentic/packages/container/specify-adapters%2F<name>/settings`)
so anonymous consumers can pull, then confirm the round-trip:

```bash
wkg oci pull ghcr.io/augentic/specify-adapters/<name>:<version> --output /tmp/<name>.wasm
```

The `specify` runtime installs the same artifacts automatically on a cold
package-pin miss (`specify:<name>@<version>`); local development keeps the
no-registry loop (`cargo make adapter <name>` + `specify adapter add`).
