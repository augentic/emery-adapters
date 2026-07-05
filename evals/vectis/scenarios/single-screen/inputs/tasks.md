# daily-quote — Tasks

Tasks are organised by build phase: core first, the iOS shell second. Every task is agent-completable through writer or reviewer work and local build / test commands.

## Core

- [ ] Scaffold the Crux shared core (`specify extension run vectis -- scaffold core QuoteApp --caps http`); commit the generated workspace and `shared` crate.
- [ ] Implement the Domain Model from `design.md` in `shared/src/app.rs`: `QuoteApp`, `Model`, `Quote`, `Event`, `ViewModel`.
- [ ] Implement `update()` arms for `Refresh` and `QuoteLoaded` covering REQ-002; route the HTTP side effect through a `Command` chain.
- [ ] Implement `view()` to project `Model` into `ViewModel` with the loading state from REQ-001 and the preserved-quote error surface from REQ-002.
- [ ] Generate spec-traced tests: one synchronous `#[test]` per scenario across REQ-001..REQ-002 with `/// Spec: daily-quote > REQ-XXX > Scenario: ...` traceability comments.
- [ ] Run the core verify-repair loop: `cargo fmt --check`, `cargo check`, `cargo clippy --all-targets`, `cargo test`.

## iOS shell

- [ ] Scaffold the iOS shell (`specify extension run vectis -- scaffold ios QuoteApp --caps http`).
- [ ] Implement `DailyQuoteView` for the ViewModel: quote text, attribution, loading indicator, inline error, and the toolbar refresh button dispatching `Event.Refresh`.
- [ ] Regenerate shell-local theme from `design-system/tokens.yaml`; render the refresh icon as the `assets.yaml` platform symbol.
- [ ] Run the iOS verify loop from the build orchestrator: `specify extension run vectis -- sync ios-scaffold`, then the scaffold's build commands.
