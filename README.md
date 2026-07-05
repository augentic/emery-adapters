# specify-adapters

First-party Specify **adapters**, extracted from the platform repo as
independently-versioned registry artifacts (RFC-48 / RFC-49 T6).

Each adapter is a self-contained tree under `{targets,sources}/<name>/`:
its `adapter.yaml` manifest, briefs, references, and rules. The platform
`specify` binary consumes an adapter as an opaque, content-addressed
artifact resolved from the global adapter store; it never compiles the
adapter itself.

Each adapter is a **guest component** (RFC-61): the adapter root doubles as a
wasm32-only cdylib package (`specify-<name>`, the bindgen/export shim built
from `src/`), with its wasm-free core logic in a `crates/core/` sub-crate
(`specify-<name>-core`) and the committed `guest.wasm` beside `adapter.yaml`.

## Layout

Every adapter — the three targets and the five sources — shares the same guest anatomy:

```text
{targets,sources}/
  <name>/             # e.g. targets/{contracts,omnia,vectis}, sources/{intent,documentation,typescript,screenshots,captures}
    adapter.yaml      #   adapter manifest (+ briefs/, references/, rules/ prose trees)
    Cargo.toml        #   `specify-<name>` — the adapter guest component (wasm32 shim)
    src/              #   shim body: bindgen, export glue, MCP reference shelf
    crates/core/      #   `specify-<name>-core` — wasm-free logic, natively tested
    guest.wasm        #   committed guest component (refreshed via `cargo make refresh-guests`)
shared/               # shared references/rules forked from the platform repo;
                      # adapter `spec-runtime` / `agent-teams` symlinks resolve here
crates/               # shared guest support: guest-kit, prose-registry,
                      # eval-driver + eval-guest, runtime-tests
evals/                # live eval harnesses against the real cursor backend
                      # (contracts, vectis)
Cargo.toml            # workspace: guest roots + `{sources,targets}/*/crates/*`
```

The `adapter.yaml` manifests carry the post-cutover field set only (`name`, `version`, `axis`, `description`, plus `platforms` where declared): the guests embed their own briefs, so nothing reads manifests for operation dispatch.

The Crux shell-detection heuristics the platform once exposed as
`specify-vectis-shell-detect` live inline in the vectis core at
`targets/vectis/crates/core/src/shell.rs` rather than as a separate
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

Build every adapter guest for wasm32-wasip2 (plus the eval guest) with `cargo make build-guests`; refresh the committed `{targets,sources}/<name>/guest.wasm` components with:

```bash
cargo make refresh-guests
```

## Publishing

`specify adapter publish --path targets/<name> --reference <registry>/<ns>/<name>:<version>`
packs the tree into a byte-deterministic single-layer OCI artifact, pushes
it, pulls it back, and verifies the content digest. CI (`.github/workflows/release.yaml`)
runs this for every adapter on a `v*` tag. Registry credentials come from
`SPECIFY_REGISTRY_TOKEN` (bearer) or `SPECIFY_REGISTRY_USERNAME` /
`SPECIFY_REGISTRY_PASSWORD` (basic).
