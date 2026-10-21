//! `composition.json` to Vue 3 single-file components.
//!
//! Draws everything the composition states, including a size's `min_*` / `max_*` bounds, a
//! child's `align_self`, a stack child's `position` (a layer anchored to one of nine points and
//! moved by `tokens.offset`), and the `x-web` facts a web capture keeps beside the core vocabulary
//! (margins, verbatim CSS, inline SVG, link targets, the media a button carries). A fact the
//! composition carries and this emitter cannot draw is written into the template as a comment
//! rather than dropped.
//!
//! The output is a standalone Vite app under `vue/`: one `<Screen>Screen.vue` per screen, an
//! `App.vue` that routes between them, `ui-strings.ts` from the copy contract, `contract.ts` with
//! the event union, `assets.ts` mapping asset names to their served paths, and the build files.
//!
//! An event hole (`TODO(event)`) raises nothing and is not given a name: the element carries
//! `data-vx-hole="event"` and a comment, so the template still says where a handler is missing.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde_json::Value;

use crate::contract::{self, Carried};
use crate::copy::Contract;
use crate::model::{
    BindKind, Body, Dimension, Direction, Event, Group, Item, Layer, Node, Padding, Screen, Size,
    Style, Surface,
};
use crate::naming::{pascal, scalar};
use crate::{GeneratedFile, HEADER};

/// The directory every generated file lives under, relative to the app.
pub const ROOT: &str = "vue";

/// The directory the views are written to, which [`crate::write()`] may prune.
pub const VIEWS: &str = "vue/src/views";

/// The directory the asset files are served from, relative to the app.
pub const PUBLIC: &str = "vue/public";

/// CSS keys the `position` of a layer already states, dropped from its verbatim `x-web.css` so the
/// two never disagree.
const POSITIONAL: &[&str] = &["position", "top", "right", "bottom", "left", "z-index", "inset"];

/// Layout-container keys the emitter derives from the core vocabulary; a verbatim `x-web.css`
/// value for one (say `grid-template-columns` on a box the page laid out as a block, where it was
/// inert) would change what the core vocabulary states.
const CORE_OWNED: &[&str] = &[
    "display",
    "flex-direction",
    "grid-template-columns",
    "grid-template-rows",
    "grid-template-areas",
    "grid-auto-flow",
];

/// Every file the Vue app needs: views, contract, copy, asset map and the Vite app around them.
///
/// `assets` is each asset's name to the path its web source is served at, relative to `public/`.
/// `inline` is each vector icon asset's name to its SVG markup: an `icon` item naming one is drawn
/// as inline SVG, so `currentColor` in it takes the item's `color`. `glyphs` is each glyph asset
/// (a character in a font, e.g. an icon font) by name: an `icon` item naming one is drawn as that
/// text, its font registered from the asset's served file by `glyphs.ts`.
#[must_use]
pub fn files(
    surface: &Surface, copy: &Contract, contract: &contract::Contract,
    assets: &BTreeMap<String, String>, inline: &BTreeMap<String, String>,
    glyphs: &BTreeMap<String, Glyph>,
) -> Vec<GeneratedFile> {
    let mut files = Vec::new();
    for screen in &surface.screens {
        files.push(GeneratedFile::new(
            format!("{VIEWS}/{}Screen.vue", pascal(&screen.slug)),
            view(screen, copy, inline, glyphs),
        ));
    }
    if !glyphs.is_empty() {
        files
            .push(GeneratedFile::new(format!("{ROOT}/src/glyphs.ts"), glyph_fonts(glyphs, assets)));
    }
    files.push(GeneratedFile::new(format!("{ROOT}/src/App.vue"), app(surface)));
    files.push(GeneratedFile::new(format!("{ROOT}/src/ui-strings.ts"), strings(copy)));
    files.push(GeneratedFile::new(format!("{ROOT}/src/contract.ts"), events(contract)));
    files.push(GeneratedFile::new(format!("{ROOT}/src/assets.ts"), asset_map(assets)));
    files.extend(scaffold());
    files
}

/// A glyph asset: the text drawn and the font family it is drawn in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    /// The character(s).
    pub text: String,
    /// The font family.
    pub family: String,
    /// A numeric weight other than the font's default.
    pub weight: Option<u32>,
}

/// One screen's component, with the glyph assets its icons may name.
fn view(
    screen: &Screen, copy: &Contract, inline: &BTreeMap<String, String>,
    glyphs: &BTreeMap<String, Glyph>,
) -> String {
    let mut out = Out {
        depth: 1,
        inline_svg: inline.clone(),
        glyphs: glyphs.clone(),
        ..Out::default()
    };

    let mut css = Css::default();
    css.set("display", "flex");
    css.set("flex-direction", "column");
    css.set("min-height", "100vh");
    if let Some(justify) = &screen.justify {
        css.set("justify-content", &justify_value(justify));
    }
    css.set(
        "align-items",
        &screen.align.as_deref().map_or_else(|| "flex-start".to_owned(), align_value),
    );
    if let Some(background) = &screen.background {
        css.set("background", &color(background));
    }
    padding(&mut css, screen.padding.as_ref());

    out.open("main", &[("class", "vx-screen".to_owned()), ("style", css.render())]);
    if screen.header.is_some() {
        out.comment("vx: a screen header is not drawn by the Vue shell yet");
    }
    match &screen.body {
        Some(Body::Nodes(nodes) | Body::Form(nodes)) => {
            for node in nodes {
                out.node(node, Parent::Column, copy);
            }
        }
        Some(Body::List(_) | Body::Grid(_)) => {
            out.comment("vx: a list or grid body is not drawn by the Vue shell yet");
        }
        None => {}
    }
    for node in &screen.footer {
        out.node(node, Parent::Column, copy);
    }
    if screen.fab.is_some() || !screen.states.is_empty() || !screen.overlays.is_empty() {
        out.comment("vx: floating buttons, states and overlays are not drawn by the Vue shell yet");
    }
    out.close("main");

    let mut script = String::new();
    if out.uses_strings {
        script.push_str("import { s } from \"../ui-strings\";\n");
    }
    if out.uses_assets {
        script.push_str("import { assets } from \"../assets\";\n");
    }
    if out.uses_glyphs {
        script.push_str("import \"../glyphs\";\n");
    }
    if out.uses_events {
        script.push_str("import type { Event } from \"../contract\";\n\n");
        script.push_str("const emit = defineEmits<{ event: [event: Event] }>();\n");
    }

    format!(
        "<!-- {HEADER} -->\n<script setup lang=\"ts\">\n{script}</script>\n\n<template>\n{}</template>\n",
        out.text
    )
}

/// The container a node is laid out in, which decides what `fill` and `align_self` mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parent {
    /// A horizontal flex container: width is the main axis.
    Row,

    /// A vertical flex container: height is the main axis.
    Column,

    /// A stack: every flow child shares one grid cell.
    Stack,

    /// A row (`true`) or column drawn as grid tracks, because two or more children fill its
    /// main axis: `fill` is then an equal share of the border-box space, which flex cannot give
    /// once the fillers have padding.
    Track(bool),

    /// An inline formatting context (`x-web.flow: inline`): the pieces of a run of text, which
    /// break across lines the way the page broke them rather than as whole flex items.
    Inline,
}

impl Parent {
    const fn of(direction: Direction) -> Self {
        match direction {
            Direction::Row => Self::Row,
            Direction::Column => Self::Column,
            Direction::Stack => Self::Stack,
        }
    }
}

