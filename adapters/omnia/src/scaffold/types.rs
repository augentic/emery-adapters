//! Reads a design type's signature into the fields and variants the scaffold lays for it.

mod python;
mod rust;
mod typescript;

use std::collections::BTreeSet;

use super::names::{Names, escape, pascal, snake};

// The last word of a `number` field's name that makes it an integer: a
// count, an id, a position, a size, a sum, or a whole duration.
const COUNTED: &[&str] =
    &["cents", "count", "id", "index", "page", "size", "total", "quantity", "seconds", "millis"];

// Clippy's `max-struct-bools`: a struct of more flags carries the
// expectation the lint would otherwise raise.
const MAX_BOOLS: usize = 3;

// What a signature declares, as far as the reader can lay it.
pub(super) struct Read {
    // The identifier the signature declares, where it spells one.
    pub name: Option<String>,
    pub shape: Shape,
}

pub(super) enum Shape {
    Struct(Struct),
    Enum(Enum),
    // A signature the reader cannot lay whole; the marker stands for it.
    Opaque { is_enum: bool },
}

pub(super) struct Struct {
    pub generics: Vec<String>,
    // The types the declaration extends, each flattened into it.
    pub bases: Vec<String>,
    pub fields: Vec<Field>,
}

pub(super) struct Field {
    // The name the wire carries.
    pub wire: String,
    pub ty: Type,
    // Whether the wire may omit the field.
    pub optional: bool,
    pub default: Option<Literal>,
    // The comment the source wrote above the field.
    pub doc: Option<String>,
}

pub(super) struct Enum {
    pub variants: Vec<Variant>,
    // Whether the source enumerates by number, so no variant has a wire
    // literal.
    pub numeric: bool,
}

pub(super) struct Variant {
    // The source's member name, or the literal where a union spells no
    // member.
    pub member: String,
    // The literal the wire carries.
    pub wire: Option<String>,
    pub doc: Option<String>,
}

// A field's default as the source spells it.
pub(super) enum Literal {
    Int(String),
    Float(String),
    Bool(bool),
    Str(String),
    // An empty collection, or a factory for one.
    Empty,
    // A member of an enumeration, `Status.PENDING`.
    Member(String, String),
}

// A type expression, the sum of both languages' spellings.
pub(super) enum Type {
    Text,
    Bool,
    Int,
    Float,
    // TypeScript's `number`, an integer or a float by the field's name.
    Number,
    Bytes,
    Date,
    Any,
    Array(Box<Self>),
    Map(Box<Self>),
    Nullable(Box<Self>),
    Named(String, Vec<Self>),
}

// What the scaffold lays for one design type.
pub(super) enum Body {
    Struct {
        generics: String,
        attrs: Vec<String>,
        fields: Vec<Member>,
        defaults: Vec<DefaultFn>,
        eq: bool,
    },
    Enum {
        variants: Vec<Member>,
        numeric: bool,
    },
    Marker {
        is_enum: bool,
    },
}

// A laid field or variant: its docs, its attributes, its name, its type
// (none for a variant), and the design or source type it carries as a
// value where the slice does not own it.
pub(super) struct Member {
    pub docs: Vec<String>,
    pub attrs: Vec<String>,
    pub name: String,
    pub ty: String,
    pub carried: Option<String>,
}

// The fn a `#[serde(default = "..")]` names, beside the struct.
pub(super) struct DefaultFn {
    pub name: String,
    pub ret: String,
    pub value: String,
    // Whether the value is a constant expression, so the fn is `const`.
    pub constant: bool,
}

// What laying a type's fields needed: a float among them, a map, the owned
// types referenced, and the names carried as values.
#[derive(Default)]
struct Laying {
    float: bool,
    owned: BTreeSet<usize>,
    carried: Vec<Carried>,
}

// A type a field carries as a value: a design type another slice declares,
// by its key, or a source type no design type declares, by its name.
enum Carried {
    Design(String),
    Foreign(String),
}

