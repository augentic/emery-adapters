# Configuration

Dependencies, workspace setup, scaffolded tooling policy, and CI/CD workflows for WASM guests.

The worked reference for everything here is the exemplar checkout — see [`exemplar.md`](exemplar.md) for the checkout contract. Its root `Cargo.toml`, tooling files, and workflows are a compiling, CI-green instance of this document's policy.

---

## Version Resolution

This document uses `<latest>` as a placeholder for dependency versions. **Do not use `<latest>` literally in generated files.** At generation time, resolve each placeholder to the actual version:

- **`omnia` / `omnia-*` crates (create mode)**: adopt the exact `{ version, repository, rev }` contract from the exemplar checkout's `exemplar.yaml`, and mirror its `[patch.crates-io]` block — the omnia crates are currently resolved from the GitHub repository at that pinned rev, pending publication. Do not `cargo search` a different omnia version: the exemplar is only proven green against its declared rev.
- **`omnia` / `omnia-*` crates (update mode)**: preserve the consumer's existing pin (see the build prompt's compatibility rules); never upgrade as a side effect.
- **`rust-version`**: use the latest stable Rust version (`rustc --version`).
- **Other crates.io dependencies**: prefer the version the exemplar's root `Cargo.toml` uses when the exemplar uses the same crate; otherwise run `cargo search <crate-name>` for the latest.
- **`wasmtime` / `wasmtime-wasi`**: must match the version the pinned `omnia` crate depends on — check omnia's `Cargo.toml` at the adopted rev.

---

## Cargo Setup

### Workspace Configuration

Model the workspace `Cargo.toml` on the exemplar's root `Cargo.toml`: resolver `"3"`, `members = ["crates/*", ...]` (the exemplar adds `guests/*`), a `[workspace.package]` table inherited by every crate, `[workspace.lints.rust]` / `[workspace.lints.clippy]` tables (all/pedantic/nursery/cargo groups plus the cherry-picked restriction lints), `[workspace.dependencies]` with every shared version declared once, and the size-optimized `[profile.release]` (`lto`, `opt-level = "s"`, `strip`).

Skip the exemplar's project-specific entries when adapting it: its `acme-*`/`gtfs`/`pulse`/`tally` internal crates, the `augentic-test` git dependency, and the `wrpc-*` patch entries. Keep the omnia stanza, the shared runtime dependencies your slice actually needs, and the lint tables.

Each crate then inherits:

```toml
[lints]
workspace = true

[package]
authors.workspace = true
edition.workspace = true
rust-version.workspace = true
version.workspace = true
# ... etc
```

### Guest Package Configuration

```toml
[lib]
crate-type = ["cdylib"]

[[example]]
name = "<guest-name>"
path = "examples/<guest-name>.rs"
```

Guest dependencies are wasm32-compatible only; native-only crates (`omnia`, `wasmtime`, `wasmtime-wasi`) go under `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` so `cargo build --target wasm32-wasip2` never pulls them. `cfg-if` supports the runtime example's conditional compilation. The exemplar's `guests/typed/Cargo.toml` is the default worked shape (`guests/axum/Cargo.toml` only when design requires the Axum escape hatch).

### Core Dependencies

| Dependency             | Purpose                                                  |
| ---------------------- | -------------------------------------------------------- |
| `anyhow`               | Error context and propagation                            |
| `axum`                 | HTTP routing (enable `json`, `macros`, `query` features) |
| `bytes`                | Efficient byte buffer for HTTP body extraction           |
| `http-body-util`       | HTTP body utilities (`Empty<Bytes>` for GET requests)    |
| `omnia-guest`          | Guest API: typed routers, capability traits, providers   |
| `omnia-wasi-http`      | HTTP server/client support                               |
| `omnia-wasi-messaging` | Message pub/sub                                          |
| `omnia-wasi-otel`      | OpenTelemetry instrumentation                            |
| `tracing`              | Structured logging                                       |
| `wasip3`               | WASI P3 HTTP exports                                     |
| `wit-bindgen`          | WIT binding generation                                   |

---

## Scaffolded Tooling Files

The standard tooling files are **not authored by the model at all**. The adapter's deterministic scaffold prelude writes every missing one at the start of each build, fill-only — existing files are never overwritten, and a prelude I/O failure fails the build before generation, so there is no situation where these files should be recreated from prose.