/// An ordered declaration list, where setting a property again replaces it in place.
#[derive(Debug, Default)]
struct Css(Vec<(String, String)>);

impl Css {
    fn set(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.0.iter_mut().find(|(existing, _)| existing == key) {
            value.clone_into(&mut entry.1);
        } else {
            self.0.push((key.to_owned(), value.to_owned()));
        }
    }

    fn has(&self, key: &str) -> bool {
        self.0.iter().any(|(existing, _)| existing == key)
    }

    fn render(&self) -> String {
        self.0.iter().map(|(key, value)| format!("{key}: {value}")).collect::<Vec<_>>().join("; ")
    }
}

/// The template being written.
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts collected during one walk")]
struct Out {
    text: String,
    depth: usize,
    uses_strings: bool,
    uses_assets: bool,
    uses_events: bool,
    uses_glyphs: bool,
    inline: bool,
    /// Vector icon assets drawn inline, by name.
    inline_svg: BTreeMap<String, String>,
    /// Glyph assets drawn as text in their font, by name.
    glyphs: BTreeMap<String, Glyph>,
}

impl Out {
    fn line(&mut self, line: &str) {
        for _ in 0..self.depth {
            self.text.push_str("  ");
        }
        self.text.push_str(line);
        self.text.push('\n');
    }

    fn comment(&mut self, text: &str) {
        self.line(&format!("<!-- {} -->", text.replace("--", "- -")));
    }

