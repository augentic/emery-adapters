# Omnia r9k migration fixture

Fixture for the `omnia-r9k` workflow eval case: migrating Propellerhead's
[`at_r9k_position_adapter`](https://bitbucket.org/Propellerhead/at_r9k_position_adapter)
TypeScript service into an Omnia WASM crate via Specify.

```text
project/legacy/at_r9k_position_adapter/   # gitignored; staged by `cargo make eval-omnia-r9k-prepare`
```

The case's `fixture` points at `project/` so only the legacy tree enters the
sandbox; the case fails with a focused error when the staged tree is absent.
The upstream tree is `UNLICENSED`, so it is never committed here.

## Populate

```bash
# from specify-adapters/; OMNIA_R9K_GIT_URL overrides the remote
cargo make eval-omnia-r9k-prepare
```

Offline, or from an existing local checkout, stage it by hand — copy the tree
into the gitignored path and strip the clone artifacts and env sidecars:

```bash
cp -R /path/to/at_r9k_position_adapter \
  examples/eval/fixtures/omnia-r9k/project/legacy/at_r9k_position_adapter
rm -rf examples/eval/fixtures/omnia-r9k/project/legacy/at_r9k_position_adapter/{.git,node_modules,.github/env.*}
```

## Run

```bash
cargo make eval omnia-r9k --restart
```

Catalog and depth: [eval README](../../README.md#omnia-legacy-migration-r9k).
