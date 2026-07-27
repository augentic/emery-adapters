# Example: Simple Counter (Render Only)

A minimal Crux app with local state and no external side-effects. Demonstrates the basic App trait, Model, Event, ViewModel, Effect, and testing patterns.

**Pin / FFI authority:** workspace pins, `shared/Cargo.toml` features, and `shared/src/ffi.rs` come from `$TEMPLATE_DIR` (BoltFFI `CoreFfi`). Example blocks below are pedagogical — do not reintroduce a `uniffi` cargo feature or `uniffi::setup_scaffolding!()`.

## Capabilities Used

- **Render** (built-in)

## Workspace `Cargo.toml`

```toml
[workspace]
members = ["shared"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.88"

# Pin versions from $TEMPLATE_DIR/Cargo.toml (workspace.dependencies) — never invent.
[workspace.dependencies]
crux_core = "{version from $TEMPLATE_DIR/Cargo.toml}"
serde = "{version from $TEMPLATE_DIR/Cargo.toml}"
facet = "{version from $TEMPLATE_DIR/shared/Cargo.toml}"
```

## `shared/Cargo.toml`

```toml
[package]
name = "shared"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[lib]
crate-type = ["cdylib", "lib", "staticlib"]

[[bin]]
name = "codegen"
required-features = ["codegen"]

[features]
shell_ios = []
shell_android = []
facet_typegen = ["crux_core/facet_typegen"]
codegen = [
    "dep:anyhow",
    "dep:clap",
    "dep:log",
    "dep:pretty_env_logger",
    "facet_typegen",
]

[dependencies]
crux_core.workspace = true
serde = { workspace = true, features = ["derive"] }
facet = "{version from $TEMPLATE_DIR/shared/Cargo.toml}"
boltffi = "{version from $TEMPLATE_DIR/shared/Cargo.toml}"
anyhow = { workspace = true, optional = true }
clap = { version = "4", optional = true, features = ["derive"] }
log = { version = "0.4", optional = true }
pretty_env_logger = { version = "0.5", optional = true }
```

## `shared/src/lib.rs`

```rust
mod app;
mod ffi;

pub use app::*;
pub use crux_core::Core;
pub use ffi::CoreFfi;
```

## `shared/src/app.rs`

```rust
use crux_core::{
    macros::effect,
    render::{render, RenderOperation},
    App, Command,
};
use facet::Facet;
use serde::{Deserialize, Serialize};

#[derive(Default)]
enum Page {
    #[default]
    Counter,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum Route {
    #[default]
    Counter,
}

#[derive(Default)]
pub struct Model {
    page: Page,
    count: isize,
}

#[derive(Facet, Serialize, Deserialize, Debug, Clone, Default)]
pub struct CounterView {
    pub count: String,
}

#[derive(Facet, Serialize, Deserialize, Debug, Clone, Default)]
#[repr(C)]
pub enum ViewModel {
    #[default]
    Counter(CounterView),
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum Event {
    Navigate(Route),
    Increment,
    Decrement,
    Reset,
}

#[effect(facet_typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
}

#[derive(Default)]
pub struct Counter;

impl App for Counter {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Navigate(route) => match route {
                Route::Counter => Command::done(),
            },
            Event::Increment => {
                model.count += 1;
                render()
            }
            Event::Decrement => {
                model.count -= 1;
                render()
            }
            Event::Reset => {
                model.count = 0;
                render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        match model.page {
            Page::Counter => ViewModel::Counter(CounterView {
                count: format!("Count is: {}", model.count),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crux_core::App as _;

    #[test]
    fn initial_view_shows_zero() {
        let app = Counter;
        let model = Model::default();

        let ViewModel::Counter(view) = app.view(&model);
        assert_eq!(view.count, "Count is: 0");
    }

    #[test]
    fn increment_updates_count() {
        let app = Counter;
        let mut model = Model::default();

        let mut cmd = app.update(Event::Increment, &mut model);
        assert_eq!(model.count, 1);

        cmd.expect_one_effect().expect_render();

        let ViewModel::Counter(view) = app.view(&model);
        assert_eq!(view.count, "Count is: 1");
    }

    #[test]
    fn decrement_updates_count() {
        let app = Counter;
        let mut model = Model::default();

        let mut cmd = app.update(Event::Decrement, &mut model);
        assert_eq!(model.count, -1);

        cmd.expect_one_effect().expect_render();

        let ViewModel::Counter(view) = app.view(&model);
        assert_eq!(view.count, "Count is: -1");
    }

    #[test]
    fn reset_sets_count_to_zero() {
        let app = Counter;
        let mut model = Model {
            count: 42,
            ..Model::default()
        };

        let mut cmd = app.update(Event::Reset, &mut model);
        assert_eq!(model.count, 0);

        cmd.expect_one_effect().expect_render();

        let ViewModel::Counter(view) = app.view(&model);
        assert_eq!(view.count, "Count is: 0");
    }

    #[test]
    fn sequence_of_events() {
        let app = Counter;
        let mut model = Model::default();

        let _ = app.update(Event::Increment, &mut model);
        let _ = app.update(Event::Increment, &mut model);
        let _ = app.update(Event::Increment, &mut model);
        let _ = app.update(Event::Decrement, &mut model);

        assert_eq!(model.count, 2);

        let ViewModel::Counter(view) = app.view(&model);
        assert_eq!(view.count, "Count is: 2");
    }
}
```

## `shared/src/ffi.rs`

Prefer the live `$TEMPLATE_DIR/shared/src/ffi.rs` (BoltFFI `#[boltffi::export]` on `CoreFfi`). Pedagogical shape:

```rust
use crux_core::{
    Core,
    bridge::{Bridge, EffectId},
};

use crate::Counter;

/// The main interface used by the shell.
pub struct CoreFfi {
    core: Bridge<Counter>,
}

impl Default for CoreFfi {
    fn default() -> Self {
        Self::new()
    }
}

#[boltffi::export]
impl CoreFfi {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: Bridge::new(Core::new()),
        }
    }

    #[must_use]
    pub fn update(&self, data: &[u8]) -> Vec<u8> {
        let mut effects = Vec::new();
        match self.core.update(data, &mut effects) {
            Ok(()) => effects,
            Err(e) => panic!("{e}"),
        }
    }

    #[must_use]
    pub fn resolve(&self, id: u32, data: &[u8]) -> Vec<u8> {
        let mut effects = Vec::new();
        match self.core.resolve(EffectId(id), data, &mut effects) {
            Ok(()) => effects,
            Err(e) => panic!("{e}"),
        }
    }

    #[must_use]
    pub fn view(&self) -> Vec<u8> {
        let mut view_model = Vec::new();
        match self.core.view(&mut view_model) {
            Ok(()) => view_model,
            Err(e) => panic!("{e}"),
        }
    }
}
```

## `rust-toolchain.toml`

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "rustc-dev"]
targets = [
    "aarch64-apple-darwin",
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "aarch64-linux-android",
    "wasm32-unknown-unknown",
    "x86_64-apple-ios",
]
adapter = "minimal"
```