// What a type name resolves to when a field is laid.
enum Ref<'a> {
    Owned(&'a str, usize),
    Param,
    Design(&'a str),
    Foreign,
}

struct Refs<'a> {
    // Each owned type's declared name and its Rust name.
    owned: &'a [(String, String)],
    params: &'a [String],
    design: &'a [String],
}

pub(super) fn read(signature: &str) -> Read {
    typescript::read(signature)
        .or_else(|| python::read(signature))
        .or_else(|| rust::read(signature))
        .unwrap_or(Read {
            name: None,
            shape: Shape::Opaque { is_enum: false },
        })
}

// The key of every type the design declares, data and mechanism alike.
pub(super) fn design_keys(design: &str) -> Vec<String> {
    design
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Type: "))
        .map(|key| key.trim_end_matches(" (mechanism)").trim().to_owned())
        .collect()
}

// The bodies of the slice's types, each named as `names` has it, the
// fields referencing one another by those names and `Eq` dropped wherever
// a float reaches.
pub(super) fn lay(
    reads: &[Read], names: &[String], design: &[String], values: &mut Names,
) -> Vec<Body> {
    let owned: Vec<(String, String)> = reads
        .iter()
        .zip(names)
        .map(|(read, name)| (read.name.clone().unwrap_or_else(|| name.clone()), name.clone()))
        .collect();
    let mut bodies = Vec::new();
    let mut floats = Vec::new();
    let mut references = Vec::new();
    for (read, name) in reads.iter().zip(names) {
        let (body, laying) = match &read.shape {
            Shape::Opaque { is_enum } => (Body::Marker { is_enum: *is_enum }, Laying::default()),
            Shape::Enum(declared) => (enumeration(declared), Laying::default()),
            Shape::Struct(declared) => {
                let refs = Refs {
                    owned: &owned,
                    params: &declared.generics,
                    design,
                };
                structure(declared, name, &refs, values)
            }
        };
        bodies.push(body);
        floats.push(laying.float);
        references.push(laying.owned);
    }

    // a type holding a float, or a type that holds one, has no `Eq`
    let mut eq = vec![true; bodies.len()];
    loop {
        let mut moved = false;
        for at in 0..bodies.len() {
            let has = !floats[at] && references[at].iter().all(|other| eq[*other]);
            if eq[at] != has {
                eq[at] = has;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    for (body, has) in bodies.iter_mut().zip(eq) {
        if let Body::Struct { eq, .. } = body {
            *eq = has;
        }
    }
    bodies
}

fn enumeration(declared: &Enum) -> Body {
    let mut claims = Names::default();
    let variants = declared
        .variants
        .iter()
        .map(|variant| {
            let name = claims.claim(&pascal(&variant.member), "2");
            let doc = variant.doc.clone().unwrap_or_else(|| {
                format!(
                    "The `{}` variant.",
                    escape(variant.wire.as_ref().unwrap_or(&variant.member))
                )
            });
            let attrs = variant
                .wire
                .as_ref()
                .filter(|wire| **wire != name)
                .map(|wire| format!("#[serde(rename = \"{}\")]", escape(wire)))
                .into_iter()
                .collect();
            Member {
                docs: vec![doc],
                attrs,
                name,
                ty: String::new(),
                carried: None,
            }
        })
        .collect();
    Body::Enum {
        variants,
        numeric: declared.numeric,
    }
}

fn structure(
    declared: &Struct, rust_name: &str, refs: &Refs<'_>, values: &mut Names,
) -> (Body, Laying) {
    let mut laying = Laying::default();
    let mut claims = Names::default();
    let mut fields = Vec::new();
    let mut defaults = Vec::new();

    // the bases first, each flattened into the struct
    for base in &declared.bases {
        let name = claims.claim(&snake(base), "_2");
        let mut field_laying = Laying::default();
        let ty = Type::Named(base.clone(), Vec::new()).rust(&name, refs, &mut field_laying);
        let mut docs = vec![format!("The fields of `{}`, flattened.", escape(base))];
        let carried = carried(&field_laying, &mut docs);
        laying.take(field_laying);
        fields.push(Member {
            docs,
            attrs: vec!["#[serde(flatten)]".to_owned()],
            name,
            ty,
            carried,
        });
    }

    for field in &declared.fields {
        let name = claims.claim(&snake(&field.wire), "_2");
        let mut field_laying = Laying::default();
        let mut ty = field.ty.rust(&field.wire, refs, &mut field_laying);
        let spelled = field.default.as_ref().and_then(|literal| spell(literal, &ty, refs));

        // a field the wire may omit with no default to stand in is an option
        if field.optional && spelled.is_none() && !ty.starts_with("Option<") {
            ty = format!("Option<{ty}>");
        }
        let mut attrs = Vec::new();
        if name != field.wire {
            attrs.push(format!("#[serde(rename = \"{}\")]", escape(&field.wire)));
        }
        match spelled {
            Some(Spelled::Value { text, constant }) => {
                let fn_name =
                    values.claim(&format!("default_{name}"), &format!("_{}", snake(rust_name)));
                attrs.push(format!("#[serde(default = \"{fn_name}\")]"));
                defaults.push(DefaultFn {
                    name: fn_name,
                    ret: ty.clone(),
                    value: text,
                    constant,
                });
            }
            Some(Spelled::Default) => attrs.push("#[serde(default)]".to_owned()),
            None => {}
        }
        let mut docs = vec![
            field.doc.clone().unwrap_or_else(|| format!("The `{}` field.", escape(&field.wire))),
        ];
        let carried = carried(&field_laying, &mut docs);
        laying.take(field_laying);
        fields.push(Member {
            docs,
            attrs,
            name,
            ty,
            carried,
        });
    }

    let mut attrs = Vec::new();
    if fields.iter().filter(|field| field.ty == "bool").count() > MAX_BOOLS {
        attrs.push(
            "#[expect(clippy::struct_excessive_bools, reason = \"the wire carries each flag\")]"
                .to_owned(),
        );
    }
    let generics = if declared.generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", declared.generics.join(", "))
    };
    let body = Body::Struct {
        generics,
        attrs,
        fields,
        defaults,
        eq: true,
    };
    (body, laying)
}

// The doc line of each name a field carries as a value, and the first of
// them for the brief.
fn carried(laying: &Laying, docs: &mut Vec<String>) -> Option<String> {
    for carried in &laying.carried {
        docs.push(match carried {
            Carried::Design(key) => format!(
                "Carries the design's `{}`, declared by another slice, as a value.",
                escape(key)
            ),
            Carried::Foreign(name) => format!(
                "Carries the source's `{}`, which no design type declares, as a value.",
                escape(name)
            ),
        });
    }
    laying.carried.first().map(|carried| match carried {
        Carried::Design(key) | Carried::Foreign(key) => key.clone(),
    })
}

// A default spelled as the value its fn answers, or as serde's own where
// the source's is an empty collection.
enum Spelled {
    Value { text: String, constant: bool },
    Default,
}

fn spell(literal: &Literal, ty: &str, refs: &Refs<'_>) -> Option<Spelled> {
    let mut constant = true;
    let text = match (literal, ty) {
        (Literal::Int(digits), "i64") => grouped(digits),
        (Literal::Int(digits), "f64") => format!("{}.0", grouped(digits)),
        (Literal::Float(text), "f64") => {
            if text.contains(['.', 'e', 'E']) {
                text.clone()
            } else {
                format!("{text}.0")
            }
        }
        (Literal::Bool(flag), "bool") => flag.to_string(),
        (Literal::Str(text), "String") => {
            constant = false;
            format!("\"{}\".to_owned()", escape(text))
        }
        (Literal::Member(owner, member), _) => {
            let Ref::Owned(rust, _) = refs.resolve(owner) else { return None };
            if rust != ty {
                return None;
            }
            format!("{rust}::{}", pascal(member))
        }
        (Literal::Empty, _) => {
            let defaulted = ty.starts_with("Vec<")
                || ty.starts_with("BTreeMap<")
                || ty.starts_with("Option<")
                || matches!(ty, "String" | "serde_json::Value");
            return defaulted.then_some(Spelled::Default);
        }
        _ => return None,
    };
    Some(Spelled::Value { text, constant })
}

// An integer literal with a separator every three digits past four, as
// the readable-literal lint asks.
fn grouped(digits: &str) -> String {
    let (sign, digits) = digits.strip_prefix('-').map_or(("", digits), |rest| ("-", rest));
    if digits.len() <= 4 {
        return format!("{sign}{digits}");
    }
    let mut out = String::new();
    for (at, ch) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at) % 3 == 0 {
            out.push('_');
        }
        out.push(ch);
    }
    format!("{sign}{out}")
}

