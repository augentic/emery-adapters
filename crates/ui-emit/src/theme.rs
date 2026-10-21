//! `tokens.json` to the Vue app's theme sheet.
//!
//! Copied from vx-emit's `theme.rs`, which fed four renderers (Swift, Kotlin, Dioxus, Vue); only
//! the CSS the Vue sheet is built from came across. A category absent from the tokens emits no
//! declarations rather than empty ones.

pub(crate) mod css;
pub mod vue;

use crate::tokens::Color;

/// Colours grouped for emission: the semantic families first, in a fixed order, then the rest.
///
/// Cosmetic. It exists so that a palette diff reads as a palette rather than as an alphabetised
/// list, which is what a designer looking at the review is trying to check.
fn groups(colors: &[Color]) -> Vec<Vec<&Color>> {
    const FAMILIES: [&str; 4] = ["primary", "secondary", "surface", "error"];

    let family = |token: &Color| {
        FAMILIES.iter().position(|family| {
            let name = token.name.strip_prefix("on-").unwrap_or(&token.name);
            name == *family || name.starts_with(&format!("{family}-"))
        })
    };

    let mut groups: Vec<Vec<&Color>> = vec![Vec::new(); FAMILIES.len() + 1];
    for token in colors {
        groups[family(token).unwrap_or(FAMILIES.len())].push(token);
    }
    groups.retain(|group| !group.is_empty());
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(name: &str) -> Color {
        Color {
            name: name.to_owned(),
            light: "#000000".to_owned(),
            dark: "#FFFFFF".to_owned(),
        }
    }

    #[test]
    fn a_palette_groups_by_family_rather_than_by_name() {
        let colors = [
            color("error"),
            color("on-primary"),
            color("text-secondary"),
            color("primary"),
            color("surface-card"),
        ];

        let grouped: Vec<Vec<&str>> = groups(&colors)
            .iter()
            .map(|group| group.iter().map(|token| token.name.as_str()).collect())
            .collect();

        assert_eq!(
            grouped,
            [
                vec!["on-primary", "primary"],
                vec!["surface-card"],
                vec!["error"],
                vec!["text-secondary"],
            ]
        );
    }
}
