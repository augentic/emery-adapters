//! Kebab-case names to the identifiers the Vue app uses.
//!
//! One module, because a screen slug becomes a view file, a route and an event argument, and the
//! only thing keeping those the same name is that they are all derived here.

/// `on-primary-container` becomes `onPrimaryContainer`.
#[must_use]
pub fn camel(kebab: &str) -> String {
    let mut out = String::with_capacity(kebab.len());
    let mut upper_next = false;
    for char in kebab.chars() {
        if char == '-' || char == '_' {
            upper_next = true;
            continue;
        }
        if upper_next {
            out.extend(char.to_uppercase());
            upper_next = false;
        } else {
            out.push(char);
        }
    }
    out
}

/// `corner-radius` becomes `CornerRadius`.
#[must_use]
pub fn pascal(kebab: &str) -> String {
    let camel = camel(kebab);
    let mut chars = camel.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// A token scalar, without a trailing `.0`.
///
/// `16` stays `16` and `1.5` stays `1.5`. Determinism matters more than round-tripping every
/// `f64` here: token values are authored decimals, not computed floats, and a generated file that
/// differs from the last one by `16` against `16.0` reads as a change.
#[must_use]
pub fn scalar(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{value:.0}");
    }
    let mut rendered = format!("{value}");
    if !rendered.contains('.') {
        rendered.push_str(".0");
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphens_are_where_the_words_are() {
        assert_eq!(camel("on-primary-container"), "onPrimaryContainer");
        assert_eq!(camel("primary"), "primary");
        assert_eq!(pascal("corner-radius"), "CornerRadius");
        assert_eq!(pascal(""), "");
    }

    #[test]
    fn scalars_drop_integral_fractions() {
        assert_eq!(scalar(16.0), "16");
        assert_eq!(scalar(1.5), "1.5");
        assert_eq!(scalar(0.38), "0.38");
        assert_eq!(scalar(0.0), "0");
    }
}
