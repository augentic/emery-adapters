//! Interaction archetypes: what is tappable, and what a nested control
//! does to the container's gesture.
//!
//! §4.4 of the design notes. A container's `event` used to reach the
//! document only through `groupProps.additionalProperties`, so "the whole
//! row is the target" and "only the circle is the target" were the same
//! document and the shell writers coin-flipped between them. The
//! `interaction` block makes the choice representable; these checks make
//! it enforceable.
//!
//! Three rules, each closing one way the choice can go missing:
//! an interactive container declares an archetype from the closed set;
//! every `ref` an `interaction` names resolves inside that container;
//! and no node carries an `event` unless it is a control in its own
//! right or an enclosing `interaction` names it as one.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// The closed archetype set. Mirrors `$defs/archetype` in
/// `composition.schema.json`; `tests/validate.rs` pins them equal.
pub const ARCHETYPES: [&str; 9] = [
    "control",
    "grid-cell",
    "row-with-leading-control",
    "row-with-trailing-control",
    "selectable-chip",
    "static",
    "swipe-row",
    "tap-target",
    "text-entry",
];

/// Item types that own their own hit target, so an `event` on one needs
/// no enclosing declaration to be unambiguous.
///
/// Every type `figma-compile` gives an `event` to has to be in here, or a
/// draft the compiler considers finished is one this crate rejects.
/// `vx/tests/controls.rs` is the only thing holding the two together —
/// the crates do not depend on each other, for the reasons
/// `vx/tests/markers.rs` sets out. That test exists because `text-area`
/// was once missing from this list while sitting in `KNOWN_ITEM_TYPES`,
/// in the schema enum and in `figma-verify`'s `EDITABLE_ITEM_TYPES`: a
/// text area owns its hit target exactly as `text-field` does, and every
/// extract carrying one failed `vx spec` on an error naming the design
/// file rather than the omission.
pub const CONTROL_ITEM_TYPES: [&str; 16] = [
    "button",
    "checkbox",
    "chip",
    "date-picker",
    "fab",
    "icon-button",
    "link",
    "picker",
    "radio",
    "search-field",
    "segmented-control",
    "slider",
    "stepper",
    "switch",
    "text-area",
    "text-field",
];

/// Archetypes that require a primary `event` — the container acts, so
/// something has to say what it does.
const REQUIRES_EVENT: [&str; 6] = [
    "control",
    "grid-cell",
    "row-with-leading-control",
    "row-with-trailing-control",
    "selectable-chip",
    "tap-target",
];

/// Archetypes that must name at least one nested control.
const REQUIRES_CONTROLS: [&str; 2] = ["row-with-leading-control", "row-with-trailing-control"];

/// Archetypes that admit nested controls at all. On every other
/// archetype a nested control is a contradiction: `tap-target` means the
/// children are decorative, and `static` means nothing is interactive.
const ADMITS_CONTROLS: [&str; 5] = [
    "grid-cell",
    "row-with-leading-control",
    "row-with-trailing-control",
    "swipe-row",
    "text-entry",
];

/// Archetypes that admit swipe actions. `swipe-row` composes with the
/// two row archetypes, so all three carry them.
const ADMITS_SWIPE: [&str; 3] =
    ["row-with-leading-control", "row-with-trailing-control", "swipe-row"];

/// One `interaction`-bearing (or `event`-bearing) container.
struct Container<'a> {
    props: &'a Value,
    path: String,
    /// Whether it sits in the row template of a `list`, which is the only place a row can be
    /// swiped: the iOS shell's `.swipeActions` exists on a `List` row and nowhere else.
    listed: bool,
}

/// Check every interactive container in the document.
pub(crate) fn check_interactions(composition: &Value, errors: &mut Vec<Finding>) {
    for (root, path) in super::items::region_roots(composition) {
        // A body that is a list is handed over as its row template, the list itself already
        // unwrapped.
        let listed = path.ends_with("/list/item");
        walk(root, &path, &BTreeSet::new(), listed, errors);
    }
}

