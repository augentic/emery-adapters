# open-gap-fab — Design

## Context

Platforms in scope: `core`, `ios`. Screen slug for My Lists is `my-lists` (`MyLists` ViewModel variant, `MyListsView` per-page struct). This slice stages an intentional open GAP on FAB activation while `Page::NewList` already exists in the page enum — writers must stay stub-faithful or close under the open-GAP contract; naming pressure alone is not closure.

## Domain Model

### `App` struct

`TodoApp` — the top-level Crux app type.

### `Model`

```rust
pub struct Model {
    pub page: Page,
    pub lists: Vec<ListRow>,
}
```

### `Page` enum

Internal navigation. `NewList` is already grounded (sibling screen / prior-slice destination) — do **not** invent it solely to close the FAB GAP; it is available if B′ closure eligibility holds.

```rust
pub enum Page {
    #[default]
    MyLists,
    NewList,
}
```

### `ListRow`

```rust
pub struct ListRow {
    pub list_id: String,
    pub title: String,
    pub item_count: String,
}
```

### `Event`

```rust
pub enum Event {
    /// FAB tap — behaviour TBD; cannot wire navigation until the open GAP closes.
    CreateList,
}
```

### `ViewModel`

```rust
pub enum ViewModel {
    MyLists(MyListsView),
    NewList(NewListView),
}

pub struct MyListsView {
    pub lists: Vec<ListRowView>,
    pub fab_visible: bool,
}

pub struct ListRowView {
    pub list_id: String,
    pub title: String,
    pub item_count: String,
}

pub struct NewListView {
    pub title_placeholder: String,
}
```

`NewListView` is carried so the grounded destination screen already exists in design; this slice does not author NewList form behaviour.

### `Effect`

`Render` only.

## Adapters

- **Render** — My Lists layout and FAB on the iOS shell.
- No HTTP / Key-Value / Time / Platform in this slice.

## iOS shell

One SwiftUI screen for My Lists: scrollable list body, FAB anchored bottom-trailing dispatching `Event.CreateList` through `Core.update(...)`. Theme from `design-system/tokens.yaml`; FAB icon from `assets.yaml` (`add` symbol). NewList chrome may exist as a stub view bound to `ViewModel::NewList` but must not be reached by inventing FAB navigation while the GAP remains open.

## Implementation constraints

- Core-first: Crux core compiles and its tests pass before the shell is written.
- No Android tree in this slice.
- Open-GAP inventiveness: default stub-faithful for `CreateList`; concrete `Page::NewList` navigation only under B′ closure of build-editable markers (see open-gap contract).

## Risks / Open Questions

- FAB tap navigation target is unanswered — core models `CreateList` but cannot wire navigation until operator reconciles (or the same build honestly closes B′ surfaces).
