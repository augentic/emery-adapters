# Cross-Project Compatibility

Cross-project producer-to-consumer compatibility reporting is deferred until a real consumer workflow needs it. The contracts target now relies on its in-guest contract validator for deterministic single-slice and merged-baseline validation, run automatically at the adapter's build and merge gates over `$PROJECT_ROOT/contracts`.

Keep any future consumer-impact report adapter-owned. It should read `registry.yaml`, root `contracts/`, and consumer workspace snapshots directly, then classify findings only when a concrete workflow needs that product surface. Historical vocabulary reserved for that future report: `additive`, `breaking`, `ambiguous`, and `unverifiable`.

These classifiers are contract-domain evidence fields — **not** the closed `Diagnostic` severity enum (`critical` / `important` / `suggestion` / `optional`). When a future consumer-impact report surfaces findings as `Diagnostic` records (see `schemas/diagnostics/diagnostic.schema.json` and §"Relationship to contracts and compatibility"), the closed severity enum sits on the envelope while the classifier and the rest of the consumer-impact payload travel inside `evidence.kind: structured` under `evidence.data`. The two dimensions are independent: a `breaking` change may carry any `Diagnostic` severity, and an `additive` change may still warrant `suggestion` or `optional`.
