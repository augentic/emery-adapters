# `documentation.survey`

Walk `$SOURCE_DIR` (the read-only CID view of the bound docs tree) and emit one `Lead` per document surface. The caller persists the catalog; this prompt returns the lead-block payload only.

## Inputs

- `$SOURCE_DIR` — read-only CID view of the bound documentation set. Absent when the binding is an inline `value`. Never write here.
- **Source key** — the plan source-binding key the engine passed on the wire. The caller stamps each lead's `source` from it; this prompt does not emit it.
- **Optional parent lead** — when present, this is a focused survey: the exception path for a parent still coarser than a buildable boundary (a monolithic document), not a second walk to recover headings unfocused survey already emitted. Return stable child leads under that parent. Inherit parent/focus from the passed record; do not look it up in `leads.md` or `slices/`.
- `$SCRATCH_DIR` — write-only scratch space; use only if intermediate state is unavoidable.

The change home and `$PROJECT_DIR` are unreachable. Do not read `plan.yaml`, `leads.md`, or `slices/`. Unfocused survey always returns the complete current set from `$SOURCE_DIR`; do not consult a catalog to decide this is a re-survey.

## What is a document surface

A lead is a document surface — a file, or a top-level section of a monolithic file. It is a surface the docs name, not a work unit. Two recognition rules, in order:

1. **One file, one lead.** When `$SOURCE_DIR` holds multiple markdown files, treat each file's top heading (the first `# ...` H1) as a lead. Files without a top heading fall back to the kebab-cased filename stem.
2. **Monolithic file.** When `$SOURCE_DIR` holds a single markdown file with multiple top-level sections, treat each H1 (or each H2 when the file uses H1 as a title only) as a lead. Focused child survey is the exception for splitting a remaining monolithic file, not a second estate walk.

Skip files that contain no behavioural content (e.g. tables of contents, license boilerplate, glossaries). When in doubt, emit the lead — downstream reconciliation and operator review sort out false positives.

## Lead id and synopsis

- `lead`: kebab-case slug derived from the surface's heading. Lowercase, strip punctuation, replace whitespace with `-`. Example: `# Password reset` -> `password-reset`. Re-surveying the same source replaces by `(source, lead)`, so stability matters more than prettiness.
- `synopsis`: a content-bearing description lifted (or lightly compressed) from the surface's opening paragraph — the first non-heading, non-list paragraph after the heading. Name the surface's behaviour and its salient constraint so a same-slug lead from another source can be matched or distinguished on content, not just the shared slug. When a surface is cross-cutting guidance that applies across the other surfaces (a conventions or approach document) rather than a discrete behaviour, say so explicitly (e.g. `cross-cutting: applies to all flows in this set`) so downstream reconciliation can recognise it and bind it wherever it informs. Prefer one line and keep it tight (~200 characters); it MAY run to a few lines when one is too thin. Do not invent content the docs do not state, and never spill extract-time detail here — that is `documentation.extract`'s job.
- `topics` (optional): an inline list of kebab-case slugs naming the surface's domains, drawn from the heading and opening paragraph (e.g. `[identity, password]`). Author them only when the docs clearly support the classification; omit the bullet when unsure. They are extra grouping context for downstream reconciliation — never a grouping the CLI computes. Keep slugs stable across re-surveys for the same surface.

## Output

Return one block per lead, in alphabetical `lead` order. The caller persists the catalog; this prompt never writes `leads.md`.

```markdown
### password-reset

- lead: password-reset
- synopsis: Account service that lets a registered user request a password reset link by email.
- topics: [identity, password]
```

Field order is fixed (`lead`, `synopsis`, then optional `topics`). Do not emit `source`; the CLI stamps it from the survey binding. Cross-source merging happens downstream in the engine, not in this prompt — see [From sources to slices](../references/emery-runtime/reconciliation.md#plan-time-leads-become-slices) for how leads reconcile into slices.

## Worked example

Bound directory layout (relative to `$SOURCE_DIR`):

```text
account.md          # top heading: "Account"
password-reset.md   # top heading: "Password reset"
```

Expected output (alphabetically by `lead`):

```markdown
### account

- lead: account
- synopsis: Account service that stores per-user identity, credential, and notification preferences.

### password-reset

- lead: password-reset
- synopsis: Account service that lets a registered user request a password reset link by email.
```

A full input/output fixture for this example lives at [`quality/fixtures/reference/sources/documentation/`](https://github.com/augentic/emery/tree/main/quality/fixtures/reference/sources/documentation/) in the repo.

## Determinism

- Emit leads sorted alphabetically by `lead`.
- Field order inside each block is fixed: `lead`, `synopsis`, then optional `topics`.
- When emitted, `topics` slugs are ordered deterministically (e.g. as they appear, deduplicated) so re-running is byte-identical.
- No timestamps, host paths, or other run-state in the output — re-running against unchanged inputs produces byte-identical blocks.

## Guardrails

- `$SOURCE_DIR` is read-only. Reads outside it surface as `source-survey-path-denied`; never attempt to widen the preopen.
- Do not write the catalog — the caller owns persistence.
- Do not emit Evidence here. Per-claim extraction is `documentation.extract`'s job, run once per lead at extract time.
- Do not invent a lead the docs do not describe. Empty inventories (`$SOURCE_DIR` parseable but no behavioural content) are valid output.
