---
id: VECTIS-009
title: Lint Suppression Forbidden
severity: important
trigger: Agent-authored core Rust or platform shell sources carry inline compiler or linter suppressions instead of structural fixes.
applicability:
  adapters: [vectis]
references:
  - label: Core hard rules
    path: adapters/targets/vectis/prose/references/hard-rules-core.md
  - label: iOS write prompt
    path: adapters/targets/vectis/prose/prompts/build/ios/write.md
  - label: Android write prompt
    path: adapters/targets/vectis/prose/prompts/build/android/write.md
---

## Rule

Agent-authored trees must compile and lint cleanly without inline suppressions. Repair by fixing structure — never by silencing the compiler or linter.

| Tree | Forbidden | Allowed (out of scope) |
| --- | --- | --- |
| `shared/src/**/*.rs` | `#[allow(...)]`, `#[expect(...)]` | Crate-level `[workspace.lints.clippy]` allows in scaffold-owned `Cargo.toml` |
| `iOS/**/*.swift` (excl. `generated/`) | `swiftlint:disable`, `swift-format-ignore` | Template-owned `iOS/project.yml` / Makefile DX from `$TEMPLATE_DIR` |
| `Android/app/src/**/*.kt` (excl. `generated/`) | `@Suppress(...)`, `@file:Suppress(...)` | Template-owned Gradle DX from `$TEMPLATE_DIR` |
| `Android/generated/**` BoltFFI / typegen output | *(out of scope — generated)* | `:shared` compiles generated sources as emitted |

## Look For

- `#[allow(dead_code)]` or `#[expect(clippy::…)]` added during verify-repair in `shared/src`.
- `// swiftlint:disable` or `// swift-format-ignore` in Swift screen or core bridge files.
- `@Suppress("UNUSED_PARAMETER")` or `@file:Suppress` in Kotlin composables or ViewModels.
- When the in-guest shell-verify gate findings riding the report-leg prompt include `lint-suppression-forbidden`, treat it as a confirmed defect and cite `rule_id: VECTIS-009`.

## Spec Guidance

Remove the suppression and apply the structural fix documented in hard-rules and the platform write prompts. For Rust, prefer per-cap anchors, distinct match arms, and helper extraction. For Swift and Kotlin, use `_` prefixes, narrow types, or minimal handlers — never disable the linter.
