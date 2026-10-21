//! `footer_menu` against the screens it names.
//!
//! The schema can say a route is kebab-case; it cannot say it is a screen. A route that names no
//! screen is a tab the router has nowhere to send, and a `screens` entry that names none is a menu
//! shown on a screen that does not exist — both are checked here, against `screens` in the same
//! document. Preflight checks the same thing against the design file (`unresolvable-tab`); this is
//! the half that also catches a hand-edited composition. The icon is checked with every other asset
//! reference, in [`crate::spec::assets::collect_asset_references`].

use std::collections::BTreeSet;

use serde_json::Value;

use super::finding::Finding;

/// Runs the footer menu's cross-checks, when the composition has one.
pub(crate) fn check_footer_menu(composition: &Value, errors: &mut Vec<Finding>) {
    let Some(menu) = composition.get("footer_menu") else { return };
    let screens: BTreeSet<&str> = composition
        .get("screens")
        .and_then(Value::as_object)
        .map(|screens| screens.keys().map(String::as_str).collect())
        .unwrap_or_default();

    for (index, screen) in strings(menu.get("screens")) {
        if !screens.contains(screen) {
            errors.push(Finding::new(
                format!("/footer_menu/screens/{index}"),
                format!(
                    "footer_menu is shown on `{screen}`, which is not a key under `screens` -- \
                     every screen the menu shows on must be one the app has"
                ),
            ));
        }
    }

    let shown: BTreeSet<&str> = strings(menu.get("screens")).map(|(_, screen)| screen).collect();
    let tab_routes: BTreeSet<&str> = menu
        .get("tabs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tab| tab.get("route").and_then(Value::as_str))
        .collect();
    for (screen, tab) in menu.get("selected").and_then(Value::as_object).into_iter().flatten() {
        let path =
            format!("/footer_menu/selected/{}", crate::spec::shared::escape_pointer_token(screen));
        if !shown.contains(screen.as_str()) {
            errors.push(Finding::new(
                &path,
                format!(
                    "footer_menu says `{screen}` shows a tab selected, but `{screen}` is not one of \
                     the menu's `screens` -- only a screen that draws the menu can select a tab"
                ),
            ));
        }
        if let Some(tab) = tab.as_str()
            && !tab_routes.contains(tab)
        {
            errors.push(Finding::new(
                &path,
                format!(
                    "footer_menu says `{screen}` shows `{tab}` selected, which is no tab's route -- \
                     a selected tab is one of the menu's tabs"
                ),
            ));
        }
    }

    let mut routes = BTreeSet::new();
    for (index, tab) in menu.get("tabs").and_then(Value::as_array).into_iter().flatten().enumerate()
    {
        let Some(route) = tab.get("route").and_then(Value::as_str) else { continue };
        let path = format!("/footer_menu/tabs/{index}/route");
        if !screens.contains(route) {
            errors.push(Finding::new(
                &path,
                format!(
                    "footer_menu tab routes to `{route}`, which is not a key under `screens` -- a \
                     tab opens a screen the app has"
                ),
            ));
        }
        if !routes.insert(route) {
            errors.push(Finding::new(
                &path,
                format!("footer_menu has a second tab routing to `{route}` -- one tab per screen"),
            ));
        }
    }
}

/// Every string in an optional array, with its index.
fn strings(value: Option<&Value>) -> impl Iterator<Item = (usize, &str)> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, value)| value.as_str().map(|text| (index, text)))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::check_footer_menu;

    fn findings(composition: &Value) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        check_footer_menu(composition, &mut errors);
        errors.into_iter().map(|f| (f.path().to_owned(), f.message().to_owned())).collect()
    }

    fn composition(screens: &[&str], routes: &[&str]) -> Value {
        json!({
            "version": 1,
            "footer_menu": {
                "event": "Navigate",
                "screens": screens,
                "tabs": routes.iter().map(|route| json!({ "route": route })).collect::<Vec<_>>(),
            },
            "screens": { "today": {}, "cities": {}, "settings": {} },
        })
    }

    #[test]
    fn a_menu_whose_routes_and_screens_are_screens_passes() {
        let clean = composition(&["today", "cities"], &["today", "cities", "settings"]);
        assert!(findings(&clean).is_empty(), "{:?}", findings(&clean));
    }

    #[test]
    fn a_route_that_is_not_a_screen_is_refused() {
        let found = findings(&composition(&["today"], &["today", "nav-cities"]));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "/footer_menu/tabs/1/route");
        assert!(found[0].1.contains("`nav-cities`"), "{}", found[0].1);
    }

    #[test]
    fn a_menu_shown_on_a_screen_that_does_not_exist_is_refused() {
        let found = findings(&composition(&["today", "hourly"], &["today"]));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, "/footer_menu/screens/1");
    }

    #[test]
    fn two_tabs_to_one_screen_are_refused() {
        let found = findings(&composition(&["today"], &["today", "today"]));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("second tab"), "{}", found[0].1);
    }

    /// `hourly` is no tab and still shows `today` selected; what is checked is that the key draws
    /// the menu and the value is a tab.
    #[test]
    fn a_selected_tab_is_a_tab_on_a_screen_that_draws_the_menu() {
        let mut menu = composition(&["today", "cities"], &["today", "cities"]);
        menu["screens"]["hourly"] = json!({});
        menu["footer_menu"]["screens"] = json!(["today", "hourly", "cities"]);
        menu["footer_menu"]["selected"] = json!({ "today": "today", "hourly": "today" });
        assert!(findings(&menu).is_empty(), "{:?}", findings(&menu));

        menu["footer_menu"]["selected"] = json!({ "settings": "today", "cities": "settings" });
        let found = findings(&menu);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().any(|(path, message)| path == "/footer_menu/selected/settings"
            && message.contains("not one of the menu's `screens`")));
        assert!(found.iter().any(|(path, message)| path == "/footer_menu/selected/cities"
            && message.contains("no tab's route")));
    }

    #[test]
    fn a_composition_without_a_menu_has_nothing_to_check() {
        assert!(findings(&json!({ "version": 1, "screens": {} })).is_empty());
    }
}
