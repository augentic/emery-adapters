# open-gap-fab — Tasks

Tasks are organised by build phase: core first, the iOS shell second. Every task is agent-completable through writer or reviewer work and local build / test commands.

## Core

- [ ] Confirm the adapter's deterministic core scaffold (`TodoApp`) is in place; commit the generated workspace and `shared` crate.
- [ ] Implement the Domain Model from `design.md` in `shared/src/`: `TodoApp`, `Model`, `Page` (including grounded `NewList`), `ListRow`, `Event::CreateList`, `ViewModel` / `MyListsView` / `NewListView`.
- [ ] Implement `update()` / `view()` for My Lists rendering (REQ-021, REQ-022, REQ-024). For `CreateList` (REQ-026 open GAP): keep stub-faithful (`render()`, page unchanged) unless the same core leg closes B′ build-editable markers and wires grounded `Page::NewList`.
- [ ] Generate spec-traced tests: one synchronous `#[test]` per scenario across REQ-021..REQ-026 with `/// Spec:` traceability comments. Open-GAP REQ-026 asserts unchanged page — never invent `Page::NewList` while markers remain.
- [ ] Run the core verify-repair loop: `cargo fmt --check`, `cargo check`, `cargo clippy --all-targets`, `cargo test`.

## iOS shell

- [ ] Confirm the adapter's deterministic iOS shell scaffold is in place.
- [ ] Implement My Lists SwiftUI view: list body, FAB dispatching `Event.CreateList`, theme from tokens, FAB icon from assets.
- [ ] Run the iOS verify loop from the build orchestrator.
