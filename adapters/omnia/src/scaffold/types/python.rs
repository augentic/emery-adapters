//! Reads a Python class: a model, a dataclass, a typed dict, or an enumeration.

use super::{
    Enum, Field, Literal, Read, Shape, Struct, Type, Variant, between, is_ident, literal, number,
    sentence, split_top,
};

// Bases that declare a class's kind rather than fields the wire carries.
const MARKER_BASES: &[&str] = &[
    "BaseModel",
    "BaseSettings",
    "TypedDict",
    "NamedTuple",
    "Protocol",
    "ABC",
    "object",
    "Generic",
    "Struct",
    "Schema",
    "Model",
];

// Bases that enumerate by number.
const NUMERIC_ENUMS: &[&str] = &["IntEnum", "IntFlag", "Flag"];

// Names that spell text on the wire.
const TEXTUAL: &[&str] = &[
    "str",
    "UUID",
    "EmailStr",
    "HttpUrl",
    "AnyUrl",
    "AnyHttpUrl",
    "SecretStr",
    "Path",
    "PurePath",
    "IPv4Address",
    "IPv6Address",
];

const DATES: &[&str] = &["datetime", "date", "time", "AwareDatetime", "NaiveDatetime"];

const ARRAYS: &[&str] = &[
    "list",
    "List",
    "set",
    "Set",
    "frozenset",
    "FrozenSet",
    "Sequence",
    "MutableSequence",
    "Iterable",
    "Collection",
    "Deque",
    "deque",
];

const MAPS: &[&str] = &["dict", "Dict", "Mapping", "MutableMapping", "DefaultDict", "OrderedDict"];

pub(super) fn read(signature: &str) -> Option<Read> {
    let lines: Vec<&str> = signature.lines().collect();
    let head_at = lines.iter().position(|line| line.trim_start().starts_with("class "))?;
    if lines[..head_at].iter().any(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with('@') && !line.starts_with('#')
    }) {
        return None;
    }
    let head = lines[head_at].trim();
    let header = head.split('#').next().unwrap_or(head).trim_end();
    if !header.ends_with(':') {
        return None;
    }
    let header = header.trim_end_matches(':').trim_end();
    let rest = header.strip_prefix("class ")?;
    let (name, bases) = match between(rest, '(', ')') {
        Some((bases, _)) => (rest.split('(').next().unwrap_or(rest).trim(), bases),
        None => (rest.trim(), ""),
    };
    let name = is_ident(name).then(|| name.to_owned());
    let bases: Vec<&str> = split_top(bases, ',')
        .map(str::trim)
        .filter(|base| !base.is_empty() && !base.contains('='))
        .collect();
    let body = &lines[head_at + 1..];

    // an enumeration where a base says so
    if let Some(base) = bases.iter().find(|base| base.ends_with("Enum") || base.ends_with("Flag")) {
        let numeric = NUMERIC_ENUMS.contains(&base.rsplit('.').next().unwrap_or(base));
        return Some(Read {
            name,
            shape: enumeration(body, numeric),
        });
    }

    let mut generics = Vec::new();
    let mut flattened = Vec::new();
    for base in bases {
        let base_name = base.split('[').next().unwrap_or(base).trim();
        let base_name = base_name.rsplit('.').next().unwrap_or(base_name);
        if base_name == "Generic" || base_name == "Protocol" {
            if let Some((params, _)) = between(base, '[', ']') {
                generics.extend(split_top(params, ',').map(str::trim).map(str::to_owned));
            }
            continue;
        }
        if MARKER_BASES.contains(&base_name) || !is_ident(base_name) {
            continue;
        }
        flattened.push(base_name.to_owned());
    }
    Some(Read {
        name,
        shape: Shape::Struct(Struct {
            generics,
            bases: flattened,
            fields: fields(body),
        }),
    })
}

fn enumeration(body: &[&str], numeric: bool) -> Shape {
    let mut variants = Vec::new();
    let mut doc = Vec::new();
    let mut numbers_only = true;
    for line in members(body) {
        let text = line.trim();
        if let Some(comment) = text.strip_prefix('#') {
            doc.push(comment.trim().to_owned());
            continue;
        }
        let Some((member, value)) = text.split_once('=') else {
            doc.clear();
            continue;
        };
        let member = member.trim();
        if !is_ident(member) || member.starts_with('_') {
            doc.clear();
            continue;
        }
        let value = value.split('#').next().unwrap_or(value).trim();
        let wire = literal(value).map(str::to_owned);
        if wire.is_some() {
            numbers_only = false;
        }
        variants.push(Variant {
            member: member.to_owned(),
            wire,
            doc: sentence(&doc),
        });
        doc.clear();
    }
    let numeric = numeric || (numbers_only && !variants.is_empty());
    Shape::Enum(Enum { variants, numeric })
}

