# Example: KV Notes (Render + Key-Value)

A Crux app that persists notes locally using the Key-Value capability. Demonstrates CRUD operations, serialization to bytes, and KV effect testing.

**Pin / FFI authority:** pins and `shared/src/ffi.rs` come from `$TEMPLATE_DIR` (BoltFFI `CoreFfi`). Do not reintroduce a `uniffi` cargo feature or `uniffi::setup_scaffolding!()`.

## Capabilities Used

- **Render** (built-in)
- **Key-Value** (`crux_kv`)

## Workspace `Cargo.toml`

```toml
[workspace]
members = ["shared"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.88"

# Pin versions from $TEMPLATE_DIR/Cargo.toml — never invent.
[workspace.dependencies]
crux_core = "{version from $TEMPLATE_DIR/Cargo.toml}"
crux_kv = "{version from $TEMPLATE_DIR/Cargo.toml}"
serde = "{version from $TEMPLATE_DIR/Cargo.toml}"
serde_json = "{version from $TEMPLATE_DIR/Cargo.toml}"
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
facet_typegen = ["crux_core/facet_typegen", "crux_kv/facet_typegen"]
codegen = [
    "dep:anyhow",
    "dep:clap",
    "dep:log",
    "dep:pretty_env_logger",
    "facet_typegen",
]

[dependencies]
crux_core.workspace = true
crux_kv.workspace = true
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
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
use crux_kv::{KeyValueOperation, error::KeyValueError};
use facet::Facet;
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "notes";

type KeyValue = crux_kv::KeyValue<Effect, Event>;

// Domain types

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    pub id: usize,
    pub title: String,
    pub body: String,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteView {
    pub id: usize,
    pub title: String,
    pub body: String,
}

// Page (internal)

#[derive(Default)]
enum Page {
    #[default]
    Loading,
    NoteList,
    Error,
}

// Route (shell-navigable destinations)

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum Route {
    #[default]
    NoteList,
}

// Model

#[derive(Default)]
pub struct Model {
    page: Page,
    notes: Vec<Note>,
    next_id: usize,
    error_message: Option<String>,
    save_error: Option<String>,
}

// Per-page view structs

#[derive(Facet, Serialize, Deserialize, Debug, Clone, Default)]
pub struct NoteListView {
    pub notes: Vec<NoteView>,
    pub count: String,
    pub error: String,
}

#[derive(Facet, Serialize, Deserialize, Debug, Clone, Default)]
pub struct ErrorView {
    pub message: String,
    pub can_retry: bool,
}

// ViewModel

#[derive(Facet, Serialize, Deserialize, Debug, Clone, Default)]
#[repr(C)]
pub enum ViewModel {
    #[default]
    Loading,
    NoteList(NoteListView),
    Error(ErrorView),
}

// Events

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum Event {
    Load,
    Add(String, String),
    Remove(usize),
    Navigate(Route),

    #[serde(skip)]
    #[facet(skip)]
    Loaded(#[facet(opaque)] Result<Option<Vec<u8>>, KeyValueError>),

    #[serde(skip)]
    #[facet(skip)]
    Saved(#[facet(opaque)] Result<Option<Vec<u8>>, KeyValueError>),
}

// Effects

#[effect(facet_typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    KeyValue(KeyValueOperation),
}

// App

#[derive(Default)]
pub struct Notes;

impl Notes {
    fn save_notes(notes: &[Note]) -> Command<Effect, Event> {
        let bytes = serde_json::to_vec(notes).unwrap_or_default();
        KeyValue::set(STORAGE_KEY, bytes).then_send(Event::Saved)
    }
}

impl App for Notes {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Load => {
                KeyValue::get(STORAGE_KEY).then_send(Event::Loaded)
            }

            Event::Loaded(Ok(Some(bytes))) => {
                let notes: Vec<Note> =
                    serde_json::from_slice(&bytes).unwrap_or_default();
                model.next_id = notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
                model.notes = notes;
                model.page = Page::NoteList;
                render()
            }

            Event::Loaded(Ok(None)) => {
                model.notes = Vec::new();
                model.next_id = 1;
                model.page = Page::NoteList;
                render()
            }

            Event::Loaded(Err(e)) => {
                model.page = Page::Error;
                model.error_message = Some(format!("Failed to load: {e}"));
                render()
            }

            Event::Add(title, body) => {
                let note = Note {
                    id: model.next_id,
                    title,
                    body,
                };
                model.next_id += 1;
                model.notes.push(note);

                render().and(Self::save_notes(&model.notes))
            }

            Event::Remove(id) => {
                model.notes.retain(|n| n.id != id);

                render().and(Self::save_notes(&model.notes))
            }

            Event::Navigate(route) => match route {
                Route::NoteList => match model.page {
                    Page::Error => {
                        model.page = Page::Loading;
                        model.error_message = None;
                        Command::event(Event::Load)
                    }
                    Page::Loading | Page::NoteList => Command::done(),
                },
            },

            Event::Saved(Ok(_)) => {
                model.save_error = None;
                render()
            }

            Event::Saved(Err(e)) => {
                model.save_error = Some(format!("Save failed: {e}"));
                render()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        match model.page {
            Page::Loading => ViewModel::Loading,
            Page::NoteList => ViewModel::NoteList(NoteListView {
                notes: model
                    .notes
                    .iter()
                    .map(|n| NoteView {
                        id: n.id,
                        title: n.title.clone(),
                        body: n.body.clone(),
                    })
                    .collect(),
                count: format!(
                    "{} note{}",
                    model.notes.len(),
                    if model.notes.len() == 1 { "" } else { "s" }
                ),
                error: model.save_error.clone().unwrap_or_default(),
            }),
            Page::Error => ViewModel::Error(ErrorView {
                message: model.error_message.clone().unwrap_or_default(),
                can_retry: true,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crux_core::App as _;

    #[test]
    fn load_requests_kv_get() {
        let app = Notes;
        let mut model = Model::default();

        let mut cmd = app.update(Event::Load, &mut model);

        let request = cmd.expect_one_effect().expect_key_value();
        assert_eq!(
            request.operation,
            KeyValueOperation::Get {
                key: "notes".to_string(),
            }
        );
    }

    #[test]
    fn initial_view_is_loading() {
        let app = Notes;
        let model = Model::default();

        assert!(matches!(app.view(&model), ViewModel::Loading));
    }

    #[test]
    fn loaded_with_data_transitions_to_note_list() {
        let app = Notes;
        let mut model = Model::default();

        let notes = vec![
            Note { id: 1, title: "First".to_string(), body: "Body 1".to_string() },
            Note { id: 2, title: "Second".to_string(), body: "Body 2".to_string() },
        ];
        let bytes = serde_json::to_vec(&notes).unwrap();

        let mut cmd = app.update(Event::Loaded(Ok(Some(bytes))), &mut model);

        assert_eq!(model.notes.len(), 2);
        assert_eq!(model.next_id, 3);
        assert!(matches!(model.page, Page::NoteList));

        cmd.expect_one_effect().expect_render();
    }

    #[test]
    fn loaded_with_none_transitions_to_empty_note_list() {
        let app = Notes;
        let mut model = Model::default();

        let mut cmd = app.update(Event::Loaded(Ok(None)), &mut model);

        assert!(model.notes.is_empty());
        assert_eq!(model.next_id, 1);
        assert!(matches!(model.page, Page::NoteList));

        cmd.expect_one_effect().expect_render();
    }

    #[test]
    fn loaded_error_transitions_to_error_view() {
        let app = Notes;
        let mut model = Model::default();

        let mut cmd = app.update(
            Event::Loaded(Err(KeyValueError::Io {
                message: "corrupt".to_string(),
            })),
            &mut model,
        );

        assert!(matches!(model.page, Page::Error));
        assert!(model.error_message.is_some());

        cmd.expect_one_effect().expect_render();

        let ViewModel::Error(view) = app.view(&model) else {
            panic!("expected Error view");
        };
        assert!(view.can_retry);
    }

    #[test]
    fn navigate_from_error_reloads() {
        let app = Notes;
        let mut model = Model {
            page: Page::Error,
            error_message: Some("failed".to_string()),
            ..Model::default()
        };

        let mut cmd = app.update(Event::Navigate(Route::NoteList), &mut model);

        assert!(matches!(model.page, Page::Loading));
        assert!(model.error_message.is_none());

        let event = cmd.expect_one_event();
        assert_eq!(event, Event::Load);
    }

    #[test]
    fn navigate_from_loading_is_noop() {
        let app = Notes;
        let mut model = Model::default();

        let cmd = app.update(Event::Navigate(Route::NoteList), &mut model);

        assert!(matches!(model.page, Page::Loading));
        assert!(cmd.is_done());
    }

    #[test]
    fn navigate_from_note_list_is_noop() {
        let app = Notes;
        let mut model = Model {
            page: Page::NoteList,
            ..Model::default()
        };

        let cmd = app.update(Event::Navigate(Route::NoteList), &mut model);

        assert!(matches!(model.page, Page::NoteList));
        assert!(cmd.is_done());
    }

    #[test]
    fn add_note_updates_model_and_saves() {
        let app = Notes;
        let mut model = Model {
            page: Page::NoteList,
            notes: Vec::new(),
            next_id: 1,
            ..Model::default()
        };

        let mut cmd = app.update(
            Event::Add("Test".to_string(), "Content".to_string()),
            &mut model,
        );

        assert_eq!(model.notes.len(), 1);
        assert_eq!(model.notes[0].id, 1);
        assert_eq!(model.notes[0].title, "Test");
        assert_eq!(model.next_id, 2);

        cmd.expect_effect().expect_render();

        let request = cmd.expect_one_effect().expect_key_value();
        assert!(matches!(
            request.operation,
            KeyValueOperation::Set { key, .. } if key == "notes"
        ));
    }

    #[test]
    fn remove_note_updates_model_and_saves() {
        let app = Notes;
        let mut model = Model {
            page: Page::NoteList,
            notes: vec![
                Note { id: 1, title: "A".to_string(), body: "".to_string() },
                Note { id: 2, title: "B".to_string(), body: "".to_string() },
            ],
            next_id: 3,
            ..Model::default()
        };

        let mut cmd = app.update(Event::Remove(1), &mut model);

        assert_eq!(model.notes.len(), 1);
        assert_eq!(model.notes[0].id, 2);

        cmd.expect_effect().expect_render();
        let _ = cmd.expect_one_effect().expect_key_value();
    }

    #[test]
    fn view_shows_note_count() {
        let app = Notes;
        let model = Model {
            page: Page::NoteList,
            notes: vec![
                Note { id: 1, title: "A".to_string(), body: "".to_string() },
            ],
            next_id: 2,
            ..Model::default()
        };

        let ViewModel::NoteList(view) = app.view(&model) else {
            panic!("expected NoteList view");
        };
        assert_eq!(view.count, "1 note");
        assert_eq!(view.notes.len(), 1);
    }

    #[test]
    fn view_pluralizes_notes() {
        let app = Notes;
        let model = Model {
            page: Page::NoteList,
            notes: vec![
                Note { id: 1, title: "A".to_string(), body: "".to_string() },
                Note { id: 2, title: "B".to_string(), body: "".to_string() },
            ],
            next_id: 3,
            ..Model::default()
        };

        let ViewModel::NoteList(view) = app.view(&model) else {
            panic!("expected NoteList view");
        };
        assert_eq!(view.count, "2 notes");
    }

    #[test]
    fn saved_ok_clears_error() {
        let app = Notes;
        let mut model = Model {
            page: Page::NoteList,
            save_error: Some("previous failure".to_string()),
            ..Model::default()
        };

        let mut cmd = app.update(Event::Saved(Ok(None)), &mut model);

        assert!(model.save_error.is_none());
        cmd.expect_one_effect().expect_render();

        let ViewModel::NoteList(view) = app.view(&model) else {
            panic!("expected NoteList view");
        };
        assert!(view.error.is_empty());
    }

    #[test]
    fn saved_error_surfaces_in_note_list_view() {
        let app = Notes;
        let mut model = Model {
            page: Page::NoteList,
            notes: vec![
                Note { id: 1, title: "A".to_string(), body: "".to_string() },
            ],
            next_id: 2,
            ..Model::default()
        };

        let mut cmd = app.update(
            Event::Saved(Err(KeyValueError::Io {
                message: "disk full".to_string(),
            })),
            &mut model,
        );

        assert!(model.save_error.is_some());
        cmd.expect_one_effect().expect_render();

        let ViewModel::NoteList(view) = app.view(&model) else {
            panic!("expected NoteList view");
        };
        assert!(!view.error.is_empty());
        assert!(view.error.contains("disk full"));
        assert_eq!(view.notes.len(), 1, "notes remain intact after save failure");
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

use crate::Notes;

pub struct CoreFfi {
    core: Bridge<Notes>,
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