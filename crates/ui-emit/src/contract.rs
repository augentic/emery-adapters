//! The Rust types the core and the shells agree about.
//!
//! This is the stage that makes a model-written core tractable. Emery asks a model to write a Crux
//! core and then discovers, at compile time, that it named the field `taskTitle` where the
//! generated `SwiftUI` reads `title` — a mismatch caught late, in a language the model was not
//! looking at, and repaired by guessing which of the two to change.
//!
//! Here the composition decides. `bind: title` on a screen means `TaskListView` has a `title`
//! field, and both the Swift that reads it and the Rust that fills it are generated from the same
//! sentence. What is left for [`vx core`] is behaviour: a `Model`, an `update()`, and the effects
//! it runs. That is a genuinely hard problem, and it is the only one left.
//!
//! # What is derived, and from what
//!
//! | Composition | Rust |
//! | --- | --- |
//! | a screen | `pub struct <Screen>View`, and a `ViewModel` variant |
//! | `bind:` on a screen | a field on that screen's view |
//! | `list.each: tasks` | `pub tasks: Vec<…>`, and an element struct from the row template |
//! | `bind:` inside a row | a field on the element struct |
//! | `event: EditTask(id)` | an `Event::EditTask` variant |
//! | `event: Navigate(Cities)` | an `Event::Navigate(Route)` variant, one however many name it |
//! | `states.<key>.when: <path>` | a `bool` field, when the condition is a bare path |
//! | `states.<key>.when: <path>.is_empty` | nothing — the collection at `<path>` answers it |
//! | `overlays.<key>` | a `bool` field |
//! | every screen | a `Route` variant |
//!
//! # Event arguments
//!
//! An argument is a name, and [`Source`] is what that name turns out to mean: a field on the row in
//! scope, a field on the screen, a screen the app navigates to, or — on a control that hands a
//! value back — the value the user just produced. The type follows from wherever it resolved, so
//! `ListNameChanged(value)` on a text field is a `String` because a text field produces one, not
//! because `String` is the fallback.
//!
//! An argument that resolves to nothing is a [`Gap`]. `vx emit` refuses the whole run over one,
//! because the alternative is a shell reading a member of a view model that has none — a compile
//! error two languages away from the sentence that caused it, in a file nobody is allowed to edit.
//!
//! [`vx core`]: https://docs.rs/vx

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::model::{
    Arg, BindKind, Collection, Condition, Event, FooterMenu, Item, Node, Screen, Surface,
};
use crate::naming::{pascal, snake};

/// The element field every row template gets whether it asked for one or not.
///
/// `ForEach(items, id: \.id)` and `key = { it.id }` are how both shells identify a row across a
/// reorder, so a row without an `id` renders but animates wrongly and loses its selection. The
/// design file never mentions it, and it is not a decision anybody would want to be asked about.
const ROW_IDENTITY: &str = "id";

/// Everything the composition fixes about the core's types.
#[derive(Debug, Clone, Default)]
pub struct Contract {
    /// One entry per screen, in slug order.
    pub screens: Vec<ScreenContract>,

    /// Every event any screen raises, by variant name.
    pub events: BTreeMap<String, Vec<Argument>>,

    /// Event arguments nothing in scope names, in the order they were found.
    ///
    /// Not an error here, because the contract is also what says *why* it is one. `vx emit`
    /// refuses to write anything while this is non-empty.
    pub gaps: Vec<Gap>,

    /// Events raised with two different argument lists, in the order they were found.
    ///
    /// Refused alongside [`Self::gaps`] and for the same reason. See [`Clash`].
    pub clashes: Vec<Clash>,

    /// Fields bound as a typed display value in one place and as another kind in another.
    ///
    /// Refused alongside [`Self::gaps`] and for the same reason. See [`Conflict`].
    pub conflicts: Vec<Conflict>,
}

/// One screen's view model.
#[derive(Debug, Clone)]
pub struct ScreenContract {
    /// The screen's kebab-case slug.
    pub slug: String,

    /// The Rust type name, `TaskListView`.
    pub view: String,

    /// The module the type lives in, `task_list`.
    pub module: String,

    /// The view's fields.
    pub fields: Fields,

    /// Element structs for the collections this screen iterates, in field order.
    pub elements: Vec<Element>,

    /// Every `Route` variant the app has, which is one per screen.
    ///
    /// The whole set rather than this screen's own, because a route is the one thing an event
    /// argument can name that is not local to the screen raising it: `Navigate(Cities)` on the
    /// today screen names a destination, and the today screen is the only scope [`Self::source`]
    /// has to answer from.
    pub routes: BTreeSet<String>,
}

impl ScreenContract {
    /// The enum a bound variant's `field` holds, among this screen's row structs, matched on the
    /// variants it spells too, so two lists each binding a `state` cannot be confused.
    #[must_use]
    pub fn choice(&self, field: &str, spelled: &[String]) -> Option<&str> {
        self.elements.iter().find_map(|element| match element.fields.get(field) {
            Some(Field::Choice { name, variants, .. }) if variants == spelled => {
                Some(name.as_str())
            }
            _ => None,
        })
    }

