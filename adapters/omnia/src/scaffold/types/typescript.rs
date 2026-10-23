//! Reads a TypeScript declaration: an interface, a type alias, a class, or an enum.

use super::{
    Enum, Field, Literal, Parsed, Shape, Struct, Type, Variant, between, is_ident, literal, number,
    param_name, sentence, split_once_top, split_top,
};

// A declaration led by one of these is TypeScript's.
const PREFIXES: &[&str] = &["export ", "declare ", "default ", "abstract "];

// A member's modifiers that change nothing of the wire.
const MODIFIERS: &[&str] = &["public ", "readonly ", "declare ", "override ", "abstract "];

// A member led by one of these is no field of the wire's.
const SKIPPED: &[&str] =
    &["private ", "protected ", "static ", "constructor", "get ", "set ", "new "];

// Generic utility types the reader resolves to their argument.
const UNWRAPPED: &[&str] = &["Promise", "Readonly", "Required", "NonNullable", "Awaited"];

// Generic utility types the reader cannot lay, carried as a value.
const OPAQUE: &[&str] =
    &["Partial", "Pick", "Omit", "Exclude", "Extract", "Parameters", "ReturnType", "InstanceType"];

pub(super) fn read(signature: &str) -> Option<Parsed> {
    let text = signature.trim();
    let mut head = text;
    let mut prefixed = false;
    while let Some(prefix) = PREFIXES.iter().find(|prefix| head.starts_with(*prefix)) {
        head = head[prefix.len()..].trim_start();
        prefixed = true;
    }
    if let Some(rest) = head.strip_prefix("interface ") {
        return Some(interface(rest));
    }
    if let Some(rest) = head.strip_prefix("type ") {
        return Some(alias(rest));
    }
    if let Some(rest) = head.strip_prefix("class ")
        && braced(rest)
    {
        return Some(class(rest));
    }
    if let Some(rest) = head.strip_prefix("enum ")
        && (prefixed || rest.contains('='))
    {
        return Some(enumeration(rest));
    }
    for keyword in ["const ", "let ", "var ", "function ", "async function "] {
        if let Some(rest) = head.strip_prefix(keyword) {
            return Some(Parsed {
                name: ident_head(rest),
                shape: Shape::Opaque { is_enum: false },
            });
        }
    }
    None
}

// Whether a class head opens a braced body, as TypeScript writes one and
// Python does not.
fn braced(rest: &str) -> bool {
    let head = rest.lines().next().unwrap_or(rest);
    let head = split_top(head, '{').next().unwrap_or(head);
    !head.trim_end().ends_with(':')
}

fn interface(rest: &str) -> Parsed {
    let name = ident_head(rest);
    let head = rest.split('{').next().unwrap_or(rest);
    let generics = generics(head);
    let bases = head
        .split_once(" extends ")
        .map(|(_, bases)| {
            split_top(bases, ',').filter_map(|base| ident_head(base.trim())).collect()
        })
        .unwrap_or_default();
    let Some((body, _)) = between(rest, '{', '}') else {
        return Parsed {
            name,
            shape: Shape::Opaque { is_enum: false },
        };
    };
    Parsed {
        name,
        shape: object(body, generics, bases),
    }
}

fn alias(rest: &str) -> Parsed {
    let name = ident_head(rest);
    let (head, value) = split_once_top(rest, '=').unwrap_or((rest, ""));
    let value = value.trim().trim_end_matches(';').trim();
    if value.is_empty() {
        return Parsed {
            name,
            shape: Shape::Opaque { is_enum: false },
        };
    }
    let generics = generics(head);
    if value.starts_with('{')
        && let Some((body, after)) = between(value, '{', '}')
        && after.trim().is_empty()
    {
        return Parsed {
            name,
            shape: object(body, generics, Vec::new()),
        };
    }
    let parts: Vec<&str> =
        split_top(value, '|').map(str::trim).filter(|part| !part.is_empty()).collect();
    if !parts.is_empty() && parts.iter().all(|part| literal(part).is_some()) {
        let variants = parts
            .iter()
            .filter_map(|part| literal(part))
            .map(|text| Variant {
                member: text.to_owned(),
                wire: Some(text.to_owned()),
                alias: false,
                doc: None,
            })
            .collect();
        return Parsed {
            name,
            shape: Shape::Enum(Enum {
                variants,
                numeric: false,
            }),
        };
    }
    Parsed {
        name,
        shape: Shape::Opaque { is_enum: false },
    }
}

