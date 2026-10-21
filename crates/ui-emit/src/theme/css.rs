//! The CSS custom properties a token set declares, shared by the Vue sheet.
//!
//! Copied from vx-emit's `theme/dioxus.rs` (the Dioxus sheet the Vue one was built on): the
//! `:root` scales, the dark scheme and the `.vectis-type-*` classes. The Dioxus sheet itself, its
//! `@font-face` rules and font module did not come across.

use std::fmt::Write as _;

use super::groups;
use crate::naming::scalar;
use crate::tokens::{Theme, Typography};

/// The custom-property prefix.
///
/// `--vectis-color-surface-card` names the target, and an unprefixed `--surface-card` would collide
/// with whatever the app's own stylesheet declares.
const PREFIX: &str = "vectis";

/// Every token at its light-scheme value.
pub(super) fn root(css: &mut String, theme: &Theme) {
    css.push_str("\n:root {\n");

    if !theme.colors.is_empty() {
        css.push_str("  /* colors */\n");
        for (index, group) in groups(&theme.colors).iter().enumerate() {
            if index > 0 {
                css.push('\n');
            }
            for token in group {
                let _ = writeln!(css, "  --{PREFIX}-color-{}: {};", token.name, token.light);
            }
        }
    }

    if !theme.typefaces.is_empty() {
        css.push_str("\n  /* typefaces */\n");
        for token in &theme.typefaces {
            let _ = writeln!(
                css,
                "  --{PREFIX}-typeface-{}: {}, {};",
                token.name,
                quoted(&token.family),
                token.fallback.as_deref().unwrap_or_else(|| fallback(&token.family)),
            );
        }
    }

    scale(css, "spacing", "spacing", &theme.spacing, "px");
    scale(css, "cornerRadius", "corner-radius", &theme.corner_radius, "px");

    if !theme.border.is_empty() {
        css.push_str("\n  /* border */\n");
        for token in &theme.border {
            let _ = writeln!(css, "  --{PREFIX}-border-{}: {}px;", token.name, scalar(token.width));

            // Usually absent — Figma keeps a border's width and its colour in different places, so
            // the colour arrives per layer from `composition.yaml` and only a hand-authored border
            // states a default here. Emitted when it is stated so the default is reachable.
            if let Some(color) = &token.color {
                let _ = writeln!(
                    css,
                    "  --{PREFIX}-border-{}-color: var(--{PREFIX}-color-{color});",
                    token.name,
                );
            }
            if let Some(radius) = &token.radius {
                let _ = writeln!(
                    css,
                    "  --{PREFIX}-border-{}-radius: var(--{PREFIX}-corner-radius-{radius});",
                    token.name,
                );
            }
        }
    }

    scale(css, "opacity", "opacity", &theme.opacity, "");
    elevation(css, theme);
    box_shadows(css, theme);

    css.push_str("}\n");
}

/// `tokens.shadow` as ready `box-shadow` values, `--vectis-box-shadow-*` (apart from the
/// elevation-derived `--vectis-shadow-*`, so a shadow and an elevation may share a name).
fn box_shadows(css: &mut String, theme: &Theme) {
    if theme.shadow.is_empty() {
        return;
    }
    css.push_str("\n  /* shadow */\n");
    for token in &theme.shadow {
        let _ = writeln!(
            css,
            "  --{PREFIX}-box-shadow-{}: {}px {}px {}px {};",
            token.name,
            scalar(token.x),
            scalar(token.y),
            scalar(token.blur),
            token.color
        );
    }
}

/// One scalar category.
pub(super) fn scale(
    css: &mut String, mark: &str, prefix: &str, tokens: &[crate::tokens::Scalar], unit: &str,
) {
    if tokens.is_empty() {
        return;
    }

    let _ = write!(css, "\n  /* {mark} */\n");
    for token in tokens {
        let _ =
            writeln!(css, "  --{PREFIX}-{prefix}-{}: {}{unit};", token.name, scalar(token.value));
    }
}

/// The elevation scale, and a shadow derived from it.
///
/// The scale is mirrored as-is because that is what the token says and what `Elevation.swift`
/// keeps: a bare number. The shadow is this target's addition, and the one place in the sheet where
/// a value is not traceable to `tokens.yaml`.
///
/// It is here because CSS has no elevation primitive — a raised surface is a `box-shadow` or it is
/// nothing — so a generated view that wants to honour `elevation: overlay` has to be given
/// something to apply. The derivation is a fixed function of the token's own value rather than a
/// table of taste, and the shadow colour is stated here rather than in `tokens.yaml` for the same
/// reason a border's colour is not stated there: the design file never bound a variable to it.
fn elevation(css: &mut String, theme: &Theme) {
    if theme.elevation.is_empty() {
        return;
    }

    css.push_str("\n  /* elevation */\n");
    for token in &theme.elevation {
        let _ = writeln!(css, "  --{PREFIX}-elevation-{}: {};", token.name, scalar(token.value));
    }

    css.push_str("\n  /* shadow, derived from elevation — see theme::dioxus::elevation */\n");
    for token in &theme.elevation {
        let _ = writeln!(css, "  --{PREFIX}-shadow-{}: {};", token.name, shadow(token.value));
    }
}