fn fields(body: &[&str]) -> Vec<Field> {
    let mut fields = Vec::new();
    let mut doc = Vec::new();
    for line in members(body) {
        let text = line.trim();
        if let Some(comment) = text.strip_prefix('#') {
            doc.push(comment.trim().to_owned());
            continue;
        }
        let Some(field) = field(text, sentence(&doc)) else {
            doc.clear();
            continue;
        };
        fields.push(field);
        doc.clear();
    }
    fields
}

// `name: type = default` read as a field; an unannotated class attribute,
// a class variable, and a callable are none.
fn field(text: &str, doc: Option<String>) -> Option<Field> {
    let (name, rest) = text.split_once(':')?;
    let name = name.trim();
    if !is_ident(name) || name.starts_with('_') || name == "model_config" {
        return None;
    }
    let rest = rest.split('#').next().unwrap_or(rest);
    let (annotation, default) = match split_top(rest, '=').collect::<Vec<_>>().as_slice() {
        [annotation, value, ..] => (annotation.trim(), Some(value.trim())),
        _ => (rest.trim(), None),
    };
    if annotation.is_empty() || annotation.starts_with("ClassVar") {
        return None;
    }
    let annotation = annotation
        .strip_prefix("Final[")
        .map_or(annotation, |inner| inner.strip_suffix(']').unwrap_or(inner));
    if annotation.starts_with("Callable") {
        return None;
    }
    let mut wire = name.to_owned();
    let mut optional = false;
    let mut literal_default = None;
    if let Some(value) = default {
        match defaulted(value) {
            Default::None | Default::Other => optional = true,
            Default::Literal(found) => {
                optional = true;
                literal_default = Some(found);
            }
            Default::Declared { alias, default } => {
                if let Some(alias) = alias {
                    wire = alias;
                }
                if let Some(found) = default {
                    optional = true;
                    literal_default = match *found {
                        Default::Literal(found) => Some(found),
                        Default::None | Default::Declared { .. } | Default::Other => None,
                    };
                }
            }
        }
    }
    Some(Field {
        wire,
        ty: parse(annotation),
        optional,
        default: literal_default,
        doc,
    })
}

// What a field's `= ..` spells.
enum Default {
    None,
    Literal(Literal),
    // A `Field(..)` or `field(..)` call: its alias and its default.
    Declared { alias: Option<String>, default: Option<Box<Self>> },
    Other,
}

fn defaulted(value: &str) -> Default {
    let value = value.trim();
    if value == "None" {
        return Default::None;
    }
    if let Some(found) = literal_of(value) {
        return Default::Literal(found);
    }
    let callee = value.split('(').next().unwrap_or(value).trim();
    if matches!(callee.rsplit('.').next().unwrap_or(callee), "Field" | "field")
        && let Some((args, _)) = between(value, '(', ')')
    {
        let mut alias = None;
        let mut default = None;
        for (at, arg) in split_top(args, ',').map(str::trim).enumerate() {
            if arg.is_empty() {
                continue;
            }
            match arg.split_once('=') {
                Some((key, value)) => match key.trim() {
                    "alias" | "serialization_alias" => {
                        alias = literal(value).map(str::to_owned);
                    }
                    "default" => default = Some(Box::new(defaulted(value))),
                    "default_factory" => default = Some(Box::new(Default::Literal(Literal::Empty))),
                    _ => {}
                },
                None if at == 0 && arg != "..." => default = Some(Box::new(defaulted(arg))),
                None => {}
            }
        }
        return Default::Declared { alias, default };
    }
    Default::Other
}

fn literal_of(value: &str) -> Option<Literal> {
    if let Some(text) = literal(value) {
        return Some(Literal::Str(text.to_owned()));
    }
    match value {
        "True" => return Some(Literal::Bool(true)),
        "False" => return Some(Literal::Bool(false)),
        "[]" | "{}" | "()" | "list()" | "dict()" | "set()" | "tuple()" | "frozenset()" => {
            return Some(Literal::Empty);
        }
        _ => {}
    }
    if let Some(found) = number(value) {
        return Some(found);
    }
    if let Some((owner, member)) = value.split_once('.')
        && is_ident(owner)
        && is_ident(member)
    {
        return Some(Literal::Member(owner.to_owned(), member.to_owned()));
    }
    None
}