/// Descend the content tree, checking each container and reporting any
/// `event` that no enclosing declaration accounts for.
///
/// `controls` carries the handles the nearest enclosing `interaction`
/// named. A declaration accounts for the events on the nodes it names —
/// and only those, which is what stops an `interaction` block from
/// blanket-excusing every event beneath it. `listed` is whether this is
/// inside a `list`'s row template.
fn walk(
    node: &Value, path: &str, controls: &BTreeSet<String>, listed: bool, errors: &mut Vec<Finding>,
) {
    match node {
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), controls, listed, errors);
            }
        }
        Value::Object(map) => {
            if map.len() != 1 {
                return;
            }
            let Some((key, props)) = map.iter().next() else {
                return;
            };
            let child = format!("{path}/{}", escape_pointer_token(key));
            match key.as_str() {
                "group" => walk_group(props, &child, controls, listed, errors),
                "list" | "grid" => {
                    // A body list reached here already unwrapped (see `check_interactions`), so a
                    // `list` key met on the walk is one nested in content. That renders as rows in
                    // the enclosing stack rather than as a platform `List`, and has no row edge to
                    // swipe from any more than a grid cell does.
                    if let Some(items) = props.get("item") {
                        walk(items, &format!("{child}/item"), &BTreeSet::new(), false, errors);
                    }
                }
                "form" => walk(props, &child, controls, listed, errors),
                item_type => check_item_event(item_type, props, &child, controls, errors),
            }
        }
        _ => {}
    }
}

fn walk_group(
    props: &Value, path: &str, controls: &BTreeSet<String>, listed: bool, errors: &mut Vec<Finding>,
) {
    let container = Container {
        props,
        path: path.to_string(),
        listed,
    };
    let interaction = props.get("interaction");
    let has_event = props.get("event").is_some();

    if let Some(interaction) = interaction {
        check_container(&container, interaction, errors);
    } else if has_event {
        errors.push(Finding::new(
            format!("{path}/event"),
            "a container carrying an `event` must declare an `interaction` block naming its \
             archetype -- without one, nothing downstream can tell whether the whole container \
             is the tap target or only a child of it. Add `interaction: { archetype: tap-target, \
             event: ... }` (or the row archetype the design calls for) and move the event onto it"
                .to_string(),
        ));
    }

    // A container that declares its own interaction rescopes its
    // children's handles; one that does not passes the enclosing scope
    // through, because `collect_refs` reaches into it just the same.
    let scoped = interaction.map(control_refs);
    if let Some(items) = props.get("items") {
        walk(items, &format!("{path}/items"), scoped.as_ref().unwrap_or(controls), listed, errors);
    }
}