    /// Every bound variant's enum this screen's rows hold.
    #[must_use]
    pub fn choices(&self) -> Vec<&str> {
        self.elements
            .iter()
            .flat_map(|element| element.fields.values())
            .filter_map(|field| match field {
                Field::Choice { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Where a shell reads `argument` from, for an event raised by a control that produces
    /// `produces` and sits inside a row when `within_element`.
    ///
    /// The order is the whole decision:
    ///
    /// 1. the control's own value, when the argument names what the control is bound to — a
    ///    checkbox that raises `Toggle(completed)` means the state it was just switched *to*, and
    ///    reading `completed` back off the row would send the state it was switched from;
    /// 2. a field on the row in scope, which is what `EditTask(id)` on a row means;
    /// 3. a field on the screen;
    /// 4. a screen, which is what `Navigate(Cities)` means — read after the fields so a screen
    ///    that binds a value sharing a route's name still answers for it, and before the produced
    ///    fallback because a route names a destination whatever control was tapped;
    /// 5. the control's own value, for the argument the composition invented a name for — a text
    ///    field with no `bind:` raising `ListNameChanged(value)` has nothing else it could mean;
    /// 6. nothing, which is a [`Gap`].
    #[must_use]
    pub fn source(
        &self, argument: &str, bind: Option<&str>, produces: Option<BindKind>, within_element: bool,
    ) -> Source {
        if let Some(kind) = produces
            && bind.is_some_and(|path| names(path, argument))
        {
            return Source::Produced(kind);
        }
        if within_element && self.elements.iter().any(|element| holds(&element.fields, argument)) {
            return Source::Element;
        }
        if holds(&self.fields, argument) {
            return Source::ViewModel;
        }
        if self.routes.contains(argument) {
            return Source::Route;
        }
        produces.map_or(Source::Unresolved, Source::Produced)
    }
}

/// Whether a bind path is the one an argument names: `draft.title` is named by `title`.
fn names(path: &str, argument: &str) -> bool {
    path == argument || path.rsplit('.').next() == Some(argument)
}

/// Whether a field set holds a scalar an argument names.
///
/// Top level only, and scalars only. An argument is spelled `viewModel.<name>` or `item.<name>`,
/// so a `title` nested under a `draft` is not something an argument called `title` can read, and a
/// collection is not a value an event can carry.
fn holds(fields: &Fields, argument: &str) -> bool {
    matches!(fields.get(argument), Some(Field::Value(_)))
}

/// The element type a collection iterates.
#[derive(Debug, Clone)]
pub struct Element {
    /// The Rust type name, `TaskListTask`.
    pub name: String,

    /// The element's fields, always including `id`.
    pub fields: Fields,
}

/// One event argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// The name as the composition wrote it, which is a path on the row in scope.
    pub name: String,

    /// What the value is, which is what says both how Rust spells it and what it means.
    pub carries: Carried,
}

impl Argument {
    /// How Rust spells this argument's type.
    #[must_use]
    pub const fn rust(&self) -> &'static str {
        self.carries.rust()
    }
}

/// What an event argument carries.
///
/// Wider than [`BindKind`] for the same reason vx's `responses` is: `BindKind` is the vocabulary of
/// what a design file can *depict* — text, a flag, a number, a moment — and a destination is none
/// of those. Collapsing a route into text would make `Navigate` take a `String`, which is a screen
/// name the core has to match on and the compiler cannot check, when the enum that makes it
/// checkable is generated four hundred lines above it in the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    /// A value the screen draws.
    Value(BindKind),

    /// A screen to go to, spelled as the `Route` variant it is.
    Route,
}

impl Carried {
    /// How Rust spells it.
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Value(kind) => kind.rust(),
            Self::Route => "Route",
        }
    }

    /// What the value means, for the kinds where the type alone does not say.
    #[must_use]
    pub const fn unit(self) -> Option<&'static str> {
        match self {
            Self::Value(kind) => kind.unit(),
            // `Route` is one screen per variant and says so in its own doc comment.
            Self::Route => None,
        }
    }
}

/// Where a shell reads an event argument's value from.
///
/// One answer, used twice: [`derive()`] types the `Event` variant from it and the view emitters spell
/// the read from it. They used to decide separately — the views spelled every argument as a read
/// off the view model whether or not a field of that name existed — and an argument naming nothing
/// became `viewModel.value` against a unit struct, which is a compile error in a file the operator
/// is told not to edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A field on the row in scope.
    Element,

    /// A field on the screen's view model.
    ViewModel,

    /// The value the control itself just produced — what was typed, chosen, toggled or set.
    ///
    /// The view model cannot answer this one: it holds what the core last pushed, which is the
    /// value *before* the interaction. A control whose change event carried a view-model read
    /// could only ever tell the core what it already knew, so text entry could not work at all.
    Produced(BindKind),

    /// A screen, named by the `Route` variant it is spelled as.
    ///
    /// Carried without the variant because the argument already is one: a route variant is
    /// `pascal(slug)`, which is exactly what the composition wrote inside the parentheses. Each
    /// emitter spells that name for its own platform rather than reading it off a field.
    Route,

    /// Nothing in scope names it. See [`Gap`].
    Unresolved,
}

/// An event argument nothing in scope names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// The screen the event is raised on.
    pub screen: String,

    /// The event, spelled as the composition spells it.
    pub event: String,

    /// The argument that names nothing.
    pub argument: String,
}

impl fmt::Display for Gap {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}: `{}` carries `{}`, and nothing on the screen is called that. Either bind a value \
             of that name on the screen, or drop the argument and let the core hold what it needs.",
            self.screen, self.event, self.argument,
        )
    }
}

/// One event raised with two different argument lists.
///
/// `Event` has one variant per name, so the first spelling is the one the contract declares and
/// every site that disagrees is generated against a variant that cannot take what it passes — a
/// shell writing `Event.CityDeleted` against a `CityDeleted(String)`. Caught here for the reason
/// [`Gap`] is: left alone it surfaces as a compile error in Swift or Kotlin, in a generated file
/// the operator is told not to edit, two languages from the sentence that caused it.
///
/// Only the *number* of arguments is compared. Two sites naming different values of the same
/// arity is ordinary — `Navigate(Today)` and `Navigate(Cities)` are one variant and one payload —
/// and it is the shape of the variant that a shell is generated against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// The screen the disagreeing site is on.
    pub screen: String,

    /// The event both sites name.
    pub event: String,

    /// The argument list the contract declared, as the first site spelled it.
    pub declared: String,

    /// The argument list this site raises it with.
    pub raised: String,
}

impl fmt::Display for Clash {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}: `{}` is raised here and as `{}` elsewhere, and one `{}` variant cannot be both. \
             Either raise it with the same arguments everywhere, or give the two a name each — \
             they are different things if they carry different values.",
            self.screen, self.raised, self.declared, self.event,
        )
    }
}

/// One field bound as two kinds of value, where at least one is a typed display value.
///
/// A struct field has one type. Before `format:` the first site to bind a name decided it and a
/// later one disagreeing went unsaid, which was tolerable while every text was a `String`. A typed
/// display value is a promise to the core about what to fill, so a site that reads the same field
/// as something else is a shell generated against a type the field does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The screen the field is on.
    pub screen: String,

    /// The struct holding it: the screen's view, or the row a collection iterates.
    pub owner: String,

    /// The field, as its bind path spells it.
    pub field: String,

    /// What the first site binds it as.
    pub kept: BindKind,

    /// What a later site binds it as.
    pub other: BindKind,
}

impl fmt::Display for Conflict {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}: `{}` on `{}` is bound as {} in one place and as {} in another, and one field has \
             one type. Bind the two as one kind of value, or give them a field each.",
            self.screen,
            self.field,
            self.owner,
            self.kept.describe(),
            self.other.describe(),
        )
    }
}