    fn tag(attrs: &[(&str, String)]) -> String {
        attrs
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(name, value)| {
                if name.starts_with('@') || name.starts_with(':') {
                    format!(" {name}=\"{value}\"")
                } else {
                    format!(" {name}=\"{}\"", attribute(value))
                }
            })
            .collect()
    }

    fn open(&mut self, element: &str, attrs: &[(&str, String)]) {
        self.line(&format!("<{element}{}>", Self::tag(attrs)));
        self.depth += 1;
    }

    fn close(&mut self, element: &str) {
        self.depth -= 1;
        self.line(&format!("</{element}>"));
    }

    fn leaf(&mut self, element: &str, attrs: &[(&str, String)], inner: &str) {
        self.line(&format!("<{element}{}>{inner}</{element}>", Self::tag(attrs)));
    }

    fn void(&mut self, element: &str, attrs: &[(&str, String)]) {
        self.line(&format!("<{element}{} />", Self::tag(attrs)));
    }

    fn node(&mut self, node: &Node, parent: Parent, copy: &Contract) {
        match node {
            Node::Group(group) => self.group(group, parent, copy),
            Node::Item(item) => self.item(item, parent, copy),
            Node::Collection(_) => {
                self.comment("vx: a collection is not drawn by the Vue shell yet");
            }
        }
    }

    fn group(&mut self, group: &Group, parent: Parent, copy: &Contract) {
        if group.component.is_some() {
            self.comment("vx: a component call is not drawn by the Vue shell yet");
            return;
        }
        let web = group.layout.web.as_ref();
        let mut css = Css::default();
        let fillers = group
            .items
            .iter()
            .filter(|node| node_size(node).along(group.direction) == Some(Dimension::Fill))
            .count();
        let tracked = fillers >= 2 && !group.wrap && group.direction != Direction::Stack;
        let run = group.direction == Direction::Row && web_str(web, "flow") == Some("inline");
        let inner = if run {
            Parent::Inline
        } else if tracked {
            Parent::Track(group.direction == Direction::Row)
        } else {
            Parent::of(group.direction)
        };

        container(&mut css, group, web, run, tracked);
        style(&mut css, &group.style);
        sizing(&mut css, &group.size, parent);
        self_alignment(&mut css, group.layout.align_self.as_deref(), &group.size, parent);
        pinned(&mut css, group.layout.pinned.as_deref());
        // A run that does not flow onto further lines keeps its pieces on one.
        if run && !group.flow_wrap && !web_flag(web, "inline_wrap") {
            css.set("white-space", "nowrap");
        }
        if parent == Parent::Inline {
            let display = match css_get(&css, "display") {
                Some("flex") => "inline-flex",
                Some("grid") => "inline-grid",
                _ => "inline",
            };
            css.set("display", display);
        }
        web_facts(&mut css, web, group.layout.position.is_some());

        let mut attrs = vec![("style", css.render())];
        if let Some(image) = &group.style.background_image {
            self.uses_assets = true;
            attrs.push((
                ":style",
                format!(
                    "{{ backgroundImage: `url(${{assets['{}']}})` }}",
                    script_string(&image.name)
                ),
            ));
        }
        if let Some(id) = &group.handle {
            attrs.push(("data-vx-ref", id.clone()));
        }
        let event = group.interaction.as_ref().and_then(|interaction| interaction.event.as_ref());
        let element = if event.is_some() { "button" } else { "div" };
        if element == "button" {
            attrs.push(("type", "button".to_owned()));
        }
        self.raise(event, &mut attrs);

        let outer = self.inline;
        self.inline = inner == Parent::Inline;
        self.open(element, &attrs);
        self.children(&group.items, inner, group.style.padding.as_ref(), copy);
        self.close(element);
        self.inline = outer;
    }

    /// A group's children, with each stack layer wrapped in the box that anchors it.
    fn children(
        &mut self, items: &[Node], inner: Parent, padding: Option<&Padding>, copy: &Contract,
    ) {
        if inner != Parent::Stack {
            for node in items {
                self.node(node, inner, copy);
            }
            return;
        }

        for (index, node) in items.iter().enumerate() {
            let layer = match node {
                Node::Group(group) => group.layout.position.clone(),
                Node::Item(item) => item.layout.position.clone(),
                Node::Collection(_) => None,
            };
            let order = (index + 1).to_string();
            if let Some(layer) = layer {
                let mut wrapper = Css::default();
                wrapper.set("position", "absolute");
                wrapper.set("inset", &inset(padding));
                wrapper.set("z-index", &order);
                wrapper.set("display", "flex");
                let (vertical, horizontal) = layer.axes();
                wrapper.set("align-items", edge(vertical));
                wrapper.set("justify-content", edge(horizontal));
                wrapper.set("pointer-events", "none");
                self.open(
                    "div",
                    &[
                        ("class", "vx-layer".to_owned()),
                        ("data-vx-position", layer.align.clone()),
                        ("style", wrapper.render()),
                    ],
                );
                self.layered(node, &layer, copy);
            } else {
                self.open("div", &[(
                    "style",
                    format!(
                        "grid-area: 1 / 1; position: relative; z-index: {order}; display: flex; min-width: 0{}",
                        flow_cell(node)
                    ),
                )]);
                self.node(node, Parent::Row, copy);
            }
            self.close("div");
        }
    }

    /// A layer's node, moved by its offset.
    fn layered(&mut self, node: &Node, layer: &Layer, copy: &Contract) {
        let mut shift = Css::default();
        shift.set("position", "relative");
        shift.set("pointer-events", "auto");
        if let Some(x) = &layer.x {
            shift.set("left", &format!("var(--vectis-offset-{x})"));
        }
        if let Some(y) = &layer.y {
            shift.set("top", &format!("var(--vectis-offset-{y})"));
        }
        let (width, height) = match node {
            Node::Group(group) => (group.size.width, group.size.height),
            Node::Item(item) => (item.size.width, item.size.height),
            Node::Collection(_) => (None, None),
        };
        if width == Some(Dimension::Fill) {
            shift.set("flex", "1 1 0%");
        }
        if height == Some(Dimension::Fill) {
            shift.set("align-self", "stretch");
        }
        shift.set("display", "flex");
        self.open("div", &[("class", "vx-offset".to_owned()), ("style", shift.render())]);
        self.node(node, Parent::Row, copy);
        self.close("div");
    }

    fn raise(&mut self, event: Option<&Event>, attrs: &mut Vec<(&str, String)>) {
        let Some(event) = event else { return };
        if event.is_hole() {
            attrs.push(("data-vx-hole", "event".to_owned()));
            self.comment("TODO(event): unfilled hole, raises nothing");
        } else {
            self.uses_events = true;
            attrs.push(("@click", format!("emit('event', {{ type: '{}' }})", event.name)));
        }
    }

    fn item(&mut self, item: &Item, parent: Parent, copy: &Contract) {
        let web = item.layout.web.as_ref();
        let mut css = Css::default();
        style(&mut css, &item.style);
        sizing(&mut css, &item.size, parent);
        self_alignment(&mut css, item.layout.align_self.as_deref(), &item.size, parent);
        pinned(&mut css, item.layout.pinned.as_deref());
        line_limit(&mut css, item.facts.line_limit);

        match item.kind.as_str() {
            "text" | "badge" | "chip" => {
                text_style(&mut css, item);
                // A run piece's edge space ("yet? " before a link) is part of the copy; a flex
                // item would trim it.
                if parent == Parent::Inline {
                    css.set("display", "inline");
                } else if item
                    .content
                    .as_deref()
                    .is_some_and(|c| c.starts_with(' ') || c.ends_with(' '))
                {
                    css.set("white-space", "pre-wrap");
                }
                web_facts(&mut css, web, item.layout.position.is_some());
                let inner = self.said(item, copy);
                self.leaf("span", &[("class", type_class(item)), ("style", css.render())], &inner);
            }
            "link" => {
                text_style(&mut css, item);
                if parent == Parent::Inline {
                    css.set("display", "inline");
                }
                web_facts(&mut css, web, item.layout.position.is_some());
                let inner = self.said(item, copy);
                let mut attrs = vec![("class", type_class(item)), ("style", css.render())];
                if let Some(href) =
                    item.facts.destination.as_deref().or_else(|| web_str(web, "href"))
                {
                    attrs.push(("href", href.to_owned()));
                }
                if let Some(target) = web_str(web, "target") {
                    attrs.push(("target", target.to_owned()));
                    attrs.push(("rel", "noopener noreferrer".to_owned()));
                }
                self.raise(item.event.as_ref(), &mut attrs);
                self.leaf("a", &attrs, &inner);
            }
            "button" => self.button(item, css, copy),
            "icon" | "image" | "avatar" => self.artwork(item, css),
            "spacer" => {
                if parent == Parent::Inline {
                    // An empty element in a run: a line break when it fills the line, nothing
                    // otherwise.
                    let breaks = item.size.width == Some(Dimension::Fill);
                    css = Css::default();
                    css.set("display", if breaks { "block" } else { "inline" });
                } else if item.size.width.is_none() && item.size.height.is_none() {
                    css.set("flex", "1 1 auto");
                }
                web_facts(&mut css, web, item.layout.position.is_some());
                self.leaf(
                    "div",
                    &[("aria-hidden", "true".to_owned()), ("style", css.render())],
                    "",
                );
            }
            "divider" => {
                if !css.has("border-top-width") {
                    css.set("border-top", "1px solid currentColor");
                }
                web_facts(&mut css, web, item.layout.position.is_some());
                self.void("hr", &[("style", css.render())]);
            }
            "text-field" | "search-field" | "text-area" => self.entry(item, css),
            other => {
                web_facts(&mut css, web, item.layout.position.is_some());
                self.comment(&format!("vx: a `{other}` item is not drawn by the Vue shell yet"));
                self.leaf(
                    "div",
                    &[("data-vx-kind", other.to_owned()), ("style", css.render())],
                    "",
                );
            }
        }
    }

    /// A text entry: an `<input>` (typed by `x-web.attr_type`) or a `<textarea>`.
    fn entry(&mut self, item: &Item, mut css: Css) {
        let web = item.layout.web.as_ref();
        text_style(&mut css, item);
        if let Some(background) = web_str(web, "background") {
            css.set("background", &color(background));
        }
        if let Some(sides) = web.and_then(|web| web.get("padding")) {
            web_sides(&mut css, "padding", sides);
        }
        web_facts(&mut css, web, item.layout.position.is_some());
        let mut attrs = vec![("class", type_class(item)), ("style", css.render())];
        if let Some(placeholder) = &item.placeholder {
            attrs.push(("placeholder", placeholder.clone()));
        }
        for attribute in ["name", "autocomplete", "inputmode"] {
            if let Some(value) = web_str(web, &format!("attr_{attribute}")) {
                attrs.push((attribute, value.to_owned()));
            }
        }
        if let Some(label) = &item.a11y.label {
            attrs.push(("aria-label", label.clone()));
        }
        if item.kind == "text-area" {
            self.leaf("textarea", &attrs, "");
        } else {
            let fallback = if item.kind == "search-field" { "search" } else { "text" };
            let typed = item.facts.content_type.as_deref().map(|kind| match kind {
                "phone" => "tel",
                other => other,
            });
            let kind = typed.or_else(|| web_str(web, "attr_type")).unwrap_or(fallback);
            attrs.push(("type", kind.to_owned()));
            self.void("input", &attrs);
        }
    }

    fn button(&mut self, item: &Item, mut css: Css, copy: &Contract) {
        let web = item.layout.web.as_ref();
        css.set("display", if self.inline { "inline-flex" } else { "flex" });
        css.set("flex-direction", "row");
        css.set("align-items", "center");
        css.set("justify-content", "center");
        if let Some(background) = web_str(web, "background") {
            css.set("background", &color(background));
        }
        if let Some(sides) = web.and_then(|web| web.get("padding")) {
            web_sides(&mut css, "padding", sides);
        }
        if let Some(align) = item.align.as_deref().or_else(|| web_str(web, "text_align")) {
            css.set("text-align", align);
            css.set(
                "justify-content",
                match align {
                    "start" | "left" => "flex-start",
                    "end" | "right" => "flex-end",
                    _ => "center",
                },
            );
        }
        if let Some(color_token) = &item.style.color {
            css.set("color", &color(color_token));
        }
        web_facts(&mut css, web, item.layout.position.is_some());

        let mut attrs = Vec::new();
        if let Some(typography) =
            item.facts.text_style.as_deref().or_else(|| web_str(web, "typography"))
        {
            attrs.push(("class", format!("vectis-type-{typography}")));
        }
        attrs.push(("type", web_str(web, "attr_type").unwrap_or("button").to_owned()));
        attrs.push(("style", css.render()));
        if let Some(label) = &item.a11y.label {
            attrs.push(("aria-label", label.clone()));
        }
        self.raise(item.event.as_ref(), &mut attrs);

        let icons: Vec<&Value> = web
            .and_then(|web| web.get("icons"))
            .and_then(Value::as_array)
            .map(|icons| icons.iter().collect())
            .unwrap_or_default();

        self.open("button", &attrs);
        for placement in ["leading", "trailing"] {
            if placement == "trailing" {
                let label = self.said(item, copy);
                if !label.is_empty() {
                    self.line(&label);
                }
            }
            for icon in &icons {
                if icon.get("placement").and_then(Value::as_str).unwrap_or("leading") != placement {
                    continue;
                }
                if let Some(Node::Item(media)) = icon.get("item").and_then(media_node) {
                    self.item(&media, Parent::Row, copy);
                }
            }
        }
        if icons.is_empty()
            && let Some(name) = &item.name
        {
            let mut media = Css::default();
            media.set("flex-shrink", "0");
            self.uses_assets = true;
            self.void(
                "img",
                &[
                    ("alt", String::new()),
                    (":src", format!("assets['{}']", script_string(name))),
                    ("style", media.render()),
                ],
            );
        }
        self.close("button");
    }

    fn artwork(&mut self, item: &Item, mut css: Css) {
        let web = item.layout.web.as_ref();
        if let Some(color_token) = &item.style.color {
            css.set("color", &color(color_token));
        }
        web_facts(&mut css, web, item.layout.position.is_some());
        if !css.has("flex-shrink") {
            css.set("flex-shrink", "0");
        }
        if self.inline {
            css.set("display", "inline-block");
        }

        if let Some(fit) = item.facts.content_mode.as_deref() {
            let fit = match fit {
                "fit" => "contain",
                "fill" => "cover",
                _ => "fill",
            };
            css.set("object-fit", fit);
        }
        // A glyph: its text in its font, the em box the item's smaller side, centred.
        if let Some(glyph) = (item.kind == "icon")
            .then(|| item.name.as_ref().and_then(|name| self.glyphs.get(name)))
            .flatten()
            .cloned()
        {
            self.uses_glyphs = true;
            css.set("display", "inline-flex");
            css.set("align-items", "center");
            css.set("justify-content", "center");
            css.set("font-family", &format!("'{}'", glyph.family.replace('\'', "\\'")));
            css.set("font-style", "normal");
            css.set("line-height", "1");
            if let Some(weight) = glyph.weight {
                css.set("font-weight", &weight.to_string());
            }
            if let (Some(Dimension::Fixed(width)), Some(Dimension::Fixed(height))) =
                (item.size.width, item.size.height)
            {
                css.set("font-size", &px(width.min(height)));
            }
            self.leaf(
                "span",
                &[("aria-hidden", "true".to_owned()), ("style", css.render())],
                &text(&glyph.text),
            );
            return;
        }
        let asset_markup = (item.kind == "icon")
            .then(|| item.name.as_ref().and_then(|name| self.inline_svg.get(name)))
            .flatten();
        if let Some(markup) =
            asset_markup.map(String::as_str).or_else(|| web_str(web, "svg_markup"))
        {
            let markup = markup.to_owned();
            self.line(&inline_svg(&markup, &css.render()));
            return;
        }
        let alt = item.a11y.label.as_deref().or_else(|| web_str(web, "alt")).unwrap_or_default();
        let mut attrs = vec![("alt", alt.to_owned())];
        match &item.name {
            Some(name) => {
                self.uses_assets = true;
                attrs.push((":src", format!("assets['{}']", script_string(name))));
            }
            None => {
                if let Some(src) = web_str(web, "src") {
                    attrs.push(("src", src.to_owned()));
                }
            }
        }
        attrs.push(("style", css.render()));
        self.void("img", &attrs);
    }

    /// What a text-bearing item says, as template text.
    fn said(&mut self, item: &Item, copy: &Contract) -> String {
        if let Some(content) = &item.content {
            if content.is_empty() {
                return String::new();
            }
            if let Some(key) = copy.string(content) {
                self.uses_strings = true;
                return format!("{{{{ s.{key} }}}}");
            }
            return text(content);
        }
        if let Some(bind) = &item.bind {
            return format!(
                "<!-- vx: bound to `{}`, which the Vue shell has no view model for yet -->",
                bind.replace("--", "- -")
            );
        }
        String::new()
    }
}

