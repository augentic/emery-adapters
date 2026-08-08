# Phase outcome contract

Emery has no per-slice `PhaseOutcome` stamp or `slice outcome set` CLI verb. `emery plan execute` (and the read-only `emery plan status` projection) classifies each phase's outcome from slice lifecycle, plan entry status, and the journal's phase-terminal events — not from an on-disk outcome field.

Durable run telemetry lives in per-writer logs at `.emery/events/<writer>.jsonl`; the closed journal event taxonomy is implemented in the engine. CLI verbs append structured JSON lines to the current journal writer's log as a side effect of each phase; adapter operations never read or write those files — their contribution to an outcome is the schema-gated report each operation answers with.

For adapter prompts the contract is: a `status: failure` report (or blocking findings the deterministic gates enforce) is how a phase signals a halt — the engine owns the resulting lifecycle state, journaling, and the loop's stop classification. Prompt-local sections describe what makes each phase's report blocking; none of them transition lifecycle or touch the journal.