fn control_refs(interaction: &Value) -> BTreeSet<String> {
    interaction
        .get("controls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|control| control.get("ref").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

/// An `event` on a leaf is only unambiguous when the leaf is a control
/// in its own right, or when an enclosing `interaction` named it.
fn check_item_event(
    item_type: &str, props: &Value, path: &str, controls: &BTreeSet<String>,
    errors: &mut Vec<Finding>,
) {
    let named =
        props.get("ref").and_then(Value::as_str).is_some_and(|handle| controls.contains(handle));
    if props.get("event").is_none() || named || CONTROL_ITEM_TYPES.contains(&item_type) {
        return;
    }
    errors.push(Finding::new(
        format!("{path}/event"),
        format!(
            "`{item_type}` carries an `event` but is neither a control nor named by an enclosing \
             `interaction` -- an orphan tap target on a decorative node is exactly the \
             circle-only toggle this vocabulary exists to prevent. Either use a control item \
             type, or give the node a `ref` and name it under the enclosing container's \
             `interaction.controls`"
        ),
    ));
}

fn check_container(container: &Container<'_>, interaction: &Value, errors: &mut Vec<Finding>) {
    let path = &container.path;
    let Some(archetype) = interaction.get("archetype").and_then(Value::as_str) else {
        // The schema requires it; a missing value already errored there.
        return;
    };
    if !ARCHETYPES.contains(&archetype) {
        // Likewise closed in the schema.
        return;
    }

    let event = interaction.get("event").or_else(|| container.props.get("event"));
    let controls: Vec<&Value> = interaction
        .get("controls")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .collect();
    let swipes = ["swipe_leading", "swipe_trailing"]
        .iter()
        .filter(|key| interaction.get(*key).is_some())
        .count();

    check_shape(path, archetype, event.is_some(), controls.len(), swipes, errors);
    if swipes > 0 && !container.listed {
        errors.push(Finding::new(
            format!("{path}/interaction"),
            "swipe actions are revealed from a row of a `list`, and this container is not in one \
             -- the iOS shell's swipe exists only on a list row, so outside one it would reveal \
             nothing. Make the swiped rows the `item` of a `list` (any `style`, `cards` \
             included), or drop `swipe_leading` / `swipe_trailing`"
                .to_string(),
        ));
    }
    check_swipe_labels(path, interaction, errors);
    check_container_event_agreement(container, interaction, errors);
    check_control_refs(container, &controls, errors);
}

/// Every revealed action the user has to read needs words someone wrote.
///
/// A delete is exempt: the platform has its own label for one ("Delete" and a trash glyph), and
/// that is what the shell shows. Anything else would have to be named after its event, which is
/// copy no designer wrote and no translator sees. An action whose event is still a hole is left
/// alone, because whether it deletes is not decided yet; the hole is its own finding.
fn check_swipe_labels(path: &str, interaction: &Value, errors: &mut Vec<Finding>) {
    for key in ["swipe_leading", "swipe_trailing"] {
        let Some(actions) = interaction.get(key).and_then(Value::as_array) else {
            continue;
        };
        for (index, action) in actions.iter().enumerate() {
            let Some(event) = action.get("event").and_then(Value::as_str) else {
                continue;
            };
            let labelled = action.get("label").and_then(Value::as_str).is_some();
            let destructive = action.get("destructive").and_then(Value::as_bool) == Some(true);
            if labelled
                || destructive
                || deletes(event)
                || crate::spec::holes::Kind::of(event).is_some()
            {
                continue;
            }
            errors.push(Finding::new(
                format!("{path}/interaction/{key}/{index}"),
                format!(
                    "swipe action `{event}` has no `label` -- only a delete has a label the \
                     platform supplies, and naming any other action after its event is copy \
                     nobody wrote. Give it the `label` the design shows"
                ),
            ));
        }
    }
}

/// Whether an event, by its name, removes the element it is raised on.
///
/// Read from the `PascalCase` words of the variant name: `CityDeleted(id)` deletes, `CityPinned(id)`
/// does not. `vx-emit`'s iOS shell reads the same words to give the button its destructive role.
fn deletes(event: &str) -> bool {
    let name = event.split('(').next().unwrap_or_default().trim();
    let mut words = Vec::new();
    let mut start = 0;
    for (index, character) in name.char_indices().skip(1) {
        if character.is_uppercase() {
            words.push(&name[start..index]);
            start = index;
        }
    }
    words.push(&name[start..]);
    words.iter().any(|word| matches!(*word, "Delete" | "Deleted" | "Remove" | "Removed"))
}

/// Each archetype's arity: whether it acts, whether it may (or must)
/// carry nested controls, and whether swipe actions belong on it.
fn check_shape(
    path: &str, archetype: &str, has_event: bool, controls: usize, swipes: usize,
    errors: &mut Vec<Finding>,
) {
    if archetype == "static" && (has_event || controls > 0 || swipes > 0) {
        errors.push(Finding::new(
            format!("{path}/interaction"),
            "archetype `static` means not interactive and not focusable, so it carries no \
             `event`, no `controls`, and no swipe actions. Choose the archetype the design \
             actually calls for, or drop the `interaction` block"
                .to_string(),
        ));
        return;
    }

    if REQUIRES_EVENT.contains(&archetype) && !has_event {
        errors.push(Finding::new(
            format!("{path}/interaction"),
            format!(
                "archetype `{archetype}` makes the container itself the primary tap target, so \
                 it must declare the `event` that target fires"
            ),
        ));
    }

    if REQUIRES_CONTROLS.contains(&archetype) && controls == 0 {
        errors.push(Finding::new(
            format!("{path}/interaction"),
            format!(
                "archetype `{archetype}` describes a row whose nested control stops propagation, \
                 so it must name that control under `controls`. A row with no nested control is \
                 `tap-target`"
            ),
        ));
    }

    if controls > 0 && !ADMITS_CONTROLS.contains(&archetype) {
        errors.push(Finding::new(
            format!("{path}/interaction/controls"),
            format!(
                "archetype `{archetype}` admits no nested controls -- its children are \
                 decorative and the whole container is the target. Use one of the row \
                 archetypes ({}) when a child owns its own gesture",
                REQUIRES_CONTROLS.join(" / "),
            ),
        ));
    }

    if swipes > 0 && !ADMITS_SWIPE.contains(&archetype) {
        errors.push(Finding::new(
            format!("{path}/interaction"),
            format!(
                "swipe actions belong to a row archetype ({}); `{archetype}` has no revealed \
                 leading or trailing edge to put them on",
                ADMITS_SWIPE.join(" / "),
            ),
        ));
    }

    if archetype == "swipe-row" && swipes == 0 {
        errors.push(Finding::new(
            format!("{path}/interaction"),
            "archetype `swipe-row` must reveal at least one action -- declare `swipe_leading` \
             or `swipe_trailing`. A row with none is `tap-target` or one of the row archetypes"
                .to_string(),
        ));
    }
}

/// A container may carry its primary `event` on the group or on the
/// interaction block, but two spellings must not disagree.
fn check_container_event_agreement(
    container: &Container<'_>, interaction: &Value, errors: &mut Vec<Finding>,
) {
    let (Some(group_event), Some(interaction_event)) = (
        container.props.get("event").and_then(Value::as_str),
        interaction.get("event").and_then(Value::as_str),
    ) else {
        return;
    };
    if group_event != interaction_event {
        errors.push(Finding::new(
            format!("{}/interaction/event", container.path),
            format!(
                "the container declares `event: {group_event}` and its interaction declares \
                 `event: {interaction_event}` -- one container, one primary target, one event. \
                 Keep the event on the `interaction` block and remove the other"
            ),
        ));
    }
}

/// Every `ref` an interaction names resolves inside that container, and
/// no two nodes in the same container answer to the same handle.
fn check_control_refs(container: &Container<'_>, controls: &[&Value], errors: &mut Vec<Finding>) {
    let mut refs: BTreeMap<String, usize> = BTreeMap::new();
    if let Some(items) = container.props.get("items") {
        collect_refs(items, &mut refs);
    }

    for (name, count) in refs.iter().filter(|(_, count)| **count > 1) {
        errors.push(Finding::new(
            format!("{}/items", container.path),
            format!(
                "`ref: {name}` appears {count} times inside this container -- a handle names one \
                 node, and an interaction that names an ambiguous handle cannot say which node \
                 owns the gesture"
            ),
        ));
    }

    for (index, control) in controls.iter().enumerate() {
        let Some(name) = control.get("ref").and_then(Value::as_str) else {
            continue;
        };
        if !refs.contains_key(name) {
            errors.push(Finding::new(
                format!("{}/interaction/controls/{index}/ref", container.path),
                format!(
                    "`ref: {name}` resolves to no node inside this container -- a control the \
                     container does not contain cannot stop propagation to it. Add `ref: {name}` \
                     to the node that owns the gesture{}",
                    available_hint(&refs),
                ),
            ));
        }
    }
}

fn available_hint(refs: &BTreeMap<String, usize>) -> String {
    let available: Vec<&str> = refs.keys().map(String::as_str).collect();
    if available.is_empty() {
        ", which currently declares none".to_string()
    } else {
        format!(" (this container declares: {})", available.join(", "))
    }
}

/// Handles declared in this container, stopping at a nested container
/// that owns its own `interaction` — its handles are its own scope.
fn collect_refs(node: &Value, out: &mut BTreeMap<String, usize>) {
    match node {
        Value::Array(entries) => {
            for entry in entries {
                collect_refs(entry, out);
            }
        }
        Value::Object(map) => {
            if map.len() != 1 {
                return;
            }
            let Some((key, props)) = map.iter().next() else {
                return;
            };
            if let Some(name) = props.get("ref").and_then(Value::as_str) {
                *out.entry(name.to_string()).or_insert(0) += 1;
            }
            if key == "group"
                && props.get("interaction").is_none()
                && let Some(items) = props.get("items")
            {
                collect_refs(items, out);
            }
        }
        _ => {}
    }
}