/// A button's media item, read back from its `x-web.icons` entry.
fn media_node(item: &Value) -> Option<Node> {
    crate::model::loose_item(item).map(|item| Node::Item(Box::new(item)))
}

/// A group's own layout: a block for a run of text, grid tracks for several fillers, flex for a
/// row or column, and a one-cell grid for a stack.
fn container(css: &mut Css, group: &Group, web: Option<&Value>, run: bool, tracked: bool) {
    match group.direction {
        Direction::Row if run => {
            css.set("display", "block");
            if let Some(justify) = &group.justify {
                let align = match justify.as_str() {
                    "center" => "center",
                    "end" => "end",
                    "space-between" => "justify",
                    _ => "start",
                };
                css.set("text-align", align);
            }
        }
        Direction::Row | Direction::Column if tracked => {
            let row = group.direction == Direction::Row;
            css.set("display", "grid");
            css.set("grid-auto-flow", if row { "column" } else { "row" });
            let tracks: Vec<&str> = group
                .items
                .iter()
                .map(|node| {
                    if node_size(node).along(group.direction) == Some(Dimension::Fill) {
                        "minmax(0, 1fr)"
                    } else {
                        "auto"
                    }
                })
                .collect();
            css.set(
                if row { "grid-template-columns" } else { "grid-template-rows" },
                &tracks.join(" "),
            );
            if let Some(justify) = &group.justify {
                css.set("justify-content", &justify_value(justify));
            }
            let align = group.align.as_deref().map_or_else(|| "center".to_owned(), grid_value);
            css.set(if row { "align-items" } else { "justify-items" }, &align);
            if let Some(gap) = &group.style.gap {
                css.set(if row { "column-gap" } else { "row-gap" }, &spacing(gap));
            }
            let cross = if row { "row_gap" } else { "column_gap" };
            if let Some(gap) = group.style.cross_gap.as_deref().or_else(|| web_str(web, cross)) {
                css.set(&cross.replace('_', "-"), &spacing(gap));
            }
        }
        Direction::Row | Direction::Column => {
            css.set("display", "flex");
            css.set(
                "flex-direction",
                if group.direction == Direction::Row { "row" } else { "column" },
            );
            if let Some(justify) = &group.justify {
                css.set("justify-content", &justify_value(justify));
            }
            css.set(
                "align-items",
                &group.align.as_deref().map_or_else(|| "center".to_owned(), align_value),
            );
            if group.wrap || group.flow_wrap || web_flag(web, "inline_wrap") {
                css.set("flex-wrap", "wrap");
            }
            let (main, cross) = if group.direction == Direction::Row {
                ("column-gap", "row_gap")
            } else {
                ("row-gap", "column_gap")
            };
            if let Some(gap) = &group.style.gap {
                css.set(main, &spacing(gap));
            }
            if let Some(gap) = group.style.cross_gap.as_deref().or_else(|| web_str(web, cross)) {
                css.set(&cross.replace('_', "-"), &spacing(gap));
            }
        }
        Direction::Stack => {
            css.set("display", "grid");
            css.set("position", "relative");
            css.set(
                "justify-items",
                &group.justify.as_deref().map_or_else(|| "start".to_owned(), grid_value),
            );
            css.set(
                "align-items",
                &group.align.as_deref().map_or_else(|| "start".to_owned(), grid_value),
            );
        }
    }
}

/// A node's size, whatever kind of node it is.
fn node_size(node: &Node) -> Size {
    match node {
        Node::Group(group) => group.size,
        Node::Item(item) => item.size,
        Node::Collection(_) => Size::default(),
    }
}

