---
id: CORE-014
title: Brief Frontmatter Forbidden
severity: important
trigger: An adapter brief under `adapters/**/prose/briefs/` opens with a YAML frontmatter fence even though briefs are resolved by path only.
rule_hints:
  - kind: path-pattern
    value: "adapters/**/prose/briefs/guidance.md"
  - kind: path-pattern
    value: "adapters/**/prose/briefs/build.md"
  - kind: path-pattern
    value: "adapters/**/prose/briefs/merge.md"
  - kind: path-pattern
    value: "adapters/**/prose/briefs/survey.md"
  - kind: path-pattern
    value: "adapters/**/prose/briefs/extract.md"
  - kind: path-pattern
    value: "adapters/**/prose/briefs/build/**/*.md"
  - kind: path-pattern
    value: "adapters/**/prose/briefs/extract/**/*.md"
  - kind: regex
    value: "^---"
    description: Flag line 1 when the brief opens with a frontmatter delimiter.
---

## Rule

Adapter briefs are not skills. The loader resolves them from `adapter.yaml` paths and never reads brief frontmatter. A leading `---` block is always drift.

## Look For

- `---` on the first line of `adapters/sources/<name>/prose/briefs/*.md` or `adapters/targets/<name>/prose/briefs/*.md`.

## Fix

Strip the frontmatter fence and rely on the body H1 for the brief title. See [docs/standards/skill-authoring.md](../../../../docs/standards/skill-authoring.md#brief-authoring).
