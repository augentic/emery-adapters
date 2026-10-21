//! The composition document, parsed into the shape an emitter renders.
//!
//! `vx spec` has already run by the time anything here is constructed, so this layer is a
//! projection rather than a second set of rules: it reads what the schema guarantees and refuses
//! only what it cannot render. Parsing into types — instead of walking a `Value` at the point of
//! emission — is what keeps four emitters agreeing about what a document *says*, leaving them to
//! disagree only about how to spell it.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::Error;
use crate::format::{Display, Format};
use crate::naming::{camel, pascal};

/// Everything one composition document asks the emitters to produce.
#[derive(Debug, Clone, Default)]
pub struct Surface {
    /// Screens, in slug order.
    pub screens: Vec<Screen>,

    /// `component:` groups, one per slug, in slug order.
    ///
    /// Structural identity guarantees every group sharing a slug shares a skeleton, so the first
    /// occurrence stands for all of them.
    pub components: Vec<Component>,

    /// Every `custom:` element the document declares, by name, each with the brief a model
    /// implements it from. Read from the document-level `custom_items` array the compiler writes.
    pub custom: Vec<CustomItem>,

    /// The app's bottom tab menu, when the composition has one: `footer_menu`.
    ///
    /// Not part of any screen. It is drawn once, by the root router around whichever screen is
    /// routed, so no screen file holds a copy of it and no component carries the selected tab as a
    /// parameter. See `handoff-footer-menu.md`.
    pub footer_menu: Option<FooterMenu>,
}

/// `footer_menu`: one bottom tab menu for the whole app.
#[derive(Debug, Clone)]
pub struct FooterMenu {
    /// The event a tab raises, by name. Its one argument is the tab's route, which is not written
    /// in the composition; see [`Self::raises`]. A hole while `TODO(event)` is unfilled.
    pub event: Event,

    /// The screens that draw the menu, in the composition's order.
    pub screens: Vec<String>,

    /// Which tab each screen shows selected, screen slug to tab route. A screen that draws the
    /// menu and is absent here selects none.
    pub selected: BTreeMap<String, String>,

    /// The bar the tabs sit in, as drawn: a group with the bar's box and no items of its own.
    pub bar: Group,

    /// The tabs, in the order the bar draws them.
    pub tabs: Vec<Tab>,
}

/// One tab of the [`FooterMenu`].
#[derive(Debug, Clone)]
pub struct Tab {
    /// The screen it opens, a screen slug.
    pub route: String,

    /// Its caption.
    pub label: String,

    /// Its icon, an `assets.yaml` key.
    pub icon: String,

    /// The tab drawn selected: its box from the selected look, holding the icon and the label,
    /// and raising the menu's event when tapped.
    pub selected: Group,

    /// The same tab drawn unselected.
    pub unselected: Group,
}

impl FooterMenu {
    /// Whether `screen` draws the menu.
    #[must_use]
    pub fn shows_on(&self, screen: &str) -> bool {
        self.screens.iter().any(|shown| shown == screen)
    }

    /// The tab `screen` shows selected, when it draws the menu and selects one.
    #[must_use]
    pub fn selected_on(&self, screen: &str) -> Option<&Tab> {
        let route = self.selected.get(screen)?;
        self.tabs.iter().find(|tab| &tab.route == route)
    }

    /// The event a tap on `tab` raises: the menu's event, carrying the tab's route as the `Route`
    /// variant it is spelled as. `None` while the event is still a hole, which only
    /// `--allow-holes` reaches.
    #[must_use]
    pub fn raises(&self, tab: &Tab) -> Option<Event> {
        (!self.event.is_hole()).then(|| Event {
            name: self.event.name.clone(),
            args: vec![pascal(&tab.route)],
        })
    }

    /// The bar as drawn with `selected` selected, or with no tab selected: the bar's box holding
    /// every tab in the look that state draws it in.
    ///
    /// One whole bar per state rather than one condition per tab. Every renderer already draws a
    /// group, and a bar is only ever in one of `tabs.len() + 1` states, so this keeps the menu on
    /// the same path every other drawn group takes instead of teaching three walkers a conditional
    /// node that nothing else in a composition needs.
    #[must_use]
    pub fn bar_with(&self, selected: Option<&str>) -> Group {
        let mut bar = self.bar.clone();
        bar.items = self
            .tabs
            .iter()
            .map(|tab| {
                let look = if selected == Some(tab.route.as_str()) {
                    &tab.selected
                } else {
                    &tab.unselected
                };
                Node::Group(Box::new(look.clone()))
            })
            .collect();
        bar
    }
}

/// One `custom:` element the document declares: an out-of-scope depiction a model fills in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomItem {
    /// The kebab-case name, which is also the item type its instances carry.
    pub name: String,

    /// The prose brief, the only thing a model has to build the element from.
    pub brief: Option<String>,
}

/// One `component:` group, lifted out of the screens that use it.
#[derive(Debug, Clone)]
pub struct Component {
    /// The kebab-case slug from `component:`.
    pub slug: String,

    /// The generated type name, which is the slug unless the slug emits more than one variant.
    pub name: String,

    /// The variant this is, as the subset of `params:` that tells the slug's placements apart.
    ///
    /// A Figma set draws its axes as variants — `importance=destructive` is a different background,
    /// a different corner radius and the error colour — and a placement selects one. Keying the
    /// lift on the slug alone took the first placement's artwork for all of them, so Delete
    /// rendered as a primary button. Axes every placement agrees on are left out, which keeps
    /// `state=default` from putting `Default` in every generated name.
    pub key: BTreeMap<String, String>,

    /// Copy that differs between the placements sharing this variant, as parameters.
    ///
    /// Structural identity makes the *shape* shared, not the words in it: one `button` variant is
    /// "Get Organised!" on the splash and "Cancel" in a dialog. The literal cannot be baked into
    /// the lifted tree, so the position becomes a parameter and each call site passes its own.
    pub copy: Vec<CopySlot>,

    /// Artwork each placement chooses, as parameters holding an `assets.yaml` key.
    ///
    /// Declared by the design — an `INSTANCE_SWAP` property, or an exposed nested instance — and
    /// passed at each call site as `{ asset: key }`. Never inferred from placements whose artwork
    /// differs: a parameter list that changed shape with how the component happened to be used was
    /// rejected for `MetricTile`.
    pub assets: Vec<CopySlot>,

    /// The group as authored, minus its `component:` marker.
    pub group: Group,

    /// Values the component binds, in first-appearance order — its parameters, because a
    /// component cannot reach a view model.
    pub bindings: Vec<Binding>,

    /// Events the component raises, in first-appearance order — its callbacks, for the same
    /// reason.
    pub events: Vec<Callback>,
}

/// One piece of copy a component takes as a parameter, and where in its tree it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopySlot {
    /// The parameter name, identical at the definition and at every call site.
    pub name: String,

    /// Child indices from the component's root down to the item that shows it.
    pub path: Vec<usize>,

    /// The component TEXT property this slot is, when the design declared it — the key each
    /// placement's `args:` passes it under. `None` for a slot inferred from placements whose words
    /// differ, which is read off each placement's tree instead.
    pub arg: Option<String>,
}

/// What one placement passes to one of its component's properties.
///
/// Declared at the call site by the compiler rather than inferred from which words or artwork
/// differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// The placement's own words.
    Literal(String),

    /// A view-model field of the screen the placement sits on.
    Bind(String),

    /// The `assets.yaml` key of the artwork the placement shows.
    Asset(String),
}

/// What a call site hands a [`CopySlot`]: literal copy, or a field to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said<'a> {
    /// Copy, to be looked up in the copy contract.
    Literal(&'a str),

    /// A view-model path, to be read through the frame in scope.
    Bind(&'a str),

    /// A view-model path holding a typed display value, to be read through the frame in scope and
    /// formatted at the call site with the device locale.
    ///
    /// Formatted where it is passed rather than inside the component, because the slot is a
    /// `String` every placement of the variant shares: one placement may hand it a literal
    /// ("Hectopascals") while another hands it a bound unit, and only a string takes both.
    Formatted(&'a str, Format),
}

/// What `placement` passes for `slot`: its declared argument when it has one, else the words its
/// own tree draws at the slot's path.
#[must_use]
pub fn said<'a>(placement: &'a Group, slot: &CopySlot) -> Said<'a> {
    match slot.arg.as_ref().and_then(|arg| placement.args.get(arg)) {
        Some(Arg::Literal(words)) => Said::Literal(words),
        Some(Arg::Bind(field)) => slot
            .arg
            .as_ref()
            .and_then(|arg| placement.formats.get(arg))
            .map_or(Said::Bind(field), |format| Said::Formatted(field, *format)),
        Some(Arg::Asset(_)) | None => Said::Literal(
            at_path(placement, &slot.path)
                .and_then(|item| item.content.as_deref())
                .unwrap_or_default(),
        ),
    }
}

/// The asset key `placement` passes for an [`Component::assets`] slot: its declared argument, else
/// the artwork its own tree draws at the slot's path.
#[must_use]
pub fn shown<'a>(placement: &'a Group, slot: &CopySlot) -> &'a str {
    match slot.arg.as_ref().and_then(|arg| placement.args.get(arg)) {
        Some(Arg::Asset(key)) => key,
        _ => {
            at_path(placement, &slot.path).and_then(|item| item.name.as_deref()).unwrap_or_default()
        }
    }
}

/// One event a component raises, named by where it sits rather than by what it is called.
///
/// The distinction is the whole point. A component is lifted once, from the first placement that
/// mentions its slug, and then *called* everywhere the slug appears — so every name in its
/// signature has to be one that every placement agrees on. A bind path is such a name: it is read
/// against whatever element is in scope, so two rows of the same card both bind `title`. An event
/// name is not. `What this tap does` is a decision per placement, so one `button` set answers
/// `CreateList` on one screen and `DeleteList` on another.
///
/// Naming the parameter after the event therefore built the struct from the first placement's
/// vocabulary and every call from its own: `ButtonComponent` declared `onGetOrganised` while the
/// confirmation dialog passed `onDeleteList`, and the shell did not compile. The site is the only
/// thing the placements share, so the site is what names the parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    /// The parameter name, identical at the definition and at every call site.
    pub name: String,

    /// The event this placement raises through it.
    pub event: Event,
}

/// One screen.
#[derive(Debug, Clone)]
pub struct Screen {
    /// The kebab-case slug, which is the map key.
    pub slug: String,

    /// The human name, which becomes the navigation title.
    pub name: String,

    /// How the screen frame arranges its body across the cross axis.
    ///
    /// The screen frame's own, not any group's inside it. A renderer that ignores this puts a body
    /// the design centred against the leading edge, which is what every renderer did before the
    /// composition carried it.
    pub align: Option<String>,

    /// How the screen frame distributes its body along the main axis. See [`Self::align`].
    pub justify: Option<String>,

    /// The colour token the screen is drawn on, from the screen frame's own fill.
    ///
    /// Painted full-bleed behind every region. Absent when the frame has no fill, and the
    /// platform's own background stands.
    pub background: Option<String>,

    /// The screen frame's own inset, from the root frame's padding.
    ///
    /// Insets header, body and footer alike, as a padded root insets every child in Figma; the
    /// background stays full-bleed. Measured from the safe area, which is where the frame's edges
    /// land on a device — the same reading a header's own top padding already gets.
    pub padding: Option<Padding>,

    /// The navigation bar.
    pub header: Option<Header>,

    /// The main content region.
    pub body: Option<Body>,

    /// Bottom bar content.
    pub footer: Vec<Node>,

    /// The floating action button.
    pub fab: Option<Item>,

    /// Alternate renderings, keyed by the condition that selects them.
    pub states: Vec<State>,

    /// Dialogs, sheets and snackbars over this screen.
    pub overlays: Vec<Overlay>,
}

/// A screen's navigation bar.
#[derive(Debug, Clone, Default)]
pub struct Header {
    /// The title, when the header sets one: literal copy, or a field of the screen's view model.
    ///
    /// Two of the shapes a placement's [`Arg`] takes, and for the same reason. A title the
    /// design declares data (`bind:` on the drawn text) has no fixed words, and a `String` here
    /// could only have carried the one sample the designer happened to draw. Never
    /// [`Arg::Asset`]: the parse reads only `bind:`, and the emitters treat one as no title.
    pub title: Option<Arg>,

    /// The typography token the title is drawn in, when the design states one.
    pub style: Option<String>,

    /// The colour token the title is drawn in, when the design states one.
    pub color: Option<String>,

    /// Where the title sits across the bar, when the design states it.
    pub align: Option<String>,

    /// The bar's own inset, when the design binds one.
    ///
    /// A header is rendered as the row the design drew rather than handed to the platform's
    /// navigation bar, and a row insets its own contents. Without this the bar sits flush against
    /// the screen edge, and the 20pt gutter the file states has nowhere to arrive.
    pub padding: Option<Padding>,