/// The extra declarations a stack's flow cell needs for a child that fills it.
fn flow_cell(node: &Node) -> String {
    let (width, height) = match node {
        Node::Group(group) => (group.size.width, group.size.height),
        Node::Item(item) => (item.size.width, item.size.height),
        Node::Collection(_) => (None, None),
    };
    let mut out = String::new();
    if width == Some(Dimension::Fill) {
        out.push_str("; justify-self: stretch");
    }
    if height == Some(Dimension::Fill) {
        out.push_str("; align-self: stretch");
    }
    out
}

/// The token-keyed visual properties every node shares.
fn style(css: &mut Css, style: &Style) {
    padding(css, style.padding.as_ref());
    if let Some(background) = &style.background {
        css.set("background", &color(background));
    }
    if let Some(radius) = &style.corner_radius {
        css.set("border-radius", &format!("var(--vectis-corner-radius-{radius})"));
    }
    // The image itself is a bound `background-image` (its URL is the asset map's); the box
    // geometry is static. Drawn once: tiling is a web fact (`x-web.css.background-repeat`).
    if let Some(image) = &style.background_image {
        css.set("background-size", background_size(image.content_mode.as_deref()));
        let (x, y) = image.fractions();
        css.set("background-position", &format!("{}% {}%", scalar(x * 100.0), scalar(y * 100.0)));
        css.set("background-repeat", "no-repeat");
    }
    if let Some(border) = &style.border {
        let colour = border.color.as_ref().map_or_else(
            || format!("var(--vectis-border-{}-color, currentColor)", border.width),
            |token| color(token),
        );
        css.set("border", &format!("var(--vectis-border-{}) solid {colour}", border.width));
    }
    if let Some(opacity) = &style.opacity {
        css.set("opacity", &format!("var(--vectis-opacity-{opacity})"));
    }
    if let Some(elevation) = &style.elevation {
        css.set("box-shadow", &format!("var(--vectis-shadow-{elevation})"));
    }
    if let Some(shadow) = &style.shadow {
        css.set("box-shadow", &format!("var(--vectis-box-shadow-{shadow})"));
    }
    if style.clip {
        css.set("overflow", "hidden");
    }
}

/// A text item's type style, colour and alignment.
fn text_style(css: &mut Css, item: &Item) {
    if let Some(color_token) = &item.style.color {
        css.set("color", &color(color_token));
    }
    if let Some(align) = &item.align {
        css.set("text-align", align);
    }
    if let Some(gradient) = &item.facts.gradient {
        let stops: Vec<String> = gradient
            .stops
            .iter()
            .map(|(token, at)| format!("{} {}%", color(token), scalar(at * 100.0)))
            .collect();
        css.set(
            "background-image",
            &format!("linear-gradient({}deg, {})", scalar(gradient.angle), stops.join(", ")),
        );
        css.set("-webkit-background-clip", "text");
        css.set("background-clip", "text");
        css.set("-webkit-text-fill-color", "transparent");
    }
}

/// A text item's type style, as the class the theme declares for it.
fn type_class(item: &Item) -> String {
    item.style_key
        .as_ref()
        .or(item.facts.text_style.as_ref())
        .map(|key| format!("vectis-type-{key}"))
        .unwrap_or_default()
}

/// `line_limit`: one line never wraps; more clamp the label at that many.
fn line_limit(css: &mut Css, limit: Option<u32>) {
    match limit {
        Some(1) => css.set("white-space", "nowrap"),
        Some(lines) => {
            css.set("display", "-webkit-box");
            css.set("-webkit-box-orient", "vertical");
            css.set("-webkit-line-clamp", &lines.to_string());
            css.set("overflow", "hidden");
        }
        None => {}
    }
}

/// `pinned`: stays at that edge of its scroll container while the content scrolls past.
fn pinned(css: &mut Css, edge: Option<&str>) {
    if let Some(edge @ ("top" | "bottom")) = edge {
        css.set("position", "sticky");
        css.set(edge, "0");
    }
}

/// The stack's content box, as the `inset` of the box a layer is anchored in.
fn inset(padding: Option<&Padding>) -> String {
    match padding {
        Some(Padding::All(token)) => spacing(token),
        Some(Padding::Sides {
            top,
            right,
            bottom,
            left,
        }) => [top, right, bottom, left]
            .iter()
            .map(|side| side.as_deref().map_or_else(|| "0".to_owned(), spacing))
            .collect::<Vec<_>>()
            .join(" "),
        None => "0".to_owned(),
    }
}

fn padding(css: &mut Css, padding: Option<&Padding>) {
    match padding {
        Some(Padding::All(token)) => css.set("padding", &spacing(token)),
        Some(Padding::Sides {
            top,
            right,
            bottom,
            left,
        }) => {
            for (side, token) in
                [("top", top), ("right", right), ("bottom", bottom), ("left", left)]
            {
                if let Some(token) = token {
                    css.set(&format!("padding-{side}"), &spacing(token));
                }
            }
        }
        None => {}
    }
}

/// Width, height and their bounds, read against the container the node sits in.
fn sizing(css: &mut Css, size: &Size, parent: Parent) {
    for (dimension, horizontal) in [(size.width, true), (size.height, false)] {
        let property = if horizontal { "width" } else { "height" };
        let main = match parent {
            Parent::Row => horizontal,
            Parent::Column => !horizontal,
            Parent::Stack | Parent::Inline => false,
            Parent::Track(row) => horizontal == row,
        };
        match dimension {
            Some(Dimension::Fixed(value)) => {
                css.set(property, &px(value));
                if main {
                    css.set("flex-shrink", "0");
                }
            }
            Some(Dimension::Fill) => {
                if let Parent::Track(row) = parent {
                    if main {
                        css.set(if horizontal { "min-width" } else { "min-height" }, "0");
                    } else {
                        css.set(if row { "align-self" } else { "justify-self" }, "stretch");
                    }
                } else if main {
                    css.set("flex", "1 1 0%");
                    css.set(if horizontal { "min-width" } else { "min-height" }, "0");
                } else if parent == Parent::Inline {
                } else if parent == Parent::Stack {
                    css.set(if horizontal { "justify-self" } else { "align-self" }, "stretch");
                } else {
                    css.set("align-self", "stretch");
                }
            }
            Some(Dimension::Hug) | None => {}
        }
    }
    for (property, bound) in [
        ("min-width", size.min_width),
        ("max-width", size.max_width),
        ("min-height", size.min_height),
        ("max-height", size.max_height),
    ] {
        if let Some(bound) = bound {
            css.set(property, &px(bound));
        }
    }
}

/// `align_self`, unless the node already fills its container's cross axis.
fn self_alignment(css: &mut Css, align_self: Option<&str>, size: &Size, parent: Parent) {
    let Some(align) = align_self else { return };
    let cross = match parent {
        Parent::Inline => return,
        Parent::Row | Parent::Track(true) => size.height,
        Parent::Column | Parent::Stack | Parent::Track(false) => size.width,
    };
    if cross == Some(Dimension::Fill) {
        return;
    }
    match parent {
        Parent::Track(false) => css.set("justify-self", &grid_value(align)),
        Parent::Track(true) => css.set("align-self", &grid_value(align)),
        _ => css.set("align-self", &align_value(align)),
    }
}