The template bodies live only in the exemplar repository (`templates/guest/`); this adapter fetches that subtree at adapter-build time and bakes it into the component — there is no committed second copy. The exemplar's `templates/guest/manifest.yaml` is the single source-to-target map. To inspect a template body, read the file at its target path in the exemplar checkout — the exemplar root **is** the rendered template set, proven by its `template-check` gate.

| File                        | Purpose                                | Written by                     |
| --------------------------- | -------------------------------------- | ------------------------------ |
| `Makefile`                  | Shim delegating to `cargo make`        | Prelude                        |
| `Makefile.toml`             | CI/dev task runner                     | Prelude                        |
| `deny.toml`                 | Dependency license/advisory/ban checks | Prelude; customize per project |
| `rust-toolchain.toml`       | Stable channel + wasm32 target         | Prelude                        |
| `rustfmt.toml`              | Formatting config (nightly `fmt`)      | Prelude                        |
| `clippy.toml`               | Lint exceptions                        | Prelude; customize per project |
| `taplo.toml`                | TOML formatting                        | Prelude                        |
| `.gitignore`                | Repo hygiene                           | Prelude                        |
| `.vscode/settings.json`     | rust-analyzer wasm32 config            | Prelude                        |
| `supply-chain/README.md`    | Cargo Vet update instructions          | Prelude                        |
| `supply-chain/config.toml`  | Cargo Vet imports + scaffold           | Prelude + `cargo vet`          |
| `supply-chain/audits.toml`  | Cargo Vet trusted publishers           | Prelude + `cargo vet`          |
| `supply-chain/imports.lock` | Imported audit data                    | Auto-generated by `cargo vet`  |

Notes on the scaffolded set:

- The toolchain channel is **stable**; formatting runs through `cargo +nightly fmt` (the `fmt` task), so a nightly toolchain must be installed alongside stable for the unstable rustfmt options.
- `clippy.toml` and the `supply-chain/` files are **seeds**: the prelude writes a working baseline and the project evolves it (`doc-valid-idents`, `allowed-duplicate-crates`, vet exemptions). Update them in the consumer project as `cargo clippy` / `cargo vet` demand; that divergence is expected.
- `deny.toml` seeds the standard license allowlist, the tokio multiple-versions ban, and the Augentic git/registry sources. Extend per project as dependencies require.

### Post-Generation: Populate Workspace-Specific Data

After all project files are generated and workspace dependencies are finalized (i.e., `Cargo.toml` and `Cargo.lock` exist), populate exemptions, policies, trusted publishers, and import data:

```bash
cargo vet regenerate imports
cargo vet regenerate exemptions
cargo vet regenerate unpublished
```

- **`regenerate imports`** — fetch audit data from the configured import sources and write `supply-chain/imports.lock`
- **`regenerate exemptions`** — add `[[exemptions.<crate>]]` entries to `supply-chain/config.toml` for any dependency not covered by imports or audits
- **`regenerate unpublished`** — add `[policy.<crate>]` entries for workspace crates with `audit-as-crates-io = true`

**Note**: `supply-chain/imports.lock` is auto-generated and is never hand-written, templated, or scaffolded.

---

## GitHub Workflows

Every guest project includes five workflow files in `.github/workflows/`, all thin wrappers over the reusable workflows in `augentic/.github`. The scaffold prelude writes all five when absent; the guest writer's only follow-up is filling the placeholders in `publish.yaml`'s deploy job.

| File           | Trigger                 | Purpose                                    |
| -------------- | ----------------------- | ------------------------------------------ |
| `audit.yaml`   | Daily schedule + manual | Security audit of dependencies             |
| `ci.yaml`      | Push to any branch      | Continuous integration (build, lint, test) |
| `patch.yaml`   | Manual                  | Create a patch release                     |
| `release.yaml` | Manual                  | Create a new release                       |
| `publish.yaml` | Manual                  | Release pipeline: CI → Publish → Deploy    |

### publish.yaml placeholders

`publish.yaml` is scaffolded with a deploy job whose `with:` block carries three `<UPPER_SNAKE>` placeholders. Replace them with concrete values:

| Placeholder         | Description                                                       |
| ------------------- | ----------------------------------------------------------------- |
| `<PACKAGE_NAME>`    | Crate name of the guest (the package name from `Cargo.toml`)      |
| `<STORAGE_ACCOUNT>` | Azure Storage account for WASM deployment                         |
| `<RESOURCE_GROUP>`  | Azure resource group for WASM deployment                          |

The deploy job inherits repository secrets (`secrets: inherit`); configure the Azure service-principal secrets (`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID`) in the repository settings.