    /// Leading (back or close) items.
    pub leading: Vec<Node>,

    /// Trailing (action) items.
    pub trailing: Vec<Node>,

    /// The rows a vertical bar stacks under its title row, in the order they were drawn.
    ///
    /// Inside the bar: its padding insets them as it insets the title row. `add-city` draws its
    /// search field here, and it rendered beside the title while this did not exist.
    pub below: Vec<Node>,

    /// The spacing token between the title row and each row `below` it.
    pub gap: Option<String>,
}

impl Header {
    /// Every node the bar draws, in every row: `leading`, `trailing`, then `below`.
    ///
    /// The one walk each whole-screen pass (events, bindings, copy, focus, custom marking) should
    /// take, so a new row cannot be added to the bar without every one of them seeing it.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.leading.iter().chain(&self.trailing).chain(&self.below)
    }

    /// [`Header::nodes`], mutably.
    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.leading.iter_mut().chain(&mut self.trailing).chain(&mut self.below)
    }

    /// The title's literal copy, when it has one rather than a binding.
    #[must_use]
    pub fn copy(&self) -> Option<&str> {
        match &self.title {
            Some(Arg::Literal(words)) => Some(words),
            _ => None,
        }
    }

    /// The view-model field the title reads, when it is bound.
    #[must_use]
    pub fn bind(&self) -> Option<&str> {
        match &self.title {
            Some(Arg::Bind(field)) => Some(field),
            _ => None,
        }
    }

    /// Whether the region holds nothing to draw.
    ///
    /// Replaces the old `is_drawn`, which asked whether the title carried its own font so the
    /// emitter could choose between a toolbar view and `navigationTitle`. Neither exists now — a
    /// header is emitted as the row the design drew it as, and a title with no `style` is that row
    /// with unstyled text in it rather than a different kind of bar. The only question left is
    /// whether there is a row at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.nodes().next().is_none()
    }
}

/// The four shapes a body region can take.
#[derive(Debug, Clone)]
pub enum Body {
    /// A collection rendered as a list.
    List(Collection),

    /// A collection rendered as a grid.
    Grid(Collection),

    /// A form: a column of labelled controls.
    Form(Vec<Node>),

    /// Free content.
    Nodes(Vec<Node>),
}

impl Body {
    /// The content nodes this region holds, whatever its shape.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        match self {
            Self::List(collection) | Self::Grid(collection) => &collection.item,
            Self::Form(nodes) | Self::Nodes(nodes) => nodes,
        }
    }

    /// The content nodes this region holds mutably, whatever its shape.
    pub fn nodes_mut(&mut self) -> &mut [Node] {
        match self {
            Self::List(collection) | Self::Grid(collection) => &mut collection.item,
            Self::Form(nodes) | Self::Nodes(nodes) => nodes,
        }
    }

    /// The collection this region iterates, when it iterates one.
    #[must_use]
    pub const fn collection(&self) -> Option<&Collection> {
        match self {
            Self::List(collection) | Self::Grid(collection) => Some(collection),
            Self::Form(_) | Self::Nodes(_) => None,
        }
    }
}

/// A data-bound repetition: `each` names the view-model collection, `item` the template rendered
/// per element.
#[derive(Debug, Clone)]
pub struct Collection {
    /// The view-model field holding the elements.
    pub each: String,

    /// List chrome, or `None` for a grid.
    pub style: Option<ListStyle>,

    /// The spacing token between rows, when the design bound one.
    pub gap: Option<String>,

    /// The spacing token insetting the rows from the screen edges, when the design bound one.
    pub inset: Option<String>,

    /// A fixed column count, for a grid.
    pub columns: Option<u32>,

    /// The grouping, when the rows arrive under headings rather than flat.
    pub section: Option<Section>,

    /// Which way the rows run. Only a collection nested in content says: a body list is a platform
    /// list and always runs down the screen.
    pub direction: Direction,

    /// Whether the rows scroll along [`Self::direction`] inside the content around them. Only a
    /// collection nested in content says; a body list is the screen's scroll already.
    pub scroll: bool,

    /// The template rendered once per element.
    pub item: Vec<Node>,
}

impl Node {
    /// Every collection nested in this subtree, in document order.
    ///
    /// Not descending into one it finds: `vx spec` refuses a collection inside another's row
    /// template, because a row is one element's view and has no view model of its own to hold a
    /// second `Vec`.
    #[must_use]
    pub fn collections(&self) -> Vec<&Collection> {
        fn gather<'a>(node: &'a Node, into: &mut Vec<&'a Collection>) {
            match node {
                Node::Collection(collection) => into.push(collection),
                Node::Group(group) => {
                    for child in &group.items {
                        gather(child, into);
                    }
                }
                Node::Item(_) => {}
            }
        }
        let mut found = Vec::new();
        gather(self, &mut found);
        found
    }
}

impl Collection {
    /// Whether this is a grid rather than a list. The schema requires `columns` on every grid and
    /// admits it on no list, so the column count is what tells the two apart once parsed.
    #[must_use]
    pub const fn is_grid(&self) -> bool {
        self.columns.is_some()
    }
}

/// The grouping of a sectioned list: `each` names the sections, and each section carries a heading
/// and its own rows.
///
/// A second level rather than a flag, because a sectioned list is two collections and the target
/// has to name both. `style: grouped` is presentation — it decides how much chrome the platform
/// draws around rows — and says nothing about where a heading's text comes from.
#[derive(Debug, Clone)]
pub struct Section {
    /// The field on a section holding its heading.
    pub title: String,

    /// The field on a section holding its rows.
    pub items: String,

    /// The typography token the heading is drawn in, when every drawn section agreed on one.
    ///
    /// Only [`ListStyle::Cards`] reads it. The other styles hand the heading to a platform
    /// sectioned list, which has its own opinion and is welcome to it.
    pub style: Option<String>,

    /// The colour token the heading is drawn in, when every drawn section agreed on one.
    pub color: Option<String>,
}

/// How much system list chrome a list wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyle {
    /// Plain rows.
    Plain,

    /// Inset-grouped rows.
    Inset,

    /// Grouped sections.
    Grouped,

    /// A scroll of card views and *no* system list chrome — the distinction the closed set exists
    /// to make.
    Cards,
}

impl ListStyle {
    fn parse(raw: &str) -> Self {
        match raw {
            "inset" => Self::Inset,
            "grouped" => Self::Grouped,
            "cards" => Self::Cards,
            _ => Self::Plain,
        }
    }
}

/// One alternate rendering of a screen.
#[derive(Debug, Clone)]
pub struct State {
    /// The state key, which is the map key.
    pub key: String,

    /// The view-model condition that selects it.
    pub when: String,

    /// Whether it replaces the body or the whole screen.
    pub replaces: Replaces,

    /// This state's header, falling back to the screen's.
    pub header: Option<Header>,

    /// This state's body.
    pub body: Option<Body>,

    /// This state's footer.
    pub footer: Vec<Node>,

    /// This state's floating action button.
    pub fab: Option<Item>,
}

impl State {
    /// What this state's `when:` says.
    #[must_use]
    pub fn condition(&self) -> Condition {
        Condition::parse(&self.when)
    }
}

/// What a state's `when:` says, classified once so that the contract and the two view emitters
/// cannot disagree about it.
///
/// They used to each decide for themselves what a path looked like, and they decided differently:
/// the contract accepted any case and the views only accepted lower case. `when: lists.isEmpty`
/// then became a field on the view model *and* a bare `lists.isEmpty` in the Swift — an
/// identifier that resolves to nothing, in a file no one is allowed to edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// A bare path naming a `bool` on the view model: `loading`.
    Flag(String),

    /// A path whose last segment tests emptiness, naming the collection it tests: `tasks.is_empty`
    /// gives `tasks`.
    ///
    /// Held apart from [`Self::Flag`] because it is not a field — the collection the body already
    /// declares answers it — and because the two platforms spell the test differently: a Swift
    /// property, a Kotlin call.
    Empty(String),

    /// Anything else: an expression the composition wrote out, which no emitter has any business
    /// rewriting.
    Expression(String),
}

impl Condition {
    /// The segments an emptiness test can be written as.
    ///
    /// `is_empty` is what the Rust side of the project reaches for and what the documented example
    /// uses; `isEmpty` is what somebody filling in a `TODO(condition)` by hand reaches for after
    /// reading the generated Swift. Both mean the same thing, and neither is worth an escalation.
    const EMPTY: [&'static str; 3] = ["is_empty", "isEmpty", "is-empty"];

    fn parse(when: &str) -> Self {
        if !is_path(when) {
            return Self::Expression(when.to_owned());
        }

        let (head, tail) = when.rsplit_once('.').unwrap_or(("", when));
        if !head.is_empty() && Self::EMPTY.contains(&tail) {
            return Self::Empty(head.to_owned());
        }
        Self::Flag(when.to_owned())
    }
}

/// Whether a `when:` is a bare dotted path rather than an expression.
///
/// Case is deliberately not part of the test: a path is authored in whichever case its author
/// reached for, and every consumer re-spells it anyway. `-` is deliberately absent, so that a
/// `count-1` stays the expression it reads as.
fn is_path(when: &str) -> bool {
    !when.is_empty()
        && when.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
        })
}

/// How much of the screen a state stands in for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replaces {
    /// Only the body region changes.
    Body,

    /// The whole screen changes.
    Screen,
}

/// A dialog, sheet or snackbar presented over a screen.
#[derive(Debug, Clone)]
pub struct Overlay {
    /// The overlay key, which is the map key.
    pub key: String,

    /// How it is presented.
    pub kind: OverlayKind,

    /// The title, when it has one.
    pub title: Option<String>,

    /// The event that presents it.
    pub trigger: Option<String>,

    /// The overlay's content.
    pub content: Vec<Node>,
}

/// How an overlay is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    /// A modal dialog.
    Dialog,

    /// A bottom sheet.
    Sheet,

    /// A transient message.
    Snackbar,
}

/// A node in a content tree.
#[derive(Debug, Clone)]
pub enum Node {
    /// A container.
    Group(Box<Group>),

    /// A leaf.
    Item(Box<Item>),

    /// A data-bound collection nested in content rather than being the whole body: `{ list: … }`
    /// or `{ grid: … }` inside a group. Rendered as rows in the enclosing stack, never as a
    /// platform list.
    Collection(Box<Collection>),
}

/// A container node.
#[derive(Debug, Clone, Default)]
pub struct Group {
    /// The shared-component slug, when this group is one.
    pub component: Option<String>,

    /// The variant properties this placement selects, when it is a component.
    ///
    /// Figma's variant axes as the instance set them — `importance=destructive`, `state=default`.
    /// What picks one lifted component out of the several a slug can produce.
    pub params: BTreeMap<String, String>,

    /// What this placement passes to its component's TEXT properties, by property key.
    ///
    /// Separate from [`Self::params`], which selects a variant and never carries content.
    pub args: BTreeMap<String, Arg>,

    /// How a bound argument shows its field, by property key, where its `args:` entry says so:
    /// `note: {bind: wind_speed_unit, format: speed-unit-name}`.
    ///
    /// Beside [`Self::args`] rather than inside [`Arg::Bind`], so that every reader of a bound
    /// argument that does not care how it is shown is left as it was.
    pub formats: BTreeMap<String, Format>,

    /// The variant each row reads from a field of its own, when this is a row placement whose
    /// drawn rows differ in one: `params: {state: {bind: hour_state}}` with its `looks:`.
    ///
    /// Not in [`Self::params`], which holds the variants the placement fixes. A bound one is a
    /// switch at runtime between the looks, each lifted as a variant of its own.
    pub bound: Option<Bound>,

    /// How it stacks its children.
    pub direction: Direction,

    /// Token-keyed visual properties.
    pub style: Style,

    /// Main-axis distribution.
    pub justify: Option<String>,

    /// Cross-axis alignment.
    pub align: Option<String>,

    /// Whether children wrap onto further lines.
    pub wrap: bool,

    /// Whether children of their own widths flow onto further lines (`flow_wrap`), as a run of
    /// text or a row of tags does. Unlike [`Self::wrap`], whose cells share one fixed width.
    pub flow_wrap: bool,

    /// Explicit sizing.
    pub size: Size,

    /// The interaction handle, when a control names this node.
    pub handle: Option<String>,

    /// The declared interaction, when this container is interactive.
    pub interaction: Option<Interaction>,

    /// Accessibility and test attributes.
    pub a11y: A11y,

    /// Where this group sits in its parent beyond the flow: its own cross-axis alignment, its
    /// layer position in a stack, and the web facts an extractor carried beside it.
    pub layout: Layout,

    /// Children.
    pub items: Vec<Node>,
}

/// A variant a row reads from its own field, and how the row draws at each value of it.
#[derive(Debug, Clone)]
pub struct Bound {
    /// The variant axis, as `params:` keys it — `state`.
    pub axis: String,

