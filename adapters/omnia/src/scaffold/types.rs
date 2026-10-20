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

// A design type's key as the design spells it: `Order`, `orders.Order`,
// or `Order (src/orders.ts#L3)` where the design keyed two declarations of
// one name by their anchors.
pub(super) struct DesignKey<'a> {
    // The stem the key leads with, where it is dotted.
    pub stem: Option<&'a str>,
    // The identifier the key declares.
    pub name: &'a str,
}

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
    // The declared field name, or the wire name where the source names one
    // alone.
    pub name: String,
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
    // Whether the wire literal is a number, so the variant's name is also
    // accepted as an alias.
    pub alias: bool,
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
    // Omitted on the wire when decorated, filled with serde's `Default`.
    Default,
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
        // Whether a field is laid over a map, so the module imports one.
        map: bool,
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
    map: bool,
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
    reads: &'a [Read],
    params: &'a [String],
    design: &'a [String],
    // The type whose fields are being laid, so a shared declared name resolves
    // to itself rather than the first slice type that declared it.
    at: usize,
}

// The scalar a field's laid type is, where it is one.
enum Scalar {
    Int,
    Float,
    Bool,
    Text,
}

// A default spelled as the value its fn answers, or as serde's own where
// the source's is an empty collection.
enum Spelled {
    Value { text: String, constant: bool },
    Default,
}