/// A `box-shadow` for an elevation value.
///
/// Zero is `none` rather than a transparent shadow, so that a view applying `elevation: none` emits
/// a property the browser can skip entirely. Above zero the offset is the token's value, the blur
/// twice it, and the alpha climbs with it.
#[expect(
    clippy::suboptimal_flops,
    reason = "`mul_add` can round differently and change the emitted bytes"
)]
fn shadow(value: f64) -> String {
    if value <= 0.0 {
        return "none".to_owned();
    }

    let alpha = (10.0 + value * 2.5).min(24.0).round();
    format!("0 {}px {}px rgb(0 0 0 / {}%)", scalar(value), scalar(value * 2.0), scalar(alpha))
}

/// The colours that answer to appearance, at their dark value.
///
/// Only the ones that differ. A palette states a light and a dark value for every colour, but a
/// swatch is a swatch in both — `swatch-blue` is `#1A73E8` either way — and restating those here
/// would put a declaration in the sheet that changes nothing. What is left is exactly the set of
/// tokens the design system means to respond to appearance, which is a thing worth being able to
/// read off the file.
pub(super) fn dark(css: &mut String, theme: &Theme) {
    let answering: Vec<_> = theme.colors.iter().filter(|token| token.light != token.dark).collect();
    if answering.is_empty() {
        return;
    }

    css.push_str("\n@media (prefers-color-scheme: dark) {\n  :root {\n");
    for token in answering {
        let _ = writeln!(css, "    --{PREFIX}-color-{}: {};", token.name, token.dark);
    }
    css.push_str("  }\n}\n");
}

/// The type scale, one class per token.
///
/// A class rather than a custom property because a text style is several declarations that travel
/// together, and the whole reason `Typography.swift` emits styles rather than bare `Font`s is that
/// splitting them loses the line height and the tracking at the call site. `.vectis-type-body-large`
/// is the same idea: the generated view names the token, not its parts.
pub(super) fn type_scale(css: &mut String, theme: &Theme) {
    for token in &theme.typography {
        let _ = write!(css, "\n.{PREFIX}-type-{} {{\n", token.name);
        for declaration in declarations(token) {
            let _ = writeln!(css, "  {declaration}");
        }
        css.push_str("}\n");
    }
}

/// One text style's declarations, in the order a reader expects them.
///
/// Everything optional is omitted rather than defaulted. A style that states no weight inherits
/// one, which is what `weight: None` means and what the native emitters do with it.
fn declarations(token: &Typography) -> Vec<String> {
    let mut out = Vec::new();

    if let Some(typeface) = &token.typeface {
        out.push(format!("font-family: var(--{PREFIX}-typeface-{typeface});"));
    }
    out.push(format!("font-size: {}px;", scalar(token.size)));
    if let Some(weight) = token.weight {
        out.push(format!("font-weight: {weight};"));
    }
    if let Some(height) = token.line_height {
        out.push(format!("line-height: {}px;", scalar(height)));
    }

    // In px, because that is how `.kerning()` reads the same number in `Typography.swift` and the
    // two shells have to agree. The token is ambiguous — Figma states tracking as a percentage or
    // in points and `tokens.yaml` records the bare number either way, so `overline`'s 15 against a
    // 10px font is either 1.5px or 15px and nothing downstream can tell. Left as the native
    // emitters read it rather than reinterpreted here, where the disagreement would be silent.
    if let Some(spacing) = token.letter_spacing {
        out.push(format!("letter-spacing: {}px;", scalar(spacing)));
    }

    out
}

/// A family name, quoted when CSS needs it quoted.
///
/// An unquoted family has to be a sequence of identifiers, so `Roboto Mono` is legal bare and
/// `1st Choice` is not. Quoting only what needs it keeps the common case reading like the token.
pub(super) fn quoted(family: &str) -> String {
    let bare = family.split(' ').all(|word| {
        !word.is_empty()
            && !word.starts_with(|first: char| first.is_ascii_digit())
            && word.chars().all(|character| character.is_alphanumeric() || character == '-')
    });

    if bare { family.to_owned() } else { format!("{family:?}") }
}

/// The generic to fall back to when the family is unavailable and `typefaces.*.fallback` states
/// no stack of its own.
///
/// A guess, and a narrow one: a family whose name says it is monospaced gets the monospace stack,
/// and everything else gets the UI sans stack. It matters because a webfont that has not loaded
/// yet, or a native shell without the family installed, renders in the fallback — and the fallback
/// being proportional where the design system said monospace is visible in a code block.
pub(super) fn fallback(family: &str) -> &'static str {
    let name = family.to_ascii_lowercase();
    if name.contains("mono") || name.contains("code") || name.contains("courier") {
        "ui-monospace, monospace"
    } else {
        "system-ui, sans-serif"
    }
}
