# Authority hierarchy

Every Evidence document carries one closed top-level `authority` class. Highest wins:

1. **`intent`** — inline operator directives (the `intent` source adapter is the only first-party emitter).
2. **`documentation`** — operator-provided written product or technical intent (internal docs, RFCs, product notes). Emitted by the `documentation` source adapter.
3. **`behaviour`** — what legacy code actually does. Emitted by behaviour sources such as `typescript` and future code or observation adapters.

Authority is a property of the whole Evidence document: one extract answer declares exactly one class, fixed per adapter. The **engine** resolves authority deterministically after every extract returns — an adapter never compares its claims against another source's, never marks winners, and never derives a status.

## How the engine applies it

When two sources' `requirement` claims share an id but their `statement` extras disagree:

- a unique highest-authority contributor wins — the spec block renders `Status: divergence` with the `[divergence]` tag, the winning statement as the operative body, and every losing value preserved as a `Note:` line naming its source and class;
- a tie at the top authority is unresolvable — the block renders `Status: conflict` with the `[conflict]` tag and one `Note:` line per contributing value, for the operator to reconcile.

The rendered `Sources:` list orders contributing source keys highest authority first. See [reconciliation.md](../reconciliation.md) for the full pipeline.

## What an extract prompt must do

- Declare the adapter's fixed class verbatim (`documentation` extracts always say `documentation`, never `intent`, even when the docs quote an operator).
- Nothing else. Precedence, winner selection, tagging, and rendering are engine-side; an extract answer that pre-resolves a disagreement (by dropping the losing value) destroys the audit trail the reviewer depends on.