    /// The row field holding the value, a path on the element in scope.
    pub field: String,

    /// One placement per value the drawn rows show, in drawn order, keyed by that value.
    ///
    /// Each is the template with the look's drawing laid over it, position by position — what the
    /// row reads and raises is the template's — and with the axis fixed to its value in `params`,
    /// which is what `lift` splits on and [`Surface::called_by`] matches.
    pub looks: Vec<(String, Group)>,
}

impl Bound {
    /// The Rust variant each value is spelled as in the contract's enum, in look order.
    #[must_use]
    pub fn variants(&self) -> Vec<String> {
        self.looks.iter().map(|(value, _)| crate::naming::look_variant(&self.axis, value)).collect()
    }
}

/// A leaf node: one of the known item types, plus its properties.
#[derive(Debug, Clone)]
pub struct Item {
    /// The item type, kebab-case.
    pub kind: String,

    /// Literal text, for the items that carry one.
    pub content: Option<String>,

    /// The component parameter supplying this item's copy, inside a lifted component.
    ///
    /// Set only on a component's own tree, and only where the placements disagreed about the
    /// words. Everywhere else the copy is the same at every call site and stays a literal.
    pub copy_param: Option<String>,

    /// The component parameter supplying this item's artwork, inside a lifted component — an
    /// [`Component::assets`] slot, set where the item draws a declared asset property.
    pub asset_param: Option<String>,

    /// Inside a component, the property this item draws, from the composition's `arg:`.
    pub arg: Option<String>,

    /// The view-model path this item displays.
    pub bind: Option<String>,

    /// How a bound text shows its value, from the composition's `format:`. The field is then a
    /// typed display value rather than a `String`; see [`BindKind::of_item`].
    pub format: Option<Format>,

    /// An asset or glyph name.
    pub name: Option<String>,

    /// Placeholder text, for entry items.
    pub placeholder: Option<String>,

    /// A fixed option list, for choosers.
    pub options: Vec<String>,

    /// A typography or variant key.
    pub style_key: Option<String>,

    /// Where the item sits across its container, for the one item that floats over a screen rather
    /// than taking a place in it.
    pub align: Option<String>,

    /// Token-keyed visual properties.
    pub style: Style,

    /// Explicit sizing.
    pub size: Size,

    /// The interaction handle, when a control names this node.
    pub handle: Option<String>,

    /// The event this item raises, when it is a control.
    pub event: Option<Event>,

    /// Whether this item is a `custom:` element — one whose kind is an open-vocabulary name declared
    /// in the document's `custom_items`, rather than one of the closed set the emitters render.
    ///
    /// A custom item is a depiction the emitters cannot build, so the view emitters write a call
    /// site to a model-implemented view rather than a body. Its contract is not inferred from the
    /// drawing: [`Self::raises`] and [`Self::binds`] carry it explicitly, so `contract::derive`
    /// still owns the `Event` enum and the view-model fields.
    pub custom: bool,

    /// The events a `custom:` element raises, declared by its `raises:` annotation.
    pub raises: Vec<Event>,

    /// The view-model fields a `custom:` element binds, declared by its `binds:` annotation.
    pub binds: Vec<String>,

    /// Accessibility and test attributes.
    pub a11y: A11y,

    /// Where this item sits in its parent beyond the flow; see [`Layout`].
    pub layout: Layout,

    /// The promoted cross-platform facts about what the item shows; see [`ItemFacts`].
    pub facts: ItemFacts,
}

/// What an item shows beyond its kind and copy, in the ui-ir core vocabulary.
///
/// Every field is optional and absent means the platform default, so a composition that states none of it renders
/// exactly as before.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemFacts {
    /// The `tokens.typography` style a non-text item (button, field, badge) sets its label in.
    pub text_style: Option<String>,

    /// The most lines the label takes before it is truncated (`1` = never wraps).
    pub line_limit: Option<u32>,

    /// A linear gradient filling the text's glyphs.
    pub gradient: Option<Gradient>,

    /// Where a link goes: a URL or an app route.
    pub destination: Option<String>,

    /// What a text entry takes: `email`, `password`, `number`, `phone`, `url`, `search`, `text`.
    pub content_type: Option<String>,

    /// How an image fills its frame: `fit` (letterboxed), `fill` (cropped), `stretch`.
    pub content_mode: Option<String>,
}

/// A linear gradient: a CSS angle (0 = upward, 90 = toward the end edge) and colour-token stops.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Degrees, clockwise from upward.
    pub angle: f64,

    /// `(colour token, position 0..=1)`, in order.
    pub stops: Vec<(String, f64)>,
}

impl Item {
    /// The kind of value this item hands back when the user changes it, if it is a control that
    /// hands one back at all.
    ///
    /// A button reports that it was pressed and nothing else; a text field reports what is now in
    /// it. That difference decides what an event raised here can carry: `ListNameChanged(value)`
    /// on a text field means the text, and there is nowhere else for it to come from — the view
    /// model holds what the core last pushed, which is the value *before* the keystroke.
    #[must_use]
    pub fn produces(&self) -> Option<BindKind> {
        match self.kind.as_str() {
            "text-field" | "search-field" | "text-area" | "picker" | "segmented-control" => {
                Some(BindKind::Text)
            }

            // A `radio` is one button of a group rather than the group, so what it hands back is
            // its own checked state — which is also the only thing Compose's `RadioButton` can
            // report, and it can only ever report it as chosen.
            "checkbox" | "switch" | "radio" => Some(BindKind::Flag),
            "slider" | "stepper" => Some(BindKind::Number),
            "date-picker" => Some(BindKind::Moment),
            _ => None,
        }
    }
}

/// Token-keyed visual properties.
///
/// Every field is a token key, never a literal — the schema rejects raw numbers here, which is
/// what lets a renderer emit a constant reference unconditionally.
#[derive(Debug, Clone, Default)]
pub struct Style {
    /// The gap between children.
    pub gap: Option<String>,

    /// Padding, uniform or per side.
    pub padding: Option<Padding>,

    /// The background colour token.
    pub background: Option<String>,

    /// The foreground colour token.
    pub color: Option<String>,

    /// The corner radius token.
    pub corner_radius: Option<String>,

    /// The elevation token.
    pub elevation: Option<String>,

    /// The border, when one is drawn.
    pub border: Option<Border>,

    /// The opacity token.
    pub opacity: Option<String>,

    /// The `tokens.shadow` drop shadow drawn behind the node.
    pub shadow: Option<String>,

    /// Whether children are clipped to the node's bounds (and corner radius).
    pub clip: bool,

    /// The spacing between wrapped lines of a flowing group (the cross-axis gap).
    pub cross_gap: Option<String>,

    /// A raster asset painted behind the node's content, inside its corner radius.
    pub background_image: Option<BackgroundImage>,
}

/// A group's background image: the asset, how it sits in the box, and the point it is pinned at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundImage {
    /// The `assets.json` raster entry.
    pub name: String,

    /// `fit`, `fill` or `stretch`; `None` draws it at its natural size.
    pub content_mode: Option<String>,

    /// `top-start` ... `bottom-end` (the box's point and the image's made to coincide).
    pub alignment: String,
}

impl BackgroundImage {
    /// The alignment as `(x, y)` fractions of the free space, each 0, 0.5 or 1 (start / centre /
    /// end along the reading direction; top / centre / bottom).
    #[must_use]
    pub fn fractions(&self) -> (f64, f64) {
        let (y, x) = match self.alignment.as_str() {
            "center" => ("center", "center"),
            other => other.split_once('-').unwrap_or((other, "start")),
        };
        let at = |part: &str| match part {
            "center" => 0.5,
            "end" | "bottom" => 1.0,
            _ => 0.0,
        };
        (at(x), at(y))
    }
}

/// A drawn border: a width from `tokens.border`, and the colour this particular layer draws it in.
///
/// The colour is per-layer because that is the only place it exists. Figma states a stroke's width
/// as a variable and its colour on the frame, so one width is drawn in as many colours as the design
/// has uses for it — the real export draws `thin` in `border-default` on 27 layers, `brand-primary`
/// on 10 and `semantic-error` on 2, and `thick` in four different swatches. Asking `tokens.border`
/// for the answer asks a question the file does not answer, which is why those entries reach
/// `vx repair` as holes it correctly refuses: a plurality would have drawn 12 of 39 hairlines and 8
/// of 10 thick borders in the wrong colour.
#[derive(Debug, Clone)]
pub struct Border {
    /// The `tokens.border` entry supplying the width.
    pub width: String,

    /// The `tokens.colors` entry this layer strokes in, when the composition states one.
    ///
    /// `None` falls back to the width token's own colour, which is what a hand-authored
    /// `border: thin` means.
    pub color: Option<String>,
}

/// Uniform or per-side padding, in token keys.
#[derive(Debug, Clone)]
pub enum Padding {
    /// One token on every side.
    All(String),

    /// Per-side tokens, any of which may be absent.
    Sides {
        /// The top inset token.
        top: Option<String>,

        /// The trailing inset token.
        right: Option<String>,

        /// The bottom inset token.
        bottom: Option<String>,

        /// The leading inset token.
        left: Option<String>,
    },
}

/// An explicit width and height, and the bounds the resolved ones are clamped into.
#[derive(Debug, Clone, Copy, Default)]
pub struct Size {
    /// The width, when set.
    pub width: Option<Dimension>,

    /// The height, when set.
    pub height: Option<Dimension>,

    /// The least the resolved width may be.
    pub min_width: Option<f64>,

    /// The most the resolved width may be.
    pub max_width: Option<f64>,

    /// The least the resolved height may be.
    pub min_height: Option<f64>,

    /// The most the resolved height may be.
    pub max_height: Option<f64>,
}

/// How a node places itself in its parent, beyond what the parent's flow decides.
///
/// Every field is optional and absent means the parent decides, so a composition that never says
/// any of it parses to [`Layout::default`] and renders exactly as it did.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    /// This child's own cross-axis alignment, overriding the parent's `align` for it alone.
    pub align_self: Option<String>,

    /// Where this child sits as a layer of its parent stack, when it is one.
    pub position: Option<Layer>,

    /// The edge (`top` / `bottom`) the node stays pinned to while its scroll container scrolls
    /// past, when it does.
    pub pinned: Option<String>,

    /// The `x-web` facts a web extractor carried beside the node, verbatim: residual CSS, margins,
    /// link targets, inline SVG. Opaque to the native emitters; the Vue emitter reads it.
    pub web: Option<Value>,
}

/// A stack child's layer position: an anchor on the parent's content box and a signed offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    /// One of the nine anchors, `top-start` through `bottom-end`.
    pub align: String,

    /// The `tokens.offset` entry moving the layer toward the end edge, when it moves.
    pub x: Option<String>,

    /// The `tokens.offset` entry moving the layer downward, when it moves.
    pub y: Option<String>,
}

impl Layer {
    /// The vertical and horizontal halves of the anchor: `("top", "start")`, `("center", "center")`.
    #[must_use]
    pub fn axes(&self) -> (&str, &str) {
        match self.align.as_str() {
            "center" => ("center", "center"),
            other => other.split_once('-').unwrap_or(("center", "center")),
        }
    }
}

impl Size {
    /// The dimension on a stack's main axis, which is the one a distribution needs claimed.
    #[must_use]
    pub const fn along(&self, direction: Direction) -> Option<Dimension> {
        match direction {
            Direction::Column => self.height,
            Direction::Row | Direction::Stack => self.width,
        }
    }
}

/// One axis of a `size:` block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    /// Expand to the available space.
    Fill,

    /// Shrink to the content.
    Hug,

    /// A fixed point or dp value.
    Fixed(f64),
}

/// Accessibility and test attributes.
#[derive(Debug, Clone, Default)]
pub struct A11y {
    /// The accessibility label.
    pub label: Option<String>,

    /// The accessibility hint.
    pub hint: Option<String>,

    /// The accessibility role.
    pub role: Option<String>,

    /// The Maestro test id.
    pub test_id: Option<String>,
}

/// A declared interaction on a container.
#[derive(Debug, Clone)]
pub struct Interaction {
    /// Which archetype's guarantees apply.
    pub archetype: Archetype,

    /// The event the container itself raises.
    pub event: Option<Event>,

    /// The event a long press raises.
    pub long_press: Option<Event>,

    /// Nested controls that stop propagation.
    pub controls: Vec<Control>,

    /// Actions revealed from the leading edge.
    pub swipe_leading: Vec<Swipe>,

    /// Actions revealed from the trailing edge.
    pub swipe_trailing: Vec<Swipe>,
}

impl Interaction {
    /// Whether this interaction reveals anything on a swipe.
    #[must_use]
    pub const fn has_swipes(&self) -> bool {
        !self.swipe_leading.is_empty() || !self.swipe_trailing.is_empty()
    }

