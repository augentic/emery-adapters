# specify-adapters

First-party Specify **adapters**, extracted from the platform repo as
independently-versioned registry artifacts (RFC-48 / RFC-49 T6, amended by
RFC-64).

Each adapter is a **guest component** (RFC-61 / RFC-64): the adapter root
doubles as a wasm32-only cdylib package (`<name>`, a hand-written
export shim over `adapter`'s shared WIT bindings), with its
wasm-free core logic in a `core/` sub-crate (`<name>-core`) and its
`prose/` trees (`prompts/`, `references/`, and `rules/` where declared)
embedded at build time. The deployable artifact is exactly the built
component — there is no `adapter.yaml` manifest and no committed wasm: the
platform `specify` binary pulls the published component from the registry
and reads its resolve-time facts through the WIT `describe` operation.

## Layout

Every adapter — the three targets and the five sources — shares the same guest anatomy:

```text
wit/                  # the contract — specify.wit, the axis worlds
{targets,sources}/
  <name>/             # e.g. targets/{contracts,omnia,vectis}, sources/{intent,documentation,typescript,screenshots,captures}
    prose/            #   agent-facing markdown (embedded into the component)
      prompts/        #   operation system-prompt fragments
      references/     #   lazy MCP reference corpus
      rules/          #   engineering standards (target adapters)
    Cargo.toml        #   `<name>` — the adapter guest component (wasm32 shim); its `version` is the adapter identity semver
    src/              #   hand-written shim: Guest impl, export glue, MCP shelf
    core/             #   `<name>-core` — wasm-free logic, natively tested
shared/
  prose/              # cross-adapter prose, same grammar as adapter prose/
    references/       #   spec-runtime bundle, replay hook docs, …
    rules/            #   UNI-* and CORE-* engineering rules
crates/               # shared guest support: adapter, prose, runtime-tests
evals/                # live eval harness against the real cursor backend,
                      # flattened like omnia's examples/: runtime.rs (the
                      # eval-driver host) + guest.rs (the eval-guest cdylib)
                      # over the per-adapter scenario trees (contracts, vectis)
Cargo.toml            # workspace: guest roots + `{sources,targets}/*` + `{sources,targets}/*/core`
```

The facts the retired `adapter.yaml` carried live wasm-native (RFC-64):
identity in the guest crate's `Cargo.toml` `version` and the wasm-pkg
reference it publishes under (`augentic:<name>@<semver>`); axis in the
exported world (`source` xor `target`); the compatibility floor and — for
targets — the declared build `inputs[]` and platforms capability in the
`describe` operation's compiled-in manifest record.

The Crux shell-detection heuristics the platform once exposed as
`vectis-shell-detect` live inline in the vectis core at
`targets/vectis/core/src/shell.rs` rather than as a separate
workspace crate.

## Building the guests

The local gate mirrors CI — run it from the repo root:

```bash
cargo make check   # fmt-check + clippy + nextest + doctests + doc
cargo make ci      # the full gate — adds cargo-vet + cargo-deny
```

The `fmt-check` arm shells out to nightly `rustfmt`, so a nightly toolchain
plus the `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet` tools must
be installed; the tasks are defined in `Makefile.toml`.

Build every adapter guest for wasm32-wasip2 (plus the eval guest) with
`cargo make build-guests`; release-build the deployable components into
`target/wasm32-wasip2/release/<name>.wasm` with:

```bash
cargo make build-guests-release
```

## Publishing

Publishing an adapter is: release-build the guest package, push the emitted
component to the registry as a standard wasm-pkg package (RFC-64) —

```bash
cargo make build-guests-release
wkg publish target/wasm32-wasip2/release/<name>.wasm --package augentic:<name>@<semver>
```

where `<semver>` is the guest crate's `Cargo.toml` `version`. CI
(`.github/workflows/release.yaml`) runs this for every adapter on a `v*`
tag. Registry credentials come from `SPECIFY_REGISTRY_USERNAME` /
`SPECIFY_REGISTRY_PASSWORD`, written into the `wkg` config's registry auth.