fn class(rest: &str) -> Parsed {
    let name = ident_head(rest);
    let head = rest.split('{').next().unwrap_or(rest);
    let generics = generics(head);
    let bases = head
        .split_once(" extends ")
        .map(|(_, bases)| {
            let bases = bases.split(" implements ").next().unwrap_or(bases);
            split_top(bases, ',').filter_map(|base| ident_head(base.trim())).collect()
        })
        .unwrap_or_default();
    let Some((body, _)) = between(rest, '{', '}') else {
        return Parsed {
            name,
            shape: Shape::Opaque { is_enum: false },
        };
    };
    Parsed {
        name,
        shape: object(body, generics, bases),
    }
}

fn enumeration(rest: &str) -> Parsed {
    let name = ident_head(rest);
    let Some((body, _)) = between(rest, '{', '}') else {
        return Parsed {
            name,
            shape: Shape::Opaque { is_enum: true },
        };
    };
    let mut variants = Vec::new();
    let mut numeric = true;
    for clause in clauses(body) {
        let text = clause.text.trim();
        if text.is_empty() {
            continue;
        }
        let (member_name, value) = match text.split_once('=') {
            Some((member_name, value)) => (member_name.trim(), Some(value.trim())),
            None => (text, None),
        };
        let member_name = literal(member_name).unwrap_or(member_name);
        let wire = value.and_then(|val| literal(val).map(str::to_owned));
        let alias = wire.is_none() && matches!(value.and_then(number), Some(Literal::Int(_)));
        let wire = wire.or_else(|| match value.and_then(number) {
            Some(Literal::Int(digits)) => Some(digits),
            _ => None,
        });
        if wire.is_some() {
            numeric = false;
        }
        variants.push(Variant {
            member: member_name.to_owned(),
            wire,
            alias,
            doc: clause.doc,
        });
    }
    Parsed {
        name,
        shape: Shape::Enum(Enum { variants, numeric }),
    }
}

// The fields of an object body: an interface's, a type literal's, or a
// class's, the methods and the members the wire never carries set aside.
fn object(body: &str, generics: Vec<String>, bases: Vec<String>) -> Shape {
    let mut fields = Vec::new();
    let mut indexed = false;
    for clause in clauses(body) {
        let (wire, json_property, mut text) = decorators(clause.text.trim());
        while let Some(modifier) = MODIFIERS.iter().find(|modifier| text.starts_with(*modifier)) {
            text = text[modifier.len()..].trim_start();
        }
        if text.is_empty() || SKIPPED.iter().any(|prefix| text.starts_with(prefix)) {
            continue;
        }
        if text.starts_with('[') {
            indexed = true;
            continue;
        }
        let Some(field) = field(text, wire, json_property, clause.doc) else { continue };
        fields.push(field);
    }
    if fields.is_empty() && bases.is_empty() && indexed {
        return Shape::Opaque { is_enum: false };
    }
    Shape::Struct(Struct {
        generics,
        bases,
        fields,
    })
}

// `name?: Type = default` read as a field; a method, a computed name, or a
// function-typed field is none.
fn field(
    text: &str, wire: Option<String>, json_property: bool, doc: Option<String>,
) -> Option<Field> {
    let (name, rest) =
        if let Some(quote) = text.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
            let (name, rest) = text[1..].split_once(quote)?;
            (name.to_owned(), rest.trim_start())
        } else {
            let end = text
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')))
                .unwrap_or(text.len());
            (text[..end].to_owned(), text[end..].trim_start())
        };
    if name.is_empty() {
        return None;
    }
    let wire = wire.unwrap_or_else(|| name.clone());
    let optional = rest.starts_with('?');
    let rest = rest.strip_prefix(['?', '!']).unwrap_or(rest).trim_start();
    if rest.starts_with(['(', '<']) {
        return None;
    }
    let (ty, default) = if let Some(typed) = rest.strip_prefix(':') {
        let typed = typed.trim();
        match split_once_top(typed, '=') {
            Some((ty, value)) => (ty.trim().to_owned(), Some(value.trim())),
            None => (typed.to_owned(), None),
        }
    } else {
        let value = rest.strip_prefix('=')?.trim();
        (inferred(value), Some(value))
    };
    if ty.is_empty() || split_top(&ty, '|').any(|part| part.contains("=>")) {
        return None;
    }
    let optional = optional || default.is_some();
    let default =
        default.and_then(default_of).or_else(|| json_property.then_some(Literal::Default));
    Some(Field {
        name,
        wire,
        ty: parse(&ty),
        optional,
        default,
        doc,
    })
}

