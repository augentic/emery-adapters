//! Reads a design type's signature into the fields and variants the scaffold lays for it.

mod python;
mod rust;
mod typescript;

use std::collections::{BTreeMap, BTreeSet};

use super::names::{Names, escape, pascal, snake};
use super::reference::blocks;

// The last word of a `number` field's name that makes it an integer: a
// count, an id, a position, a size, a sum, or a whole measure.
const COUNTED: &[&str] = &[
    "cents", "count", "id", "index", "page", "size", "total", "quantity", "seconds", "millis",
    "grams", "days", "ms", "bytes", "units",
];

// The SQL column types a schema in the design declares a whole number by.
const SQL_INTEGERS: &[&str] = &["integer", "int", "bigint", "smallint", "serial", "bigserial"];

// The SQL column types a schema in the design declares a fraction by.
const SQL_FLOATS: &[&str] =
    &["numeric", "decimal", "real", "double", "float", "float4", "float8", "money"];

// How many words either side of `integer` a prose sentence names the field in.
const INTEGER_REACH: usize = 3;

// Clippy's `max-struct-bools`: a struct of more flags carries the
// expectation the lint would otherwise raise.
const MAX_BOOLS: usize = 3;

// A design type's key as the design spells it: `Order`, `orders.Order`,
// or `Order (src/orders.ts#L3)` where the design keyed two declarations of
// one name by their anchors.
pub struct DesignKey<'a> {
    // The stem the key leads with, where it is dotted.
    pub stem: Option<&'a str>,
    // The identifier the key declares.
    pub name: &'a str,
}

// What a signature declares, as far as the reader can lay it.
pub struct Parsed {
    // The identifier the signature declares, where it spells one.
    pub name: Option<String>,
    pub shape: Shape,
}

pub enum Shape {
    Struct(Struct),
    Enum(Enum),
    Table(Table),
    // A signature the reader cannot lay whole; the marker stands for it.
    Opaque { is_enum: bool },
}

// A constant array of object literals: a reference table the code reads,
// each row the scalars it spells under the wire's keys.
pub struct Table {
    // The declared name of the element type each row is one of.
    pub element: String,
    pub rows: Vec<Vec<(String, Literal)>>,
}

pub struct Struct {
    pub generics: Vec<String>,
    // The types the declaration extends, each flattened into it.
    pub bases: Vec<String>,
    pub fields: Vec<Field>,
}

pub struct Field {
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

pub struct Enum {
    pub variants: Vec<Variant>,
    // Whether the source enumerates by number, so no variant has a wire
    // literal.
    pub numeric: bool,
}

pub struct Variant {
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
pub enum Literal {
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
pub enum Type {
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
pub enum Body {
    Struct {
        generics: String,
        attrs: Vec<String>,
        fields: Vec<LaidField>,
        defaults: Vec<DefaultFn>,
        eq: bool,
        // Whether a field is laid over a map, so the module imports one.
        map: bool,
    },
    Enum {
        variants: Vec<LaidVariant>,
        numeric: bool,
    },
    // A reference table: the rows as the JSON constant of a data module of
    // the stem's crate, and the fn reading them into the element type,
    // since the seed's lints refuse a file compiled in.
    Table {
        element: String,
        // The data module's name.
        module: String,
        json: String,
    },
    Marker {
        is_enum: bool,
    },
}

// A laid field: its docs, its attributes, its name, its type, and the
// design or source type it carries as a value where the slice does not
// own it.
pub struct LaidField {
    pub docs: Vec<String>,
    pub attrs: Vec<String>,
    pub name: String,
    pub ty: String,
    pub carried: Option<String>,
}

// A laid variant: its docs, its attributes, and its name.
pub struct LaidVariant {
    pub docs: Vec<String>,
    pub attrs: Vec<String>,
    pub name: String,
}

// The fn a `#[serde(default = "..")]` names, beside the struct.
pub struct DefaultFn {
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
    parsed: &'a [Parsed],
    params: &'a [String],
    design: &'a Design,
    // The type whose fields are being laid, so a shared declared name resolves
    // to itself rather than the first slice type that declared it.
    at: usize,
}

// What the slice's design says of its types beyond the signature being
// laid: every key it declares, and the kind of each number field it
// spells as a whole number or a fraction.
pub struct Design {
    pub keys: Vec<String>,
    numbers: BTreeMap<String, Scalar>,
}

impl Design {
    // Reads the design and the signatures the slice owns once: the keys of
    // the `Type:` blocks, and each number field's kind from a typed
    // declaration in any signature (`i64`, `int`, `float`), a column of a
    // SQL schema the design holds (`weight INTEGER`, `total NUMERIC`), or
    // the word `integer` beside a number field's name in the design's
    // prose. A name the sources spell both ways is dropped, so the field's
    // name decides it.
    pub fn read<'a>(design: &'a str, owned: impl IntoIterator<Item = &'a str>) -> Self {
        let keys = design_keys(design);
        let mut spelled: BTreeMap<String, Option<Scalar>> = BTreeMap::new();
        let mut spell = |name: &str, scalar: Scalar| {
            let name = snake(name);
            spelled
                .entry(name)
                .and_modify(|held| {
                    if *held != Some(scalar) {
                        *held = None;
                    }
                })
                .or_insert(Some(scalar));
        };

