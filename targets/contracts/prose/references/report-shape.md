# Report Shape

Output formats for the contracts build format verifiers (`openapi`, `asyncapi`, `json-schema`) and — by convention — the matching alignment / import reports produced by the author and importer paths.

The verifier runs in two modes:

| Surface                            | Output format                                  | Caller                                         | Trigger                                        |
| ---------------------------------- | ---------------------------------------------- | ---------------------------------------------- | ---------------------------------------------- |
| Format verifier `single` (default) | Markdown                                       | contracts verify prompt in the engine's verify phase | Post-author or post-import; the engine's verify → repair rounds |
| Format verifier `cross-project`    | JSON envelope from the adapter's in-guest contract validator | contracts adapter merge prompt                  | Post-merge baseline validation gate            |

`single` mode is human-readable; the engine drives bounded verify → repair rounds (RFC-90) from its findings — the verifier itself never repairs or retries. Format-verifier `cross-project` mode describes the adapter's in-guest contract validator and preserves its baseline-validation JSON envelope.

Both modes share the **read-only** contract — the verifier MUST NOT generate, modify, or delete any files in either mode.

## Severity levels

The severity vocabulary is shared across formats and modes:

| Severity                   | Markdown glyph | Meaning                                                                                                                                                                                                        |
| -------------------------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `FAIL` (`error` in YAML)   | `✗`            | A hard failure. The artefact does not conform; the engine routes the finding to a repair dispatch before the build can succeed.                                                                                                   |
| `WARN` (`warning` in YAML) | `⚠`            | A finding that requires human review. Common in cross-format compatibility checks where the conservative output is "the wire shape changed in a backwards-incompatible direction; the operator should triage." |
| `INFO` (`info` in YAML)    | `ℹ`            | A neutral observation. Common when the consumer's view matches the producer's update or when the consumer has no prior view.                                                                                   |

Single-mode markdown reports use `FAIL` / `WARN` / `INFO` words plus the corresponding glyph in summary tables. Future consumer-impact reports may use a separate classification vocabulary when a real workflow needs it.

### Relationship to `Diagnostic`

The verifier's `FAIL` / `WARN` / `INFO` ladder is the report-local severity vocabulary for the markdown surface and is distinct from the closed `Diagnostic` severity enum (`critical` / `important` / `suggestion` / `optional`) used by the structured `Diagnostic` schema (see `schemas/diagnostics/diagnostic.schema.json` embedded in the `emery` binary from [`augentic/emery`](https://github.com/augentic/emery)). When a caller re-surfaces a verifier finding as a `Diagnostic`, the contracts-specific evidence (operation id, schema pointer, channel, message, compatibility classification, `change-kind`, the raw `findings[].detail`) lives inside the structured-finding payload as `evidence.kind: structured` with the contract data under `evidence.data`; the `Diagnostic` `rule-id`, `target-adapter`, `source-adapter`, `evidence`, `confidence`, and `related-rule-ids` fields use kebab-case on the wire, with `target-adapter: contracts`.

Compatibility classifications like `additive`, `breaking`, `ambiguous`, and `unverifiable` (see [`cross-project-compatibility.md`](./cross-project-compatibility.md)) are **not** the `Diagnostic` severity enum — they remain contract-domain evidence fields and travel inside `evidence.data` alongside the same closed severity enum on the envelope (see `schemas/diagnostics/diagnostic.schema.json`).

## Single-mode output (markdown)

Each format-skill verifier produces a markdown report of the same shape. The check names are format-specific (`$ref Resolution` / `Schema Metadata` / `Binding Completeness` for OpenAPI and AsyncAPI; an extra `Duplicate $id` and `Cross-format Consumer Compatibility` section for JSON Schema), but the structure is identical.

### When issues are found

```markdown
## Validation Report (<Format>)

### <Check 1 name>
- ✗ <file path> — <one-sentence description>
- ✓ <count> of <total> <thing> resolve

### <Check 2 name>
- ✗ <file path> — <one-sentence description>
- ⚠ <file path> — <one-sentence description>
- ✓ <count> of <total> <thing> have <property>

### <Check 3 name>
- ✓ All <thing> verified

### Summary
- **Checks passed:** <N> of <M>
- **Issues found:** <N> (<X> fail, <Y> warn)
```

### When all checks pass

```markdown
## Validation Report (<Format>)

All checks passed (<N> $ref pointers, <N> schemas, <N> bindings verified).
```

### Per-finding format

Each finding is a single bullet with a glyph, a file path (relative to the slice directory), and a one-sentence description. Common shapes:

```
FAIL: contracts/http/user-api.yaml — $ref "../schemas/missing-type.yaml" does not resolve (checked change contracts/schemas/ and baseline contracts/schemas/)
FAIL: contracts/schemas/user-registration.yaml — missing required field "$id"
FAIL: contracts/schemas/error-response.yaml — "description" is empty
WARN: contracts/schemas/oauth-token.yaml — appears in spec but has no protocol binding (may be shared vocabulary — verify intent)
WARN: contracts/schemas/payment.yaml — "$schema" is Draft 7; expected Draft 2020-12 (importer normalisation needed)
```

### Single-mode exit semantics

`single` mode preserves classical exit semantics: zero on a clean report, non-zero on read errors. A clean report with `WARN`-only findings still exits zero — `WARN` is informational for human review, not a blocker. Only `FAIL` findings block, and the engine — never the verifier — drives the repair rounds that follow.