/// A view's fields, in name order.
///
/// Ordered rather than insertion-ordered so that adding a `bind:` to the middle of a screen moves
/// one line of the generated struct rather than all of them.
pub type Fields = BTreeMap<String, Field>;

/// One field of a generated struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    /// A scalar the shell displays.
    Value(BindKind),

    /// A repeated element, naming the struct its rows are.
    Repeated(String),

    /// A nested object, from a dotted bind path like `author.name`.
    Nested(Fields),

    /// A scalar two sites bind as different kinds, one of them a typed display value. Rendered as
    /// the first, and reported as a [`Conflict`] that `vx emit` refuses.
    Conflict {
        /// What the first site binds it as.
        kept: BindKind,
        /// What a later site binds it as.
        other: BindKind,
    },

    /// The value of a bound variant: an enum named `name` whose variants are the values the drawn
    /// rows show, one per look, so the core cannot pick a value nothing can draw.
    Choice {
        /// The enum's name — the struct holding the field, then the field.
        name: String,
        /// Its Rust variants, in look order; the first is the default.
        variants: Vec<String>,
        /// The values they spell, in the same order, for the doc comments.
        values: Vec<String>,
    },
}

/// Derives the contract from a parsed composition.
#[must_use]
pub fn derive(surface: &Surface) -> Contract {
    let mut contract = Contract::default();

    // Every shell sends this before it renders anything, and the core has to have somewhere to
    // initialise from. It is the one event no design file will ever draw.
    contract.events.insert("Start".to_owned(), Vec::new());

    // Up front, because a navigation argument resolves against the screens the app ends up with
    // and not the ones seen so far: the today screen's tab bar names `Settings` long before the
    // settings screen is reached.
    let routes: BTreeSet<String> =
        surface.screens.iter().map(|screen| pascal(&screen.slug)).collect();

    for screen in &surface.screens {
        contract.screens.push(screen_contract(screen, &routes, &mut contract.events));
    }

    // Once, at the end, because an argument resolves against every field the screen ended up with
    // — a footer button may name a value the body binds below it, and the first pass had not seen
    // it yet.
    for (screen, derived) in surface.screens.iter().zip(&contract.screens) {
        resolve(screen, derived, &mut contract.events, &mut contract.gaps);
        clashes(screen, &contract.events, &mut contract.clashes);
    }

    if let Some(menu) = &surface.footer_menu {
        menu_event(menu, &mut contract);
    }

    for screen in &contract.screens {
        conflicts(&screen.slug, &screen.view, &screen.fields, &mut contract.conflicts);
        for element in &screen.elements {
            conflicts(&screen.slug, &element.name, &element.fields, &mut contract.conflicts);
        }
    }

    contract
}

/// Every [`Field::Conflict`] in a struct's fields, nested ones included, as a [`Conflict`].
fn conflicts(screen: &str, owner: &str, fields: &Fields, into: &mut Vec<Conflict>) {
    for (path, field) in fields {
        match field {
            Field::Conflict { kept, other } => into.push(Conflict {
                screen: screen.to_owned(),
                owner: owner.to_owned(),
                field: path.clone(),
                kept: *kept,
                other: *other,
            }),
            Field::Nested(inner) => conflicts(screen, owner, inner, into),
            Field::Value(_) | Field::Repeated(_) | Field::Choice { .. } => {}
        }
    }
}

/// Declares the event the footer menu's tabs raise, or reports the screen site it disagrees with.
///
/// The menu is drawn by the root router rather than by a screen, so no screen walk sees its taps.
/// Its event always carries exactly one argument, the tapped tab's route, which makes it a
/// `Route`: a screen that raises the same event with another shape — no argument, two, or a value
/// it reads off a field — is a site the router's call could not compile against, and is reported
/// as a [`Clash`] for the reason every other disagreement is.
fn menu_event(menu: &FooterMenu, contract: &mut Contract) {
    let Some(event) = menu.tabs.first().and_then(|tab| menu.raises(tab)) else { return };
    let route = Argument {
        name: "route".to_owned(),
        carries: Carried::Route,
    };

    let Some(declared) = contract.events.get(&event.name) else {
        contract.events.insert(event.name, vec![route]);
        return;
    };
    if declared.len() == 1 && declared[0].carries == Carried::Route {
        return;
    }

    contract.clashes.push(Clash {
        screen: "footer_menu".to_owned(),
        event: event.name.clone(),
        declared: spelled(&event.name, declared.iter().map(|argument| argument.name.as_str())),
        raised: format!("{}(<tab route>)", event.name),
    });
}

fn screen_contract(
    screen: &Screen, routes: &BTreeSet<String>, events: &mut BTreeMap<String, Vec<Argument>>,
) -> ScreenContract {
    let module = snake(&screen.slug);
    let view = format!("{}View", pascal(&screen.slug));

    let mut fields = Fields::new();
    let mut elements = Vec::new();
    let mut nested = Vec::new();

    for body in screen.bodies() {
        match body.collection() {
            Some(collection) => {
                repeated(screen, collection, &mut fields, &mut elements, events);
            }
            // Content outside a collection reads the screen's own fields.
            None => {
                for node in body.nodes() {
                    walk(node, &mut fields, events);
                    nested_in(node, &mut nested);
                }
            }
        }
    }

    if let Some(header) = &screen.header {
        // A bound title reads its field off the screen's view model exactly as a bound `text` does.
        if let Some(path) = header.bind() {
            insert(&mut fields, path, BindKind::Text);
        }
        for node in header.nodes() {
            walk(node, &mut fields, events);
            nested_in(node, &mut nested);
        }
    }
    for node in &screen.footer {
        walk(node, &mut fields, events);
        nested_in(node, &mut nested);
    }
    if let Some(fab) = &screen.fab {
        leaf(fab, &mut fields, events);
    }
    for overlay in &screen.overlays {
        // The flag that presents it. The shell reads this to decide whether the sheet is up, and
        // dismissing it is an event rather than a write, because the core owns the state.
        insert(&mut fields, &overlay.key, BindKind::Flag);
        events.entry(format!("Dismiss{}", pascal(&overlay.key))).or_default();
        for node in &overlay.content {
            walk(node, &mut fields, events);
            nested_in(node, &mut nested);
        }
    }

    // A collection nested in content is the same `Vec<Row>` a body list is: its `each` is a field on
    // the screen's view model, and its rows are an element struct of their own.
    for collection in nested {
        repeated(screen, collection, &mut fields, &mut elements, events);
    }

    for state in &screen.states {
        // `when:` is a condition on the view model. A bare path is a field; an expression is
        // something the core computes, and naming a field for it would be inventing one. An
        // emptiness test is neither: the collection the body already declares answers it.
        if let Condition::Flag(path) = state.condition() {
            insert(&mut fields, &path, BindKind::Flag);
        }
    }

    ScreenContract {
        slug: screen.slug.clone(),
        view,
        module,
        fields,
        elements,
        routes: routes.clone(),
    }
}

