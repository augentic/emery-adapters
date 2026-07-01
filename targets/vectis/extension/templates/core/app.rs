use crux_core::{
    App, Command,
    macros::effect,
    render::{RenderOperation, render},
};
use facet::Facet;
use serde::{Deserialize, Serialize};
<<<CAP:http
use crux_http::HttpRequest;
CAP:http>>>
<<<CAP:kv
use crux_kv::KeyValueOperation;
CAP:kv>>>
<<<CAP:time
use crux_time::TimeRequest;
CAP:time>>>
<<<CAP:platform
use crux_platform::PlatformRequest;
CAP:platform>>>

#[derive(Default)]
enum Page {
    #[default]
    Home,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub enum Route {
    #[default]
    Home,
}

#[derive(Default)]
pub struct Model {
    page: Page,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default)]
pub struct HomeView {
    pub message: String,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default)]
#[repr(C)]
pub enum ViewModel {
    #[default]
    Loading,
    Home(HomeView),
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug)]
#[repr(C)]
pub enum Event {
    Navigate(Route),
    <<<CAP:http
    FetchData,
    #[serde(skip)]
    #[facet(skip)]
    Fetched(#[facet(opaque)] crux_http::Result<crux_http::Response<Vec<u8>>>),
    CAP:http>>>
    <<<CAP:kv
    LoadData,
    #[serde(skip)]
    #[facet(skip)]
    Loaded(#[facet(opaque)] Result<Option<Vec<u8>>, crux_kv::KeyValueError>),
    CAP:kv>>>
}

#[effect(facet_typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    <<<CAP:http
    Http(HttpRequest),
    CAP:http>>>
    <<<CAP:kv
    KeyValue(KeyValueOperation),
    CAP:kv>>>
    <<<CAP:time
    Time(TimeRequest),
    CAP:time>>>
    <<<CAP:platform
    Platform(PlatformRequest),
    CAP:platform>>>
}

// Capability type aliases. Render-only anchor stubs keep each alias live
// without inline lint suppressions; writer skills replace the stubs with
// real capability wiring during Update Mode.
<<<CAP:http
type Http = crux_http::Http<Effect, Event>;

#[must_use]
const fn http_capability_anchor() -> usize {
    std::mem::size_of::<Http>()
}
CAP:http>>>
<<<CAP:kv
type KeyValue = crux_kv::KeyValue<Effect, Event>;

#[must_use]
const fn kv_capability_anchor() -> usize {
    std::mem::size_of::<KeyValue>()
}
CAP:kv>>>
<<<CAP:time
type Time = crux_time::Time<Effect, Event>;

#[must_use]
const fn time_capability_anchor() -> usize {
    std::mem::size_of::<Time>()
}
CAP:time>>>
<<<CAP:platform
type Platform = crux_platform::Platform<Effect, Event>;

#[must_use]
const fn platform_capability_anchor() -> usize {
    std::mem::size_of::<Platform>()
}
CAP:platform>>>

// Caps without dedicated update arms (time, platform) are touched from
// Navigate so their aliases stay live under `-D warnings`.
const fn touch_orphan_capability_types() {
    <<<CAP:time
    let _ = time_capability_anchor();
    CAP:time>>>
    <<<CAP:platform
    let _ = platform_capability_anchor();
    CAP:platform>>>
}

#[derive(Default)]
pub struct __APP_STRUCT__;

impl App for __APP_STRUCT__ {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    // Render-only baseline: every capability event resolves to a bare
    // `render()` call. Per-cap anchor stubs give each arm a distinct
    // body so the scaffold stays clippy-clean under `-D warnings`.
    // Writer skills replace these arms with real logic during Update Mode.
    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Navigate(Route::Home) => {
                model.page = Page::Home;
                touch_orphan_capability_types();
                render()
            }
            <<<CAP:http
            Event::FetchData | Event::Fetched(_) => {
                let _ = http_capability_anchor();
                render()
            }
            CAP:http>>>
            <<<CAP:kv
            Event::LoadData | Event::Loaded(_) => {
                let _ = kv_capability_anchor();
                render()
            }
            CAP:kv>>>
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        match model.page {
            Page::Home => ViewModel::Home(HomeView {
                message: "Hello from __APP_NAME__".to_string(),
            }),
        }
    }
}
