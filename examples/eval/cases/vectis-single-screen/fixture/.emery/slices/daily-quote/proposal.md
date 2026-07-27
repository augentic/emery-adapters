# daily-quote — Proposal

## Why

Operators want the smallest possible cross-platform feature to prove the vectis build leg end-to-end: one screen, one refresh action, a Crux shared core, and a single iOS shell.

## What Changes

- New `daily-quote` feature backed by a Crux shared core with an HTTP capability.
- iOS shell (SwiftUI, single navigation stack) rendering the core's ViewModel directly.
- No Android shell in this slice — the project's declared platform set is `core, ios`.

## Domains

### New Domains

- **daily-quote** — a read-only Daily Quote screen: one quote with attribution, a loading state, and a refresh action.

### Modified Domains

None — this is a greenfield slice.

## Platforms

- core
- ios

## Impact

- The slice depends on a local HTTP backend that returns one `Quote` record per request; no external API contracts change.
- No persistence: the quote lives only in the core `Model`.
- Shell-local theme code renders from `design-system/tokens.yaml`; the only asset is a symbol-kind refresh icon, so no exports are materialized.