// Strips leading `@..` decorators from `text`, answering the wire name a
// `@JsonProperty` or `@Expose` spells, whether `@JsonProperty` was among
// them, and what follows the decorators.
fn decorators(mut text: &str) -> (Option<String>, bool, &str) {
    let mut wire = None;
    let mut json_property = false;
    text = text.trim_start();
    while let Some(after_at) = text.strip_prefix('@') {
        let end = after_at
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '.')))
            .unwrap_or(after_at.len());
        let name = after_at[..end].rsplit('.').next().unwrap_or("");
        if !is_ident(name) {
            break;
        }
        if name == "JsonProperty" {
            json_property = true;
        }
        let rest = after_at[end..].trim_start();
        if rest.starts_with('(') {
            let Some((args, after_parens)) = between(rest, '(', ')') else {
                break;
            };
            if let Some(found) = decorator_wire(name, args) {
                wire = Some(found);
            }
            text = after_parens.trim_start();
        } else {
            text = rest;
        }
    }
    (wire, json_property, text)
}

fn decorator_wire(name: &str, args: &str) -> Option<String> {
    if !matches!(name, "JsonProperty" | "Expose") {
        return None;
    }
    let first = split_top(args, ',').next()?.trim();
    if let Some(quoted) = literal(first) {
        return (!quoted.is_empty()).then(|| quoted.to_owned());
    }
    let (body, _) = between(first, '{', '}')?;
    split_top(body, ',').find_map(|entry| {
        let (key, value) = entry.split_once(':')?;
        (key.trim() == "name").then(|| literal(value)).flatten().map(str::to_owned)
    })
}