/// The `x-web` facts that are CSS: margins, padding, sizes the core cannot spell, and the verbatim
/// declarations. A layer's positional declarations are left out, because `position` states them.
fn web_facts(css: &mut Css, web: Option<&Value>, layered: bool) {
    let Some(web) = web else { return };
    if let Some(margin) = web.get("margin") {
        web_sides(css, "margin", margin);
    }
    // An item's own padding and background, which the core vocabulary only has on groups.
    if let Some(padding) = web.get("padding") {
        web_sides(css, "padding", padding);
    }
    if let Some(background) = web.get("background").and_then(Value::as_str) {
        css.set("background", &color(background));
    }
    if let Some(size) = web.get("size").and_then(Value::as_object) {
        for (key, value) in size {
            if let Some(raw) = value.get("css").and_then(Value::as_str) {
                css.set(&key.replace('_', "-"), raw);
            }
        }
    }
    if let Some(inferred) = web.get("css_inferred").and_then(Value::as_object) {
        for (key, value) in inferred {
            if let Some(value) = value.as_str() {
                css.set(key, value);
            }
        }
    }
    if let Some(declarations) = web.get("css").and_then(Value::as_object) {
        for (key, value) in declarations {
            let Some(value) = value.as_str() else { continue };
            if layered && POSITIONAL.contains(&key.as_str()) {
                continue;
            }
            if CORE_OWNED.contains(&key.as_str()) && css.has(key) {
                continue;
            }
            css.set(key, value);
        }
    }
}

fn web_sides(css: &mut Css, property: &str, sides: &Value) {
    if let Some(token) = sides.as_str() {
        css.set(property, &spacing(token));
        return;
    }
    for side in ["top", "right", "bottom", "left"] {
        if let Some(token) = sides.get(side).and_then(Value::as_str) {
            css.set(&format!("{property}-{side}"), &spacing(token));
        }
    }
}

/// CSS `background-size` for a background image's `content_mode`.
fn background_size(mode: Option<&str>) -> &'static str {
    match mode {
        Some("fit") => "contain",
        Some("fill") => "cover",
        Some("stretch") => "100% 100%",
        _ => "auto",
    }
}

fn css_get<'a>(css: &'a Css, key: &str) -> Option<&'a str> {
    css.0.iter().find(|(existing, _)| existing == key).map(|(_, value)| value.as_str())
}

fn web_str<'a>(web: Option<&'a Value>, key: &str) -> Option<&'a str> {
    web.and_then(|web| web.get(key)).and_then(Value::as_str)
}

fn web_flag(web: Option<&Value>, key: &str) -> bool {
    web.and_then(|web| web.get(key)).and_then(Value::as_bool).unwrap_or(false)
}

/// An inline SVG with the node's declarations folded into its root element's `style`.
fn inline_svg(markup: &str, declarations: &str) -> String {
    let Some(start) = markup.find("<svg") else { return markup.to_owned() };
    let open_end = markup[start..].find('>').map_or(markup.len(), |end| start + end);
    let open = &markup[start..open_end];
    open.find(" style=\"")
        .map_or_else(
            || {
                let insert = start + "<svg".len();
                format!(
                    "{} style=\"{}\"{}",
                    &markup[..insert],
                    attribute(declarations),
                    &markup[insert..]
                )
            },
            |at| {
                let insert = start + at + " style=\"".len();
                format!("{}{}; {}", &markup[..insert], attribute(declarations), &markup[insert..])
            },
        )
        .replace('\n', " ")
}

fn color(token: &str) -> String {
    format!("var(--vectis-color-{token})")
}

fn spacing(token: &str) -> String {
    format!("var(--vectis-spacing-{token})")
}

fn px(value: f64) -> String {
    if value == 0.0 { "0".to_owned() } else { format!("{}px", scalar(value)) }
}

fn justify_value(justify: &str) -> String {
    match justify {
        "start" => "flex-start".to_owned(),
        "end" => "flex-end".to_owned(),
        other => other.to_owned(),
    }
}

fn align_value(align: &str) -> String {
    match align {
        "start" | "top" | "leading" => "flex-start".to_owned(),
        "end" | "bottom" | "trailing" => "flex-end".to_owned(),
        other => other.to_owned(),
    }
}

fn grid_value(align: &str) -> String {
    match align {
        "top" | "leading" | "space-between" | "space-around" | "space-evenly" => "start".to_owned(),
        "bottom" | "trailing" => "end".to_owned(),
        other => other.to_owned(),
    }
}

/// A 9-point anchor's axis as a flex alignment.
fn edge(axis: &str) -> &'static str {
    match axis {
        "top" | "start" => "flex-start",
        "bottom" | "end" => "flex-end",
        _ => "center",
    }
}

fn attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

fn text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace("{{", "{&#123;")
}

fn script_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

/// The root component: the first screen, or the one the location hash names.
fn app(surface: &Surface) -> String {
    let mut imports = String::new();
    let mut table = String::new();
    for screen in &surface.screens {
        let component = format!("{}Screen", pascal(&screen.slug));
        let _ = writeln!(imports, "import {component} from \"./views/{component}.vue\";");
        let _ = writeln!(table, "  \"{}\": {component},", script_string(&screen.slug));
    }
    let first = surface.screens.first().map(|screen| screen.slug.clone()).unwrap_or_default();
    format!(
        "<!-- {HEADER} -->\n<script setup lang=\"ts\">\nimport {{ computed, ref }} from \"vue\";\n{imports}\nconst screens = {{\n{table}}} as const;\n\nconst route = ref(window.location.hash.slice(1) || \"{first}\");\nwindow.addEventListener(\"hashchange\", () => {{\n  route.value = window.location.hash.slice(1) || \"{first}\";\n}});\nconst current = computed(() => screens[route.value as keyof typeof screens] ?? screens[\"{first}\"]);\n</script>\n\n<template>\n  <component :is=\"current\" />\n</template>\n"
    )
}

/// `ui-strings.ts`: every literal the screens say, under the key the copy contract gave it.
fn strings(copy: &Contract) -> String {
    let mut out = format!("// {HEADER}\n\nexport const s = {{\n");
    for (literal, key) in copy.strings() {
        let _ = writeln!(out, "  {key}: {},", Value::String(literal.to_owned()));
    }
    out.push_str("} as const;\n");
    out
}

/// `contract.ts`: the events the screens raise, as a discriminated union. Holes are not events.
fn events(contract: &contract::Contract) -> String {
    let mut out = format!("// {HEADER}\n\nexport type Event =\n");
    if contract.events.is_empty() {
        out.push_str("  never;\n");
        return out;
    }
    let variants: Vec<String> = contract
        .events
        .iter()
        .map(|(name, args)| {
            let fields = args.iter().fold(String::new(), |mut fields, arg| {
                let _ = write!(fields, "; {}: {}", arg.name, ts_type(arg.carries));
                fields
            });
            format!("  | {{ type: \"{name}\"{fields} }}")
        })
        .collect();
    out.push_str(&variants.join("\n"));
    out.push_str(";\n");
    out
}

const fn ts_type(carried: Carried) -> &'static str {
    match carried {
        Carried::Value(BindKind::Flag) => "boolean",
        Carried::Value(BindKind::Number | BindKind::Moment) => "number",
        Carried::Value(_) | Carried::Route => "string",
    }
}

