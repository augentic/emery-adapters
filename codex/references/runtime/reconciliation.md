# From sources to a spec

How one `emery specify` run turns bound sources into a reviewable specification, and where an extract prompt's output lands in that pipeline.

## The pipeline

1. **Extract** — the engine dispatches one `extract` per authored source binding. Each call receives the whole bound source (a read-only workspace view, or an inline value) and returns one Evidence document: a document-level `authority` class and a flat list of typed claims. There is no survey step and no lead catalog; extraction mines the entire source in one pass.
2. **Gate** — the engine validates every claim against the closed required-extras table before anything else runs: a `requirement` claim must carry a `statement` extra, a `criterion` claim a `criterion` extra, an `example` claim a `replay-digest` extra. A claim missing its required extra fails the whole run closed (typed `bad_request`) naming the source, claim, and missing key. There is no partial acceptance and no fallback to `synopsis`.
3. **Reconcile** — deterministic engine code (no model) groups `requirement` claims by their dotted-kebab `id` across all sources. Within a group it compares the `statement` extras: matching statements are `agreed`; disagreeing statements resolve by authority precedence (a unique highest-authority contributor wins as `divergence`; a tie at the top authority is a `conflict`). A requirement with no `criterion` claim whose id equals it or extends it (`<requirement-id>.<suffix>`) gets an appended `[unknown]` acceptance gap row.
4. **Synthesise** — a model renders `spec.md` around the reconciliation rows (which it must reproduce verbatim — a fail-closed AST and row gate refuses drift) and `design.md` from the full claim set.
5. **Commit** — the engine writes the generation atomically and swaps the `current` pointer. Adapters never write artifacts; the returned Evidence is the entire contribution.

## What this means for extract prompts

- **Claim ids are the cross-source join key.** Reconciliation only ever connects claims whose ids are byte-equal. Derive ids from the domain concept the claim describes (`session.timeout`, `password-reset.expiry`), never from file layout, position, or invented counters — two independent sources describing the same behaviour must converge on the same id.
- **`requirement` claims are the reconciliation currency.** Only `kind: requirement` claims form spec requirement blocks and can agree, diverge, or conflict. Detail kinds (`section`, `excerpt`, `type`, `call`, …) reach synthesis as supporting context but never form rows.
- **`criterion` ids must extend their requirement's id.** The acceptance-coverage rule keys on the prefix: a criterion for `session.timeout` must carry id `session.timeout` or `session.timeout.<suffix>`. A criterion with an unrelated id leaves its requirement flagged `[unknown]`.
- **Statements are compared, so quote precisely.** The `statement` extra is the value reconciliation compares (whitespace-normalised, nothing else). Paraphrase drift between sources manufactures false conflicts; verbatim quoting where the source is prose, and precise present-tense observation where it is code, keeps agreement honest.
- **Gaps are preserved, never guessed.** When the source does not answer something, emit nothing for it. The engine renders missing coverage as `[unknown]`; a fabricated claim corrupts the spec silently, an absent one is surfaced to the reviewer.
- **Do not pre-resolve disagreement.** Stamp the adapter's fixed `authority` class and emit the claim the source supports. Precedence, winners, and `Status:` are engine-side; dropping a value another source will contradict destroys the audit trail.