impl Laying {
    fn take(&mut self, other: Self) {
        self.float |= other.float;
        self.owned.extend(other.owned);
        self.carried.extend(other.carried);
    }
}

impl Refs<'_> {
    fn resolve(&self, name: &str) -> Ref<'_> {
        if self.params.iter().any(|param| param == name) {
            return Ref::Param;
        }
        if let Some((at, (_, rust))) =
            self.owned.iter().enumerate().find(|(_, (declared, _))| declared == name)
        {
            return Ref::Owned(rust, at);
        }
        if let Some(key) = self.design.iter().find(|key| declared_of(key) == name) {
            return Ref::Design(key);
        }
        Ref::Foreign
    }
}

impl Type {
    // The Rust type of the field `wire` names, noting what laying it needed.
    fn rust(&self, wire: &str, refs: &Refs<'_>, laying: &mut Laying) -> String {
        match self {
            Self::Text | Self::Date => "String".to_owned(),
            Self::Bool => "bool".to_owned(),
            Self::Int => "i64".to_owned(),
            Self::Float => {
                laying.float = true;
                "f64".to_owned()
            }
            Self::Number => {
                if counted(wire) {
                    "i64".to_owned()
                } else {
                    laying.float = true;
                    "f64".to_owned()
                }
            }
            Self::Bytes => "Vec<u8>".to_owned(),
            Self::Any => "serde_json::Value".to_owned(),
            Self::Array(inner) => format!("Vec<{}>", inner.rust(wire, refs, laying)),
            Self::Map(inner) => format!("BTreeMap<String, {}>", inner.rust(wire, refs, laying)),
            Self::Nullable(inner) => {
                let inner = inner.rust(wire, refs, laying);
                if inner.starts_with("Option<") { inner } else { format!("Option<{inner}>") }
            }
            Self::Named(name, args) => match refs.resolve(name) {
                Ref::Owned(rust, at) => {
                    laying.owned.insert(at);
                    if args.is_empty() {
                        rust.to_owned()
                    } else {
                        let args: Vec<String> =
                            args.iter().map(|arg| arg.rust(wire, refs, laying)).collect();
                        format!("{rust}<{}>", args.join(", "))
                    }
                }
                Ref::Param => name.clone(),
                Ref::Design(key) => {
                    laying.carried.push(Carried::Design(key.to_owned()));
                    "serde_json::Value".to_owned()
                }
                Ref::Foreign => {
                    laying.carried.push(Carried::Foreign(name.clone()));
                    "serde_json::Value".to_owned()
                }
            },
        }
    }
}