    /// Every event this interaction can raise, in a stable order.
    pub fn events(&self) -> impl Iterator<Item = &Event> {
        self.event
            .iter()
            .chain(self.long_press.iter())
            .chain(self.controls.iter().map(|control| &control.event))
            .chain(self.swipe_leading.iter().map(|swipe| &swipe.event))
            .chain(self.swipe_trailing.iter().map(|swipe| &swipe.event))
    }
}

/// The nine interaction archetypes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    /// Not interactive.
    Static,

    /// A control that owns its own hit target.
    Control,

    /// The whole container is the tap target.
    TapTarget,

    /// A tappable row with a control at its leading edge.
    RowWithLeadingControl,

    /// A tappable row with a control at its trailing edge.
    RowWithTrailingControl,

    /// A tappable grid cell.
    GridCell,

    /// A row with edge-revealed actions.
    SwipeRow,

    /// A focusable text field.
    TextEntry,

    /// A chip that carries a selected state.
    SelectableChip,
}

impl Archetype {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "static" => Self::Static,
            "control" => Self::Control,
            "tap-target" => Self::TapTarget,
            "row-with-leading-control" => Self::RowWithLeadingControl,
            "row-with-trailing-control" => Self::RowWithTrailingControl,
            "grid-cell" => Self::GridCell,
            "swipe-row" => Self::SwipeRow,
            "text-entry" => Self::TextEntry,
            "selectable-chip" => Self::SelectableChip,
            _ => return None,
        })
    }

    /// Whether the whole container is one tap target — the archetypes whose guarantee is that you
    /// cannot miss the row.
    ///
    /// `Control` is one of them because its primary target *is* the container: the archetype table
    /// names it "a container acting as one control, owning its own target". Left out, a frame
    /// annotated `control` emitted its children and dropped the event on the floor — which is the
    /// annotation a button drawn as a variant set reaches for, because `item-type: button` would
    /// discard the depiction its `importance` axis lives in.
    ///
    /// A leaf control needs no block at all — it is a control by item type — so admitting this one
    /// does not put a `Button` around a `TextField`.
    #[must_use]
    pub const fn is_whole_container_tappable(self) -> bool {
        matches!(
            self,
            Self::Control
                | Self::TapTarget
                | Self::GridCell
                | Self::RowWithLeadingControl
                | Self::RowWithTrailingControl
                | Self::SwipeRow
        )
    }
}

/// One nested control that consumes its own tap.
#[derive(Debug, Clone)]
pub struct Control {
    /// The handle of the node this control targets.
    pub handle: String,

    /// The event it raises.
    pub event: Event,

    /// The name of the equivalent accessibility custom action.
    pub a11y_action: Option<String>,

    /// Whether the tap also reaches the container.
    pub propagates: bool,
}

/// One edge-revealed swipe action.
#[derive(Debug, Clone)]
pub struct Swipe {
    /// The event it raises.
    pub event: Event,

    /// The glyph shown in the revealed action.
    pub icon: Option<String>,

    /// The label shown in the revealed action.
    pub label: Option<String>,

    /// The background colour token.
    pub background: Option<String>,

    /// The foreground colour token.
    pub tint: Option<String>,

    /// Whether the action is destructive.
    pub destructive: bool,
}

/// A core event, parsed out of its `Name(arg, arg)` spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// The variant name, exactly as the core spells it.
    pub name: String,

    /// Argument expressions, as authored.
    ///
    /// These are bind paths on the element in scope, which is why they are carried as written
    /// rather than resolved here: what `id` means depends on which `ForEach` you are inside.
    pub args: Vec<String>,
}

impl Event {
    /// Parses `EditTask(id)` into its name and argument list.
    ///
    /// The schema has already constrained the spelling, so anything without parentheses is a
    /// nullary event.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        let Some((name, rest)) = raw.split_once('(') else {
            return Self {
                name: raw.to_owned(),
                args: Vec::new(),
            };
        };

        let args = rest
            .trim_end_matches(')')
            .split(',')
            .map(str::trim)
            .filter(|arg| !arg.is_empty())
            .map(str::to_owned)
            .collect();

        Self {
            name: name.trim().to_owned(),
            args,
        }
    }

    /// Whether this is an unfilled `TODO(event)` rather than an event.
    ///
    /// `parse` reads it as the event `TODO` taking one argument called `event`, because the
    /// spelling of a marker is the spelling of a nullary-plus-argument event and nothing in the
    /// grammar separates them. A hole is a decision nobody has made, so the name it parses to is
    /// not one the contract may declare — `Event::TODO` is a variant no shell dispatches and no
    /// `behaviour.yaml` is allowed to name, which is how `crates/vx/tests/behaviour.rs` found it.
    #[must_use]
    pub fn is_hole(&self) -> bool {
        crate::spec::holes::Kind::of(&self.spelled()).is_some()
    }

    /// The event as the composition spells it, for a message that quotes it back.
    #[must_use]
    pub fn spelled(&self) -> String {
        if self.args.is_empty() {
            return self.name.clone();
        }
        format!("{}({})", self.name, self.args.join(", "))
    }
}

/// One value a component reads from the element in scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The parameter this value arrives as, in the neutral spelling each emitter cases for itself.
    ///
    /// The authored path wherever every placement binds the same one, which is the common case and
    /// the readable one: a row in a list binds `title` on each of its elements, so `title` is what
    /// the parameter should be called. Positional — `value`, `value2` — only where the placements
    /// disagree, because there is then no single path to name it after. See `name_bindings`.
    pub name: String,

    /// The authored bind path.
    pub path: String,

    /// What the item displaying it needs the value to be.
    pub kind: BindKind,
}

/// The type a bound value must have for the item that displays it.
///
/// Inferred from the item kind rather than declared, because the composition describes a UI and
/// the core owns the types. A wrong guess is a compile error at the call site, not a silent
/// defect, which is what makes inferring it acceptable at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindKind {
    /// Displayed as text.
    Text,

    /// Displayed as an on/off state.
    Flag,

    /// Displayed as a magnitude.
    Number,

    /// Displayed as a point in time.
    ///
    /// Epoch milliseconds on all three sides, rather than each platform's own date type. `Date` and
    /// `LocalDateTime` do not cross the FFI, and a formatted string is a display decision made in
    /// the core, where the locale is not known. An integer is the one spelling that is the same
    /// value on both shells; turning it into a `Date` is vx's Swift view emitter's job, at the one
    /// line that needs one.
    Moment,

    /// Displayed through a `format:`: a typed value the view formats with the device locale.
    ///
    /// Declared rather than inferred, which is the exception to the rule above: a `text` shows a
    /// `String` unless the composition says what kind of value it is, and then the core fills a
    /// moment, a measurement, a unit or a number, and writes no words. Separate from [`Self::Moment`],
    /// which is what a date picker hands back and is a bare `i64`.
    Displayed(Display),
}

impl BindKind {
    /// The kind an item of this type needs its bound value to be.
    #[must_use]
    pub fn of(item_kind: &str) -> Self {
        match item_kind {
            "checkbox" | "switch" | "radio" => Self::Flag,
            "slider" | "stepper" | "progress-indicator" => Self::Number,
            "date-picker" => Self::Moment,
            _ => Self::Text,
        }
    }

    /// The kind an item needs its bound value to be: its `format:`'s, where it declares one, and
    /// otherwise its type's ([`Self::of`]).
    #[must_use]
    pub fn of_item(item: &Item) -> Self {
        item.format.map_or_else(|| Self::of(&item.kind), Self::of_format)
    }

    /// The kind a field shown in `format` holds.
    #[must_use]
    pub const fn of_format(format: Format) -> Self {
        Self::Displayed(format.display())
    }

    /// The kind in words, for a refusal naming two of them.
    #[must_use]
    pub fn describe(self) -> String {
        match self {
            Self::Text => "text".to_owned(),
            Self::Flag => "a flag".to_owned(),
            Self::Number => "a number".to_owned(),
            Self::Moment => "a date picker's epoch milliseconds".to_owned(),
            Self::Displayed(display) => format!("a `{}` shown through `format:`", display.rust()),
        }
    }

    /// What the number means, for the kinds where the type alone does not say.
    ///
    /// `i64` is not a unit. The shells divide and multiply by a thousand at the one line that turns
    /// the integer into a `Date`, and the core — which is written against the generated contract
    /// and nothing else — has no way to see that line. Left unsaid, a core picked seconds, and
    /// every due date in the built app rendered in the year 57774.
    #[must_use]
    pub const fn unit(self) -> Option<&'static str> {
        match self {
            Self::Moment => Some("Epoch milliseconds, which is what both shells send and expect."),
            Self::Displayed(display) => Some(display.unit()),
            Self::Text | Self::Flag | Self::Number => None,
        }
    }

    /// How Rust spells it.
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Text => "String",
            Self::Flag => "bool",
            Self::Number => "f64",
            Self::Moment => "i64",
            Self::Displayed(display) => display.rust(),
        }
    }

    /// How Swift spells it.
    #[must_use]
    pub const fn swift(self) -> &'static str {
        match self {
            Self::Text => "String",
            Self::Flag => "Bool",
            Self::Number => "Double",
            Self::Moment => "Int64",
            Self::Displayed(display) => display.swift(),
        }
    }

    /// How Kotlin spells it.
    #[must_use]
    pub const fn kotlin(self) -> &'static str {
        match self {
            Self::Text => "String",
            Self::Flag => "Boolean",
            Self::Number => "Double",
            Self::Moment => "Long",
            Self::Displayed(display) => display.kotlin(),
        }
    }
}

/// How a group stacks its children.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Direction {
    /// Along the main axis.
    Row,

    /// Down the cross axis — the default, and what a region with no `direction:` means.
    #[default]
    Column,

    /// Overlaid, back to front.
    Stack,
}

impl Direction {
    fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("row") => Self::Row,
            Some("stack") => Self::Stack,
            _ => Self::Column,
        }
    }
}

