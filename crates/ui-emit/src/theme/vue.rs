//! `tokens.yaml` to `vue/src/theme.css`.
//!
//! The same custom properties the Dioxus sheet declares — `--vectis-color-*`, `--vectis-typeface-*`
//! with its fallback stack, the spacing, radius, border, opacity and elevation scales and one
//! `.vectis-type-*` class per text style — so a value is spelled the same way in both web shells.
//!
//! Two additions. `--vectis-offset-*` carries `tokens.offset`, the signed lengths a stack layer is
//! moved by, which only this shell draws. And a reset: the Vue views state every visual fact the
//! composition carries, so the user agent's own margins, button chrome and link colour are taken
//! away first rather than left to add facts the design system never stated.
//!
//! No `@font-face` rules. A typeface whose `source` is `system` is the browser's to resolve through
//! its stack, and a pinned webfont is `vx assets`' business, which has no Vue install yet.

use super::css::{dark, root, scale, type_scale};
use crate::tokens::Theme;
use crate::{GeneratedFile, HEADER};

/// Where the sheet is written, relative to the app directory.
pub const PATH: &str = "vue/src/theme.css";

/// The user-agent reset, applied before any token.
///
/// What it takes away is what a browser adds on its own: margins, padding and borders on every
/// box, the button's chrome, the link's colour and underline, and inline replaced elements sitting
/// on a text baseline. Every box is `border-box`, because the sizes `composition.yaml` states are
/// measured outer edges.
const RESET: &str = "\n*, ::before, ::after {\n  box-sizing: border-box;\n  margin: 0;\n  padding: 0;\n  border: 0 solid;\n}\n\nhtml, body {\n  margin: 0;\n  padding: 0;\n}\n\nbutton, input, select, textarea {\n  font: inherit;\n  color: inherit;\n  letter-spacing: inherit;\n  background: transparent;\n  text-align: inherit;\n}\n\nbutton {\n  cursor: pointer;\n}\n\na {\n  color: inherit;\n  text-decoration: inherit;\n}\n\nimg, svg {\n  display: block;\n}\n";

/// The theme sheet, or nothing for a token set with nothing in it.
#[must_use]
pub fn files(theme: &Theme) -> Vec<GeneratedFile> {
    let mut css = format!("/* {HEADER} */\n");
    css.push_str(RESET);

    if !theme.is_empty() {
        root(&mut css, theme);
        if !theme.offset.is_empty() {
            css.push_str("\n:root {");
            scale(&mut css, "offset", "offset", &theme.offset, "px");
            css.push_str("}\n");
        }
        dark(&mut css, theme);
        type_scale(&mut css, theme);
    }
    vec![GeneratedFile::new(PATH.to_owned(), css)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{"version": 1, "colors": {"ink": {"light": "#111111", "dark": "#111111"}}, "typefaces": {"system": {"family": "-apple-system", "source": "system", "fallback": "system-ui, Segoe UI, sans-serif"}}, "typography": {"body": {"typeface": "system", "size": 14, "lineHeight": 20}}, "spacing": {"space-8": 8}, "offset": {"offset-m15": -15, "offset-10": 10}}"##;

    fn sheet() -> String {
        let theme = Theme::parse(SAMPLE).expect("the sample parses");
        files(&theme).remove(0).body
    }

    #[test]
    fn a_negative_offset_is_a_signed_custom_property() {
        let css = sheet();
        assert!(css.contains("--vectis-offset-offset-m15: -15px;"), "{css}");
        assert!(css.contains("--vectis-offset-offset-10: 10px;"), "{css}");
    }

    #[test]
    fn a_system_typeface_keeps_its_whole_css_stack() {
        let css = sheet();
        assert!(
            css.contains(
                "--vectis-typeface-system: -apple-system, system-ui, Segoe UI, sans-serif;"
            ),
            "{css}"
        );
        assert!(!css.contains("@font-face"), "{css}");
    }

    #[test]
    fn the_sheet_shares_the_dioxus_names() {
        let css = sheet();
        assert!(css.contains("--vectis-color-ink: #111111;"), "{css}");
        assert!(css.contains("--vectis-spacing-space-8: 8px;"), "{css}");
        assert!(css.contains(".vectis-type-body {"), "{css}");
        assert!(css.contains("box-sizing: border-box;"), "{css}");
    }
}