/// `glyphs.ts`: registers each glyph font family from its served file, once per family, so a
/// glyph draws in it (the font's URL honours the app's base, as `assets.ts` does).
#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "`${base}${path}` is a JavaScript template literal the emit writes"
)]
fn glyph_fonts(glyphs: &BTreeMap<String, Glyph>, assets: &BTreeMap<String, String>) -> String {
    let mut fonts: BTreeMap<&str, &str> = BTreeMap::new();
    for (name, glyph) in glyphs {
        if let Some(path) = assets.get(name) {
            fonts.entry(glyph.family.as_str()).or_insert(path.as_str());
        }
    }
    let mut out = format!(
        "// {HEADER}\n\nconst base = import.meta.env.BASE_URL;\n\nconst fonts: [string, string][] = [\n"
    );
    for (family, path) in fonts {
        let _ = writeln!(out, "  ['{}', '{}'],", script_string(family), script_string(path));
    }
    out.push_str("];\n\nfor (const [family, path] of fonts) {\n  const face = new FontFace(family, `url(${base}${path})`);\n  document.fonts.add(face);\n  void face.load();\n}\n");
    out
}

/// `assets.ts`: each asset's name to the URL it is served at.
fn asset_map(assets: &BTreeMap<String, String>) -> String {
    let mut out = format!(
        "// {HEADER}\n\nconst base = import.meta.env.BASE_URL;\n\nexport const assets: Record<string, string> = {{\n"
    );
    for (name, path) in assets {
        let _ = writeln!(
            out,
            "  \"{}\": `${{base}}{}`,",
            script_string(name),
            path.trim_start_matches('/').replace('`', "\\`").replace("${", "\\${")
        );
    }
    out.push_str("};\n");
    out
}