        // the typed declarations, and the number fields left for the prose
        let mut undecided = BTreeSet::new();
        let signatures = blocks(design).into_iter().filter_map(fenced_signature).chain(owned);
        for signature in signatures {
            let Shape::Struct(declared) = read(signature).shape else { continue };
            for field in &declared.fields {
                match field.ty.declared_scalar() {
                    Some(Declared::Int) => spell(&field.name, Scalar::Int),
                    Some(Declared::Float) => spell(&field.name, Scalar::Float),
                    Some(Declared::Number) => {
                        undecided.insert(snake(&field.name));
                    }
                    None => {}
                }
            }
        }

        // the schema's columns
        for line in design.lines() {
            let tokens: Vec<&str> = line
                .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '(' | ')'))
                .collect();
            for pair in tokens.windows(2) {
                let name = pair[0].trim_matches(|ch| matches!(ch, '"' | '`' | '[' | ']'));
                if !is_identifier(name) {
                    continue;
                }
                let column = pair[1].to_ascii_lowercase();
                if SQL_INTEGERS.contains(&column.as_str()) {
                    spell(name, Scalar::Int);
                } else if SQL_FLOATS.contains(&column.as_str()) {
                    spell(name, Scalar::Float);
                }
            }
        }

        // the prose, for a number field a sentence calls an integer
        for block in blocks(design) {
            if block.starts_with("```") || block.starts_with("Type: ") || block.starts_with('#') {
                continue;
            }
            let words: Vec<String> = block
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .filter(|word| !word.is_empty())
                .map(snake)
                .collect();
            for (at, word) in words.iter().enumerate() {
                if word != "integer" && word != "integers" {
                    continue;
                }
                let from = at.saturating_sub(INTEGER_REACH);
                let to = (at + INTEGER_REACH + 1).min(words.len());
                for near in &words[from..to] {
                    if undecided.contains(near) {
                        spell(near, Scalar::Int);
                    }
                }
            }
        }

        Self {
            keys,
            numbers: spelled
                .into_iter()
                .filter_map(|(name, scalar)| Some((name, scalar?)))
                .collect(),
        }
    }

    // The kind of the number field `wire` names, where the design spells
    // it, else by the name's last word.
    fn number(&self, wire: &str) -> Scalar {
        match self.numbers.get(&snake(wire)) {
            Some(scalar) => *scalar,
            None if counted(wire) => Scalar::Int,
            None => Scalar::Float,
        }
    }
}

// The scalar a field's laid type is, where it is one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scalar {
    Int,
    Float,
    Bool,
    Text,
}

// What a signature declares a numeric field as: a whole number, a
// fraction, or TypeScript's `number`, which says neither.
enum Declared {
    Int,
    Float,
    Number,
}

// A default spelled as the value its fn answers, or as serde's own where
// the source's is an empty collection.
enum Spelled {
    Value { text: String, constant: bool },
    Default,
}