// A type annotation as the reader lays it.
fn parse(text: &str) -> Type {
    let text = text.trim();
    if let Some(quoted) = literal(text) {
        return parse(quoted);
    }

    // a union: optional where an arm is `None`
    let arms: Vec<&str> =
        split_top(text, '|').map(str::trim).filter(|arm| !arm.is_empty()).collect();
    if arms.len() > 1 {
        return union(&arms);
    }

    if let Some((args, after)) = between(text, '[', ']')
        && after.trim().is_empty()
        && let Some(name) = text.split('[').next().map(str::trim)
    {
        let name = name.rsplit('.').next().unwrap_or(name);
        let args: Vec<&str> = split_top(args, ',').map(str::trim).collect();
        return match name {
            "Optional" => {
                Type::Nullable(Box::new(args.first().map_or(Type::Any, |arg| parse(arg))))
            }
            "Union" => union(&args),
            "Annotated" | "Final" | "Required" | "NotRequired" | "ReadOnly" => {
                args.first().map_or(Type::Any, |arg| parse(arg))
            }
            "Literal" => {
                if args.iter().all(|arg| number(arg).is_some()) {
                    Type::Int
                } else {
                    Type::Text
                }
            }
            "tuple" | "Tuple" => match args.as_slice() {
                [element, "..."] => Type::Array(Box::new(parse(element))),
                _ => Type::Any,
            },
            _ if ARRAYS.contains(&name) => {
                Type::Array(Box::new(args.first().map_or(Type::Any, |arg| parse(arg))))
            }
            _ if MAPS.contains(&name) => {
                Type::Map(Box::new(args.get(1).map_or(Type::Any, |arg| parse(arg))))
            }
            _ if is_ident(name) => {
                Type::Named(name.to_owned(), args.iter().map(|arg| parse(arg)).collect())
            }
            _ => Type::Any,
        };
    }

    let name = text.rsplit('.').next().unwrap_or(text);
    match name {
        _ if TEXTUAL.contains(&name) => Type::Text,
        "int" => Type::Int,
        "float" | "Decimal" => Type::Float,
        "bool" => Type::Bool,
        "bytes" | "bytearray" => Type::Bytes,
        _ if DATES.contains(&name) => Type::Date,
        _ if ARRAYS.contains(&name) || name == "tuple" || name == "Tuple" => {
            Type::Array(Box::new(Type::Any))
        }
        _ if MAPS.contains(&name) => Type::Map(Box::new(Type::Any)),
        "Any" | "object" | "None" | "type" | "Type" => Type::Any,
        _ if is_ident(name) => Type::Named(name.to_owned(), Vec::new()),
        _ => Type::Any,
    }
}

fn union(arms: &[&str]) -> Type {
    let nullable = arms.contains(&"None");
    let rest: Vec<&str> = arms.iter().copied().filter(|arm| *arm != "None").collect();
    let inner = match rest.as_slice() {
        [one] => parse(one),
        _ => Type::Any,
    };
    if nullable { Type::Nullable(Box::new(inner)) } else { inner }
}

// The lines of the class body at its own indent, a method's or a nested
// class's body and a docstring set aside, each line joined with what its
// open brackets run on to. The indent is the fields', since a comment may
// stand at any column.
fn members<'a>(body: &'a [&'a str]) -> Vec<String> {
    let indent = body
        .iter()
        .filter(|line| {
            let text = line.trim();
            !text.is_empty() && !text.starts_with('#')
        })
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or_default();
    let mut members = Vec::new();
    let mut docstring: Option<&str> = None;
    let mut open = String::new();
    for line in body {
        let depth = line.len() - line.trim_start().len();
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if let Some(quote) = docstring {
            if text.contains(quote) {
                docstring = None;
            }
            continue;
        }
        if !open.is_empty() {
            open.push(' ');
            open.push_str(text);
            if balanced(&open) {
                members.push(std::mem::take(&mut open));
            }
            continue;
        }
        if depth > indent {
            continue;
        }
        if let Some(quote) = ["\"\"\"", "'''"].iter().find(|quote| text.starts_with(*quote)) {
            if text.len() == quote.len() || !text[quote.len()..].contains(quote) {
                docstring = Some(quote);
            }
            continue;
        }
        if text.starts_with("def ")
            || text.starts_with("async def ")
            || text.starts_with("class ")
            || text.starts_with('@')
            || text == "..."
            || text == "pass"
        {
            continue;
        }
        if balanced(text) {
            members.push(text.to_owned());
        } else {
            text.clone_into(&mut open);
        }
    }
    if !open.is_empty() {
        members.push(open);
    }
    members
}

// Whether every bracket `text` opens, it closes.
fn balanced(text: &str) -> bool {
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    depth <= 0
}
