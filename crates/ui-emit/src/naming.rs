//! Kebab-case names to platform identifiers.
//!
//! One module rather than a helper per emitter, because the spellings have to agree across four
//! languages. A screen slug becomes a Rust struct, a Swift struct, a Kotlin composable and a test
//! id, and the only thing keeping those four the same name is that they are all derived here.

/// Swift *reserved* words — illegal as a declaration name without backticks.
///
/// Contextual keywords (`set`, `open`, `required`) are legal declaration names and deliberately
/// absent: backticking one would be correct but noisy, and noise in generated code is read as a
/// bug in the generator.
const SWIFT_KEYWORDS: &[&str] = &[
    "Any",
    "Self",
    "as",
    "associatedtype",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "continue",
    "default",
    "defer",
    "deinit",
    "do",
    "else",
    "enum",
    "extension",
    "fallthrough",
    "false",
    "fileprivate",
    "for",
    "func",
    "guard",
    "if",
    "import",
    "in",
    "init",
    "inout",
    "internal",
    "is",
    "let",
    "nil",
    "operator",
    "precedencegroup",
    "private",
    "protocol",
    "public",
    "repeat",
    "rethrows",
    "return",
    "self",
    "static",
    "struct",
    "subscript",
    "super",
    "switch",
    "throw",
    "throws",
    "true",
    "try",
    "typealias",
    "var",
    "where",
    "while",
];

/// Kotlin hard keywords, which must be backticked wherever they appear as a name.
const KOTLIN_KEYWORDS: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
];

/// Rust keywords, which take an `r#` prefix rather than backticks.
///
/// `crate`, `self`, `super` and `Self` are absent: they cannot be raw identifiers at all, so a
/// field named one of them is a design-system problem rather than a spelling one.
const RUST_KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "do", "dyn",
    "else", "enum", "extern", "false", "final", "fn", "for", "if", "impl", "in", "let", "loop",
    "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref", "return", "static",
    "struct", "trait", "true", "type", "typeof", "unsafe", "unsized", "use", "virtual", "where",
    "while", "yield",
];

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

/// `button` becomes `ButtonComponent`, on every platform.
///
/// The suffix is not decoration. A design system names its components after what they are — Button,
/// Text Field, Toggle, Card — and those are the names the UI frameworks already use, so an unsuffixed
/// `Button` shadows `SwiftUI`'s throughout the module and is ambiguous against
/// `androidx.compose.material3.Button` under the `material3.*` wildcard. The generated views then
/// fail to compile on their own output, and the errors land in whichever *hand-written* file called
/// the framework type — so they read as a bug in that file rather than in the emitter, and neither
/// the ownership guard nor a model can tell the difference.
///
/// `Component` rather than `View`, which reads better and is taken: `{slug}View` is the view-model
/// namespace, so a component and a screen sharing a slug would collide there instead.
#[must_use]
pub fn component_type(slug: &str) -> String {
    format!("{}Component", pascal(slug))
}

/// `taskList` becomes `task_list`.
///
/// The inverse of [`camel`], for the Rust side: a bind path is authored in whichever case its
/// author reached for, and a Rust field is snake case whatever that was.
#[must_use]
pub fn snake(mixed: &str) -> String {
    let mut out = String::with_capacity(mixed.len() + 4);
    for (index, char) in mixed.chars().enumerate() {
        if char == '-' || char == ' ' {
            out.push('_');
        } else if char.is_ascii_uppercase() {
            if index > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(char.to_lowercase());
        } else {
            out.push(char);
        }
    }
    out
}

/// `task-list-row` becomes `TASK_LIST_ROW`.
#[must_use]
pub fn upper_snake(kebab: &str) -> String {
    snake(kebab).to_uppercase()
}

/// A Swift member name: camel case, backticked when it collides with a keyword.
#[must_use]
pub fn swift_member(name: &str) -> String {
    quote(&camel(name), SWIFT_KEYWORDS, "`", "`")
}

/// A Kotlin member name: camel case, backticked when it collides with a hard keyword.
#[must_use]
pub fn kotlin_member(name: &str) -> String {
    quote(&camel(name), KOTLIN_KEYWORDS, "`", "`")
}

/// A Rust field name: snake case, `r#`-prefixed when it collides with a keyword.
#[must_use]
pub fn rust_field(name: &str) -> String {
    quote(&snake(name), RUST_KEYWORDS, "r#", "")
}

/// A Rust enum variant or type name.
#[must_use]
pub fn rust_type(name: &str) -> String {
    pascal(name)
}