/// Adds the field a collection iterates and the element structs it iterates over.
fn repeated(
    screen: &Screen, collection: &Collection, fields: &mut Fields, elements: &mut Vec<Element>,
    events: &mut BTreeMap<String, Vec<Argument>>,
) {
    let derived = elements_of(screen, collection, events);
    if let Some(outer) = derived.first() {
        fields.insert(collection.each.clone(), Field::Repeated(outer.name.clone()));
    }
    for element in derived {
        if !elements.iter().any(|existing: &Element| existing.name == element.name) {
            elements.push(element);
        }
    }
}

/// Every collection nested in a subtree; see [`Node::collections`].
fn nested_in<'a>(node: &'a Node, into: &mut Vec<&'a Collection>) {
    into.extend(node.collections());
}

/// The element structs a collection iterates, outermost first.
///
/// One for a flat list, two for a sectioned one — and the outer struct is what `each` holds, which
/// is why the caller reads the type off the front rather than assuming the row.
fn elements_of(
    screen: &Screen, collection: &Collection, events: &mut BTreeMap<String, Vec<Argument>>,
) -> Vec<Element> {
    let rows = element(screen, collection, events);

    let Some(section) = &collection.section else {
        return vec![rows];
    };

    let mut fields = Fields::new();
    insert(&mut fields, ROW_IDENTITY, BindKind::Text);
    insert(&mut fields, &section.title, BindKind::Text);
    fields.insert(section.items.clone(), Field::Repeated(rows.name.clone()));

    let sections = Element {
        name: format!("{}{}", pascal(&screen.slug), pascal(&singular(&collection.each))),
        fields,
    };
    vec![sections, rows]
}

/// The element struct a collection's rows are.
fn element(
    screen: &Screen, collection: &Collection, events: &mut BTreeMap<String, Vec<Argument>>,
) -> Element {
    // In a sectioned list `each` names the sections, so the rows take their name from the field a
    // section holds them in — otherwise both structs would be called the same thing.
    let source = collection.section.as_ref().map_or(&collection.each, |section| &section.items);
    let name = format!("{}{}", pascal(&screen.slug), pascal(&singular(source)));

    let mut fields = Fields::new();
    insert(&mut fields, ROW_IDENTITY, BindKind::Text);
    for node in &collection.item {
        walk(node, &mut fields, events);
    }

    // Row events name row fields. Resolving them here rather than at the top means `EditTask(id)`
    // gets the row's `id` type rather than a guess.
    for node in &collection.item {
        retype(node, &fields, events);
    }

    name_choices(&name, &mut fields);
    Element { name, fields }
}

/// Names each bound variant's enum for the struct holding it and the field: `TodayHour` and
/// `state` give `TodayHourState`, which is unique across the contract because the struct is.
fn name_choices(owner: &str, fields: &mut Fields) {
    for (path, field) in fields.iter_mut() {
        if let Field::Choice { name, .. } = field {
            *name = choice_name(owner, path);
        }
    }
}

/// The enum a bound variant's field holds, as every shell names it.
#[must_use]
pub fn choice_name(owner: &str, field: &str) -> String {
    format!("{owner}{}", pascal(field))
}

/// Adds every field and event a subtree needs.
fn walk(node: &Node, fields: &mut Fields, events: &mut BTreeMap<String, Vec<Argument>>) {
    match node {
        Node::Item(item) => leaf(item, fields, events),
        Node::Group(group) => {
            if let Some(interaction) = &group.interaction {
                for event in interaction.events() {
                    record(event, fields, events);
                }
            }
            // A component placement whose instance binds a TEXT property reads that field off the
            // view model in scope, exactly as a bound `text` does — the component itself has none.
            // One shown through a `format:` reads a typed value instead, formatted at the call site.
            for (name, arg) in &group.args {
                if let Arg::Bind(path) = arg {
                    let kind = group
                        .formats
                        .get(name)
                        .map_or(BindKind::Text, |format| BindKind::of_format(*format));
                    insert(fields, path, kind);
                }
            }
            // A row reading its variant from a field: the field holds one of the drawn values.
            // Named once the struct holding it is, in `name_choices`.
            if let Some(bound) = &group.bound {
                fields.entry(bound.field.clone()).or_insert_with(|| Field::Choice {
                    name: String::new(),
                    variants: bound.variants(),
                    values: bound.looks.iter().map(|(value, _)| value.clone()).collect(),
                });
            }
            for child in &group.items {
                walk(child, fields, events);
            }
        }
        // Its rows read their element rather than this scope; `nested_in` finds it separately.
        Node::Collection(_) => {}
    }
}

fn leaf(item: &Item, fields: &mut Fields, events: &mut BTreeMap<String, Vec<Argument>>) {
    if let Some(path) = &item.bind {
        insert(fields, path, BindKind::of_item(item));
    }
    if let Some(event) = &item.event {
        record(event, fields, events);
    }

    // A custom element declares its contract rather than drawing it: `binds:` names the view-model
    // fields it reads and `raises:` the events it sends. They are recorded exactly like a control's
    // — so the `Event` enum and the view-model struct are unchanged in kind — but a custom element
    // produces its own values, so its event arguments are not resolved against the screen the way
    // `resolve` treats a drawn control's. They keep the `String` `record` gives an argument that
    // names no field, which is the seam type `vx custom` implements against.
    if item.custom {
        for path in &item.binds {
            insert(fields, path, BindKind::Text);
        }
        for event in &item.raises {
            record(event, fields, events);
        }
    }
}

