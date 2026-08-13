# Runtime capture extract

The engine invokes this prompt once per terminal `(source, lead)` pair whose adapter is `captures`. Your job: locate the matching `tests/data/replays/<handler>/` directory under `$SOURCE_DIR`, read every scenario capture, and emit one Evidence YAML document. The caller persists it; this prompt returns the body only.

## Binding

The engine passes the source key and a read-only CID view of the capture tree as `$SOURCE_DIR`. The capture layout is the one `/capture:wiretapper` writes — see [capture-format reference](../references/capture-format.md) for the per-file TestDef shape (`input`, `params`, `http_requests`, `output`; `setup` is test-harness-only).

## References

Load both references — they own everything the prompt does not spell out.

- [`../references/capture-format.md`](../references/capture-format.md) — on-disk wire format and behavioural vs non-evidence fields.
- [`../references/extraction-mapping.md`](../references/extraction-mapping.md) — Evidence YAML shape, claim-field detail, capture JSON → claim field mapping, 64 KiB inline cap, determinism rules, worked example, path rules, anti-patterns, and failure modes.

## Inputs

- **`$SOURCE_DIR`** — read-only CID view of the bound capture root. Absent when the binding is an inline `value`.
- **Terminal lead** — the catalog lead the engine passed on `input.focus`. Its id matches the `tests/data/replays/<lead>/` directory name verbatim. Do not look it up in `leads.md`, `discovery.md`, or `slices/`. Child extraction inherits parent context from the passed record.
- **Source key** — the plan source-binding key the engine passed on the wire.
- **`$SCRATCH_DIR`** — write-only scratch space; use only for unavoidable intermediate state.

The change home and `$PROJECT_DIR` are unreachable, host env is unreadable, the network is denied. Do not read `plan.yaml`, `leads.md`, `discovery.md`, or `slices/`. Writes back into `$SOURCE_DIR` are denied.

## Claim grain

One `kind: example` claim per scenario file. A handler directory with 47 `<scenario>.json` files yields 47 example claims; synthesis reconciles them later through the `requirement` / `criterion` claims contributed by sibling sources (see [From sources to slices](../references/emery-runtime/reconciliation.md#slice-time-evidence-becomes-a-spec)). The per-handler grain is the lead (`survey`'s output); the per-scenario grain is the claim. This adapter does not collapse scenarios into a representative subset — every scenario the operator captured contributes one claim, and the 64 KiB inline cap (see references) handles the bulk case.

## Output skeleton

Emit Evidence YAML per [`extraction-mapping.md`](../references/extraction-mapping.md). Minimal shape:

```yaml
authority: behaviour
lead: <lead>
claims:
  - kind: example
    id: <lead>.<scenario-stem>
    path: tests/data/replays/<lead>/<scenario>.json
    replay-digest: sha256:<hex>
    statement: "<single-line summary>"
    input: { ... }
    output: { ... }
```

The caller persists this document, deriving the `(slice, source)` identity from the path and the binding; neither is written in-document.

See the references for the full field table, the worked three-scenario example, determinism rules, the 64 KiB inline cap, path rules, anti-patterns, and failure modes.