// The type a class field's initializer spells where it declares none.
fn inferred(value: &str) -> String {
    if literal(value).is_some() {
        "string".to_owned()
    } else if matches!(value, "true" | "false") {
        "boolean".to_owned()
    } else if number(value).is_some() {
        "number".to_owned()
    } else if value.starts_with('[') {
        "unknown[]".to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn default_of(value: &str) -> Option<Literal> {
    let value = value.trim().trim_end_matches(';').trim();
    if let Some(text) = literal(value) {
        return Some(Literal::Str(text.to_owned()));
    }
    match value {
        "true" => return Some(Literal::Bool(true)),
        "false" => return Some(Literal::Bool(false)),
        "[]" | "{}" | "new Map()" | "new Set()" => return Some(Literal::Empty),
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

// A type expression as the reader lays it.
pub(super) fn parse(text: &str) -> Type {
    let text = text.trim();
    if text.starts_with('(')
        && let Some((inner, after)) = between(text, '(', ')')
        && after.trim().is_empty()
        && !inner.trim().is_empty()
    {
        return parse(inner);
    }

    // a union: nullable where a part is `null` or `undefined`
    let parts: Vec<&str> =
        split_top(text, '|').map(str::trim).filter(|part| !part.is_empty()).collect();
    if parts.len() > 1 {
        let nullable = parts.iter().any(|part| matches!(*part, "null" | "undefined"));
        let rest: Vec<&str> =
            parts.iter().copied().filter(|part| !matches!(*part, "null" | "undefined")).collect();
        let inner = match rest.as_slice() {
            [] => Type::Any,
            [one] => parse(one),
            many if many.iter().all(|part| literal(part).is_some()) => Type::Text,
            many if many.iter().all(|part| number(part).is_some()) => Type::Number,
            many if many.iter().all(|part| matches!(*part, "true" | "false")) => Type::Bool,
            _ => Type::Any,
        };
        return if nullable { Type::Nullable(Box::new(inner)) } else { inner };
    }
    if split_top(text, '&').count() > 1 {
        return Type::Any;
    }

    let text = text.strip_prefix("readonly ").unwrap_or(text).trim();
    if let Some(element) = text.strip_suffix("[]") {
        return Type::Array(Box::new(parse(element)));
    }
    if text.starts_with(['{', '[']) || text.contains("=>") {
        return Type::Any;
    }
    if literal(text).is_some() {
        return Type::Text;
    }
    if number(text).is_some() {
        return Type::Number;
    }
    if let Some((generic, after)) = between(text, '<', '>')
        && after.trim().is_empty()
        && let Some(head) = text.split('<').next().map(str::trim)
    {
        let name = head.rsplit('.').next().unwrap_or(head);
        if !is_ident(name) {
            return Type::Any;
        }
        let args: Vec<&str> = split_top(generic, ',').map(str::trim).collect();
        return match name {
            "Array" | "ReadonlyArray" | "Set" | "ReadonlySet" => {
                Type::Array(Box::new(args.first().map_or(Type::Any, |arg| parse(arg))))
            }
            "Record" | "Map" | "ReadonlyMap" => {
                Type::Map(Box::new(args.get(1).map_or(Type::Any, |arg| parse(arg))))
            }
            _ if UNWRAPPED.contains(&name) => args.first().map_or(Type::Any, |arg| parse(arg)),
            _ if OPAQUE.contains(&name) => Type::Any,
            _ => Type::Named(name.to_owned(), args.iter().map(|arg| parse(arg)).collect()),
        };
    }
    let name = text.rsplit('.').next().unwrap_or(text);
    match name {
        "string" => Type::Text,
        "number" => Type::Number,
        "boolean" | "true" | "false" => Type::Bool,
        "bigint" => Type::Int,
        "Date" => Type::Date,
        "Buffer" | "Uint8Array" => Type::Bytes,
        _ if is_ident(name)
            && !matches!(
                name,
                "any" | "unknown" | "object" | "never" | "void" | "null" | "undefined" | "symbol"
            ) =>
        {
            Type::Named(name.to_owned(), Vec::new())
        }
        _ => Type::Any,
    }
}

// The type parameters a head declares, `T` and `U` for `Page<T, U extends Base>`.
fn generics(head: &str) -> Vec<String> {
    let head = head.split(" extends ").next().unwrap_or(head);
    between(head, '<', '>')
        .map(|(params, _)| {
            split_top(params, ',').map(param_name).filter(|param| !param.is_empty()).collect()
        })
        .unwrap_or_default()
}

// The identifier `text` leads with.
fn ident_head(text: &str) -> Option<String> {
    let text = text.trim_start();
    let end = text
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$')))
        .unwrap_or(text.len());
    let head = &text[..end];
    is_ident(head).then(|| head.to_owned())
}

// One clause of a body, raw, with the comment written above it.
struct Clause {
    text: String,
    doc: Option<String>,
}

// The clauses of a braced body, each ended by a top-level `;`, `,`, or a
// line break where the clause is whole, its comments read as its doc.
fn clauses(body: &str) -> Vec<Clause> {
    let mut clauses: Vec<Clause> = Vec::new();
    let mut doc: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut chars = body.chars().peekable();
    let mut previous = ' ';
    // whether the line still holds the clause just ended, so a comment
    // after it on the line is that clause's
    let mut trailing = false;
    while let Some(ch) = chars.next() {
        if let Some(open) = quote {
            text.push(ch);
            if ch == open && previous != '\\' {
                quote = None;
            }
            previous = ch;
            continue;
        }
        match ch {
            '/' if depth == 0 && text.trim().is_empty() && chars.peek() == Some(&'/') => {
                let line: String = chars.by_ref().take_while(|ch| *ch != '\n').collect();
                let line = line.trim_start_matches('/').trim().to_owned();
                match clauses.last_mut() {
                    Some(last) if trailing && last.doc.is_none() => {
                        last.doc = sentence(&[line]);
                    }
                    _ => doc.push(line),
                }
                trailing = false;
                text.clear();
            }
            '/' if depth == 0 && text.trim().is_empty() && chars.peek() == Some(&'*') => {
                let mut block = String::new();
                let mut last = ' ';
                for ch in chars.by_ref() {
                    if last == '*' && ch == '/' {
                        block.pop();
                        break;
                    }
                    block.push(ch);
                    last = ch;
                }
                for line in block.lines() {
                    let line = line.trim().trim_start_matches('*').trim();
                    if !line.is_empty() {
                        doc.push(line.to_owned());
                    }
                }
                text.clear();
            }
            '"' | '\'' | '`' => {
                quote = Some(ch);
                text.push(ch);
            }
            '(' | '[' | '{' | '<' => {
                depth += 1;
                text.push(ch);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                text.push(ch);
            }
            '>' if previous != '=' => {
                depth -= 1;
                text.push(ch);
            }
            ';' | ',' if depth <= 0 => {
                trailing = flush(&mut text, &mut doc, &mut clauses);
            }
            '\n' if depth <= 0 => {
                trailing = false;
                let whole = !text.trim().is_empty()
                    && !decorators(&text).2.is_empty()
                    && !text.trim_end().ends_with([':', '|', '&', '=', '<', '(', '?'])
                    && !continues(chars.clone());
                if whole {
                    flush(&mut text, &mut doc, &mut clauses);
                } else {
                    text.push(' ');
                }
            }
            _ => {
                if !ch.is_whitespace() {
                    trailing = false;
                }
                text.push(ch);
            }
        }
        previous = ch;
    }
    flush(&mut text, &mut doc, &mut clauses);
    clauses
}

// Whether the next line continues a clause: a union or intersection arm.
fn continues(rest: std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let next: String = rest.skip_while(|ch| ch.is_whitespace()).take(1).collect();
    matches!(next.as_str(), "|" | "&")
}

// Ends the clause `text` holds, answering whether it held one.
fn flush(text: &mut String, doc: &mut Vec<String>, clauses: &mut Vec<Clause>) -> bool {
    let clause = text.trim();
    let held = !clause.is_empty();
    if held {
        clauses.push(Clause {
            text: clause.to_owned(),
            doc: sentence(doc),
        });
    }
    doc.clear();
    text.clear();
    held
}
