# specify-adapters

First-party Specify **adapters**, extracted from the platform repo as
independently-versioned registry artifacts (RFC-48 / RFC-49 T6).

Each adapter is a self-contained tree under `{targets,sources}/<name>/`:
its `adapter.yaml` manifest, briefs, references, rules, and — for adapters
that ship a WASI extension — a co-located `extension/` crate plus the
committed `adapter.wasm` it builds to. The platform `specify` binary
consumes an adapter as an opaque, content-addressed artifact resolved from
the global adapter store; it never compiles the extension itself.

## Layout

```text
targets/
  contracts/          # API contract authoring + validation (bundles the `contract` extension)
  vectis/             # Crux cross-platform target (bundles the `vectis` extension)
sources/              # source adapters (intent, documentation, typescript, captures, screenshots)
shared/               # shared references/rules forked from the platform repo;
                      # adapter `spec-runtime` / `agent-teams` symlinks resolve here
Cargo.toml            # workspace: members = each `**/extension` crate
```

The Crux shell-detection heuristics the platform exposes as
`specify-vectis-shell-detect` are forked inline into the vectis extension at
`targets/vectis/extension/src/shell.rs` rather than as a separate
workspace crate.

## Building the extensions

The local gate mirrors CI — run it from the repo root:

```bash
cargo make check   # fmt-check + clippy + nextest + doctests + doc
cargo make ci      # the full gate — adds cargo-vet + cargo-deny
```

The `fmt-check` arm shells out to nightly `rustfmt`, so a nightly toolchain
plus the `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet` tools must
be installed; the tasks are defined in `Makefile.toml`.

Refresh the committed `targets/<name>/adapter.wasm` wasm32-wasip2
component with:

```bash
specify adapter build --path targets/<name> --refresh-extension
```

For fast local iteration on an extension crate alone, workspace builds
still work (`cargo build --target wasm32-wasip2 --release -p specify-contract
-p specify-vectis`), but only `specify adapter build` copies the release
binary into the committed `adapter.wasm` beside `adapter.yaml`.

## Publishing

`specify adapter publish --path targets/<name> --reference <registry>/<ns>/<name>:<version>`
packs the tree into a byte-deterministic single-layer OCI artifact, pushes
it, pulls it back, and verifies the content digest. CI (`.github/workflows/release.yaml`)
runs this for every adapter on a `v*` tag. Registry credentials come from
`SPECIFY_REGISTRY_TOKEN` (bearer) or `SPECIFY_REGISTRY_USERNAME` /
`SPECIFY_REGISTRY_PASSWORD` (basic).