impl<'a> DesignKey<'a> {
    pub fn parse(key: &'a str) -> Self {
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

pub fn read(signature: &str) -> Parsed {
    typescript::read(signature)
        .or_else(|| python::read(signature))
        .or_else(|| rust::read(signature))
        .unwrap_or(Parsed {
            name: None,
            shape: Shape::Opaque { is_enum: false },
        })
}

// The key of every type the design declares, data and mechanism alike.
fn design_keys(design: &str) -> Vec<String> {
    design
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Type: "))
        .map(|key| key.trim_end_matches(" (mechanism)").trim().to_owned())
        .collect()
}

// The signature a `Type:` block fences, where it is one.
fn fenced_signature(block: &str) -> Option<&str> {
    let (first, rest) = block.split_once('\n')?;
    if !first.starts_with("Type: ") {
        return None;
    }
    let (_, fenced) = rest.split_once('\n')?;
    if !rest.starts_with("```") {
        return None;
    }
    Some(fenced.rsplit_once("```").map_or(fenced, |(signature, _)| signature))
}

fn is_identifier(word: &str) -> bool {
    word.starts_with(|ch: char| ch.is_ascii_alphabetic())
        && word.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

// The bodies of the slice's types, each named as `names` has it, the
// fields referencing one another by those names and `Eq` dropped wherever
// a float reaches.
pub fn lay(parsed: &[Parsed], names: &[String], design: &Design, values: &mut Names) -> Vec<Body> {
    let owned: Vec<(String, String)> = parsed
        .iter()
        .zip(names)
        .map(|(parsed, name)| (parsed.name.clone().unwrap_or_else(|| name.clone()), name.clone()))
        .collect();
    let mut bodies = Vec::new();
    let mut floats = Vec::new();
    let mut references = Vec::new();
    for (at, (item, name)) in parsed.iter().zip(names).enumerate() {
        let (body, laying) = match &item.shape {
            Shape::Opaque { is_enum } => (Body::Marker { is_enum: *is_enum }, Laying::default()),
            Shape::Enum(declared) => (declared.lay(), Laying::default()),
            Shape::Table(table) => (table.lay(name, parsed, &owned), Laying::default()),
            Shape::Struct(declared) => {
                let refs = Refs {
                    owned: &owned,
                    parsed,
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

// The suffix of a table's data module after its reader's name, so the
// module reads as the rows and never as a part.
const DATA_MODULE_SUFFIX: &str = "_data";

impl Table {
    // Whether the slice lays the table's element as a struct, so the rows
    // deserialise into a type the module declares.
    pub fn laid_over(&self, parsed: &[Parsed]) -> bool {
        parsed.iter().any(|item| {
            item.name.as_deref() == Some(self.element.as_str())
                && matches!(item.shape, Shape::Struct(_))
        })
    }

    // The data module and its reader where the slice lays the element type;
    // the marker otherwise, since rows no laid type reads are rows the turn
    // would have to type itself.
    fn lay(&self, name: &str, parsed: &[Parsed], owned: &[(String, String)]) -> Body {
        let element = self
            .laid_over(parsed)
            .then(|| owned.iter().find(|(declared, _)| *declared == self.element))
            .flatten();
        let Some((_, element)) = element else {
            return Body::Marker { is_enum: false };
        };
        Body::Table {
            element: element.clone(),
            module: format!("{name}{DATA_MODULE_SUFFIX}"),
            json: self.json(),
        }
    }

    // The rows as a JSON array, one object per line under the wire's keys
    // in the order the source spells them.
    fn json(&self) -> String {
        let mut text = String::from("[\n");
        let count = self.rows.len();
        for (at, row) in self.rows.iter().enumerate() {
            let cells: Vec<String> = row
                .iter()
                .map(|(key, cell)| format!("{}: {}", json_string(key), cell.json()))
                .collect();
            text.push_str("  {");
            text.push_str(&cells.join(", "));
            text.push('}');
            if at + 1 < count {
                text.push(',');
            }
            text.push('\n');
        }
        text.push(']');
        text
    }
}

impl Literal {
    // The cell as JSON spells it; a default no table cell is reads as
    // `null`, since the reader admits none.
    fn json(&self) -> String {
        match self {
            Self::Int(text) | Self::Float(text) => text.clone(),
            Self::Bool(flag) => flag.to_string(),
            Self::Str(text) => json_string(text),
            Self::Empty | Self::Default | Self::Member(..) => "null".to_owned(),
        }
    }
}

// `text` as a JSON string literal.
fn json_string(text: &str) -> String {
    emery_sdk::serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
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
                LaidVariant {
                    docs: vec![doc],
                    attrs,
                    name,
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
            let carried = field_laying.append_carried_docs(&mut docs);
            laying.take(field_laying);
            fields.push(LaidField {
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
            let carried = field_laying.append_carried_docs(&mut docs);
            laying.take(field_laying);
            fields.push(LaidField {
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
        let text = match (self, ty.scalar(wire, refs)) {
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

    // Appends to `docs` one line per name a field carries as a value, and
    // answers the first of them for the brief.
    fn append_carried_docs(&self, docs: &mut Vec<String>) -> Option<String> {
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
        if let Some(key) = self.design.keys.iter().find(|key| DesignKey::parse(key).name == name) {
            return Ref::Design(key);
        }
        Ref::Foreign
    }

    fn variant(&self, at: usize, member: &str) -> Option<String> {
        let Shape::Enum(declared) = &self.parsed.get(at)?.shape else { return None };
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
            Self::Number => match refs.design.number(wire) {
                Scalar::Int => "i64".to_owned(),
                Scalar::Float | Scalar::Bool | Scalar::Text => {
                    laying.float = true;
                    "f64".to_owned()
                }
            },
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

    fn scalar(&self, wire: &str, refs: &Refs<'_>) -> Option<Scalar> {
        match self {
            Self::Int => Some(Scalar::Int),
            Self::Float => Some(Scalar::Float),
            Self::Number => Some(refs.design.number(wire)),
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

    // What the signature declares a numeric field as, through an option.
    fn declared_scalar(&self) -> Option<Declared> {
        match self {
            Self::Int => Some(Declared::Int),
            Self::Float => Some(Declared::Float),
            Self::Number => Some(Declared::Number),
            Self::Nullable(inner) => inner.declared_scalar(),
            Self::Text
            | Self::Date
            | Self::Bool
            | Self::Bytes
            | Self::Any
            | Self::Array(_)
            | Self::Map(_)
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
    pub const fn is_enum(&self) -> bool {
        match self {
            Self::Enum { .. } => true,
            Self::Marker { is_enum } => *is_enum,
            Self::Struct { .. } | Self::Table { .. } => false,
        }
    }

    pub const fn is_marker(&self) -> bool {
        matches!(self, Self::Marker { .. })
    }

    pub const fn is_table(&self) -> bool {
        matches!(self, Self::Table { .. })
    }

    pub const fn uses_map(&self) -> bool {
        matches!(self, Self::Struct { map: true, .. })
    }

    // The fields carried as values for a type the slice does not own, each
    // with the name it stands for.
    pub fn carried(&self) -> impl Iterator<Item = (&str, &str)> {
        let fields: &[LaidField] = match self {
            Self::Struct { fields, .. } => fields,
            Self::Enum { .. } | Self::Table { .. } | Self::Marker { .. } => &[],
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
// brackets, and string literals.
fn split_top(text: &str, sep: char) -> impl Iterator<Item = &str> {
    let mut start = 0;
    let mut done = false;
    let mut positions = top_level(text, sep);
    std::iter::from_fn(move || {
        if let Some(at) = positions.next() {
            let part = &text[start..at];
            start = at + sep.len_utf8();
            Some(part)
        } else if !done {
            done = true;
            Some(&text[start..])
        } else {
            None
        }
    })
}

// `text` split at the first `sep` outside brackets, parentheses, braces,
// angle brackets, and string literals; none where no such `sep` stands.
fn split_once_top(text: &str, sep: char) -> Option<(&str, &str)> {
    let at = top_level(text, sep).next()?;
    Some((&text[..at], &text[at + sep.len_utf8()..]))
}

// The byte offset of each `sep` in `text` outside brackets, parentheses,
// braces, angle brackets, and string literals; the `=>` of an arrow is no
// `=` and its `>` closes nothing.
fn top_level(text: &str, sep: char) -> impl Iterator<Item = usize> + '_ {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut previous = ' ';
    text.char_indices().filter_map(move |(at, ch)| {
        let found = if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            false
        } else {
            let arrow = ch == '=' && text[at + 1..].starts_with('>');
            match ch {
                '"' | '\'' | '`' => quote = Some(ch),
                '(' | '[' | '{' | '<' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                '>' if previous != '=' => depth -= 1,
                _ => {}
            }
            ch == sep && depth <= 0 && !arrow
        };
        previous = ch;
        found.then_some(at)
    })
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