impl<'a> DesignKey<'a> {
    pub(super) fn parse(key: &'a str) -> Self {
        let dotted = key.split(" (").next().unwrap_or(key).trim();
        match dotted.rsplit_once('.') {
            Some((head, name)) => Self {
                stem: head.split('.').next(),
                name,
            },
            None => Self {
                stem: None,
                name: dotted,
            },
        }
    }
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
    for (at, (read, name)) in reads.iter().zip(names).enumerate() {
        let (body, laying) = match &read.shape {
            Shape::Opaque { is_enum } => (Body::Marker { is_enum: *is_enum }, Laying::default()),
            Shape::Enum(declared) => (declared.lay(), Laying::default()),
            Shape::Struct(declared) => {
                let refs = Refs {
                    owned: &owned,
                    reads,
                    params: &declared.generics,
                    design,
                    at,
                };
                declared.lay(name, &refs, values)
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

impl Enum {
    fn lay(&self) -> Body {
        let mut names = Names::default();
        let variants = self
            .variants
            .iter()
            .map(|variant| {
                let name = names.reserve(&pascal(&variant.member), "2");
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
                    .map(|wire| {
                        if variant.alias {
                            format!(
                                "#[serde(rename = \"{}\", alias = \"{}\")]",
                                escape(wire),
                                escape(&name)
                            )
                        } else {
                            format!("#[serde(rename = \"{}\")]", escape(wire))
                        }
                    })
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
            numeric: self.numeric,
        }
    }

    // The variant identifier `lay` claims for `member`, where the enumeration
    // declares it.
    fn variant(&self, member: &str) -> Option<String> {
        let mut names = Names::default();
        self.variants
            .iter()
            .map(|variant| (&variant.member, names.reserve(&pascal(&variant.member), "2")))
            .find(|(declared, _)| *declared == member)
            .map(|(_, name)| name)
    }
}

impl Struct {
    fn lay(&self, rust_name: &str, refs: &Refs<'_>, values: &mut Names) -> (Body, Laying) {
        let mut laying = Laying::default();
        let mut names = Names::default();
        let mut fields = Vec::new();
        let mut defaults = Vec::new();

        // the bases first, each flattened into the struct
        for base in &self.bases {
            let name = names.reserve(&snake(base), "_2");
            let mut field_laying = Laying::default();
            let ty = Type::Named(base.clone(), Vec::new()).rust(&name, refs, &mut field_laying);
            let mut docs = vec![format!("The fields of `{}`, flattened.", escape(base))];
            let carried = field_laying.carried(&mut docs);
            laying.take(field_laying);
            fields.push(Member {
                docs,
                attrs: vec!["#[serde(flatten)]".to_owned()],
                name,
                ty,
                carried,
            });
        }

        for field in &self.fields {
            let name = names.reserve(&snake(&field.name), "_2");
            let mut field_laying = Laying::default();
            let mut ty = field.ty.rust(&field.name, refs, &mut field_laying);
            let spelled = field
                .default
                .as_ref()
                .and_then(|literal| literal.spell(&field.ty, &field.name, refs));

            // a field the wire may omit with no default to stand in is an option
            if field.optional && spelled.is_none() && !field.ty.is_nullable() {
                ty = format!("Option<{ty}>");
            }
            let mut attrs = Vec::new();
            if name != field.wire {
                attrs.push(format!("#[serde(rename = \"{}\")]", escape(&field.wire)));
            }
            match spelled {
                Some(Spelled::Value { text, constant }) => {
                    let fn_name = values
                        .reserve(&format!("default_{name}"), &format!("_{}", snake(rust_name)));
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
                field
                    .doc
                    .clone()
                    .unwrap_or_else(|| format!("The `{}` field.", escape(&field.wire))),
            ];
            let carried = field_laying.carried(&mut docs);
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
                "#[expect(clippy::struct_excessive_bools, reason = \"the wire carries each \
                 flag\")]"
                    .to_owned(),
            );
        }
        let generics = if self.generics.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.generics.join(", "))
        };
        let body = Body::Struct {
            generics,
            attrs,
            fields,
            defaults,
            eq: true,
            map: laying.map,
        };
        (body, laying)
    }
}

impl Literal {
    // The default as the field's laid type takes it, or none where the
    // source's literal does not fit the type.
    fn spell(&self, ty: &Type, wire: &str, refs: &Refs<'_>) -> Option<Spelled> {
        let mut constant = true;
        let text = match (self, ty.scalar(wire)) {
            (Self::Int(digits), Some(Scalar::Int)) => grouped(digits),
            (Self::Int(digits), Some(Scalar::Float)) => format!("{}.0", grouped(digits)),
            (Self::Float(text), Some(Scalar::Float)) => {
                if text.contains(['.', 'e', 'E']) {
                    text.clone()
                } else {
                    format!("{text}.0")
                }
            }
            (Self::Bool(flag), Some(Scalar::Bool)) => flag.to_string(),
            (Self::Str(text), Some(Scalar::Text)) => {
                constant = false;
                format!("\"{}\".to_owned()", escape(text))
            }
            (Self::Member(owner, member), _) => {
                // the field's type is the owned enumeration the member belongs to
                let Type::Named(name, args) = ty else { return None };
                let (Ref::Owned(rust, at), Ref::Owned(of, _)) =
                    (refs.resolve(name), refs.resolve(owner))
                else {
                    return None;
                };
                if !args.is_empty() || rust != of {
                    return None;
                }
                format!("{rust}::{}", refs.variant(at, member)?)
            }
            (Self::Empty, _) => return ty.defaults(refs).then_some(Spelled::Default),
            (Self::Default, _) => return ty.has_default(refs).then_some(Spelled::Default),
            _ => return None,
        };
        Some(Spelled::Value { text, constant })
    }
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
        self.map |= other.map;
        self.owned.extend(other.owned);
        self.carried.extend(other.carried);
    }

    // The doc line of each name a field carries as a value, and the first
    // of them for the brief.
    fn carried(&self, docs: &mut Vec<String>) -> Option<String> {
        for carried in &self.carried {
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
        self.carried.first().map(|carried| match carried {
            Carried::Design(key) | Carried::Foreign(key) => key.clone(),
        })
    }
}

impl Refs<'_> {
    fn resolve(&self, name: &str) -> Ref<'_> {
        if self.params.iter().any(|param| param == name) {
            return Ref::Param;
        }
        if let Some((declared, rust)) = self.owned.get(self.at)
            && declared == name
        {
            return Ref::Owned(rust, self.at);
        }
        if let Some((at, (_, rust))) =
            self.owned.iter().enumerate().find(|(_, (declared, _))| declared == name)
        {
            return Ref::Owned(rust, at);
        }
        if let Some(key) = self.design.iter().find(|key| DesignKey::parse(key).name == name) {
            return Ref::Design(key);
        }
        Ref::Foreign
    }

    fn variant(&self, at: usize, member: &str) -> Option<String> {
        let Shape::Enum(declared) = &self.reads.get(at)?.shape else { return None };
        declared.variant(member)
    }
}

impl Type {
    const fn is_nullable(&self) -> bool {
        matches!(self, Self::Nullable(_))
    }

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
            Self::Map(inner) => {
                laying.map = true;
                format!("BTreeMap<String, {}>", inner.rust(wire, refs, laying))
            }
            Self::Nullable(inner) => {
                let laid = inner.rust(wire, refs, laying);
                if inner.is_nullable() { laid } else { format!("Option<{laid}>") }
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

    fn scalar(&self, wire: &str) -> Option<Scalar> {
        match self {
            Self::Int => Some(Scalar::Int),
            Self::Float => Some(Scalar::Float),
            Self::Number => Some(if counted(wire) { Scalar::Int } else { Scalar::Float }),
            Self::Bool => Some(Scalar::Bool),
            Self::Text | Self::Date => Some(Scalar::Text),
            Self::Bytes
            | Self::Any
            | Self::Array(_)
            | Self::Map(_)
            | Self::Nullable(_)
            | Self::Named(..) => None,
        }
    }

    // Whether the laid type has the `Default` serde fills an omitted field
    // with: a text, a collection, an option, or a value.
    fn defaults(&self, refs: &Refs<'_>) -> bool {
        match self {
            Self::Text
            | Self::Date
            | Self::Any
            | Self::Bytes
            | Self::Array(_)
            | Self::Map(_)
            | Self::Nullable(_) => true,
            Self::Named(name, _) => matches!(refs.resolve(name), Ref::Design(_) | Ref::Foreign),
            Self::Bool | Self::Int | Self::Float | Self::Number => false,
        }
    }

    // Whether the laid type implements `Default`, scalars included.
    fn has_default(&self, refs: &Refs<'_>) -> bool {
        matches!(self, Self::Bool | Self::Int | Self::Float | Self::Number) || self.defaults(refs)
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

    pub(super) const fn uses_map(&self) -> bool {
        matches!(self, Self::Struct { map: true, .. })
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

// A comment's lines as one doc line, ended with a full stop, each word
// spelled as an identifier in backticks, as the doc-markdown lint asks.
fn sentence(lines: &[String]) -> Option<String> {
    let joined = lines
        .iter()
        .flat_map(|line| line.split_whitespace())
        .map(backticked)
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        return None;
    }
    if joined.ends_with(['.', '!', '?']) { Some(joined) } else { Some(format!("{joined}.")) }
}

// A word in backticks where it spells an identifier: a path, a name with
// an underscore, or a camel-case name past its first letter.
fn backticked(word: &str) -> String {
    if word.contains('`') {
        return word.to_owned();
    }
    let core = word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
    if core.is_empty() {
        return word.to_owned();
    }
    let camel = core.starts_with(|ch: char| ch.is_ascii_uppercase())
        && core.chars().filter(char::is_ascii_uppercase).count() > 1
        && core.chars().filter(char::is_ascii_lowercase).count() > 1;
    let identifier = (core.contains('_') && core != "_")
        || core.contains("::")
        || camel
        || word.contains("::")
        || word.ends_with("()");
    if !identifier {
        return word.to_owned();
    }
    let start = word.find(core).unwrap_or(0);
    let end = start + core.len();
    let (lead, tail) = (&word[..start], &word[end..]);
    tail.strip_prefix("()")
        .map_or_else(|| format!("{lead}`{core}`{tail}"), |after| format!("{lead}`{core}()`{after}"))
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
// with what follows the closer; a string literal closes nothing, an
// escaped quote ends none.
fn between(text: &str, open: char, close: char) -> Option<(&str, &str)> {
    let start = text.find(open)?;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (at, ch) in text[start..].char_indices() {
        if let Some(opened) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == opened {
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
        .find(|word| is_ident(word))
        .unwrap_or_default()
        .to_owned()
}

// Whether `text` is one identifier.
fn is_ident(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