/// Parses a validated composition document into a [`Surface`].
///
/// # Errors
///
/// When the document is not the shape the composition schema guarantees, which downstream of
/// `vx spec` means the caller skipped it.
pub fn parse(document: &Value) -> Result<Surface, Error> {
    let screens = document.get("screens").and_then(Value::as_object).ok_or_else(|| {
        Error::Malformed("composition has no `screens` mapping to emit".to_owned())
    })?;

    let mut surface = Surface::default();
    for (slug, entry) in screens {
        surface.screens.push(screen(slug, entry));
    }

    // The document declares its custom elements once, at the root, each with the brief `vx custom`
    // builds it from. Read them before the item walk marks their instances, so an instance's kind
    // is recognised as a declared name rather than an unknown item type.
    surface.custom = document
        .get("custom_items")
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(|entry| {
                    Some(CustomItem {
                        name: string(entry, "name")?,
                        brief: string(entry, "description"),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // A custom element's instances carry its name as their item kind; mark them so the emitters
    // write a call site rather than reaching for a renderer that does not exist.
    let names: BTreeSet<&str> = surface.custom.iter().map(|one| one.name.as_str()).collect();
    if !names.is_empty() {
        for screen in &mut surface.screens {
            mark_screen(screen, &names);
        }
    }

    surface.footer_menu = document.get("footer_menu").map(footer_menu).transpose()?;

    // Components are collected after every screen is parsed, because what a component *is* depends
    // on all of its placements at once: which variant axes vary, and which words do.
    let mut placements: BTreeMap<String, Vec<Group>> = BTreeMap::new();
    for screen in &surface.screens {
        for node in screen.nodes() {
            collect_placements(node, &mut placements);
        }
    }
    surface.components = placements.iter().flat_map(|(slug, all)| lift(slug, all)).collect();

    // A component is rendered with its parameters and nothing else in scope, so a collection inside
    // one would iterate a view-model field the component cannot see. Refused here rather than
    // emitted as a `ForEach` over nothing.
    if let Some(component) =
        surface.components.iter().find(|component| holds_collection(&component.group))
    {
        return Err(Error::Malformed(format!(
            "component `{}` holds a `list` or `grid` in its content, and a component has no view \
             model to iterate one from. Place the collection beside the component instead of \
             inside it.",
            component.slug
        )));
    }

    // The lifted tree is the first placement's, so a bound item's `format:` there decides the
    // parameter's type and how the component shows it. A placement showing the same position some
    // other way would be called with a value of another type, or shown in a style it did not ask
    // for. Refused by name rather than resolved for the first placement.
    for (slug, all) in &placements {
        for placement in all {
            // A collected placement has had its `component:` taken off, so it is matched on the
            // slug it was collected under and the variant it selects, as `called_by` would.
            let Some(component) = surface.components.iter().find(|component| {
                &component.slug == slug
                    && component.key.iter().all(|(axis, value)| {
                        placement.params.get(axis).map_or(value.is_empty(), |v| v == value)
                    })
            }) else {
                continue;
            };
            if let Some(refusal) = format_disagreement(component, placement) {
                return Err(Error::Malformed(refusal));
            }
        }
    }

    Ok(surface)
}

/// Every bound item's path and `format:` in a subtree, in walk order.
fn bound_formats(group: &Group, into: &mut Vec<(String, Option<Format>)>) {
    for node in &group.items {
        match node {
            Node::Group(child) => bound_formats(child, into),
            Node::Item(item) => {
                if let Some(path) = &item.bind {
                    into.push((path.clone(), item.format));
                }
            }
            Node::Collection(_) => {}
        }
    }
}

/// Where `placement` shows a bound item in another `format:` than the component it calls does.
fn format_disagreement(component: &Component, placement: &Group) -> Option<String> {
    let mut lifted = Vec::new();
    bound_formats(&component.group, &mut lifted);
    let mut placed = Vec::new();
    bound_formats(placement, &mut placed);
    if lifted.len() != placed.len() {
        return None;
    }

    let spelled = |format: Option<Format>| {
        format.map_or_else(
            || "no `format:`".to_owned(),
            |format| format!("`format: {}`", format.style()),
        )
    };
    lifted.iter().zip(&placed).find(|(one, other)| one.1 != other.1).map(|(one, other)| {
        format!(
            "component `{}` shows its bound `{}` with {} in one placement and its bound `{}` with \
             {} in another. A component draws each position one way, so its placements have to \
             agree; give the position one `format:` everywhere, or make the placements different \
             components.",
            component.slug,
            one.0,
            spelled(one.1),
            other.0,
            spelled(other.1),
        )
    })
}

/// `footer_menu`, with each tab's two looks built into the groups a renderer draws.
///
/// A look is a tab's box as compile read it off the variant, which of icon and label it draws
/// first, the icon's drawn size, and the label's style and colour; it carries neither the icon nor
/// the copy, which are the tab's own and the same in both looks. So the drawn tab is assembled
/// here, once for every renderer: the look's box, holding the icon and the label in the look's
/// order, tapped as a whole to raise the menu's event.
///
/// # Errors
///
/// A look without its `order` or its icon size. The schema requires both, so this is only reached
/// by skipping `vx spec`; it is refused rather than drawn in a conventional order at the asset's
/// own size, which is a guess at two facts the file carries.
fn footer_menu(menu: &Value) -> Result<FooterMenu, Error> {
    let mut built = FooterMenu {
        event: Event::parse(&string(menu, "event").unwrap_or_default()),
        screens: string_list(menu, "screens"),
        selected: menu
            .get("selected")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(screen, tab)| Some((screen.clone(), tab.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default(),
        bar: group(menu.get("bar").unwrap_or(&Value::Null)),
        tabs: Vec::new(),
    };

    for tab in menu.get("tabs").and_then(Value::as_array).into_iter().flatten() {
        // All three are required by the schema, and `vx spec` holds each to more than presence: a
        // route is a key under `screens` (`composition::menu`), an icon an `assets.yaml` key
        // (`assets::collect_asset_references`), and a label is not blank. So none of these can be
        // missing here unless the gate was skipped, and an empty one is refused rather than drawn
        // as a tab that shows nothing.
        let field = |key: &str| {
            string(tab, key).filter(|value| !value.trim().is_empty()).ok_or_else(|| {
                Error::Malformed(format!(
                    "a footer_menu tab has no `{key}`, or a blank one. Run `vx spec`."
                ))
            })
        };
        let route = field("route")?;
        let label = field("label")?;
        let icon = field("icon")?;
        let raises = built
            .raises(&Tab {
                route: route.clone(),
                label: String::new(),
                icon: String::new(),
                selected: Group::default(),
                unselected: Group::default(),
            })
            .map(|event| event.spelled());

        let look = |key: &str| -> Result<Group, Error> {
            let mut props = tab.get(key).cloned().unwrap_or_else(|| Value::Object(Map::new()));
            let caption = props.as_object_mut().and_then(|look| look.remove("label"));
            let order = props.as_object_mut().and_then(|look| look.remove("order"));
            let drawn = props.as_object_mut().and_then(|look| look.remove("icon"));
            let refused = |what: &str| {
                Error::Malformed(format!(
                    "footer_menu tab `{route}` has no {what} in its `{key}` look, which a drawn \
                     tab cannot be drawn without. Run `vx spec`."
                ))
            };
            let icon_first = match order.as_ref().and_then(Value::as_array).map(Vec::as_slice) {
                Some([first, _]) => first.as_str() == Some("icon"),
                _ => return Err(refused("`order`")),
            };
            let size = drawn
                .as_ref()
                .and_then(|icon| icon.get("size"))
                .cloned()
                .ok_or_else(|| refused("icon `size`"))?;
            let mut text = Map::new();
            text.insert("content".to_owned(), Value::String(label.clone()));
            for slot in ["style", "color"] {
                if let Some(value) = caption.as_ref().and_then(|caption| caption.get(slot)) {
                    text.insert(slot.to_owned(), value.clone());
                }
            }
            let mut drawn = group(&props);
            let glyph = Node::Item(Box::new(item_from(
                "icon",
                &serde_json::json!({ "name": icon, "size": size }),
            )));
            let words = Node::Item(Box::new(item_from("text", &Value::Object(text))));
            drawn.items = if icon_first { vec![glyph, words] } else { vec![words, glyph] };
            drawn.interaction = Some(Interaction {
                archetype: Archetype::TapTarget,
                event: raises.as_deref().map(Event::parse),
                long_press: None,
                controls: Vec::new(),
                swipe_leading: Vec::new(),
                swipe_trailing: Vec::new(),
            });
            Ok(drawn)
        };

        built.tabs.push(Tab {
            selected: look("selected")?,
            unselected: look("unselected")?,
            route,
            label,
            icon,
        });
    }

    Ok(built)
}

/// Whether a subtree holds a collection nested in content.
fn holds_collection(group: &Group) -> bool {
    group.items.iter().any(|node| match node {
        Node::Collection(_) => true,
        Node::Group(child) => holds_collection(child),
        Node::Item(_) => false,
    })
}

/// Every placement of every slug, in screen order.
fn collect_placements(node: &Node, into: &mut BTreeMap<String, Vec<Group>>) {
    let group = match node {
        Node::Group(group) => group,
        // A row component placed in a nested collection is placed there exactly as it is in a
        // body list's template, which `Screen::nodes` hands over already unwrapped.
        Node::Collection(collection) => {
            for child in &collection.item {
                collect_placements(child, into);
            }
            return;
        }
        Node::Item(_) => return,
    };

    // A row binding its variant is placed once per look, each fixing the axis to its value, so
    // `lift` splits the looks into components of their own exactly as it splits placed variants.
    if let Some(bound) = &group.bound {
        for (_, look) in &bound.looks {
            collect_placements(&Node::Group(Box::new(look.clone())), into);
        }
        return;
    }

    if let Some(slug) = &group.component {
        let mut lifted = (**group).clone();
        lifted.component = None;
        into.entry(slug.clone()).or_default().push(lifted);
    }

    for child in &group.items {
        collect_placements(child, into);
    }
}

/// One slug's placements, as the components they need.
///
/// Two questions, in order. Which variant axes tell the placements apart — those split the slug
/// into separate components, because a variant is different artwork rather than different content.
/// Then, within one variant, which copy differs — that becomes a parameter.
fn lift(slug: &str, placements: &[Group]) -> Vec<Component> {
    let varying = varying_axes(placements);

    let mut variants: BTreeMap<Vec<(String, String)>, Vec<&Group>> = BTreeMap::new();
    for placement in placements {
        let key = varying
            .iter()
            .map(|axis| (axis.clone(), placement.params.get(axis).cloned().unwrap_or_default()))
            .collect();
        variants.entry(key).or_default().push(placement);
    }

    variants
        .into_iter()
        .map(|(key, group)| {
            let mut lifted = group[0].clone();
            let (mut copy, assets) = declared_args(&mut lifted);
            let taken: BTreeSet<String> =
                copy.iter().chain(&assets).map(|slot| slot.name.clone()).collect();
            let inferred = varying_copy(&group, &taken);
            for slot in &inferred {
                if let Some(item) = at_path_mut(&mut lifted, &slot.path) {
                    item.copy_param = Some(slot.name.clone());
                }
            }
            copy.extend(inferred);

            let mut bindings = Vec::new();
            let mut events = Vec::new();
            gather(&lifted, &mut bindings, &mut events);
            name_bindings(&mut bindings, &group);

            Component {
                slug: slug.to_owned(),
                name: component_name(slug, &key),
                key: key.into_iter().collect(),
                copy,
                assets,
                group: lifted,
                bindings,
                events,
            }
        })
        .collect()
}

/// The `params:` axes whose value is not the same at every placement.
fn varying_axes(placements: &[Group]) -> Vec<String> {
    let mut axes: BTreeMap<&str, Option<&str>> = BTreeMap::new();
    let mut varying = BTreeSet::new();

    for placement in placements {
        for (axis, value) in &placement.params {
            match axes.entry(axis.as_str()).or_insert(Some(value.as_str())) {
                Some(seen) if *seen == value.as_str() => {}
                slot => {
                    *slot = None;
                    varying.insert(axis.clone());
                }
            }
        }
    }

    // An axis one placement states and another omits also tells them apart.
    for placement in placements {
        for axis in axes.keys() {
            if !placement.params.contains_key(*axis) {
                varying.insert((*axis).to_owned());
            }
        }
    }

    varying.into_iter().collect()
}

/// The component's declared properties, one slot per property, marking every item that draws one
/// so it renders the parameter rather than the first placement's words or artwork. Returns the copy
/// slots — texts — and the asset slots — icons and images — apart, because they are different types
/// at every definition and every call site.
///
/// Declared, so nothing is compared: a property is a parameter whether or not the placements happen
/// to agree, because the call site states it. The walk stops at a nested component, whose items
/// draw *its* properties and which is called with its own `args:` rather than inlined.
fn declared_args(lifted: &mut Group) -> (Vec<CopySlot>, Vec<CopySlot>) {
    let mut copy: Vec<CopySlot> = Vec::new();
    let mut assets: Vec<CopySlot> = Vec::new();
    mark_args(lifted, &mut Vec::new(), &mut copy, &mut assets);
    (copy, assets)
}

fn mark_args(
    group: &mut Group, prefix: &mut Vec<usize>, copy: &mut Vec<CopySlot>,
    assets: &mut Vec<CopySlot>,
) {
    for (index, node) in group.items.iter_mut().enumerate() {
        prefix.push(index);
        match node {
            Node::Group(child) if child.component.is_none() => {
                mark_args(child, prefix, copy, assets);
            }
            // A collection's rows read their element, not the component's parameters.
            Node::Group(_) | Node::Collection(_) => {}
            Node::Item(item) => {
                if let Some(arg) = item.arg.clone() {
                    let name = camel(&arg);
                    let artwork = matches!(item.kind.as_str(), "icon" | "image" | "avatar");
                    let slots = if artwork { &mut *assets } else { &mut *copy };
                    if !slots.iter().any(|slot| slot.name == name) {
                        slots.push(CopySlot {
                            name: name.clone(),
                            path: prefix.clone(),
                            arg: Some(arg),
                        });
                    }
                    if artwork {
                        item.asset_param = Some(name);
                    } else {
                        item.copy_param = Some(name);
                    }
                }
            }
        }
        prefix.pop();
    }
}

/// The positions where the placements of one variant show different words.
///
/// Only positions no TEXT property declares: those are [`declared_args`]' already, and comparing
/// them would name a declared `label` `label2`. `taken` holds the declared names, which an inferred
/// slot's ordinal skips.
fn varying_copy(placements: &[&Group], taken: &BTreeSet<String>) -> Vec<CopySlot> {
    let first = placements[0];
    let mut found = Vec::new();
    let mut paths = Vec::new();
    copy_positions(first, &mut Vec::new(), &mut paths);

    let mut position = 0;
    for path in paths {
        if at_path(first, &path).is_some_and(|item| item.arg.is_some()) {
            continue;
        }
        let mine = at_path(first, &path).and_then(|item| item.content.as_deref());
        let differs = placements[1..]
            .iter()
            .any(|other| at_path(other, &path).and_then(|item| item.content.as_deref()) != mine);
        if differs {
            let name = loop {
                position += 1;
                let name = ordinal(position, "label");
                if !taken.contains(&name) {
                    break name;
                }
            };
            found.push(CopySlot {
                name,
                path,
                arg: None,
            });
        }
    }

    found
}

/// A slot name from its position: the bare stem first, then numbered.
///
/// Shared by the copy slots and the bind slots so that `label`/`label2` and `value`/`value2` cannot
/// drift apart, and so the common component — one slot of its kind — reads without a number.
fn ordinal(position: usize, stem: &str) -> String {
    if position == 1 { stem.to_owned() } else { format!("{stem}{position}") }
}

/// Settles what to call the bind slots, now that every placement of this variant has been seen.
///
/// The authored path is a good parameter name exactly when every placement binds the same one, and a
/// collection's rows always do — each element of `tasks` has its own `title` — which is why naming
/// the parameter after the path survived this long. Two hardcoded placements are the case that
/// breaks it: the settings screen draws one `setting-row/switch` bound to `night_palette` and
/// another bound to `show_precipitation`, and a component declaring `nightPalette` is one the second
/// placement cannot call. It was a Swift and Kotlin compile error in generated code, which is the
/// one kind `vx fix` may not repair.
///
/// So a slot the placements disagree about is numbered instead, exactly as [`varying_copy`] numbers
/// the words they disagree about. The call site then has to read the name off the component rather
/// than off its own tree, because there is no longer one path it could derive it from.
fn name_bindings(bindings: &mut [Binding], placements: &[&Group]) {
    let spellings: Vec<Vec<String>> = placements
        .iter()
        .map(|placement| {
            let mut found = Vec::new();
            gather(placement, &mut found, &mut Vec::new());
            found.into_iter().map(|binding| binding.path).collect()
        })
        .collect();

    // A placement that gathered a different number of slots is one no positional correspondence can
    // be read off, so nothing is renamed. Structural identity says this cannot happen; if it ever
    // does, the authored paths are what the output already used, and an arbitrary rename would be
    // worse than a name that reads oddly.
    if spellings.iter().any(|found| found.len() != bindings.len()) {
        return;
    }

    let mut numbered = 0;
    for (index, binding) in bindings.iter_mut().enumerate() {
        if spellings.iter().all(|found| found[index] == binding.path) {
            continue;
        }
        numbered += 1;
        binding.name = ordinal(numbered, "value");
    }
}

fn copy_positions(group: &Group, prefix: &mut Vec<usize>, into: &mut Vec<Vec<usize>>) {
    for (index, node) in group.items.iter().enumerate() {
        prefix.push(index);
        match node {
            Node::Group(child) => copy_positions(child, prefix, into),
            Node::Item(item) => {
                if item.content.is_some() {
                    into.push(prefix.clone());
                }
            }
            Node::Collection(_) => {}
        }
        prefix.pop();
    }
}

/// The item a [`CopySlot`] path points at, which a call site walks to read its own words.
#[must_use]
pub fn at_path<'a>(group: &'a Group, path: &[usize]) -> Option<&'a Item> {
    let (index, rest) = path.split_first()?;
    match group.items.get(*index)? {
        Node::Group(child) => at_path(child, rest),
        Node::Item(item) if rest.is_empty() => Some(item),
        Node::Item(_) | Node::Collection(_) => None,
    }
}

fn at_path_mut<'a>(group: &'a mut Group, path: &[usize]) -> Option<&'a mut Item> {
    let (index, rest) = path.split_first()?;
    match group.items.get_mut(*index)? {
        Node::Group(child) => at_path_mut(child, rest),
        Node::Item(item) if rest.is_empty() => Some(item),
        Node::Item(_) | Node::Collection(_) => None,
    }
}

/// The generated type name: the slug, plus whatever tells its variants apart.
fn component_name(slug: &str, key: &[(String, String)]) -> String {
    let variant: String = key.iter().map(|(_, value)| pascal(value)).collect();
    format!("{}{variant}Component", pascal(slug))
}

impl Screen {
    /// Every content node this screen owns, across all its regions.
    #[must_use]
    pub fn nodes(&self) -> Vec<&Node> {
        let mut nodes = Vec::new();
        region(&mut nodes, self.header.as_ref(), self.body.as_ref(), &self.footer);
        for state in &self.states {
            region(&mut nodes, state.header.as_ref(), state.body.as_ref(), &state.footer);
        }
        for overlay in &self.overlays {
            nodes.extend(overlay.content.iter());
        }
        nodes
    }

    /// Every body region this screen can show, the default first.
    pub fn bodies(&self) -> impl Iterator<Item = &Body> {
        self.body.iter().chain(self.states.iter().filter_map(|state| state.body.as_ref()))
    }
}

fn region<'a>(
    into: &mut Vec<&'a Node>, header: Option<&'a Header>, body: Option<&'a Body>,
    footer: &'a [Node],
) {
    if let Some(header) = header {
        into.extend(header.nodes());
    }
    if let Some(body) = body {
        into.extend(body.nodes());
    }
    into.extend(footer.iter());
}

impl Surface {
    /// The component a placement calls, matched on its slug and the variant it selects.
    ///
    /// The key holds only the axes that tell the slug's variants apart, so a placement matches on
    /// those and may state any number of others.
    #[must_use]
    pub fn called_by(&self, group: &Group) -> Option<&Component> {
        let slug = group.component.as_ref()?;
        self.components.iter().find(|component| {
            &component.slug == slug
                && component.key.iter().all(|(axis, value)| {
                    group.params.get(axis).map_or(value.is_empty(), |v| v == value)
                })
        })
    }
}

/// Every value a subtree reads and every event it raises, in first-appearance order.
///
/// A component's parameter list, because a generated component has no view model and no core of
/// its own. The walk order is what makes the list agree between the definition and each call site;
/// see [`Callback`] for why the events are named from it rather than from themselves.
pub fn gather(group: &Group, bindings: &mut Vec<Binding>, events: &mut Vec<Callback>) {
    let mut sites = Sites::default();
    walk_slots(group, bindings, events, &mut sites);
}

/// Running counts per kind of event site, so a second one of a kind is `onTap2` rather than a
/// duplicate `onTap`.
#[derive(Default)]
struct Sites {
    tap: usize,
    long_press: usize,
    leading: usize,
    trailing: usize,
    items: BTreeMap<String, usize>,

    /// Handles already spoken for by a `controls[]` entry.
    ///
    /// A nested control is written twice — once as the item that draws it, once as the block that
    /// says what it does — and `ref` is what ties the two together. Both carry the event, so
    /// without this the checkbox in a task row takes two parameters that mean one thing.
    claimed: BTreeSet<String>,
}

impl Sites {
    /// `on<Stem>` the first time and `on<Stem><n>` after it, which keeps the common component —
    /// one tap and nothing else — reading as `onTap`.
    fn name(counter: usize, stem: &str) -> String {
        if counter == 1 { format!("on{stem}") } else { format!("on{stem}{counter}") }
    }
}

fn walk_slots(
    group: &Group, bindings: &mut Vec<Binding>, events: &mut Vec<Callback>, sites: &mut Sites,
) {
    if let Some(interaction) = &group.interaction {
        if let Some(event) = &interaction.event {
            sites.tap += 1;
            events.push(Callback {
                name: Sites::name(sites.tap, "Tap"),
                event: event.clone(),
            });
        }
        if let Some(event) = &interaction.long_press {
            sites.long_press += 1;
            events.push(Callback {
                name: Sites::name(sites.long_press, "LongPress"),
                event: event.clone(),
            });
        }
        // A nested control names itself: `ref` is required on one, and it is the handle the
        // composition already uses to point at the child.
        for control in &interaction.controls {
            sites.claimed.insert(control.handle.clone());
            events.push(Callback {
                name: format!("on{}", pascal(&control.handle)),
                event: control.event.clone(),
            });
        }
        for (edge, swipes) in [
            ("SwipeLeading", &interaction.swipe_leading),
            ("SwipeTrailing", &interaction.swipe_trailing),
        ] {
            for swipe in swipes {
                let counter = if edge == "SwipeLeading" {
                    sites.leading += 1;
                    sites.leading
                } else {
                    sites.trailing += 1;
                    sites.trailing
                };
                events.push(Callback {
                    name: Sites::name(counter, edge),
                    event: swipe.event.clone(),
                });
            }
        }
    }

    for node in &group.items {
        match node {
            Node::Group(child) => walk_slots(child, bindings, events, sites),
            // Unreachable from a lifted component: `parse` refuses a component holding one.
            Node::Collection(_) => {}
            Node::Item(item) => {
                // The path names the parameter for now, and `name_bindings` settles it once every
                // placement has been seen — this walk sees one tree and cannot know whether the
                // others spell it the same way. Deduped on the path and the kind rather than on the
                // whole binding, so that the name settled later cannot split one slot into two.
                if let Some(path) = &item.bind {
                    let kind = BindKind::of_item(item);
                    if !bindings.iter().any(|held| held.path == *path && held.kind == kind) {
                        bindings.push(Binding {
                            name: path.clone(),
                            path: path.clone(),
                            kind,
                        });
                    }
                }
                if let Some(event) = &item.event
                    && !item.handle.as_ref().is_some_and(|handle| sites.claimed.contains(handle))
                {
                    let name = item.handle.as_ref().map_or_else(
                        || {
                            let counter = sites.items.entry(item.kind.clone()).or_default();
                            *counter += 1;
                            Sites::name(*counter, &pascal(&item.kind))
                        },
                        |handle| format!("on{}", pascal(handle)),
                    );
                    events.push(Callback {
                        name,
                        event: event.clone(),
                    });
                }
            }
        }
    }
}

/// Marks every custom-item instance in a screen, across all of its regions.
fn mark_screen(screen: &mut Screen, names: &BTreeSet<&str>) {
    let mark_region =
        |header: Option<&mut Header>, body: Option<&mut Body>, footer: &mut [Node], names| {
            if let Some(header) = header {
                for node in header.nodes_mut() {
                    mark_custom(node, names);
                }
            }
            if let Some(body) = body {
                for node in body.nodes_mut() {
                    mark_custom(node, names);
                }
            }
            for node in footer.iter_mut() {
                mark_custom(node, names);
            }
        };

    mark_region(screen.header.as_mut(), screen.body.as_mut(), &mut screen.footer, names);
    if let Some(fab) = &mut screen.fab {
        fab.custom = names.contains(fab.kind.as_str());
    }
    for state in &mut screen.states {
        mark_region(state.header.as_mut(), state.body.as_mut(), &mut state.footer, names);
        if let Some(fab) = &mut state.fab {
            fab.custom = names.contains(fab.kind.as_str());
        }
    }
    for overlay in &mut screen.overlays {
        for node in &mut overlay.content {
            mark_custom(node, names);
        }
    }
}

/// Marks a node and its descendants, setting [`Item::custom`] where the kind names a declaration.
fn mark_custom(node: &mut Node, names: &BTreeSet<&str>) {
    match node {
        Node::Item(item) => item.custom = names.contains(item.kind.as_str()),
        Node::Group(group) => {
            for child in &mut group.items {
                mark_custom(child, names);
            }
        }
        Node::Collection(collection) => {
            for child in &mut collection.item {
                mark_custom(child, names);
            }
        }
    }
}

fn screen(slug: &str, entry: &Value) -> Screen {
    Screen {
        slug: slug.to_owned(),
        name: string(entry, "name").unwrap_or_else(|| slug.to_owned()),
        align: string(entry, "align"),
        justify: string(entry, "justify"),
        background: string(entry, "background"),
        padding: entry.get("padding").map(padding),
        header: entry.get("header").map(header),
        body: entry.get("body").map(body),
        footer: nodes(entry.get("footer")),
        fab: entry.get("fab").map(|value| item_from("fab", value)),
        states: map_entries(entry.get("states"), state),
        overlays: map_entries(entry.get("overlays"), overlay),
    }
}

fn map_entries<T>(node: Option<&Value>, parse_one: fn(&str, &Value) -> T) -> Vec<T> {
    node.and_then(Value::as_object)
        .map(|map| map.iter().map(|(key, value)| parse_one(key, value)).collect())
        .unwrap_or_default()
}

fn state(key: &str, entry: &Value) -> State {
    State {
        key: key.to_owned(),
        when: string(entry, "when").unwrap_or_default(),
        replaces: match string(entry, "replaces").as_deref() {
            Some("screen") => Replaces::Screen,
            _ => Replaces::Body,
        },
        header: entry.get("header").map(header),
        body: entry.get("body").map(body),
        footer: nodes(entry.get("footer")),
        fab: entry.get("fab").map(|value| item_from("fab", value)),
    }
}

fn overlay(key: &str, entry: &Value) -> Overlay {
    Overlay {
        key: key.to_owned(),
        kind: match string(entry, "kind").as_deref() {
            Some("sheet") => OverlayKind::Sheet,
            Some("snackbar") => OverlayKind::Snackbar,
            _ => OverlayKind::Dialog,
        },
        title: string(entry, "title"),
        trigger: string(entry, "trigger"),
        content: nodes(entry.get("content")),
    }
}

fn header(entry: &Value) -> Header {
    Header {
        title: entry.get("title").and_then(|title| match title {
            Value::String(words) => Some(Arg::Literal(words.clone())),
            _ => string(title, "bind").map(Arg::Bind),
        }),
        style: string(entry, "style"),
        color: string(entry, "color"),
        align: string(entry, "align"),
        padding: entry.get("padding").map(padding),
        leading: nodes(entry.get("leading")),
        trailing: nodes(entry.get("trailing")),
        below: nodes(entry.get("below")),
        gap: string(entry, "gap"),
    }
}

fn body(entry: &Value) -> Body {
    if let Some(list) = entry.get("list") {
        let mut collection = collection(list);
        collection.style =
            Some(string(list, "style").map_or(ListStyle::Plain, |raw| ListStyle::parse(&raw)));
        return Body::List(collection);
    }

    if let Some(grid) = entry.get("grid") {
        return Body::Grid(grid_collection(grid));
    }

    if let Some(form) = entry.get("form") {
        return Body::Form(nodes(Some(form)));
    }

    Body::Nodes(nodes(Some(entry)))
}

/// The pattern a `list` holds, wherever it sits. `style` is left to the caller: only a body list
/// has platform chrome to choose.
fn collection(list: &Value) -> Collection {
    Collection {
        each: string(list, "each").unwrap_or_default(),
        style: None,
        gap: string(list, "gap"),
        inset: string(list, "inset"),
        columns: None,
        section: list.get("section").map(|section| Section {
            title: string(section, "title").unwrap_or_default(),
            items: string(section, "items").unwrap_or_default(),
            style: string(section, "style"),
            color: string(section, "color"),
        }),
        direction: Direction::parse(string(list, "direction").as_deref()),
        scroll: list.get("scroll").and_then(Value::as_bool).unwrap_or(false),
        item: nodes(list.get("item")),
    }
}

/// The pattern a `grid` holds, wherever it sits.
fn grid_collection(grid: &Value) -> Collection {
    Collection {
        columns: grid.get("columns").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()),
        section: None,
        ..collection(grid)
    }
}