/// Resolves every argument of every event a screen raises, once all its fields are known.
///
/// Two outputs: an argument the control itself produces gets that control's type rather than the
/// `String` [`lookup`] falls back to, and an argument nothing names becomes a [`Gap`].
fn resolve(
    screen: &Screen, derived: &ScreenContract, events: &mut BTreeMap<String, Vec<Argument>>,
    gaps: &mut Vec<Gap>,
) {
    for (event, bind, produces, within_element) in sites(screen) {
        // An unfilled `TODO(event)` parses as `TODO` carrying `event`. It declares nothing (see
        // [`record`]), so its "argument" is the marker's own spelling and has nothing to resolve.
        if event.is_hole() {
            continue;
        }
        for argument in &event.args {
            match derived.source(argument, bind.as_deref(), produces, within_element) {
                Source::Produced(kind) => {
                    retype_argument(events, event, argument, Carried::Value(kind));
                }
                Source::Route => retype_argument(events, event, argument, Carried::Route),
                Source::Element | Source::ViewModel => {}
                Source::Unresolved => gaps.push(Gap {
                    screen: screen.slug.clone(),
                    event: event.spelled(),
                    argument: argument.clone(),
                }),
            }
        }
    }
}

/// Reports every site on a screen that raises an event the contract declared a different shape for.
///
/// Read off the recorded arguments rather than by comparing sites with each other, because the
/// recorded list *is* what the `Event` enum will hold: the first writer won, and a site that
/// disagrees with it is a site the generated shell will not compile.
fn clashes(screen: &Screen, events: &BTreeMap<String, Vec<Argument>>, into: &mut Vec<Clash>) {
    for (event, ..) in sites(screen) {
        let Some(declared) = events.get(&event.name) else { continue };
        if declared.len() == event.args.len() {
            continue;
        }

        into.push(Clash {
            screen: screen.slug.clone(),
            event: event.name.clone(),
            declared: spelled(&event.name, declared.iter().map(|argument| argument.name.as_str())),
            raised: event.spelled(),
        });
    }
}

/// An event as a composition would write it: the name, and its arguments in parentheses.
fn spelled<'a>(name: &str, arguments: impl Iterator<Item = &'a str>) -> String {
    let arguments: Vec<&str> = arguments.collect();
    if arguments.is_empty() { name.to_owned() } else { format!("{name}({})", arguments.join(", ")) }
}

/// Every event a screen raises, with the control that raises it and whether a row is in scope.
fn sites(screen: &Screen) -> Vec<(&Event, Option<String>, Option<BindKind>, bool)> {
    let mut found = Vec::new();

    for body in screen.bodies() {
        let within_element = body.collection().is_some();
        for node in body.nodes() {
            raised(node, within_element, false, &mut found);
        }
    }
    if let Some(header) = &screen.header {
        for node in header.nodes() {
            raised(node, false, false, &mut found);
        }
    }
    for node in &screen.footer {
        raised(node, false, false, &mut found);
    }
    if let Some(fab) = &screen.fab
        && let Some(event) = &fab.event
    {
        found.push((event, fab.bind.clone(), fab.produces(), false));
    }
    for overlay in &screen.overlays {
        for node in &overlay.content {
            raised(node, false, false, &mut found);
        }
    }

    found
}

/// `hoisted` marks a subtree that was lifted into a shared component, whose events reach the
/// screen as closures taking no argument. The screen fills those arguments in from its own scope,
/// so a value the control produces has nowhere to travel — the component cannot hand it up and the
/// screen cannot see it. Reported as unresolved rather than emitted, because the alternative is a
/// read against a component that has no such parameter.
fn raised<'a>(
    node: &'a Node, within_element: bool, hoisted: bool,
    into: &mut Vec<(&'a Event, Option<String>, Option<BindKind>, bool)>,
) {
    match node {
        Node::Item(item) => {
            if let Some(event) = &item.event {
                let produces = if hoisted { None } else { item.produces() };
                into.push((event, item.bind.clone(), produces, within_element));
            }
        }
        Node::Group(group) => {
            if let Some(interaction) = &group.interaction {
                // A container raises its events by being tapped, so it produces nothing — even
                // when a control it names does. `controls:` refers to an item in the subtree, and
                // that item is visited on its own account below.
                for event in interaction.events() {
                    into.push((event, None, None, within_element));
                }
            }
            for child in &group.items {
                raised(child, within_element, hoisted || group.component.is_some(), into);
            }
        }
        // A nested collection's rows raise their events with a row in scope, as a body list's do.
        Node::Collection(collection) => {
            for child in &collection.item {
                raised(child, true, hoisted, into);
            }
        }
    }
}

/// Improves a recorded argument's type, where resolution knows better than the field lookup did.
///
/// Two callers and one rule: the control that produced the value knows its kind, and a name that
/// turned out to be a screen is a route. Both only ever replace the text [`lookup`] falls back to
/// for a name no field answered, so a resolved type is never overwritten by a later site.
fn retype_argument(
    events: &mut BTreeMap<String, Vec<Argument>>, event: &Event, argument: &str, carries: Carried,
) {
    let Some(recorded) = events.get_mut(&event.name) else { return };
    if recorded.len() != event.args.len() {
        return;
    }

    for (position, name) in event.args.iter().enumerate() {
        if name == argument && recorded[position].carries == Carried::Value(BindKind::Text) {
            recorded[position].carries = carries;
        }
    }
}

/// Records an event, giving each argument the type of the field it names.
///
/// An unfilled `TODO(event)` records nothing. `vx emit` refuses a spec with surviving holes, so
/// this is only reachable under `--allow-holes` — but the variant it used to add was `Event::TODO`
/// taking a `String` called `event`, which is a name the design file never said and the shells
/// dispatch nowhere.
fn record(event: &Event, fields: &Fields, events: &mut BTreeMap<String, Vec<Argument>>) {
    if event.is_hole() {
        return;
    }

    let arguments: Vec<Argument> = event
        .args
        .iter()
        .map(|name| Argument {
            name: name.clone(),
            carries: Carried::Value(lookup(fields, name)),
        })
        .collect();

    // First writer wins for the argument list, but a later one that resolved a type the first
    // could not is an improvement rather than a conflict.
    let entry = events.entry(event.name.clone()).or_insert_with(|| arguments.clone());
    if entry.len() == arguments.len() {
        let default = Carried::Value(BindKind::Text);
        for (existing, found) in entry.iter_mut().zip(arguments) {
            if existing.carries == default && found.carries != default {
                *existing = found;
            }
        }
    }
}

