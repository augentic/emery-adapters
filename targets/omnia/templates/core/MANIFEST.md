# Omnia core templates

Static base-repo tooling files for an Omnia guest workspace, written by the
adapter's deterministic scaffold prelude at the start of every build when the
target path is absent. Existing files are never overwritten.

Source-to-target mappings live in [`../manifest.yaml`](../manifest.yaml);
`build.rs` generates `src/scaffold/templates/registry.rs` from it and fails the
build on an orphan or missing source.

| Concern | Files |
| ------- | ----- |
| Task runner | `Makefile` (shim), `Makefile.toml` (`check` = fmt/lint/test/test-docs/doc; `ci` = check + vet + deny) |
| Toolchain | `rust-toolchain.toml` (nightly + `wasm32-wasip2`), `rustfmt.toml`, `clippy.toml`, `taplo.toml` |
| Supply chain | `deny.toml`, `supply-chain-{README.md,config.toml,audits.toml}` — `supply-chain/imports.lock` and exemptions are populated by `cargo vet regenerate` once `Cargo.lock` exists |
| CI/CD | `workflow-{audit,ci,patch,release,publish}.yaml` — thin wrappers over `augentic/.github` reusable workflows; `publish.yaml` carries `<PACKAGE_NAME>` / `<STORAGE_ACCOUNT>` / `<RESOURCE_GROUP>` placeholders the guest writer fills in |
| Editor / repo hygiene | `vscode-settings.json`, `cargo-config.toml`, `gitignore` |

The model still owns the workspace `Cargo.toml`, guest `src/lib.rs`,
`examples/`, and every domain crate — this assembly is tooling only.
