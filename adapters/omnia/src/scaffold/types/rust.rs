//! Reads a Rust declaration through `syn`: a struct of named fields, or an enum of unit variants.

use syn::punctuated::Punctuated;
use syn::{
    Attribute, Expr, ExprLit, Fields, GenericArgument, GenericParam, Item, Lit, Meta,
    PathArguments, Token, TypePath,
};

use super::{Enum, Field, Read, Shape, Struct, Type, Variant, sentence};

// A declaration `syn` refuses, or one that is no struct or enum, is none
// of the reader's: the key names the type and the marker stands for it.
pub(super) fn read(signature: &str) -> Option<Read> {
    let (name, shape) = match syn::parse_str::<Item>(signature).ok()? {
        Item::Struct(item) => {
            let shape = match &item.fields {
                Fields::Named(fields) => Shape::Struct(Struct {
                    generics: generics(&item.generics),
                    bases: Vec::new(),
                    fields: fields.named.iter().filter_map(field).collect(),
                }),
                Fields::Unnamed(_) | Fields::Unit => Shape::Opaque { is_enum: false },
            };
            (item.ident, shape)
        }
        Item::Enum(item) => {
            let shape = item.variants.iter().map(variant).collect::<Option<Vec<_>>>().map_or(
                Shape::Opaque { is_enum: true },
                |variants| {
                    Shape::Enum(Enum {
                        variants,
                        numeric: false,
                    })
                },
            );
            (item.ident, shape)
        }
        _ => return None,
    };
    Some(Read {
        name: Some(name.to_string()),
        shape,
    })
}

fn field(field: &syn::Field) -> Option<Field> {
    let ident = field.ident.as_ref()?.to_string();
    let name = ident.strip_prefix("r#").unwrap_or(&ident);
    Some(Field {
        wire: rename(&field.attrs).unwrap_or_else(|| name.to_owned()),
        ty: parse(&field.ty),
        optional: false,
        default: None,
        doc: docs(&field.attrs),
    })
}

// A unit variant, or none where the variant carries a payload.
fn variant(variant: &syn::Variant) -> Option<Variant> {
    matches!(variant.fields, Fields::Unit).then(|| Variant {
        member: variant.ident.to_string(),
        wire: rename(&variant.attrs),
        doc: docs(&variant.attrs),
    })
}

// The type parameters a declaration takes, lifetimes and constants aside.
fn generics(generics: &syn::Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(param) => Some(param.ident.to_string()),
            GenericParam::Lifetime(_) | GenericParam::Const(_) => None,
        })
        .collect()
}

// A type as the reader lays it: the standard containers and scalars by
// their last segment, another path as a reference, anything else carried
// as a value.
fn parse(ty: &syn::Type) -> Type {
    match ty {
        syn::Type::Paren(paren) => parse(&paren.elem),
        syn::Type::Group(group) => parse(&group.elem),
        syn::Type::Reference(reference) => parse(&reference.elem),
        syn::Type::Slice(slice) => Type::Array(Box::new(parse(&slice.elem))),
        syn::Type::Array(array) => Type::Array(Box::new(parse(&array.elem))),
        syn::Type::Path(path) => named(path),
        _ => Type::Any,
    }
}

fn named(path: &TypePath) -> Type {
    let Some(last) = path.path.segments.last() else { return Type::Any };
    let args: Vec<&syn::Type> = match &last.arguments {
        PathArguments::None => Vec::new(),
        PathArguments::AngleBracketed(bracketed) => bracketed
            .args
            .iter()
            .filter_map(|arg| match arg {
                GenericArgument::Type(ty) => Some(ty),
                _ => None,
            })
            .collect(),
        PathArguments::Parenthesized(_) => return Type::Any,
    };
    let first = || args.first().map_or(Type::Any, |arg| parse(arg));
    let name = last.ident.to_string();
    match name.as_str() {
        "Vec" | "VecDeque" | "BTreeSet" | "HashSet" => Type::Array(Box::new(first())),
        "Box" | "Arc" | "Rc" => first(),
        "Option" => Type::Nullable(Box::new(first())),
        "BTreeMap" | "HashMap" => {
            Type::Map(Box::new(args.get(1).map_or(Type::Any, |arg| parse(arg))))
        }
        "String" | "str" | "char" => Type::Text,
        "bool" => Type::Bool,
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" => Type::Int,
        "f32" | "f64" => Type::Float,
        _ => Type::Named(name, args.iter().map(|arg| parse(arg)).collect()),
    }
}

// The `///` lines above an item as one doc line.
fn docs(attrs: &[Attribute]) -> Option<String> {
    let lines: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            let Meta::NameValue(pair) = &attr.meta else { return None };
            if !pair.path.is_ident("doc") {
                return None;
            }
            string(&pair.value).map(|text| text.trim().to_owned())
        })
        .collect();
    sentence(&lines)
}

// The wire name a `#[serde(rename = "..")]` gives the item.
fn rename(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().filter(|attr| attr.path().is_ident("serde")).find_map(|attr| {
        let nested = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).ok()?;
        nested.iter().find_map(|meta| {
            let Meta::NameValue(pair) = meta else { return None };
            pair.path.is_ident("rename").then(|| string(&pair.value)).flatten()
        })
    })
}

fn string(expr: &Expr) -> Option<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(text), ..
    }) = expr
    else {
        return None;
    };
    Some(text.value())
}