/// Re-resolves a row's events once every row field is known.
///
/// A row's `event: Toggle(completed)` may be written above the `checkbox` that declares
/// `completed`, so the first pass sees a name with no field behind it.
fn retype(node: &Node, fields: &Fields, events: &mut BTreeMap<String, Vec<Argument>>) {
    match node {
        Node::Item(item) => {
            if let Some(event) = &item.event {
                record(event, fields, events);
            }
        }
        Node::Group(group) => {
            if let Some(interaction) = &group.interaction {
                for event in interaction.events() {
                    record(event, fields, events);
                }
            }
            for child in &group.items {
                retype(child, fields, events);
            }
        }
        // A row template holds no collection of its own; `vx spec` refuses one.
        Node::Collection(_) => {}
    }
}

/// What the field a name refers to holds, or text when it refers to nothing.
fn lookup(fields: &Fields, name: &str) -> BindKind {
    match fields.get(name) {
        Some(Field::Value(kind)) => *kind,
        _ => BindKind::Text,
    }
}

/// Adds a field for a bind path.
///
/// A dotted path is a nested object: `author.name` is a `name` on an `author`. Kept nested rather
/// than flattened to `author_name`, because that is what the generated Swift reads.
///
/// The first site to bind a name decides its type. A later site binding it as another kind is a
/// [`Conflict`] where either kind is a typed display value, and left unsaid otherwise, which is
/// what every composition without a `format:` has always had.
fn insert(fields: &mut Fields, path: &str, kind: BindKind) {
    let Some((head, rest)) = path.split_once('.') else {
        let field = fields.entry(path.to_owned()).or_insert(Field::Value(kind));
        if let Field::Value(kept) = *field
            && kept != kind
            && (matches!(kept, BindKind::Displayed(_)) || matches!(kind, BindKind::Displayed(_)))
        {
            *field = Field::Conflict { kept, other: kind };
        }
        return;
    };

    let nested = fields.entry(head.to_owned()).or_insert_with(|| Field::Nested(Fields::new()));
    if let Field::Nested(inner) = nested {
        insert(inner, rest, kind);
    }
}

