# daily-quote — Design

## Context

Platforms in scope: `core`, `ios`. The slice realises the `daily-quote` feature on a Crux shared core with an HTTP adapter; the iOS shell renders the core's ViewModel directly. `tokens.yaml` is operator-curated and consumed by the shell writer; the only asset is a symbol-kind refresh icon, so no exports are materialized.

## Domain Model

### `App` struct

`QuoteApp` — the top-level Crux app type.

### `Model`

```rust
pub struct Model {
    pub quote: Option<Quote>,
    pub loading: bool,
    pub last_error: Option<String>,
}
```

### `Quote`

```rust
pub struct Quote {
    pub text: String,
    pub author: String,
}
```

### `Event`

```rust
pub enum Event {
    Refresh,
    #[serde(skip)]
    QuoteLoaded(Result<Quote, String>),
}
```

`Refresh` issues one HTTP GET through the `Http` capability; `QuoteLoaded` is the internal callback that folds the result into `Model` (success replaces `quote`, failure sets `last_error` and preserves the previous quote per REQ-002).

### `ViewModel`

```rust
pub struct ViewModel {
    pub quote_text: String,
    pub quote_author: String,
    pub loading: bool,
    pub error: Option<String>,
}
```

## Adapters

- **HTTP** — `GET /quote` returns `{"text": "...", "author": "..."}`; the endpoint host is shell-configured.

## iOS shell

One SwiftUI screen (`DailyQuoteView`) bound to the ViewModel: quote text, author attribution, a loading indicator while `loading`, an inline error message when `error` is set, and a toolbar refresh button dispatching `Event.Refresh` through `Core.update(...)`. Theme code renders from `design-system/tokens.yaml`; the refresh icon renders as the platform symbol declared in `assets.yaml` (`arrow.clockwise`).

## Implementation constraints

- Core-first: the Crux core compiles and its tests pass before the shell is written.
- No persistence, no navigation stack beyond the single screen, no Android tree in this slice.