/// A core `Event` variant as Swift spells it.
///
/// Rust's `EditTask` crosses the FFI boundary as `.editTask`. Only the leading character changes:
/// the rest of the name is the core's and is not ours to re-spell.
#[must_use]
pub fn swift_case(variant: &str) -> String {
    let mut chars = variant.chars();
    chars.next().map_or_else(String::new, |first| first.to_lowercase().chain(chars).collect())
}

/// The contract's Rust variant for one value of a bound variant: `now` is `Now`.
///
/// The FFI writes a Swift case as the variant in lower camel case and escapes nothing, so a value
/// whose case would be a Swift keyword — weather's `state=default` — takes the axis after it:
/// `DefaultState`, which is `.defaultState`. Kotlin's entries are upper case and need no such care.
#[must_use]
pub fn look_variant(axis: &str, value: &str) -> String {
    let variant = pascal(value);
    if SWIFT_KEYWORDS.contains(&swift_case(&variant).as_str()) {
        format!("{variant}{}", pascal(axis))
    } else {
        variant
    }
}

/// A variant of an all-unit core enum — `Route`, say — as Kotlin spells it.
///
/// Mirrors `facet_generate` 0.17.2, the generator behind `crux_core` 0.19's
/// `type_generation::facet::CodeGenerator::kotlin`: an enum whose variants are all units becomes an
/// `enum class`, and each entry is written as the Rust variant's name passed through
/// `str::to_uppercase` (`generation/kotlin/emitter/mod.rs`, the `VariantFormat::Unit` /
/// `VariantContext::EnumClass` arm). Nothing marks where the words were, so `AddCity` is `ADDCITY`
/// rather than `ADD_CITY`, `Step2` is `STEP2` and `HttpError` is `HTTPERROR`. A kebab id is taken
/// to its Rust variant with [`pascal`] first.
///
/// An enum with any payload variant is a `sealed interface` instead, whose members keep the Rust
/// name unchanged — which is why `Event.EditTask` needs nothing from here.
#[must_use]
pub fn kotlin_enum_entry(variant: &str) -> String {
    variant.to_uppercase()
}

fn quote(identifier: &str, keywords: &[&str], open: &str, close: &str) -> String {
    if keywords.contains(&identifier) {
        format!("{open}{identifier}{close}")
    } else {
        identifier.to_owned()
    }
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
    fn snake_case_reads_both_spellings() {
        assert_eq!(snake("taskList"), "task_list");
        assert_eq!(snake("task-list"), "task_list");
        assert_eq!(snake("task_list"), "task_list");
        assert_eq!(snake("URL"), "u_r_l");
        assert_eq!(upper_snake("task-list-row"), "TASK_LIST_ROW");
    }

    /// The facet Kotlin generator upper-cases the Rust variant and adds no separator, so neither a
    /// word boundary, a digit nor an acronym leaves a mark.
    #[test]
    fn a_kotlin_enum_entry_is_the_rust_variant_upper_cased_without_separators() {
        assert_eq!(kotlin_enum_entry("Today"), "TODAY");
        assert_eq!(kotlin_enum_entry("AddCity"), "ADDCITY");
        assert_eq!(kotlin_enum_entry(&pascal("location-access")), "LOCATIONACCESS");
        assert_eq!(kotlin_enum_entry(&pascal("step-2")), "STEP2");
        assert_eq!(kotlin_enum_entry(&pascal("step-2-review")), "STEP2REVIEW");
        assert_eq!(kotlin_enum_entry("HTTPError"), "HTTPERROR");
        assert_eq!(kotlin_enum_entry(&pascal("http-error")), "HTTPERROR");
    }

    #[test]
    fn a_name_that_is_a_keyword_is_quoted_the_way_its_language_quotes_one() {
        assert_eq!(swift_member("default"), "`default`");
        assert_eq!(kotlin_member("object"), "`object`");
        assert_eq!(rust_field("type"), "r#type");

        assert_eq!(swift_member("subtle"), "subtle");
        assert_eq!(kotlin_member("none"), "none");
        assert_eq!(rust_field("title"), "title");
    }

    #[test]
    fn an_event_keeps_its_name_across_the_boundary() {
        assert_eq!(swift_case("EditTask"), "editTask");
        assert_eq!(swift_case("ClearCompleted"), "clearCompleted");
        assert_eq!(swift_case(""), "");
    }

    #[test]
    fn scalars_drop_integral_fractions() {
        assert_eq!(scalar(16.0), "16");
        assert_eq!(scalar(1.5), "1.5");
        assert_eq!(scalar(0.38), "0.38");
        assert_eq!(scalar(0.0), "0");
    }
}
