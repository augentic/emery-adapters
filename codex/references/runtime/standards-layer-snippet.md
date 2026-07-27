# Standards layer (runtime excerpt)

Emery separates workflow, artifacts, and engineering standards. Workflow **mutates** `.emery/` state through CLI verbs. Artifacts **record** product intent. Engineering standards **constrain** generated and hand-written code via rules under `codex/rules/` and per-adapter `prose/rules/` overlays, embedded in each target adapter's component and applied by its build review prompts.

Standards enforcement is **not** a workflow phase: findings may block a pipeline but never transition a slice or write lifecycle fields. Plan **Gate 1** (`emery plan approve`) is operator approval of a plan, not engineering-standards enforcement.

Full triad and enforcement tables: [Workflow, standards, and artifacts](https://emery.augentic.io/explanation/standards-layer.html).