impl Body {
    pub(super) const fn is_enum(&self) -> bool {
        match self {
            Self::Enum { .. } => true,
            Self::Marker { is_enum } => *is_enum,
            Self::Struct { .. } => false,
        }
    }

    pub(super) const fn is_marker(&self) -> bool {
        matches!(self, Self::Marker { .. })
    }

    pub(super) fn uses_map(&self) -> bool {
        match self {
            Self::Struct { fields, .. } => {
                fields.iter().any(|field| field.ty.contains("BTreeMap<"))
            }
            Self::Enum { .. } | Self::Marker { .. } => false,
        }
    }

    // The fields carried as values for a type the slice does not own, each
    // with the name it stands for.
    pub(super) fn carried(&self) -> impl Iterator<Item = (&str, &str)> {
        let fields: &[Member] = match self {
            Self::Struct { fields, .. } => fields,
            Self::Enum { .. } | Self::Marker { .. } => &[],
        };
        fields.iter().filter_map(|field| Some((field.name.as_str(), field.carried.as_deref()?)))
    }
}

// Whether a `number` field is an integer, by its name's last word.
fn counted(wire: &str) -> bool {
    snake(wire).rsplit('_').next().is_some_and(|word| COUNTED.contains(&word))
}

// The identifier a design type's key declares: `Order` for `orders.Order`
// and for `Order (src/orders.ts#L3)`.
fn declared_of(key: &str) -> &str {
    let key = key.split(" (").next().unwrap_or(key);
    key.rsplit('.').next().unwrap_or(key)
}