fn nodes(value: Option<&Value>) -> Vec<Node> {
    value
        .and_then(Value::as_array)
        .map(|array| array.iter().filter_map(node).collect())
        .unwrap_or_default()
}

/// A content node is a single-key map: `{ group: … }` or `{ text: … }`.
fn node(value: &Value) -> Option<Node> {
    let object = value.as_object()?;
    let (key, props) = object.iter().next()?;

    match key.as_str() {
        "group" => Some(Node::Group(Box::new(group(props)))),
        "list" => Some(Node::Collection(Box::new(collection(props)))),
        "grid" => Some(Node::Collection(Box::new(grid_collection(props)))),
        _ => Some(Node::Item(Box::new(item_from(key, props)))),
    }
}

fn group(props: &Value) -> Group {
    let mut template = drawn_group(props);
    template.bound = bound(props, &template);
    template
}

/// A group as the composition draws it, before any bound variant is read.
fn drawn_group(props: &Value) -> Group {
    Group {
        component: string(props, "component"),
        params: props
            .get("params")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(name, value)| {
                        value.as_str().map(|value| (name.clone(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        args: props
            .get("args")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(name, value)| {
                        let arg = match value {
                            Value::String(words) => Arg::Literal(words.clone()),
                            _ => string(value, "asset")
                                .map(Arg::Asset)
                                .or_else(|| string(value, "bind").map(Arg::Bind))?,
                        };
                        Some((name.clone(), arg))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        formats: props
            .get("args")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(name, value)| {
                        Some((
                            name.clone(),
                            string(value, "format").as_deref().and_then(Format::parse)?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        bound: None,
        direction: Direction::parse(string(props, "direction").as_deref()),
        style: style(props),
        justify: string(props, "justify"),
        align: string(props, "align"),
        wrap: props.get("wrap").and_then(Value::as_bool).unwrap_or(false),
        flow_wrap: props.get("flow_wrap").and_then(Value::as_bool).unwrap_or(false),
        size: size(props.get("size")),
        handle: string(props, "ref"),
        layout: layout(props),
        interaction: props.get("interaction").map(interaction),
        a11y: a11y(props),
        items: nodes(props.get("items")),
    }
}

/// The one `params` entry a row binds, with a placement per look. `None` for a group binding none,
/// and for one binding several, which `vx spec` refuses.
fn bound(props: &Value, template: &Group) -> Option<Bound> {
    let params = props.get("params")?.as_object()?;
    let mut bound = params.iter().filter_map(|(axis, value)| Some((axis, string(value, "bind")?)));
    let (axis, field) = bound.next()?;
    if bound.next().is_some() {
        return None;
    }

    let looks = props
        .get("looks")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(value, look)| {
            let mut placement = drawn_group(look);
            lay_over(&mut placement, template);
            placement.component.clone_from(&template.component);
            placement.params = template.params.clone();
            placement.params.insert(axis.clone(), value.clone());
            (value.clone(), placement)
        })
        .collect();

    Some(Bound {
        axis: axis.clone(),
        field,
        looks,
    })
}

/// Lays what `template` reads and raises over `look`, position by position: a look is only how the
/// row is drawn, so its fields, events, handles and accessibility are the template's.
fn lay_over(look: &mut Group, template: &Group) {
    look.args.clone_from(&template.args);
    look.formats.clone_from(&template.formats);
    look.handle.clone_from(&template.handle);
    look.interaction.clone_from(&template.interaction);
    look.a11y.clone_from(&template.a11y);

    for (drawn, read) in look.items.iter_mut().zip(&template.items) {
        match (drawn, read) {
            (Node::Group(drawn), Node::Group(read)) => lay_over(drawn, read),
            (Node::Item(drawn), Node::Item(read)) => {
                // A bound item's words are a sample of its field, which the template's already
                // stands for. Each look keeping its own would put a second sample of one field in
                // the copy contract — `15:00` beside `14:00` — as a string nothing reads.
                if read.bind.is_some() {
                    drawn.content.clone_from(&read.content);
                }
                drawn.bind.clone_from(&read.bind);
                drawn.format = read.format;
                drawn.event.clone_from(&read.event);
                drawn.handle.clone_from(&read.handle);
                drawn.arg.clone_from(&read.arg);
                drawn.custom = read.custom;
                drawn.raises.clone_from(&read.raises);
                drawn.binds.clone_from(&read.binds);
                drawn.a11y.clone_from(&read.a11y);
            }
            _ => {}
        }
    }
}

/// An item outside any composition, spelled `{ <kind>: <props> }` — what a web capture keeps in
/// `x-web` for the media a button carries.
#[must_use]
pub fn loose_item(value: &Value) -> Option<Item> {
    let object = value.as_object()?;
    if object.len() != 1 {
        return None;
    }
    let (kind, props) = object.iter().next()?;
    Some(item_from(kind, props))
}

fn item_from(kind: &str, props: &Value) -> Item {
    let empty = Value::Object(Map::new());
    let props = if props.is_null() { &empty } else { props };

    Item {
        kind: kind.to_owned(),
        content: string(props, "content"),
        copy_param: None,
        asset_param: None,
        arg: string(props, "arg"),
        bind: string(props, "bind"),
        format: string(props, "format").as_deref().and_then(Format::parse),
        name: string(props, "name").or_else(|| string(props, "icon")),
        placeholder: string(props, "placeholder"),
        options: props
            .get("options")
            .and_then(Value::as_array)
            .map(|array| array.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default(),
        style_key: string(props, "style"),
        align: string(props, "align"),
        style: style(props),
        size: size(props.get("size")),
        handle: string(props, "ref"),
        event: string(props, "event").map(|raw| Event::parse(&raw)),
        custom: false,
        raises: string_list(props, "raises").iter().map(|raw| Event::parse(raw)).collect(),
        binds: string_list(props, "binds"),
        a11y: a11y(props),
        layout: layout(props),
        facts: facts(props),
    }
}

/// An item's [`ItemFacts`].
fn facts(props: &Value) -> ItemFacts {
    ItemFacts {
        text_style: string(props, "text_style"),
        line_limit: props
            .get("line_limit")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok()),
        gradient: props.get("gradient").and_then(|g| {
            let stops: Vec<(String, f64)> = g
                .get("stops")?
                .as_array()?
                .iter()
                .filter_map(|stop| Some((string(stop, "color")?, stop.get("at")?.as_f64()?)))
                .collect();
            (stops.len() >= 2).then(|| Gradient {
                angle: g.get("angle").and_then(Value::as_f64).unwrap_or(180.0),
                stops,
            })
        }),
        destination: string(props, "destination"),
        content_type: string(props, "content_type"),
        content_mode: string(props, "content_mode"),
    }
}

/// A node's [`Layout`], read from its `align_self`, `position` and `x-web`.
fn layout(props: &Value) -> Layout {
    Layout {
        align_self: string(props, "align_self"),
        position: props.get("position").and_then(|position| {
            Some(Layer {
                align: string(position, "align")?,
                x: position.get("offset").and_then(|offset| string(offset, "x")),
                y: position.get("offset").and_then(|offset| string(offset, "y")),
            })
        }),
        pinned: string(props, "pinned"),
        web: props.get("x-web").filter(|web| web.is_object()).cloned(),
    }
}

/// A directive carried through as a JSON array of strings, or empty when absent.
fn string_list(props: &Value, key: &str) -> Vec<String> {
    props
        .get(key)
        .and_then(Value::as_array)
        .map(|array| array.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn interaction(props: &Value) -> Interaction {
    Interaction {
        archetype: string(props, "archetype")
            .and_then(|raw| Archetype::parse(&raw))
            .unwrap_or(Archetype::Static),
        event: string(props, "event").map(|raw| Event::parse(&raw)),
        long_press: string(props, "long_press").map(|raw| Event::parse(&raw)),
        controls: array(props.get("controls"), control),
        swipe_leading: array(props.get("swipe_leading"), swipe),
        swipe_trailing: array(props.get("swipe_trailing"), swipe),
    }
}

fn array<T>(node: Option<&Value>, parse_one: fn(&Value) -> T) -> Vec<T> {
    node.and_then(Value::as_array)
        .map(|entries| entries.iter().map(parse_one).collect())
        .unwrap_or_default()
}

fn control(entry: &Value) -> Control {
    Control {
        handle: string(entry, "ref").unwrap_or_default(),
        event: Event::parse(&string(entry, "event").unwrap_or_default()),
        a11y_action: string(entry, "a11y_action"),
        propagates: entry.get("propagates").and_then(Value::as_bool).unwrap_or(false),
    }
}

fn swipe(entry: &Value) -> Swipe {
    Swipe {
        event: Event::parse(&string(entry, "event").unwrap_or_default()),
        icon: string(entry, "icon"),
        label: string(entry, "label"),
        background: string(entry, "background"),
        tint: string(entry, "tint"),
        destructive: entry.get("destructive").and_then(Value::as_bool).unwrap_or(false),
    }
}

fn style(props: &Value) -> Style {
    Style {
        gap: string(props, "gap"),
        padding: props.get("padding").map(padding),
        background: string(props, "background"),
        color: string(props, "color"),
        corner_radius: string(props, "corner_radius"),
        elevation: string(props, "elevation"),
        border: props.get("border").and_then(border),
        opacity: string(props, "opacity"),
        shadow: string(props, "shadow"),
        clip: props.get("clip").and_then(Value::as_bool).unwrap_or(false),
        cross_gap: string(props, "cross_gap"),
        background_image: props.get("background_image").and_then(|bg| {
            Some(BackgroundImage {
                name: string(bg, "name")?,
                content_mode: string(bg, "content_mode"),
                alignment: string(bg, "alignment").unwrap_or_else(|| "top-start".to_owned()),
            })
        }),
    }
}

/// A border in either spelling: the bare width token, or the width with the colour this layer
/// strokes it in.
fn border(value: &Value) -> Option<Border> {
    if let Some(width) = value.as_str() {
        return Some(Border {
            width: width.to_owned(),
            color: None,
        });
    }

    Some(Border {
        width: string(value, "width")?,
        color: string(value, "color"),
    })
}

/// Uniform or per side. The logical sides (`start` / `end`) read as left / right: the shells lay
/// out left to right, and a physical side, when both are given, wins.
fn padding(value: &Value) -> Padding {
    value.as_str().map_or_else(
        || Padding::Sides {
            top: string(value, "top"),
            right: string(value, "right").or_else(|| string(value, "end")),
            bottom: string(value, "bottom"),
            left: string(value, "left").or_else(|| string(value, "start")),
        },
        |token| Padding::All(token.to_owned()),
    )
}

fn size(value: Option<&Value>) -> Size {
    let Some(value) = value else {
        return Size::default();
    };
    let bound = |key: &str| value.get(key).and_then(Value::as_f64);
    Size {
        width: value.get("width").and_then(dimension),
        height: value.get("height").and_then(dimension),
        min_width: bound("min_width"),
        max_width: bound("max_width"),
        min_height: bound("min_height"),
        max_height: bound("max_height"),
    }
}

fn dimension(value: &Value) -> Option<Dimension> {
    if let Some(number) = value.as_f64() {
        return Some(Dimension::Fixed(number));
    }
    match value.as_str()? {
        "fill" => Some(Dimension::Fill),
        "hug" => Some(Dimension::Hug),
        _ => None,
    }
}

fn a11y(props: &Value) -> A11y {
    A11y {
        label: string(props, "label"),
        hint: string(props, "hint"),
        role: string(props, "role"),
        test_id: string(props, "test_id"),
    }
}

fn string(node: &Value, key: &str) -> Option<String> {
    node.get(key).and_then(Value::as_str).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_splits_into_a_name_and_its_arguments() {
        assert_eq!(
            Event::parse("EditTask(id)"),
            Event {
                name: "EditTask".to_owned(),
                args: vec!["id".to_owned()]
            }
        );
        assert_eq!(
            Event::parse("ClearCompleted"),
            Event {
                name: "ClearCompleted".to_owned(),
                args: Vec::new()
            }
        );
        assert_eq!(Event::parse("Move(from, to)").args, ["from", "to"]);
    }

    #[test]
    fn a_condition_that_is_a_path_is_one_in_whichever_case_it_was_written() {
        assert_eq!(Condition::parse("loading"), Condition::Flag("loading".to_owned()));
        assert_eq!(Condition::parse("draft.dirty"), Condition::Flag("draft.dirty".to_owned()));
        assert_eq!(Condition::parse("hasResults"), Condition::Flag("hasResults".to_owned()));
    }

    #[test]
    fn an_emptiness_test_names_the_collection_rather_than_a_field() {
        assert_eq!(Condition::parse("tasks.is_empty"), Condition::Empty("tasks".to_owned()));
        assert_eq!(Condition::parse("lists.isEmpty"), Condition::Empty("lists".to_owned()));
        assert_eq!(
            Condition::parse("day.tasks.is_empty"),
            Condition::Empty("day.tasks".to_owned())
        );

        // A name that merely ends in the word is a field like any other, and a bare `is_empty`
        // has no collection to test.
        assert_eq!(
            Condition::parse("tasks_are_empty"),
            Condition::Flag("tasks_are_empty".to_owned())
        );
        assert_eq!(Condition::parse("is_empty"), Condition::Flag("is_empty".to_owned()));
    }

    #[test]
    fn a_condition_that_is_an_expression_is_left_alone() {
        for expression in ["tasks.count > 0", "!loading", "count-1", ""] {
            assert_eq!(
                Condition::parse(expression),
                Condition::Expression(expression.to_owned()),
                "{expression}"
            );
        }
    }

    #[test]
    fn a_bound_value_takes_its_type_from_the_item_showing_it() {
        assert_eq!(BindKind::of("text"), BindKind::Text);
        assert_eq!(BindKind::of("checkbox"), BindKind::Flag);
        assert_eq!(BindKind::of("slider"), BindKind::Number);
    }

    /// A component is rendered with its parameters alone, so a collection inside one has nothing to
    /// iterate. The refusal is read by a person at `vx emit`, so it has to read as one sentence.
    #[test]
    fn a_component_holding_a_collection_is_refused_in_plain_words() {
        let document = serde_json::json!({ "version": 1, "screens": { "today": { "body": [
            { "group": { "component": "forecast", "items": [
                { "list": { "each": "hours", "item": [{ "text": { "bind": "time" } }] } },
            ] } },
        ] } } });

        let error = parse(&document).expect_err("a component cannot iterate");
        assert!(matches!(error, Error::Malformed(_)), "{error:?}");
        let message = error.to_string();
        assert!(message.contains("component `forecast`"), "{message}");
        assert!(!message.contains("  "), "no runs of spaces: {message:?}");
    }

    #[test]
    fn a_document_without_screens_is_refused() {
        let error = parse(&serde_json::json!({ "version": 1 })).expect_err("no screens");
        assert!(matches!(error, Error::Malformed(_)));
    }

    #[test]
    fn a_list_body_carries_its_binding_and_its_template() {
        let document = serde_json::json!({
            "screens": {
                "task-list": {
                    "name": "TaskListScreen",
                    "body": { "list": { "each": "tasks", "item": [{ "text": { "bind": "title" } }] } }
                }
            }
        });

        let surface = parse(&document).expect("a list body parses");
        let screen = &surface.screens[0];
        let Some(Body::List(collection)) = &screen.body else {
            panic!("expected a list body");
        };

        assert_eq!(collection.each, "tasks");
        assert_eq!(collection.item.len(), 1);
    }

    /// A text drawing a declared TEXT property is a parameter named for the property, whatever the
    /// placements' words — and each call site passes its own argument, literal or bound, rather than
    /// whatever the first placement's tree happened to draw.
    #[test]
    fn a_declared_argument_is_a_named_parameter_each_placement_fills() {
        let tile = |label: &str, value: Value, drawn: Option<&str>| {
            serde_json::json!({ "group": {
                "component": "metric-tile",
                "args": { "label": label, "dew-point": value },
                "items": [
                    { "text": { "arg": "label", "content": label } },
                    { "text": { "arg": "dew-point", "content": drawn } },
                    { "text": { "content": "Now" } }
                ]
            } })
        };
        let document = serde_json::json!({ "screens": { "today": { "name": "Today", "body": [
            tile("HUMIDITY", serde_json::json!({ "bind": "dew_point" }), None),
            tile("HUMIDITY", serde_json::json!("11°"), Some("11°"))
        ] } } });

        let surface = parse(&document).expect("the document parses");
        let component = &surface.components[0];

        let names: Vec<&str> = component.copy.iter().map(|slot| slot.name.as_str()).collect();
        assert_eq!(names, ["label", "dewPoint"], "declared even where the placements agree");
        assert_eq!(component.copy[1].arg.as_deref(), Some("dew-point"));

        let Some(Node::Group(first)) = surface.screens[0].nodes().first().copied() else {
            panic!("the first placement is a group");
        };
        assert_eq!(said(first, &component.copy[0]), Said::Literal("HUMIDITY"));
        assert_eq!(said(first, &component.copy[1]), Said::Bind("dew_point"));
    }

    /// An icon drawing a declared asset property is an asset parameter, not copy, and each call
    /// site passes the key it chose — not the first placement's artwork, which is what drew every
    /// weather tile as a droplet.
    #[test]
    fn a_declared_asset_argument_is_an_asset_parameter_each_placement_fills() {
        let tile = |label: &str, icon: &str| {
            serde_json::json!({ "group": {
                "component": "metric-tile",
                "args": { "label": label, "icon": { "asset": icon } },
                "items": [
                    { "icon": { "arg": "icon", "name": icon } },
                    { "text": { "arg": "label", "content": label } }
                ]
            } })
        };
        let document = serde_json::json!({ "screens": { "today": { "name": "Today", "body": [
            tile("HUMIDITY", "icon-drop-16"),
            tile("WIND", "icon-wind")
        ] } } });

        let surface = parse(&document).expect("the document parses");
        let component = &surface.components[0];

        let copy: Vec<&str> = component.copy.iter().map(|slot| slot.name.as_str()).collect();
        let assets: Vec<&str> = component.assets.iter().map(|slot| slot.name.as_str()).collect();
        assert_eq!(copy, ["label"]);
        assert_eq!(assets, ["icon"]);
        assert_eq!(
            at_path(&component.group, &component.assets[0].path).unwrap().asset_param.as_deref(),
            Some("icon")
        );

        let nodes = surface.screens[0].nodes();
        let [Node::Group(first), Node::Group(second)] = nodes[..] else {
            panic!("two placements");
        };
        assert_eq!(shown(first, &component.assets[0]), "icon-drop-16");
        assert_eq!(shown(second, &component.assets[0]), "icon-wind");
    }

    #[test]
    fn a_component_is_lifted_once_however_many_screens_use_it() {
        let row = serde_json::json!({
            "group": {
                "component": "task-row",
                "items": [{ "text": { "bind": "title" } }],
                "interaction": { "archetype": "tap-target", "event": "EditTask(id)" }
            }
        });
        let document = serde_json::json!({
            "screens": {
                "a": { "name": "A", "body": [row] },
                "b": { "name": "B", "body": [row] }
            }
        });

        let surface = parse(&document).expect("the document parses");
        assert_eq!(surface.components.len(), 1);

        let component = &surface.components[0];
        assert_eq!(component.slug, "task-row");
        assert_eq!(
            component.bindings,
            [Binding {
                name: "title".to_owned(),
                path: "title".to_owned(),
                kind: BindKind::Text
            }],
            "both placements bind the same path, so the path names the parameter",
        );
        assert_eq!(
            component.events,
            [Callback {
                name: "onTap".to_owned(),
                event: Event::parse("EditTask(id)")
            }]
        );
        assert!(component.group.component.is_none(), "the marker does not survive the lift");
    }

    /// One `button` slug, four placements: two variants, and copy that varies inside each.
    fn buttons() -> Value {
        let placement = |importance: &str, says: &str, event: &str| {
            serde_json::json!({
                "group": {
                    "component": "button",
                    "params": { "importance": importance, "state": "default" },
                    "background": if importance == "primary" { "brand-primary" } else { "surface-background" },
                    "interaction": { "archetype": "control", "event": event },
                    "items": [{ "text": { "content": says } }]
                }
            })
        };

        serde_json::json!({
            "screens": {
                "splash": {
                    "name": "Splash",
                    "body": [placement("primary", "Get Organised!", "CreateList")]
                },
                "confirm": {
                    "name": "Confirm",
                    "body": [
                        placement("primary", "Cancel", "Dismiss"),
                        placement("destructive", "Delete", "DeleteList")
                    ]
                }
            }
        })
    }

    #[test]
    fn a_variant_axis_that_varies_splits_the_slug_into_its_own_components() {
        let surface = parse(&buttons()).expect("the document parses");

        let names: Vec<_> = surface.components.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(names, ["ButtonDestructiveComponent", "ButtonPrimaryComponent"]);

        // `state` is `default` at every placement, so it names nothing. Were it in the key, every
        // component would be a `…DefaultComponent`.
        for component in &surface.components {
            assert_eq!(component.key.keys().collect::<Vec<_>>(), ["importance"]);
        }

        // The reason the split matters: the variants are drawn differently, and keying on the slug
        // alone gave all of them the first placement's artwork.
        let destructive = &surface.components[0];
        let primary = &surface.components[1];
        assert_eq!(destructive.group.style.background.as_deref(), Some("surface-background"));
        assert_eq!(primary.group.style.background.as_deref(), Some("brand-primary"));
    }

    #[test]
    fn copy_that_differs_between_placements_of_one_variant_becomes_a_parameter() {
        let surface = parse(&buttons()).expect("the document parses");
        let primary =
            surface.components.iter().find(|one| one.name.contains("Primary")).expect("primary");

        // Two primary placements, "Get Organised!" and "Cancel".
        assert_eq!(primary.copy.len(), 1);
        assert_eq!(primary.copy[0].name, "label");
        assert_eq!(
            at_path(&primary.group, &primary.copy[0].path).unwrap().copy_param.as_deref(),
            Some("label")
        );

        // The destructive variant has one placement, so nothing varies and the word stays a
        // literal rather than becoming a parameter nobody needs.
        let destructive = surface
            .components
            .iter()
            .find(|one| one.name.contains("Destructive"))
            .expect("destructive");
        assert!(destructive.copy.is_empty());
    }

    #[test]
    fn a_placement_resolves_to_the_variant_its_params_select() {
        let surface = parse(&buttons()).expect("the document parses");
        let confirm =
            surface.screens.iter().find(|screen| screen.slug == "confirm").expect("confirm");
        let items = confirm.body.as_ref().expect("a body").nodes();

        let called = |node: &Node| {
            let Node::Group(group) = node else { panic!("a placement is a group") };
            surface.called_by(group).expect("the placement resolves").name.clone()
        };
        assert_eq!(called(&items[0]), "ButtonPrimaryComponent");
        assert_eq!(called(&items[1]), "ButtonDestructiveComponent");
    }

    #[test]
    fn two_placements_of_one_component_agree_on_the_parameter_name() {
        let placement = |event: &str| {
            serde_json::json!({
                "group": {
                    "component": "button",
                    "interaction": { "archetype": "control", "event": event },
                    "items": [{ "text": { "content": "Go" } }]
                }
            })
        };

        let document = serde_json::json!({
            "screens": {
                "splash": { "name": "Splash", "body": [placement("CreateList")] },
                "confirm": { "name": "Confirm", "body": [placement("DeleteList")] }
            }
        });

        let surface = parse(&document).expect("the document parses");
        let component = &surface.components[0];

        // The lift takes the first placement, so naming the parameter after the event it happened
        // to raise is what put `onCreateList` in the signature and `onDeleteList` at the second
        // call site. The site names it instead, and the two agree.
        assert_eq!(component.events.len(), 1);
        assert_eq!(component.events[0].name, "onTap");

        for screen in &surface.screens {
            let mut bindings = Vec::new();
            let mut events = Vec::new();
            let node = &screen.body.as_ref().expect("the screen has a body").nodes()[0];
            let Node::Group(group) = node else { panic!("the placement is a group") };
            gather(group, &mut bindings, &mut events);
            assert_eq!(events[0].name, component.events[0].name);
        }
    }
}
