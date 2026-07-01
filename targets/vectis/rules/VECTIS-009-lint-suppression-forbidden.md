---
id: VECTIS-009
title: Lint Suppression Forbidden
severity: important
trigger: Agent-authored core Rust or platform shell sources carry inline compiler or linter suppressions instead of structural fixes.
applicability:
  adapters: [vectis]
rule_hints:
  - kind: regex
    value: '#\[(allow|expect)\('
    description: Inline Rust lint suppressions under shared/src.
  - kind: regex
    value: swiftlint:disable|swift-format-ignore
    description: Swift lint/format disable comments under iOS agent-authored sources (excluding generated/).
  - kind: regex
    value: '@Suppress\(|@file:Suppress'
    description: Kotlin suppressions under Android/app/src and Android/shared/src (excluding generated/).
references:
  - label: Core hard rules
    path: adapters/targets/vectis/references/hard-rules-core.md
  - label: iOS write brief
    path: adapters/targets/vectis/briefs/build/ios/write.md
  - label: Android write brief
    path: adapters/targets/vectis/briefs/build/android/write.md
---

## Rule

Agent-authored trees must compile and lint cleanly without inline suppressions. Repair by fixing structure — never by silencing the compiler or linter.

| Tree | Forbidden | Allowed (out of scope) |
| --- | --- | --- |
| `shared/src/**/*.rs` | `#[allow(...)]`, `#[expect(...)]` | Crate-level `[workspace.lints.clippy]` allows in scaffold-owned `Cargo.toml` |
| `iOS/**/*.swift` (excl. `generated/`) | `swiftlint:disable`, `swift-format-ignore` | CLI-owned `iOS/project.yml` strict flags |
| `Android/app/src/**/*.kt`, `Android/shared/src/**/*.kt` (excl. `generated/`) | `@Suppress(...)`, `@file:Suppress(...)` | Gradle `allWarningsAsErrors` in CLI-owned build scripts |

`specify-ignore:` comments are not a build-verify escape hatch and are not scanned by this rule.

## Look For

- `#[allow(dead_code)]` or `#[expect(clippy::…)]` added during verify-repair in `shared/src`.
- `// swiftlint:disable` or `// swift-format-ignore` in Swift screen or core bridge files.
- `@Suppress("UNUSED_PARAMETER")` or `@file:Suppress` in Kotlin composables or ViewModels.
- When `vectis verify --mode verify` reports `lint-suppression-forbidden`, treat it as a confirmed defect and cite `rule_id: VECTIS-009`.

## Spec Guidance

Remove the suppression and apply the structural fix documented in hard-rules and the platform write briefs. For Rust, prefer per-cap anchors, distinct match arms, and helper extraction. For Swift and Kotlin, use `_` prefixes, narrow types, or minimal handlers — never disable the linter.