// `text` split at each `sep` outside brackets, parentheses, braces, angle
// brackets, and string literals; the `>` of a `=>` closes nothing.
fn split_top(text: &str, sep: char) -> impl Iterator<Item = &str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    let mut escaped = false;
    let mut previous = ' ';
    for (at, ch) in text.char_indices() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
        } else {
            match ch {
                '"' | '\'' | '`' => quote = Some(ch),
                '(' | '[' | '{' | '<' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                '>' if previous != '=' => depth -= 1,
                _ if ch == sep && depth <= 0 => {
                    parts.push(&text[start..at]);
                    start = at + ch.len_utf8();
                }
                _ => {}
            }
        }
        previous = ch;
    }
    parts.push(&text[start..]);
    parts.into_iter()
}

// The text between the first `open` of `text` and its matching `close`,
// with what follows the closer.
fn between(text: &str, open: char, close: char) -> Option<(&str, &str)> {
    let start = text.find(open)?;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (at, ch) in text[start..].char_indices() {
        if let Some(opened) = quote {
            if ch == opened {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '"' | '\'' | '`') {
            quote = Some(ch);
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                let end = start + at;
                return Some((
                    &text[start + open.len_utf8()..end],
                    &text[end + close.len_utf8()..],
                ));
            }
        }
    }
    None
}

// The quoted text of a string literal, `"pending"` or `'pending'`.
fn literal(text: &str) -> Option<&str> {
    let text = text.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = text.strip_prefix(quote).and_then(|rest| rest.strip_suffix(quote))
            && !inner.contains(quote)
        {
            return Some(inner);
        }
    }
    None
}

// A number as the source spells it, its separators dropped: its digits, or
// its digits and point.
fn number(text: &str) -> Option<Literal> {
    let text: String =
        text.trim().trim_end_matches(['L', 'n']).chars().filter(|ch| *ch != '_').collect();
    let unsigned = text.strip_prefix('-').unwrap_or(&text);
    if unsigned.is_empty() || !unsigned.starts_with(|ch: char| ch.is_ascii_digit()) {
        return None;
    }
    if unsigned.bytes().all(|byte| byte.is_ascii_digit()) {
        return Some(Literal::Int(text.clone()));
    }
    unsigned
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'-' | b'+'))
        .then(|| Literal::Float(text.clone()))
}

// The name a type parameter declares: `T` from `T extends Foo = Bar`.
fn param_name(text: &str) -> String {
    text.trim()
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .find(|word| !word.is_empty())
        .unwrap_or_default()
        .to_owned()
}

// Whether `text` is one identifier.
fn is_ident(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
