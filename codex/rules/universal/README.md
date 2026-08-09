# Shared engineering standards (UNI-\*)

Shared **engineering standards** catalog — target-agnostic rules under `codex/`. Codex is the on-disk rule format; these files are durable policy, not workflow state or slice artifacts. Read by every code-generating target adapter's build review prompt during the build phase of `emery plan execute`: the pack is embedded into each adapter component via a `prose/rules/universal` symlink and served by the adapter's references server. Findings cite a rule here as a stable `rule_id` (for example `UNI-014`) alongside a report-local occurrence id (for example `UNI-3`) in `REVIEW.md`.

See [docs/explanation/standards-layer.md](https://github.com/augentic/emery/blob/main/docs/explanation/standards-layer.md) for how engineering standards relate to workflow, artifacts, and `docs/standards/` (authoring house style).

This directory owns the `UNI-*` namespace. Target-specific rules live in per-adapter overlays under `targets/<name>/prose/rules/` (omnia: `OMNIA-*` / `RUST-*` / `SEC-*`; contracts: `IFACE-*`; vectis: `VECTIS-*`). Source-adapter overlays under `sources/<name>/prose/rules/` share a single namespace, `SRC-*`, so any new source adapter that grows an overlay opts into `SRC-*` without coordinating a per-adapter namespace. Namespace ownership is applied in review — keep ids unique and prefix-correct when adding rules.

Sibling shared hook directory: [`codex/references/replay/`](../../references/replay/) — shared build-time replay hook contract for targets that opt in.

## Rule inventory

Rules are grouped by severity (highest first). `UNI-*` ids are stable citation keys — they are not renumbered when severity or grouping changes.

**Enforcement mode.** Every `UNI-*` rule is applied as a **model-assisted review finding** by the target build review prompts during the build phase; none gate deterministically. Rules are agent-readable prose — there is no deterministic hint machinery.

### Critical

| ID      | File                                                           |
| ------- | -------------------------------------------------------------- |
| UNI-002 | [`unvalidated-input.md`](unvalidated-input.md)                 |
| UNI-006 | [`concurrency-issues.md`](concurrency-issues.md)               |
| UNI-010 | [`unhandled-exceptions.md`](unhandled-exceptions.md)           |
| UNI-018 | [`hardcoded-secrets.md`](hardcoded-secrets.md)                 |
| UNI-019 | [`injection-vulnerabilities.md`](injection-vulnerabilities.md) |
| UNI-020 | [`unsafe-deserialization.md`](unsafe-deserialization.md)       |
| UNI-021 | [`missing-auth.md`](missing-auth.md)                           |

### Important

| ID      | File                                                                   |
| ------- | ---------------------------------------------------------------------- |
| UNI-001 | [`uninitialised-defaults.md`](uninitialised-defaults.md)               |
| UNI-003 | [`serialization-failures.md`](serialization-failures.md)               |
| UNI-004 | [`logic-bugs.md`](logic-bugs.md)                                       |
| UNI-005 | [`resource-leaks.md`](resource-leaks.md)                               |
| UNI-007 | [`chatty-external-calls.md`](chatty-external-calls.md)                 |
| UNI-008 | [`instrumentation-issues.md`](instrumentation-issues.md)               |
| UNI-009 | [`handle-then-throw.md`](handle-then-throw.md)                         |
| UNI-011 | [`missing-timeout-retry.md`](missing-timeout-retry.md)                 |
| UNI-012 | [`persisted-state-compatibility.md`](persisted-state-compatibility.md) |
| UNI-014 | [`hardcoded-configuration.md`](hardcoded-configuration.md)             |
| UNI-015 | [`stale-closure-captures.md`](stale-closure-captures.md)               |
| UNI-017 | [`type-safety-erosion.md`](type-safety-erosion.md)                     |

### Suggestion

| ID      | File                                                   |
| ------- | ------------------------------------------------------ |
| UNI-013 | [`dead-code.md`](dead-code.md)                         |
| UNI-016 | [`error-message-quality.md`](error-message-quality.md) |

## File shape

Each rule is a small markdown file with YAML frontmatter followed by a required `## Rule` heading. The minimum form:

```markdown
---
id: UNI-NNN
title: Short human title
severity: critical | important | suggestion | optional
trigger: One-sentence condition that tells a reviewer when this rule matters.
---

## Rule

What the rule actually requires, in prose.

## Look For

- Concrete code patterns or smells that hint the rule is being violated.
```

Optional frontmatter fields (`applicability`, `references`, `deprecated`) may ride along. `id` must be globally unique across every codex tree.

## How rules are consumed

Target review prompts read this directory directly and apply each rule with target-specific heuristics:

- **Omnia** — [`targets/omnia/prose/prompts/build/review.md`](../../../targets/omnia/prose/prompts/build/review.md) phase 3 ("Universal checks (lead)") applies every `UNI-*` rule in the inventory above, skipping rules already covered by the SEC / COR / QUA specialists per the table in [`review-categories.md`](../../../targets/omnia/prose/references/review-categories.md).
- **Vectis** — [`targets/vectis/prose/references/review/universal-checks.md`](../../../targets/vectis/prose/references/review/universal-checks.md) lists the Crux/Rust heuristics for each `UNI-*` and the overlaps to skip.
- **Contracts** — [`docs/reference/targets/contracts.md`](https://github.com/augentic/emery/blob/main/docs/reference/targets/contracts.md) cites its overlay alongside this shared set.

A review finding always carries:

- a report-local occurrence id (`UNI-1`, `UNI-2`, …) that restarts in each `REVIEW.md`, and
- a stable `rule_id` (`UNI-014`, `OMNIA-002`, …) that cites the codex file.

Adapter overlays are preferred over the shared rule when both match — e.g. a hardcoded secret in Omnia handler code maps to `SEC-001`, not `UNI-018`.

## Adding or evolving rules

1. Pick the next free `UNI-NNN`. Do not reuse retired ids; mark old rules with a `deprecated:` block in the frontmatter and keep the file so historical citations still resolve.
2. Create the file with the frontmatter and `## Rule` heading shown above.
3. Wire the new id into any target review references that should apply it (Omnia [`review-categories.md`](../../../targets/omnia/prose/references/review-categories.md), Vectis [`universal-checks.md`](../../../targets/vectis/prose/references/review/universal-checks.md), etc.) — no check verifies that every consumer cites every rule, so coverage is a manual concern.
4. Confirm frontmatter validity, the `## Rule` body heading, namespace ownership, and id uniqueness across the shared tree and every per-adapter overlay in review.

`README.md` files (case-insensitive) under any codex directory are reserved for index pages like this one — they are never treated as rules.
