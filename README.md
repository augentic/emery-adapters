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
    Cargo.toml        #   `<name>` — the adapter component; its `version` is the adapter identity semver
    src/              #   wasm-free adapter logic + the wasm32-only `guest` shim module
    tests/            #   native integration suite (one auto-discovered binary per area)
shared/
  prose/              # cross-adapter prose, same grammar as adapter prose/
    references/       #   spec-runtime bundle, replay hook docs, …
    rules/            #   UNI-* engineering rules
crates/               # shared guest support (adapter, prose); native tests use
                      # Omnia's recorded scripted model harness
harness/                # hosted-deployment and native workflow harnesses
                      # package flattened like omnia's examples/: harness.rs
                      # (the package lib — the
                      # shared host-side harness) + composed.rs (the
                      # model-free composed-deployment tests hosting the
                      # built adapter guest components on the Omnia runtime)
                      # + live.rs (the live eval runner) + runtime.rs (the
                      # eval-driver host) + guest.rs (the eval-guest cdylib)
                      # over the per-adapter scenario trees (contracts,
                      # vectis)
  native/             # `specify-dev`: linked-adapter engine runtime and
                      # scripted/replay full-loop integration suite — a
                      # standalone workspace excluded from the root, pinned
                      # to a declared Specify engine revision
Cargo.toml            # workspace: `crates/*` + `{sources,targets}/*` + `harness`
                      # (excludes `harness/native`)
```

Identity lives in the guest crate's `Cargo.toml` `version` and the wasm-pkg
reference it publishes under (`specify:<name>@<semver>`). Axis is the exported
world (`source` xor `target`). The compatibility floor and — for targets — the
declared build `inputs[]` and platforms capability are compiled into the
`describe` operation's manifest record.

Crux shell-detection heuristics live in `targets/vectis/src/shell.rs`.

## Building the guests

The local gate mirrors CI — run it from the repo root:

```bash
cargo make check   # fmt-check + clippy + nextest + doctests + doc
cargo make ci      # the full gate — adds cargo-vet + cargo-deny
```

The `fmt-check` arm uses nightly `rustfmt`, while component development and publishing use nightly Cargo Script. Install a nightly toolchain plus the `cargo-make`, `cargo-nextest`, `cargo-deny`, and `cargo-vet` tools; the tasks are defined in `Makefile.toml`.

Release-build every adapter for wasm32-wasip2 (components land
at `target/wasm32-wasip2/release/<name>.wasm`):

```bash
cargo make release
```

The `harness` package keeps composed WASM/WIT conformance (`harness/composed.rs`) distinct from live prompt-quality evaluation (`harness/live.rs`). Composed tests build guests from source on first use when artifacts are absent under `target/wasm32-wasip2/debug/`.

The `specify-dev` package under `harness/native/` is a **standalone workspace**, deliberately excluded from the root: it links every adapter crate in-process and consumes Specify's engine crates from a revision-pinned git source, so ordinary adapter commands never resolve (or authenticate to) that private dependency. It provides the fast, model-free full-loop and seam suite without coupling the engine repository back to concrete adapters:

```bash
cargo make native-test     # nextest over harness/native (its own manifest/lock)
cargo make native-lint     # clippy -D warnings over harness/native
cargo make native-run -- --project-dir /path/to/project plan status
```

Two compatibility choices are independent, for first- and third-party adapter authors alike: the **WIT contract version** an adapter targets (`wit/specify.wit`, the publish-time compatibility floor), and — only for this optional native harness — the **engine revision** its manifest pins. The pin is the harness's declared, verified engine revision; it advances deliberately (edit the `rev` values in `harness/native/Cargo.toml`, run `cargo update --manifest-path harness/native/Cargo.toml`, and commit its lockfile), not with every engine commit.

For sibling co-development against uncommitted engine changes, use the `cargo make dev -- {check,run,live}` loop: the sibling specify checkout's dev script patches the pin to its working tree with generated `--config` flags. Never commit path patches or hand-edit the pin for local work.

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