/// The Vite app around the views.
fn scaffold() -> Vec<GeneratedFile> {
    let package = format!(
        "{{\n  \"//\": \"{HEADER}\",\n  \"name\": \"vx-vue-shell\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": {{\n    \"dev\": \"vite\",\n    \"build\": \"vue-tsc --noEmit && vite build\",\n    \"preview\": \"vite preview\"\n  }},\n  \"dependencies\": {{\n    \"vue\": \"3.5.13\"\n  }},\n  \"devDependencies\": {{\n    \"@vitejs/plugin-vue\": \"5.2.1\",\n    \"typescript\": \"5.7.2\",\n    \"vite\": \"6.0.7\",\n    \"vue-tsc\": \"2.2.0\"\n  }}\n}}\n"
    );
    vec![
        GeneratedFile::new(format!("{ROOT}/package.json"), package),
        GeneratedFile::new(
            format!("{ROOT}/index.html"),
            format!(
                "<!doctype html>\n<!-- {HEADER} -->\n<html lang=\"en\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n    <title>vx</title>\n  </head>\n  <body>\n    <div id=\"app\"></div>\n    <script type=\"module\" src=\"/src/main.ts\"></script>\n  </body>\n</html>\n"
            ),
        ),
        GeneratedFile::new(
            format!("{ROOT}/vite.config.ts"),
            format!(
                "// {HEADER}\nimport {{ defineConfig }} from \"vite\";\nimport vue from \"@vitejs/plugin-vue\";\n\nexport default defineConfig({{\n  plugins: [vue()],\n  base: \"./\",\n}});\n"
            ),
        ),
        GeneratedFile::new(
            format!("{ROOT}/tsconfig.json"),
            format!(
                "// {HEADER}\n{{\n  \"compilerOptions\": {{\n    \"target\": \"ES2020\",\n    \"module\": \"ESNext\",\n    \"moduleResolution\": \"Bundler\",\n    \"strict\": true,\n    \"jsx\": \"preserve\",\n    \"isolatedModules\": true,\n    \"skipLibCheck\": true,\n    \"noEmit\": true,\n    \"types\": [\"vite/client\"]\n  }},\n  \"include\": [\"src/**/*.ts\", \"src/**/*.vue\"]\n}}\n"
            ),
        ),
        GeneratedFile::new(
            format!("{ROOT}/src/main.ts"),
            format!(
                "// {HEADER}\nimport {{ createApp }} from \"vue\";\nimport \"./theme.css\";\nimport App from \"./App.vue\";\n\ncreateApp(App).mount(\"#app\");\n"
            ),
        ),
        GeneratedFile::new(
            format!("{ROOT}/src/env.d.ts"),
            format!(
                "// {HEADER}\n/// <reference types=\"vite/client\" />\n\ndeclare module \"*.vue\" {{\n  import type {{ DefineComponent }} from \"vue\";\n  const component: DefineComponent<object, object, unknown>;\n  export default component;\n}}\n"
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model;

    fn emitted(body: &Value) -> (String, String) {
        let document =
            json!({ "version": 1, "screens": { "home": { "name": "Home", "body": body } } });
        let surface = model::parse(&document).expect("the composition parses");
        let copy = Contract::derive(&surface);
        let contract = contract::derive(&surface);
        let files =
            files(&surface, &copy, &contract, &BTreeMap::new(), &BTreeMap::new(), &BTreeMap::new());
        let pick = |path: &str| {
            files
                .iter()
                .find(|file| file.path == path)
                .map(|file| file.body.clone())
                .unwrap_or_default()
        };
        (pick("vue/src/views/HomeScreen.vue"), pick("vue/src/contract.ts"))
    }

    #[test]
    fn size_bounds_and_align_self_reach_the_style() {
        let (view, _) = emitted(&json!([{ "group": {
            "direction": "column",
            "size": { "width": "fill", "max_width": 320, "min_height": 800 },
            "items": [{ "text": { "content": "Hi", "align_self": "end" } }]
        } }]));
        assert!(view.contains("max-width: 320px"), "{view}");
        assert!(view.contains("min-height: 800px"), "{view}");
        assert!(view.contains("align-self: flex-end"), "{view}");
    }

    #[test]
    fn a_stack_layer_is_anchored_and_moved_by_its_offset_token() {
        let (view, _) = emitted(&json!([{ "group": {
            "direction": "stack",
            "padding": "md",
            "items": [
                { "text": { "content": "Under" } },
                { "group": {
                    "direction": "column",
                    "position": { "align": "bottom-end", "offset": { "x": "offset-m15", "y": "offset-10" } },
                    "x-web": { "css": { "position": "absolute", "top": "3px", "box-shadow": "none" } },
                    "items": [{ "text": { "content": "Over" } }]
                } }
            ]
        } }]));
        assert!(view.contains("data-vx-position=\"bottom-end\""), "{view}");
        assert!(view.contains("align-items: flex-end; justify-content: flex-end"), "{view}");
        assert!(view.contains("inset: var(--vectis-spacing-md)"), "{view}");
        assert!(view.contains("left: var(--vectis-offset-offset-m15)"), "{view}");
        assert!(view.contains("top: var(--vectis-offset-offset-10)"), "{view}");
        assert!(view.contains("grid-area: 1 / 1; position: relative; z-index: 1"), "{view}");
        assert!(view.contains("z-index: 2"), "{view}");
        assert!(view.contains("box-shadow: none"), "the rest of the verbatim CSS stays: {view}");
        assert!(!view.contains("top: 3px"), "the layer's own offsets are the position's: {view}");
    }

    #[test]
    fn an_event_hole_raises_nothing_and_names_nothing() {
        let (view, contract) =
            emitted(&json!([{ "button": { "content": "Go", "event": "TODO(event)" } }]));
        assert!(view.contains("data-vx-hole=\"event\""), "{view}");
        assert!(view.contains("TODO(event): unfilled hole"), "{view}");
        assert!(!view.contains("@click"), "{view}");
        assert!(!contract.contains("TODO"), "a hole is not an event: {contract}");
    }

    #[test]
    fn a_real_event_is_emitted_into_the_union() {
        let (view, contract) =
            emitted(&json!([{ "button": { "content": "Go", "event": "Start" } }]));
        assert!(view.contains("@click=\"emit('event', { type: 'Start' })\""), "{view}");
        assert!(contract.contains("{ type: \"Start\" }"), "{contract}");
    }

    #[test]
    fn an_inline_run_breaks_like_text() {
        let (view, _) = emitted(&json!([{ "group": {
            "direction": "row",
            "align": "baseline",
            "justify": "center",
            "x-web": { "flow": "inline", "inline_wrap": true },
            "items": [{ "text": { "content": "Don't have one? " } }, { "link": { "content": "Sign up", "x-web": { "href": "/signup" } } }]
        } }]));
        assert!(view.contains("display: block; text-align: center"), "{view}");
        assert!(view.contains("display: inline"), "{view}");
        assert!(view.contains("href=\"/signup\""), "{view}");
    }

    #[test]
    fn a_buttons_media_and_inferred_paint_are_drawn() {
        let (view, _) = emitted(&json!([{ "button": {
            "content": "",
            "icon": "svg-1",
            "x-web": {
                "background": "surface",
                "icons": [{ "placement": "leading", "item": { "icon": {
                    "name": "svg-1",
                    "size": { "width": 16, "height": 16 },
                    "x-web": { "svg_markup": "<svg viewBox=\"0 0 24 24\"><path d=\"M0 0\"/></svg>", "css_inferred": { "scale": "0" } }
                } } }]
            }
        } }]));
        assert!(view.contains("<svg style=\"width: 16px"), "{view}");
        assert!(view.contains("scale: 0"), "{view}");
        assert!(view.contains("background: var(--vectis-color-surface)"), "{view}");
    }

    #[test]
    fn verbatim_css_does_not_override_the_layout_the_core_states() {
        let (view, _) = emitted(&json!([{ "group": {
            "direction": "row",
            "items": [
                { "text": { "content": "a", "size": { "width": "fill" } } },
                { "text": { "content": "b", "size": { "width": "fill" } } }
            ],
            "x-web": { "css": { "grid-template-columns": "repeat(4, minmax(0px, 1fr))" } }
        } }]));
        assert!(view.contains("grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)"), "{view}");
        assert!(!view.contains("repeat(4"), "{view}");
    }

    #[test]
    fn the_promoted_core_facts_are_drawn_without_x_web() {
        let (view, _) = emitted(&json!([{ "group": {
            "direction": "row",
            "flow_wrap": true,
            "cross_gap": "sm",
            "clip": true,
            "shadow": "card",
            "pinned": "top",
            "padding": { "start": "md", "end": "lg" },
            "items": [
                { "text": { "content": "One line", "line_limit": 1 } },
                { "link": { "content": "Docs", "destination": "/docs" } },
                { "text-field": { "placeholder": "you@x", "content_type": "phone", "padding": "sm", "background": "surface" } },
                { "button": { "content": "Go", "text_style": "label", "align": "start" } },
                { "image": { "name": "hero", "label": "Hero", "content_mode": "fill" } },
                { "text": { "content": "Glow", "gradient": { "angle": 90, "stops": [
                    { "color": "a", "at": 0 }, { "color": "b", "at": 1 }
                ] } } }
            ]
        } }]));
        for needle in [
            "flex-wrap: wrap",
            "row-gap: var(--vectis-spacing-sm)",
            "overflow: hidden",
            "box-shadow: var(--vectis-box-shadow-card)",
            "position: sticky; top: 0",
            "padding-right: var(--vectis-spacing-lg)",
            "padding-left: var(--vectis-spacing-md)",
            "white-space: nowrap",
            "href=\"/docs\"",
            "type=\"tel\"",
            "background: var(--vectis-color-surface)",
            "class=\"vectis-type-label\"",
            "justify-content: flex-start",
            "alt=\"Hero\"",
            "object-fit: cover",
            "linear-gradient(90deg, var(--vectis-color-a) 0%, var(--vectis-color-b) 100%)",
        ] {
            assert!(view.contains(needle), "{needle} missing: {view}");
        }
    }

    #[test]
    fn a_run_that_does_not_flow_stays_on_one_line() {
        let (view, _) = emitted(&json!([{ "group": {
            "direction": "row",
            "x-web": { "flow": "inline" },
            "items": [{ "text": { "content": "a " } }, { "text": { "content": "b" } }]
        } }]));
        assert!(
            view.contains("display: block; white-space: nowrap")
                || view.contains("white-space: nowrap"),
            "{view}"
        );
    }

    #[test]
    fn an_icon_naming_a_vector_asset_is_inline_svg() {
        let document = json!({ "version": 1, "screens": { "home": { "name": "Home", "body": [
            { "icon": { "name": "star", "color": "ink", "size": { "width": 16, "height": 16 } } }
        ] } } });
        let surface = model::parse(&document).expect("the composition parses");
        let copy = Contract::derive(&surface);
        let contract = contract::derive(&surface);
        let inline = BTreeMap::from([(
            "star".to_owned(),
            "<svg fill=\"currentColor\" viewBox=\"0 0 1 1\"><path d=\"M0 0\"/></svg>".to_owned(),
        )]);
        let files = files(&surface, &copy, &contract, &BTreeMap::new(), &inline, &BTreeMap::new());
        let view =
            &files.iter().find(|f| f.path == "vue/src/views/HomeScreen.vue").expect("view").body;
        assert!(view.contains("color: var(--vectis-color-ink)\" fill=\"currentColor\""), "{view}");
        assert!(!view.contains("<img"), "{view}");
    }

    #[test]
    fn a_background_image_and_a_glyph_are_drawn() {
        let document = json!({ "version": 1, "screens": { "home": { "name": "Home", "body": [
            { "group": { "background_image": { "name": "hero", "content_mode": "fill", "alignment": "bottom-end" },
                         "items": [{ "icon": { "name": "star", "color": "ink", "size": { "width": 24, "height": 30 } } }] } }
        ] } } });
        let surface = model::parse(&document).expect("the composition parses");
        let copy = Contract::derive(&surface);
        let contract = contract::derive(&surface);
        let served = BTreeMap::from([
            ("hero".to_owned(), "assets/hero.jpg".to_owned()),
            ("star".to_owned(), "assets/icons.woff2".to_owned()),
        ]);
        let glyphs = BTreeMap::from([(
            "star".to_owned(),
            Glyph {
                text: "\u{f620}".to_owned(),
                family: "icons".to_owned(),
                weight: None,
            },
        )]);
        let files = files(&surface, &copy, &contract, &served, &BTreeMap::new(), &glyphs);
        let view =
            &files.iter().find(|f| f.path == "vue/src/views/HomeScreen.vue").expect("view").body;
        assert!(
            view.contains(":style=\"{ backgroundImage: `url(${assets['hero']})` }\""),
            "{view}"
        );
        assert!(view.contains("background-size: cover; background-position: 100% 100%; background-repeat: no-repeat"), "{view}");
        assert!(view.contains("font-family: 'icons'"), "{view}");
        assert!(view.contains("font-size: 24px"), "{view}");
        assert!(view.contains(">\u{f620}</span>"), "{view}");
        assert!(view.contains("import \"../glyphs\";"), "{view}");
        let fonts = &files.iter().find(|f| f.path == "vue/src/glyphs.ts").expect("glyphs.ts").body;
        assert!(fonts.contains("['icons', 'assets/icons.woff2']"), "{fonts}");
    }
}
