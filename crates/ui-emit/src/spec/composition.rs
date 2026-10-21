//! The checks a JSON Schema cannot express.
//!
//! Everything here needs either a second document (does this token name exist in `tokens.yaml`?)
//! or a view of the tree the schema's local keyword scope does not reach (does this `ref` resolve
//! to a node inside this container?). Ported from the Vectis adapter's composition validator.

pub mod arguments;
pub mod finding;
pub mod formats;
pub mod interaction;
pub mod items;
pub mod layers;
pub mod menu;
pub mod nesting;
pub mod refs;
pub mod test_ids;
pub mod variants;
pub mod wrap;

use serde_json::Value;

pub use self::finding::Finding;
use crate::spec::Report;

/// Runs every cross-artifact check over a composition.
///
/// `tokens` and `assets` are optional because a spec may legitimately reference neither. When one
/// is absent its reference checks are skipped rather than failing every reference: the missing
/// manifest is already a finding of its own, and burying it under two hundred unresolved
/// references helps nobody.
pub fn check(
    composition: &Value, tokens: Option<&Value>, assets: Option<&Value>, report: &mut Report,
) {
    let mut errors: Vec<Finding> = Vec::new();

    if let Some(screens) = composition.get("screens") {
        test_ids::check_test_ids(screens, "/screens", &mut errors);
    }

    if let Some(tokens) = tokens {
        refs::resolve_token_references(composition, tokens, &mut errors);
    }
    if let Some(assets) = assets {
        refs::resolve_asset_references(composition, assets, &mut errors);
    }

    // The item vocabulary and the `style` closure need the document's own `custom_items` and the
    // type scale, so they cannot live in the schema.
    items::check_items(composition, tokens, &mut errors);

    // A field's type is one fact about every place it is shown, and only a text shows a formatted
    // value; the schema sees one item at a time.
    formats::check_formats(composition, &mut errors);

    // Interaction depth is structural: it needs nothing but the document itself.
    interaction::check_interactions(composition, &mut errors);

    // What an event must carry is a fact about the control raising it, and about whether it sits
    // in a row template.
    arguments::check_arguments(composition, &mut errors);

    // vx's `check_agreement` (one argument count per event name) read the behaviour vocabulary,
    // which did not come across; `contract::derive` reports the same disagreement as a `Clash`.

    // A tab's route and the screens the menu shows on are keys of `screens`, which is a fact about
    // two parts of one document.
    menu::check_footer_menu(composition, &mut errors);

    // A collection inside a row template would need a view model the row does not have.
    nesting::check_nested_collections(composition, &mut errors);

    // A layer belongs to the stack directly above it, and its offset names a second document.
    layers::check_layers(composition, tokens, &mut errors);

    // A wrapping row's cells are its children's widths, which is a fact about siblings.
    if let Some(screens) = composition.get("screens") {
        wrap::check_wrapping_rows(screens, "/screens", &mut errors);
    }

    report.errors.extend(errors);
}
