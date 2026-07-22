# specify-adapters

First-party Specify **adapters** — independently versioned wasm-pkg registry
artifacts consumed by the platform `specify` binary.

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
crates/               # shared guest support (prose), the repo's dev-only
                      # test-support crate (testkit — the recording model
                      # harness over omnia-testkit's scripted double), and
                      # lab — the unpublished composition binary owning the
                      # first-party catalog over the engine's native host:
                      # the live `eval` trial and the prompt scenarios under
                      # crates/lab/scenarios/; the `adapter` SDK, `native`
                      # host, and `eval` library are git dependencies on
                      # augentic/specify, resolved from the sibling
                      # checkout by the committed path patch
examples/
  wasm/               # Omnia-hosted wasm deployment: host + specify guest
                      # + omnia.toml + fixture tree (see its README.md)
  static/             # in-process native host over first-party adapters
Cargo.toml            # workspace: `examples/{wasm,static}` + `crates/*`
                      # + `{sources,targets}/*`
```

Identity lives in the guest crate's `Cargo.toml` `version` and the wasm-pkg
reference it publishes under (`specify:<name>@<semver>`). Axis is the exported
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

The `fmt` arm uses nightly `rustfmt`, while component development and publishing use nightly Cargo Script. Install a nightly toolchain plus the `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet` tools; the tasks are defined in `Makefile.toml`.

Release-build every adapter for wasm32-wasip2 (components land
at `target/wasm32-wasip2/release/<name>.wasm`):

```bash
cargo make release
```

The `lab` crate at `crates/lab/` is a native-only, unpublished workspace member: it links every adapter crate in-process, owns the first-party catalog declaration (`crates/lab/src/lib.rs`) over the engine-owned `native` host, and drives Specify's `eval` library (lib-only here; the engine's own composition binary lives on that package's `cli` feature) — both consumed from revision-pinned git sources like the `adapter` SDK. It carries the live `cargo make eval` trial plus the single-operation prompt scenarios (see [TESTING.md](TESTING.md)) without coupling the engine repository back to concrete adapters. The eval rungs run **natively** over the linked crates and prove prompt quality; WASM/WIT conformance stays with the wasm example (`cargo make wasm-run`), and the wasm32 component tasks exclude `lab`. A third-party adapter joining this shim needs both a Cargo dependency in `crates/lab/Cargo.toml` and a catalog entry in `crates/lab/src/lib.rs` — a scenario directory alone cannot link a Rust crate. The development entry point:

```bash
cargo make specify -- --project-dir /path/to/project plan status
```

Two compatibility choices are independent, for first- and third-party adapter authors alike: the **WIT contract version** an adapter targets (the `specify:adapter` WIT package, embedded in the `adapter` SDK and published from `augentic/specify`'s `wit/specify.wit`), and the **engine revision** the workspace resolves for the `adapter` SDK, `guest` crate, `native` host, and `eval` library. The engine crates are declared as git dependencies on `augentic/specify`; today the committed path patch resolves them from the sibling `../specify` checkout, and once the exposing engine revision is published the manifest pins it explicitly (add `rev` values in the root `Cargo.toml`, run `cargo update`, and commit the lockfile). The pin advances deliberately, not with every engine commit.

For sibling co-development against uncommitted engine changes, the committed `[patch."https://github.com/augentic/specify.git"]` section in the root `Cargo.toml` resolves the engine crates from the sibling `../specify` working tree.

## Publishing

Release-build, then push components to the registry as wasm-pkg packages:

```bash
cargo make release
cargo make publish
```

Each identity's `<semver>` is the guest crate's `Cargo.toml` `version`.
Publishing is idempotent: each identity is probed first and skipped when already
present. CI (`.github/workflows/release.yaml`) runs the same tasks on a `v*`
tag, authenticated by `GITHUB_TOKEN`; local emergency publishing uses the
developer's own token in their `wkg` config.
