# specify-adapters

First-party Specify **adapters**, extracted from the platform repo as
independently-versioned registry artifacts (RFC-48 / RFC-49 T6).

Each adapter is a self-contained tree under `adapters/{targets,sources}/<name>/`:
its `adapter.yaml` manifest, briefs, references, rules, and — for adapters
that ship a WASI extension — a co-located `extension/` crate plus the
committed `adapter.wasm` it builds to. The platform `specify` binary
consumes an adapter as an opaque, content-addressed artifact resolved from
the global adapter store; it never compiles the extension itself.

## Layout

```text
adapters/
  targets/
    contracts/        # API contract authoring + validation (bundles the `contract` extension)
    vectis/           # Crux cross-platform target (bundles the `vectis` extension)
  shared/             # shared references/rules forked from the platform repo;
                      # adapter `spec-runtime` / `agent-teams` symlinks resolve here
shared/
  vectis-shell-detect/  # stdlib-only Crux shell heuristics, forked from the platform
Cargo.toml            # workspace: members = each `**/extension` crate + shared/vectis-shell-detect
```

## Building the extensions

```bash
cargo build --workspace                                   # typecheck the extension crates
cargo clippy --workspace --all-targets -- -D warnings     # lint posture (matches CI)
cargo build --target wasm32-wasip2 --release -p specify-contract -p specify-vectis
```

The committed `adapters/targets/<name>/adapter.wasm` is the wasm32-wasip2
release component for that adapter; refresh it via
`specify adapter build --path adapters/targets/<name> --refresh-extension`.

## Publishing

`specify adapter publish --path adapters/targets/<name> --reference <registry>/<ns>/<name>:<version>`
packs the tree into a byte-deterministic single-layer OCI artifact, pushes
it, pulls it back, and verifies the content digest. CI (`.github/workflows/release.yaml`)
runs this for every adapter on a `v*` tag. Registry credentials come from
`SPECIFY_REGISTRY_TOKEN` (bearer) or `SPECIFY_REGISTRY_USERNAME` /
`SPECIFY_REGISTRY_PASSWORD` (basic).
