# Contract identity and version

The canonical identity & version rules for every top-level OpenAPI / AsyncAPI document emitted into `$SLICE_DIR/contracts/` (root key `openapi:` or `asyncapi:`):

- **SemVer `info.version` (MUST).** `info.version` parses as SemVer per [semver.org](https://semver.org), including optional prerelease labels (`1.0.0-draft.1`). New contracts pick an initial version (typically `0.1.0` or `1.0.0`). Do not bump the baseline's `info.version` automatically — version policy is a platform decision, not an authoring decision.
- **Kebab-case `info.x-emery-id` (SHOULD).** New top-level contracts set `info.x-emery-id` to a kebab-case slug (typically the file stem; `^[a-z][a-z0-9-]*$`, ≤ 64 characters). The id is a rename-stable hint that survives file moves and version bumps; path-based references stay canonical.
- **Import posture.** The import sub-flows preserve any source `info.x-emery-id` verbatim — even when malformed — and surface non-SemVer `info.version` values as `[manual review required]` rather than auto-rewriting (see [`import-upgrade-policy.md`](import-upgrade-policy.md)).
- **Enforcement.** The author sub-flows enforce both rules; the format verifiers re-check them in single mode (SemVer `info.version`; kebab-case + ≤ 64-char `info.x-emery-id` when present; in-slice uniqueness on declared ids). The **cross-repo** uniqueness check is not build-time work — it is the merge gate's job (the adapter's in-guest validator: `contract.version-is-semver`, `contract.id-format`, `contract.id-unique`).
