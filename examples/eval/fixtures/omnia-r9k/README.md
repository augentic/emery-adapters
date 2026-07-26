# Omnia r9k migration fixture

Fixture for the `omnia-r9k` workflow eval case: migrating Propellerhead's
[`at_r9k_position_adapter`](https://bitbucket.org/Propellerhead/at_r9k_position_adapter)
TypeScript service into an Omnia WASM crate via Specify.

```text
project/legacy/at_r9k_position_adapter/   # gitignored; populated by prepare.sh
```

The case's `fixture` points at `project/` so only the legacy tree enters the
sandbox; the case fails with a focused error when the prepared tree is absent.

## Populate

```bash
# from specify-adapters/
cargo make eval-omnia-r9k-prepare

# or point at a local checkout:
OMNIA_R9K_SOURCE=/path/to/at_r9k_position_adapter cargo make eval-omnia-r9k-prepare
```

The prepare script tries, in order: `OMNIA_R9K_SOURCE`, a sibling
`test-spec` checkout, then `git clone` of the Bitbucket URL
(`OMNIA_R9K_GIT_URL` overrides the remote). The upstream tree is
`UNLICENSED`, so it is never committed here.

## Run

```bash
cargo make eval omnia-r9k --restart
```

Catalog and depth: [eval README](../../README.md#omnia-legacy-migration-r9k).
