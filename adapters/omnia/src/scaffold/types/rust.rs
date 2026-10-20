//! Reads a Rust declaration: a struct of named fields, or an enum of unit variants.

use super::typescript::sentence;
use super::{
    Enum, Field, Read, Shape, Struct, Type, Variant, between, is_ident, literal, param_name,
    split_top,
};

pub(super) fn read(signature: &str) -> Option<Read> {
    let text = signature.trim();
    let head_at = text.lines().position(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with("///") && !line.starts_with("#[")
    })?;
    let head = text.lines().nth(head_at)?.trim();
    let mut head = head;
    for prefix in ["pub(crate) ", "pub(super) ", "pub ", "pub(self) "] {
        head = head.strip_prefix(prefix).unwrap_or(head);
    }
    let body_at = text.lines().take(head_at).map(|line| line.len() + 1).sum::<usize>();
    let rest = &text[body_at..];
    if let Some(after) = head.strip_prefix("struct ") {
        let name = ident_head(after);
        let Some((body, _)) = between(rest, '{', '}') else {
            return Some(Read {
                name,
                shape: Shape::Opaque { is_enum: false },
            });
        };
        if after.split('{').next().is_some_and(|signature| signature.contains('(')) {
            return Some(Read {
                name,
                shape: Shape::Opaque { is_enum: false },
            });
        }
        let generics = generics(after.split('{').next().unwrap_or(after));
        return Some(Read {
            name,
            shape: Shape::Struct(Struct {
                generics,
                bases: Vec::new(),
                fields: fields(body),
            }),
        });
    }
    if let Some(after) = head.strip_prefix("enum ") {
        let name = ident_head(after);
        let Some((body, _)) = between(rest, '{', '}') else {
            return Some(Read {
                name,
                shape: Shape::Opaque { is_enum: true },
            });
        };
        return Some(Read {
            name,
            shape: variants(body).map_or(Shape::Opaque { is_enum: true }, |variants| {
                Shape::Enum(Enum {
                    variants,
                    numeric: false,
                })
            }),
        });
    }
    None
}

fn fields(body: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    for member in members(body) {
        let text = member.text.trim();
        let text = text.strip_prefix("pub(crate) ").unwrap_or(text);
        let text = text.strip_prefix("pub(super) ").unwrap_or(text);
        let text = text.strip_prefix("pub ").unwrap_or(text);
        let Some((name, ty)) = text.split_once(':') else { continue };
        let name = name.trim().trim_start_matches("r#");
        if !is_ident(name) {
            continue;
        }
        fields.push(Field {
            wire: member.rename.unwrap_or_else(|| name.to_owned()),
            ty: parse(ty.trim()),
            optional: false,
            default: None,
            doc: member.doc,
        });
    }
    fields
}

// The unit variants of an enum body, or none where a variant carries a
// payload.
fn variants(body: &str) -> Option<Vec<Variant>> {
    let mut variants = Vec::new();
    for member in members(body) {
        let text = member.text.trim();
        let name = text.split('=').next().unwrap_or(text).trim();
        if !is_ident(name) {
            return None;
        }
        variants.push(Variant {
            member: name.to_owned(),
            wire: member.rename,
            doc: member.doc,
        });
    }
    Some(variants)
}

// A type as the reader lays it: the standard containers and scalars by
// name, another identifier as a reference, anything else carried as a value.
fn parse(text: &str) -> Type {
    let text = text.trim().trim_end_matches(',').trim();
    if let Some((args, after)) = between(text, '<', '>')
        && after.trim().is_empty()
        && let Some(name) = text.split('<').next().map(str::trim)
    {
        let args: Vec<&str> = split_top(args, ',').map(str::trim).collect();
        return match name {
            "Vec" | "VecDeque" | "BTreeSet" | "HashSet" | "Box" | "Arc" | "Rc" => {
                let inner = args.first().map_or(Type::Any, |arg| parse(arg));
                if matches!(name, "Box" | "Arc" | "Rc") {
                    inner
                } else {
                    Type::Array(Box::new(inner))
                }
            }
            "Option" => Type::Nullable(Box::new(args.first().map_or(Type::Any, |arg| parse(arg)))),
            "BTreeMap" | "HashMap" => {
                Type::Map(Box::new(args.get(1).map_or(Type::Any, |arg| parse(arg))))
            }
            _ if is_ident(name) => {
                Type::Named(name.to_owned(), args.iter().map(|arg| parse(arg)).collect())
            }
            _ => Type::Any,
        };
    }
    match text {
        "String" | "&str" | "&'static str" | "str" | "char" => Type::Text,
        "bool" => Type::Bool,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => Type::Int,
        "f32" | "f64" => Type::Float,
        _ if is_ident(text) => Type::Named(text.to_owned(), Vec::new()),
        _ => Type::Any,
    }
}

fn generics(head: &str) -> Vec<String> {
    between(head, '<', '>')
        .map(|(params, _)| {
            split_top(params, ',')
                .filter(|param| !param.trim_start().starts_with('\''))
                .map(param_name)
                .filter(|param| !param.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn ident_head(text: &str) -> Option<String> {
    let text = text.trim_start();
    let end =
        text.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).unwrap_or(text.len());
    let head = &text[..end];
    is_ident(head).then(|| head.to_owned())
}

struct Member {
    text: String,
    doc: Option<String>,
    rename: Option<String>,
}

// The members of a braced body, each with the `///` lines above it and the
// `serde(rename)` among its attributes.
fn members(body: &str) -> Vec<Member> {
    let mut members = Vec::new();
    let mut doc = Vec::new();
    let mut rename = None;
    let mut open = String::new();
    for line in body.lines() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if !open.is_empty() {
            open.push(' ');
            open.push_str(text);
            if depth(&open) <= 0 {
                members.push(Member {
                    text: std::mem::take(&mut open).trim_end_matches(',').to_owned(),
                    doc: sentence(&doc),
                    rename: rename.take(),
                });
                doc.clear();
            }
            continue;
        }
        if let Some(comment) = text.strip_prefix("///") {
            doc.push(comment.trim().to_owned());
            continue;
        }
        if text.starts_with("//") {
            continue;
        }
        if text.starts_with("#[") {
            if let Some(at) = text.find("rename = ")
                && let Some(value) = text[at + "rename = ".len()..].split([')', ',']).next()
            {
                rename = literal(value).map(str::to_owned);
            }
            continue;
        }
        if depth(text) > 0 {
            text.clone_into(&mut open);
            continue;
        }
        members.push(Member {
            text: text.trim_end_matches(',').to_owned(),
            doc: sentence(&doc),
            rename: rename.take(),
        });
        doc.clear();
    }
    members
}

fn depth(text: &str) -> i32 {
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            _ => {}
        }
    }
    depth
}
