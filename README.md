# specify-adapters

First-party Specify **adapters** — independently versioned wasm-pkg registry
artifacts consumed by the platform `specify` binary.

Each adapter is a **guest component**: the adapter root is a wasm32-only cdylib
package (`<name>`, a hand-written export shim over `adapter`'s shared WIT
bindings), with wasm-free core logic in a `core/` sub-crate (`<name>-core`) and
`prose/` trees (`prompts/`, `references/`, and `rules/` where declared)
embedded at build time. The deployable artifact is the built component — no
manifest file, no committed wasm. The platform resolves the published component
from the registry and reads resolve-time facts through the WIT `describe`
operation.

## Layout

Every adapter — the three targets and the five sources — shares the same guest anatomy:

```text
wit/                  # the contract — wit/specify.wit, the axis worlds
{targets,sources}/
  <name>/             # e.g. targets/{contracts,omnia,vectis}, sources/{intent,documentation,typescript,screenshots,captures}
    prose/            #   agent-facing markdown (embedded into the component)
      prompts/        #   operation system-prompt fragments
      references/     #   lazy MCP reference corpus
      rules/          #   engineering standards (target adapters)
    Cargo.toml        #   `<name>` — the adapter guest component (wasm32 shim); its `version` is the adapter identity semver
    src/              #   hand-written shim: Guest impl, export glue, MCP references
    core/             #   `<name>-core` — wasm-free logic, natively tested
shared/
  prose/              # cross-adapter prose, same grammar as adapter prose/
    references/       #   spec-runtime bundle, replay hook docs, …
    rules/            #   UNI-* and CORE-* engineering rules
crates/               # shared guest support (adapter, prose) + the composed-
                      # deployment tests (tests/) hosting the built adapter
                      # guest components on the Omnia runtime
evals/                # live eval harness against the real cursor backend,
                      # flattened like omnia's examples/: runtime.rs (the
                      # eval-driver host) + guest.rs (the eval-guest cdylib)
                      # over the per-adapter scenario trees (contracts, vectis)
Cargo.toml            # workspace: guest roots + `{sources,targets}/*` + `{sources,targets}/*/core`
```

Identity lives in the guest crate's `Cargo.toml` `version` and the wasm-pkg
reference it publishes under (`specify:<name>@<semver>`). Axis is the exported
world (`source` xor `target`). The compatibility floor and — for targets — the
declared build `inputs[]` and platforms capability are compiled into the
`describe` operation's manifest record.

Crux shell-detection heuristics live in `targets/vectis/core/src/shell.rs`.

## Building the guests

The local gate mirrors CI — run it from the repo root:

```bash
cargo make check   # fmt-check + clippy + nextest + doctests + doc
cargo make ci      # the full gate — adds cargo-vet + cargo-deny
```

The `fmt-check` arm shells out to nightly `rustfmt`, so a nightly toolchain
plus the `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet` tools must
be installed; the tasks are defined in `Makefile.toml`.

Debug-build the eval guest only:

```bash
cargo make debug
```

Release-build every workspace member for wasm32-wasip2 (adapter components land
at `target/wasm32-wasip2/release/<name>.wasm`):

```bash
cargo make release
```

The composed-deployment tests build guests from source on first use when artifacts
are absent under `target/wasm32-wasip2/debug/`.

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