/// `tasks` becomes `task`, so a collection's element type is not `TaskListTasks`.
///
/// The three suffixes a plural field name actually uses, and no attempt at English beyond them:
/// a name with no plural ending keeps it, so `data` stays `TaskListData`. Irregulars come out
/// odd — `series` gives `Serie` — which is a name rather than a defect, and a person who minds
/// can rename the field.
fn singular(plural: &str) -> String {
    for (suffix, replacement) in [("ies", "y"), ("ses", "s"), ("s", "")] {
        if let Some(stem) = plural.strip_suffix(suffix)
            && !stem.is_empty()
        {
            return format!("{stem}{replacement}");
        }
    }
    plural.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;

    fn contract(document: &serde_json::Value) -> Contract {
        derive(&model::parse(document).expect("the composition parses"))
    }

    #[test]
    fn a_list_screen_gets_a_view_a_row_and_the_events_the_row_raises() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "task-list": {
                    "name": "TaskListScreen",
                    "body": { "list": {
                        "each": "tasks",
                        "item": [{ "group": {
                            "interaction": { "archetype": "tap-target", "event": "EditTask(id)" },
                            "items": [
                                { "text": { "bind": "title" } },
                                { "checkbox": { "bind": "completed", "event": "Toggle(id)" } }
                            ]
                        }}]
                    }}
                }
            }
        }));

        let screen = &contract.screens[0];
        assert_eq!(screen.view, "TaskListView");
        assert_eq!(screen.module, "task_list");
        assert_eq!(screen.fields.get("tasks"), Some(&Field::Repeated("TaskListTask".to_owned())));

        let row = &screen.elements[0];
        assert_eq!(row.fields.get("id"), Some(&Field::Value(BindKind::Text)));
        assert_eq!(row.fields.get("title"), Some(&Field::Value(BindKind::Text)));
        assert_eq!(row.fields.get("completed"), Some(&Field::Value(BindKind::Flag)));

        assert!(contract.events.contains_key("EditTask"));
        assert!(contract.events.contains_key("Toggle"));
        assert!(contract.events.contains_key("Start"), "every shell sends this one");
    }

    /// A sectioned list is two structs, and `each` holds the outer one.
    ///
    /// The row type is named for the field a section holds its rows in rather than for `each`,
    /// which is the whole reason the naming is not shared with the flat case: both structs would
    /// otherwise be called `ScheduledSection`, and the second would silently win.
    #[test]
    fn a_sectioned_list_gets_a_section_struct_holding_the_rows() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "scheduled": {
                    "name": "Scheduled",
                    "body": { "list": {
                        "each": "sections",
                        "section": { "title": "heading", "items": "tasks" },
                        "item": [{ "text": { "bind": "title" } }]
                    }}
                }
            }
        }));

        let screen = &contract.screens[0];
        assert_eq!(
            screen.fields.get("sections"),
            Some(&Field::Repeated("ScheduledSection".to_owned())),
            "`each` holds the sections, not the rows"
        );

        let section = &screen.elements[0];
        assert_eq!(section.name, "ScheduledSection");
        assert_eq!(section.fields.get("heading"), Some(&Field::Value(BindKind::Text)));
        assert_eq!(section.fields.get("tasks"), Some(&Field::Repeated("ScheduledTask".to_owned())));

        let row = &screen.elements[1];
        assert_eq!(row.name, "ScheduledTask");
        assert_eq!(row.fields.get("title"), Some(&Field::Value(BindKind::Text)));
    }

    #[test]
    fn an_event_argument_takes_the_type_of_the_row_field_it_names() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": { "name": "A", "body": { "list": {
                    "each": "rows",
                    "item": [{ "group": {
                        "interaction": { "archetype": "tap-target", "event": "SetDone(done)" },
                        "items": [{ "switch": { "bind": "done" } }]
                    }}]
                }}}
            }
        }));

        assert_eq!(
            contract.events["SetDone"],
            [Argument {
                name: "done".to_owned(),
                carries: Carried::Value(BindKind::Flag)
            }]
        );
    }

    #[test]
    fn an_argument_naming_nothing_the_row_declares_is_a_string() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": { "name": "A", "body": { "list": {
                    "each": "rows",
                    "item": [{ "text": { "bind": "title", "event": "Open(slug)" } }]
                }}}
            }
        }));

        assert_eq!(contract.events["Open"][0].rust(), "String");
    }

    #[test]
    fn a_drawn_state_becomes_a_flag_only_when_its_condition_is_a_path() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [{ "text": { "content": "hi" } }],
                    "states": {
                        "empty": { "when": "tasks_are_empty", "replaces": "body",
                                   "body": [{ "text": { "content": "none" } }] },
                        "loading": { "when": "tasks.len() == 0", "replaces": "body",
                                     "body": [{ "text": { "content": "…" } }] }
                    }
                }
            }
        }));

        let fields = &contract.screens[0].fields;
        assert_eq!(fields.get("tasks_are_empty"), Some(&Field::Value(BindKind::Flag)));
        assert_eq!(fields.len(), 1, "an expression is the core's business, not a field");
    }

    #[test]
    fn an_overlay_gets_a_flag_and_a_way_to_close_it() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [{ "text": { "content": "hi" } }],
                    "overlays": {
                        "confirm-delete": { "kind": "dialog", "title": "Sure?",
                                            "content": [{ "text": { "content": "yes" } }] }
                    }
                }
            }
        }));

        assert_eq!(
            contract.screens[0].fields.get("confirm-delete"),
            Some(&Field::Value(BindKind::Flag))
        );
        assert!(contract.events.contains_key("DismissConfirmDelete"));
    }

    #[test]
    fn a_dotted_bind_path_nests_rather_than_flattens() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": { "name": "A", "body": [{ "text": { "bind": "author.name" } }] }
            }
        }));

        let Some(Field::Nested(author)) = contract.screens[0].fields.get("author") else {
            panic!("expected a nested field");
        };
        assert_eq!(author.get("name"), Some(&Field::Value(BindKind::Text)));
    }

    /// A component placement whose instance binds a TEXT property reads that field off the screen's
    /// view model, so the screen has to declare it. The component itself cannot: it has no view
    /// model, only the arguments each call site passes. A literal argument is copy and adds nothing.
    #[test]
    fn a_bound_component_argument_is_a_screen_field() {
        let tile = |args: serde_json::Value| {
            serde_json::json!({ "group": {
                "component": "metric-tile",
                "args": args,
                "items": [
                    { "text": { "arg": "label" } },
                    { "text": { "arg": "value" } }
                ]
            } })
        };
        let contract = contract(&serde_json::json!({
            "screens": { "today": { "name": "Today", "body": [
                tile(serde_json::json!({ "label": "HUMIDITY", "value": { "bind": "humidity" } })),
                tile(serde_json::json!({ "label": "WIND", "value": "18 km/h" }))
            ] } }
        }));

        let fields = &contract.screens[0].fields;
        assert_eq!(fields.get("humidity"), Some(&Field::Value(BindKind::Text)));
        assert_eq!(fields.len(), 1, "literal arguments are copy, not fields: {fields:?}");
    }

    /// A bound header title reads its field off the screen's view model, so the screen declares it
    /// — and a field bound by a control in the bar's `below` rows is declared too. A literal title
    /// is copy and adds nothing.
    #[test]
    fn a_bound_header_title_and_what_is_below_the_title_row_are_screen_fields() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "city-detail": { "name": "CityDetail", "header": {
                    "title": { "bind": "city_name" },
                    "below": [{ "search-field": { "bind": "query", "event": "QueryChanged(query)" } }]
                }, "body": [] },
                "settings": { "name": "Settings", "header": { "title": "Settings" }, "body": [] }
            }
        }));

        let fields = |slug: &str| {
            &contract.screens.iter().find(|screen| screen.slug == slug).expect(slug).fields
        };
        assert_eq!(fields("city-detail").get("city_name"), Some(&Field::Value(BindKind::Text)));
        assert!(fields("city-detail").contains_key("query"), "{:?}", fields("city-detail"));
        assert!(fields("settings").is_empty(), "a literal title is copy: {:?}", fields("settings"));
    }

    #[test]
    fn plural_collections_get_singular_element_names() {
        assert_eq!(singular("tasks"), "task");
        assert_eq!(singular("entries"), "entry");
        assert_eq!(singular("addresses"), "address");
        assert_eq!(singular("data"), "data", "a name with no plural ending keeps it");
        assert_eq!(singular("s"), "s", "and stripping one down to nothing is not an improvement");
    }

    /// The defect this resolution exists for: a control with no `bind:` raising an event that
    /// carries a name no field answers to. It used to become a read off a view model that had no
    /// such member, in a file nobody is allowed to edit.
    #[test]
    fn a_control_with_no_binding_types_its_event_from_the_value_the_control_produces() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "new-list": {
                    "name": "NewList",
                    "body": [
                        { "text-field": { "placeholder": "List name",
                                          "event": "ListNameChanged(value)" } },
                        { "slider": { "event": "WidthChanged(value)" } },
                        { "date-picker": { "event": "DateSelected(date)" } }
                    ]
                }
            }
        }));

        assert!(contract.gaps.is_empty(), "{:?}", contract.gaps);
        assert_eq!(contract.events["ListNameChanged"][0].rust(), "String");
        assert_eq!(contract.events["WidthChanged"][0].rust(), "f64");
        assert_eq!(
            contract.events["DateSelected"][0].rust(),
            "i64",
            "a moment crosses the boundary as epoch milliseconds, bound or produced"
        );
    }

    /// The bound case is the one that cannot be got from the view model *even though* the field
    /// exists: the view model holds what the core last pushed, so a checkbox reporting `completed`
    /// back would send the state it was switched away from.
    #[test]
    fn a_bound_control_reporting_its_own_value_reports_the_new_one() {
        let surface = model::parse(&serde_json::json!({
            "screens": {
                "task-detail": {
                    "name": "Task",
                    "body": [{ "checkbox": { "bind": "draft.complete",
                                             "event": "CompleteChanged(complete)" } }]
                }
            }
        }))
        .expect("the composition parses");

        let contract = derive(&surface);
        let screen = &contract.screens[0];

        assert_eq!(
            screen.source("complete", Some("draft.complete"), Some(BindKind::Flag), false),
            Source::Produced(BindKind::Flag),
        );
        assert_eq!(contract.events["CompleteChanged"][0].rust(), "bool");
    }

    /// A row field named from a row is unchanged, and named from anywhere else is a gap: the
    /// dialog is emitted outside every `ForEach`, so there is no row for `id` to come off.
    #[test]
    fn a_row_field_named_where_no_row_is_in_scope_is_a_gap_rather_than_a_read() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "task-list": {
                    "name": "TaskList",
                    "body": { "list": {
                        "each": "tasks",
                        "item": [{ "group": {
                            "interaction": { "archetype": "tap-target", "event": "EditTask(id)" },
                            "items": [{ "text": { "bind": "title" } }]
                        }}]
                    }},
                    "overlays": { "confirm-delete": {
                        "kind": "dialog",
                        "title": "Delete?",
                        "content": [{ "button": { "content": "Delete",
                                                  "event": "DeleteTask(id)" } }]
                    }}
                }
            }
        }));

        let [gap] = contract.gaps.as_slice() else {
            panic!("one gap, got {:?}", contract.gaps);
        };
        assert_eq!(gap.screen, "task-list");
        assert_eq!(gap.event, "DeleteTask(id)");
        assert_eq!(gap.argument, "id");
        assert!(gap.to_string().contains("nothing on the screen is called"), "{gap}");
    }

    /// A button produces nothing, so an argument it names and the screen does not is unanswerable
    /// — and saying so here is the whole point: the alternative surfaces in Xcode.
    /// A component's events reach the screen as closures taking nothing, so the value stays inside
    /// the component and the screen fills the argument from a scope that has no answer.
    #[test]
    fn a_control_lifted_into_a_component_cannot_hand_its_value_up_and_says_so() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "new-list": {
                    "name": "NewList",
                    "body": [{ "group": {
                        "component": "name-field",
                        "items": [{ "text-field": { "placeholder": "List name",
                                                    "event": "ListNameChanged(value)" } }]
                    }}]
                }
            }
        }));

        assert_eq!(contract.gaps.len(), 1, "{:?}", contract.gaps);
        assert_eq!(contract.gaps[0].event, "ListNameChanged(value)");
    }

    /// The composition a tab bar produces, and the case the resolver had no scope for: `Cities`
    /// is not a field on the today screen and never could be, because it is a destination. It
    /// used to be reported as a gap, which refused the whole run over the one argument whose
    /// meaning the contract generates four hundred lines above the event that carries it.
    #[test]
    fn an_argument_naming_a_screen_is_the_route_it_is_spelled_as() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "today": {
                    "name": "Today",
                    "body": [{ "text": { "content": "18°" } }],
                    "footer": [
                        { "icon-button": { "icon": "cities", "event": "Navigate(Cities)" } },
                        { "icon-button": { "icon": "add", "event": "Navigate(AddCity)" } }
                    ]
                },
                "cities": { "name": "Cities", "body": [{ "text": { "content": "saved" } }] },
                "add-city": { "name": "AddCity", "body": [{ "text": { "content": "search" } }] }
            }
        }));

        assert!(contract.gaps.is_empty(), "{:?}", contract.gaps);
        assert_eq!(contract.events["Navigate"][0].carries, Carried::Route);
        assert_eq!(
            contract.events["Navigate"][0].rust(),
            "Route",
            "one variant however many destinations name it"
        );
    }

    /// A kebab slug becomes one Pascal variant, and that spelling is what the annotation writes.
    /// Were the two derived differently, `Navigate(AddCity)` would resolve against nothing and the
    /// failure would read as a missing screen.
    #[test]
    fn a_route_argument_is_the_pascal_of_the_slug_it_names() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "city-detail": {
                    "name": "CityDetail",
                    "body": [{ "button": { "content": "Back", "event": "Navigate(Cities)" } }]
                },
                "cities": { "name": "Cities", "body": [{ "text": { "content": "saved" } }] }
            }
        }));

        let screen = &contract.screens[0];
        assert!(screen.routes.contains("CityDetail"), "{:?}", screen.routes);
        assert!(screen.routes.contains("Cities"), "{:?}", screen.routes);
        assert_eq!(screen.source("Cities", None, None, false), Source::Route);
        assert_eq!(
            screen.source("Melbourne", None, None, false),
            Source::Unresolved,
            "a name that is no screen is still a gap"
        );
    }

    /// The field wins. A route is read after both lookups so that a screen binding a value whose
    /// name collides with a destination still reads its own field rather than navigating.
    #[test]
    fn a_bound_field_outranks_a_route_of_the_same_name() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "today": {
                    "name": "Today",
                    "body": [{ "text": { "bind": "Cities", "event": "Pick(Cities)" } }]
                },
                "cities": { "name": "Cities", "body": [{ "text": { "content": "saved" } }] }
            }
        }));

        assert_eq!(contract.screens[0].source("Cities", None, None, false), Source::ViewModel);
        assert_eq!(contract.events["Pick"][0].rust(), "String");
    }

    /// The defect a gap fix can walk straight into: dropping an argument on one of two sites that
    /// name the same event leaves a variant the other site cannot call. It compiled here and
    /// failed in Kotlin, on a file the operator may not edit.
    #[test]
    fn one_event_raised_with_two_argument_lists_is_refused() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "cities": {
                    "name": "Cities",
                    "body": { "list": {
                        "each": "cities",
                        "item": [{ "text": { "bind": "city", "event": "CityDeleted(id)" } }]
                    }}
                },
                "city-detail": {
                    "name": "CityDetail",
                    "body": [{ "button": { "content": "Delete", "event": "CityDeleted" } }]
                }
            }
        }));

        let [clash] = contract.clashes.as_slice() else {
            panic!("one clash, got {:?}", contract.clashes);
        };
        assert_eq!(clash.screen, "city-detail");
        assert_eq!(clash.declared, "CityDeleted(id)");
        assert_eq!(clash.raised, "CityDeleted");
        assert!(clash.to_string().contains("give the two a name each"), "{clash}");
    }

    /// The case that must not be caught. Every destination names the one `Navigate`, and a variant
    /// is shaped by how many arguments it takes rather than by what they are called.
    #[test]
    fn two_sites_naming_different_values_of_the_same_arity_agree() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "today": {
                    "name": "Today",
                    "body": [{ "button": { "content": "Cities", "event": "Navigate(Cities)" } }]
                },
                "cities": {
                    "name": "Cities",
                    "body": [{ "button": { "content": "Today", "event": "Navigate(Today)" } }]
                }
            }
        }));

        assert!(contract.clashes.is_empty(), "{:?}", contract.clashes);
        assert!(contract.gaps.is_empty(), "{:?}", contract.gaps);
    }

    #[test]
    fn an_argument_naming_nothing_on_a_control_that_produces_nothing_is_a_gap() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "new-list": {
                    "name": "NewList",
                    "body": [{ "button": { "content": "Save", "event": "SaveList(name)" } }]
                }
            }
        }));

        assert_eq!(contract.gaps.len(), 1, "{:?}", contract.gaps);
        assert_eq!(contract.gaps[0].argument, "name");
    }
}
