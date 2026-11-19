//! What the scaffold and the checks read of Rust through `syn`: the parse, the lines, and the shapes several rules share.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Range, RangeInclusive};
use std::path::Path as StdPath;
use std::{fmt, mem};

use emery_sdk::Error;
use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprLit, ExprMethodCall, Fields, FnArg, ImplItemConst, Item, ItemConst,
    ItemFn, ItemImpl, ItemStruct, ItemTrait, ItemUse, Lit, LitStr, Macro, MetaList, Path, Token,
    Type, TypeParamBound, UseTree, Visibility, WherePredicate,
};

// Where `syn` refused a file, and why.
pub struct Unparsed {
    pub line: usize,
    pub message: String,
}

// Every name a file mentions: each identifier at the first line it stands
// on, and each path of two or more segments, read from the syntax tree and
// from the token streams of its macros and attribute lists. A name inside a
// string literal or a comment is no mention.
#[derive(Default)]
pub struct Mentions {
    idents: BTreeMap<String, usize>,
    paths: Vec<Vec<String>>,
}

impl Mentions {
    pub fn of(file: &syn::File) -> Self {
        let mut mentions = Self::default();
        mentions.visit_file(file);
        mentions
    }

    pub fn of_item(item: &Item) -> Self {
        let mut mentions = Self::default();
        mentions.visit_item(item);
        mentions
    }

    // Whether `name` stands anywhere as an identifier.
    pub fn contains(&self, name: &str) -> bool {
        self.idents.contains_key(name)
    }

    // The first line `name` stands on as an identifier.
    pub fn first(&self, name: &str) -> Option<usize> {
        self.idents.get(name).copied()
    }

    // Whether a path leads with `head` and goes on, `<head>::<item>`.
    pub fn path_leads(&self, head: &str) -> bool {
        self.paths.iter().any(|path| path.first().is_some_and(|first| first == head))
    }

    // Whether a path ends with `tail`, `<stem>::router`.
    pub fn path_ends(&self, tail: &[&str]) -> bool {
        self.paths.iter().any(|path| {
            path.len() >= tail.len()
                && path.iter().rev().zip(tail.iter().rev()).all(|(segment, name)| segment == name)
        })
    }

    pub fn paths(&self) -> impl Iterator<Item = &[String]> {
        self.paths.iter().map(Vec::as_slice)
    }

    pub fn idents(&self) -> impl Iterator<Item = &str> {
        self.idents.keys().map(String::as_str)
    }

    fn ident(&mut self, ident: &Ident) {
        let at = line(ident.span());
        self.idents
            .entry(ident.to_string())
            .and_modify(|first| *first = (*first).min(at))
            .or_insert(at);
    }

    fn path(&mut self, segments: Vec<String>) {
        if segments.len() > 1 {
            self.paths.push(segments);
        }
    }

    // The identifiers and `a::b` paths of a stream the parser left as
    // tokens: a macro's arguments, an attribute list's.
    fn tokens(&mut self, stream: TokenStream) {
        let mut path: Vec<String> = Vec::new();
        let mut colons = 0;
        for token in stream {
            match token {
                TokenTree::Ident(ident) => {
                    self.ident(&ident);
                    if colons != 2 {
                        self.path(mem::take(&mut path));
                    }
                    path.push(ident.to_string());
                    colons = 0;
                }
                TokenTree::Punct(punct) if punct.as_char() == ':' => colons += 1,
                TokenTree::Group(group) => {
                    self.path(mem::take(&mut path));
                    colons = 0;
                    self.tokens(group.stream());
                }
                TokenTree::Punct(_) | TokenTree::Literal(_) => {
                    self.path(mem::take(&mut path));
                    colons = 0;
                }
            }
        }
        self.path(path);
    }
}

impl<'ast> Visit<'ast> for Mentions {
    fn visit_ident(&mut self, node: &'ast Ident) {
        self.ident(node);
    }

    fn visit_path(&mut self, node: &'ast Path) {
        self.path(node.segments.iter().map(|segment| segment.ident.to_string()).collect());
        visit::visit_path(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut paths = Vec::new();
        use_paths(&node.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.path(path);
        }
        visit::visit_item_use(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        self.tokens(node.tokens.clone());
        visit::visit_macro(self, node);
    }

    fn visit_meta_list(&mut self, node: &'ast MetaList) {
        self.tokens(node.tokens.clone());
        visit::visit_meta_list(self, node);
    }
}

// Every path a `use` tree imports, `use a::{b::c, d}` being `a::b::c` and
// `a::d`, a glob's last segment `*`.
fn use_paths(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    let leaf = |prefix: &[String], last: &str| {
        prefix.iter().cloned().chain([last.to_owned()]).collect::<Vec<_>>()
    };
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            use_paths(&path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => out.push(leaf(prefix, &name.ident.to_string())),
        UseTree::Rename(rename) => out.push(leaf(prefix, &rename.ident.to_string())),
        UseTree::Glob(_) => out.push(leaf(prefix, "*")),
        UseTree::Group(group) => {
            for item in &group.items {
                use_paths(item, prefix, out);
            }
        }
    }
}

// A `pub const` at the top of a file: its line, its name, and its value
// where that is a string literal.
pub struct Const {
    pub line: usize,
    pub name: String,
    pub value: Option<String>,
}

pub fn parse(text: &str) -> Result<syn::File, Unparsed> {
    syn::parse_file(text).map_err(|err| Unparsed {
        line: line(err.span()),
        message: err.to_string(),
    })
}

pub fn line(span: Span) -> usize {
    span.start().line
}

// Every bare `pub const` among the items of `file`, in order.
pub fn pub_consts(file: &syn::File) -> Vec<Const> {
    file.items
        .iter()
        .filter_map(|item| {
            let Item::Const(item) = item else { return None };
            is_pub(&item.vis).then(|| Const {
                line: line(item.const_token.span),
                name: item.ident.to_string(),
                value: match &*item.expr {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(lit), ..
                    }) => Some(lit.value()),
                    _ => None,
                },
            })
        })
        .collect()
}

const fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

// The line of the bare `pub fn <name>` among the items of `file`, the `pub`
// bare: a `pub(crate)` one nothing calls is the compiler's to refuse.
pub fn pub_fn(file: &syn::File, name: &str) -> Option<usize> {
    file.items.iter().find_map(|item| {
        let Item::Fn(item) = item else { return None };
        (is_pub(&item.vis) && item.sig.ident == name).then(|| line(item.sig.fn_token.span))
    })
}

// A bare `pub fn` a stem crate exports from a module (`path` from the crate
// root, `customers::customers::authenticate`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedFn {
    pub path: String,
    pub file: String,
    pub line: usize,
    pub signature: String,
    pub doc: Option<String>,
}

// A bare `pub fn` among the items of a file: its name, the lines its
// signature runs over (`pub` to the body's opening brace), and the first
// line of its doc where it carries one.
pub struct Exported {
    pub name: String,
    pub from: usize,
    pub to: usize,
    pub doc: Option<String>,
}

pub fn exported_fns(file: &syn::File) -> Vec<Exported> {
    file.items
        .iter()
        .filter_map(|item| {
            let Item::Fn(item) = item else { return None };
            let Visibility::Public(pub_token) = &item.vis else { return None };
            let doc = item.attrs.iter().find_map(|attr| {
                if !attr.path().is_ident("doc") {
                    return None;
                }
                let syn::Meta::NameValue(pair) = &attr.meta else { return None };
                let Expr::Lit(ExprLit {
                    lit: Lit::Str(text), ..
                }) = &pair.value
                else {
                    return None;
                };
                let text = text.value();
                let trimmed = text.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_owned())
            });
            Some(Exported {
                name: item.sig.ident.to_string(),
                from: line(pub_token.span),
                to: line(item.block.brace_token.span.open()),
                doc,
            })
        })
        .collect()
}

// The signature of an exported fn as `text` spells it, its lines joined
// on one and the body's opening brace dropped.
pub fn signature(text: &str, exported: &Exported) -> String {
    let joined = text
        .lines()
        .skip(exported.from.saturating_sub(1))
        .take(exported.to.saturating_sub(exported.from) + 1)
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    joined.split('{').next().unwrap_or(&joined).trim_end().to_owned()
}

// The `pub fn`s the stem crate at `crates/<dir>/` exports from its part
// modules, in path order; a file `syn` refuses is passed over, since the
// checks name it.
pub fn crate_exported_fns(root: &StdPath, dir: &str) -> Result<Vec<ExportedFn>, Error> {
    let stem = crate::layout::stem_of(dir);
    let crate_ident = crate::layout::stem_ident(stem);
    let mut exports = Vec::new();
    for source in crate::layout::sources(&root.join("crates").join(dir).join("src"))? {
        let Some(module) = source.file_stem().and_then(|name| name.to_str()) else { continue };
        if module == "lib" {
            continue;
        }
        let text = crate::layout::read(&source)?;
        let Ok(file) = parse(&text) else { continue };
        let relative = crate::layout::relative(root, &source);
        for exported in exported_fns(&file) {
            if exported.name == "handlers" {
                continue;
            }
            exports.push(ExportedFn {
                path: format!("{crate_ident}::{module}::{}", exported.name),
                file: relative.clone(),
                line: exported.from,
                signature: signature(&text, &exported),
                doc: exported.doc,
            });
        }
    }
    Ok(exports)
}

// The modules `file` declares from files beside it, `pub mod reading;`, by
// name; one with a body inline declares no file.
pub fn mods(file: &syn::File) -> Vec<String> {
    file.items
        .iter()
        .filter_map(|item| {
            let Item::Mod(item) = item else { return None };
            item.content.is_none().then(|| item.ident.to_string())
        })
        .collect()
}

// The declaration an item opens with, `fn read`, `struct Order`, `mod
// tests`, and the line it stands on, for the items a finding names; a `use`
// or an `extern crate` is none.
pub fn item_head(item: &Item) -> Option<(usize, String)> {
    let (span, head) = match item {
        Item::Fn(item) => (item.sig.fn_token.span, format!("fn {}", item.sig.ident)),
        Item::Struct(item) => (item.struct_token.span, format!("struct {}", item.ident)),
        Item::Enum(item) => (item.enum_token.span, format!("enum {}", item.ident)),
        Item::Union(item) => (item.union_token.span, format!("union {}", item.ident)),
        Item::Const(item) => (item.const_token.span, format!("const {}", item.ident)),
        Item::Static(item) => (item.static_token.span, format!("static {}", item.ident)),
        Item::Type(item) => (item.type_token.span, format!("type {}", item.ident)),
        Item::Trait(item) => (item.trait_token.span, format!("trait {}", item.ident)),
        Item::TraitAlias(item) => (item.trait_token.span, format!("trait {}", item.ident)),
        Item::Mod(item) => (item.mod_token.span, format!("mod {}", item.ident)),
        Item::Impl(item) => {
            let self_name = match &*item.self_ty {
                syn::Type::Path(path) => {
                    path.path.segments.last().map(|segment| segment.ident.to_string())
                }
                _ => None,
            };
            let trait_name = item.trait_.as_ref().and_then(|(path, _)| {
                path.segments.last().map(|segment| segment.ident.to_string())
            });
            let is_blanket = self_name
                .as_deref()
                .is_some_and(|name| item.generics.type_params().any(|param| param.ident == name));
            let name = if is_blanket { trait_name.or(self_name) } else { self_name.or(trait_name) };
            let head = name.map_or_else(|| "impl".to_owned(), |name| format!("impl {name}"));
            (item.impl_token.span, head)
        }
        Item::Macro(item) => {
            let segment = item.mac.path.segments.last()?;
            (segment.ident.span(), format!("{}!", segment.ident))
        }
        _ => return None,
    };
    Some((line(span), head))
}

// How a top-level item's declaration is named by touch points and findings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemHead {
    Use,
    Named(String),
}

impl fmt::Display for ItemHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use => f.write_str("use"),
            Self::Named(name) => f.write_str(name),
        }
    }
}

// A top-level item's opening line and declaration head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredItem {
    pub line: usize,
    pub head: ItemHead,
}

pub fn declared_item(item: &Item) -> DeclaredItem {
    match item {
        Item::Use(item) => DeclaredItem {
            line: line(item.use_token.span),
            head: ItemHead::Use,
        },
        Item::ExternCrate(item) => DeclaredItem {
            line: line(item.extern_token.span),
            head: ItemHead::Named(format!("extern crate {}", item.ident)),
        },
        Item::ForeignMod(item) => DeclaredItem {
            line: line(item.abi.extern_token.span),
            head: ItemHead::Named("extern".to_owned()),
        },
        _ => {
            let (line, name) = item_head(item).unwrap_or_else(|| (1, "item".to_owned()));
            DeclaredItem {
                line,
                head: ItemHead::Named(name),
            }
        }
    }
}

// Every top-level item of `file` that names a crate `matches_crate` accepts,
// either by a `<crate>::..` path or through a symbol imported from one by a
// top-level `use`.
pub fn items_mentioning(
    file: &syn::File, matches_crate: impl Fn(&str) -> bool,
) -> Vec<DeclaredItem> {
    let mut imported = BTreeSet::new();
    for item in &file.items {
        if let Item::Use(item) = item {
            collect_imported(&item.tree, None, false, &matches_crate, &mut imported);
        }
    }

    let mut found = Vec::new();
    for item in &file.items {
        let mentions_target = if let Item::Use(item) = item {
            use_mentions(&item.tree, &matches_crate)
        } else {
            let mentions = Mentions::of_item(item);
            mentions.paths().any(|path| path.first().is_some_and(|head| matches_crate(head)))
                || imported.iter().any(|symbol| mentions.contains(symbol))
        };
        if mentions_target {
            found.push(declared_item(item));
        }
    }
    found
}

fn collect_imported(
    tree: &UseTree, parent: Option<&Ident>, under_target: bool,
    matches_crate: &impl Fn(&str) -> bool, symbols: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            let targeted = under_target || matches_crate(&path.ident.to_string());
            if targeted || parent.is_none() {
                collect_imported(&path.tree, Some(&path.ident), targeted, matches_crate, symbols);
            }
        }
        UseTree::Name(name) if under_target => {
            if name.ident == "self" {
                if let Some(module) = parent {
                    let module = module.to_string();
                    if !matches_crate(&module) {
                        symbols.insert(module);
                    }
                }
            } else {
                symbols.insert(name.ident.to_string());
            }
        }
        UseTree::Rename(rename)
            if (under_target || (parent.is_none() && matches_crate(&rename.ident.to_string())))
                && rename.rename != "_" =>
        {
            symbols.insert(rename.rename.to_string());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_imported(item, parent, under_target, matches_crate, symbols);
            }
        }
        _ => {}
    }
}

fn use_mentions(tree: &UseTree, matches_crate: &impl Fn(&str) -> bool) -> bool {
    match tree {
        UseTree::Path(path) => matches_crate(&path.ident.to_string()),
        UseTree::Name(name) => matches_crate(&name.ident.to_string()),
        UseTree::Rename(rename) => matches_crate(&rename.ident.to_string()),
        UseTree::Group(group) => group.items.iter().any(|item| use_mentions(item, matches_crate)),
        UseTree::Glob(_) => false,
    }
}

// The line an expression starts on, for the shapes a finding is spelled at.
pub fn head_line(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Call(call) => head_line(&call.func),
        Expr::MethodCall(call) => head_line(&call.receiver),
        Expr::Await(awaited) => head_line(&awaited.base),
        Expr::Try(tried) => head_line(&tried.expr),
        Expr::Field(field) => head_line(&field.base),
        Expr::Path(path) => path.path.segments.first().map(|segment| line(segment.ident.span())),
        Expr::Macro(mac) => mac.mac.path.segments.first().map(|segment| line(segment.ident.span())),
        Expr::Reference(reference) => Some(line(reference.and_token.span)),
        Expr::Return(returned) => Some(line(returned.return_token.span)),
        Expr::Tuple(tuple) => Some(line(tuple.paren_token.span.open())),
        Expr::Paren(paren) => Some(line(paren.paren_token.span.open())),
        Expr::Block(block) => Some(line(block.block.brace_token.span.open())),
        _ => None,
    }
}

pub fn path_ends(path: &Path, tail: &[&str]) -> bool {
    path.segments.len() >= tail.len()
        && path
            .segments
            .iter()
            .rev()
            .zip(tail.iter().rev())
            .all(|(segment, name)| segment.ident == name)
}

// The marker a `todo!("REQ-..")` carries: the line of the call and its text.
pub fn todo_marker(mac: &Macro) -> Option<(usize, String)> {
    if !path_ends(&mac.path, &["todo"]) {
        return None;
    }
    let text = mac.parse_body::<LitStr>().ok()?.value();
    text.starts_with("REQ-").then(|| (line(mac.bang_token.span), text))
}

// The top-level declarations of a file, partitioned by namespace, with the
// string constants `pub(crate) const` declares.
#[derive(Default)]
pub struct Declared {
    pub fns: Vec<String>,
    pub types: Vec<String>,
    pub constants: Vec<String>,
    pub paths: Vec<(String, String)>,
}

impl Declared {
    pub fn of(file: &syn::File) -> Self {
        let mut declared = Self::default();
        for item in &file.items {
            match item {
                Item::Fn(item) => declared.fns.push(item.sig.ident.to_string()),
                Item::Struct(item) => declared.types.push(item.ident.to_string()),
                Item::Enum(item) => declared.types.push(item.ident.to_string()),
                Item::Type(item) => declared.types.push(item.ident.to_string()),
                Item::Union(item) => declared.types.push(item.ident.to_string()),
                Item::Trait(item) => declared.types.push(item.ident.to_string()),
                Item::Const(item) => {
                    let name = item.ident.to_string();
                    if is_crate(&item.vis)
                        && let Expr::Lit(ExprLit {
                            lit: Lit::Str(lit), ..
                        }) = &*item.expr
                    {
                        declared.paths.push((name.clone(), lit.value()));
                    }
                    declared.constants.push(name);
                }
                Item::Static(item) => declared.constants.push(item.ident.to_string()),
                _ => {}
            }
        }
        declared
    }
}

fn is_crate(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Restricted(restricted) if restricted.path.is_ident("crate"))
}

// The closing line of the last top-level `use` item in `file`.
pub fn last_use_line(file: &syn::File) -> Option<usize> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Use(item) => Some(line(item.semi_token.span)),
            _ => None,
        })
        .max()
}

// Whether `item` is the stem entry's `pub trait Requires` or its empty `impl`.
pub fn is_requires_item(item: &Item) -> bool {
    match item {
        Item::Trait(item) => is_requires_trait(item),
        Item::Impl(item) => is_requires_impl(item),
        _ => false,
    }
}

fn is_requires_trait(item: &ItemTrait) -> bool {
    is_pub(&item.vis) && item.ident == "Requires" && item.items.is_empty()
}

fn is_requires_impl(item: &ItemImpl) -> bool {
    item.modifiers.polarity.is_none()
        && item.items.is_empty()
        && item.trait_.as_ref().is_some_and(|(path, _)| path_ends(path, &["Requires"]))
}

fn is_blanket_requires_impl(item: &ItemImpl) -> bool {
    if !is_requires_impl(item) || item.generics.params.len() != 1 {
        return false;
    }
    let Some(param) = item.generics.type_params().next() else { return false };
    matches!(&*item.self_ty, Type::Path(path) if path.path.is_ident(&param.ident))
}

// Whether `file`'s `Requires` trait and its `impl` both bound on `trait_name`.
pub fn requires_bounds(file: &syn::File, trait_name: &str) -> bool {
    let trait_has = file.items.iter().any(|item| {
        let Item::Trait(item) = item else { return false };
        is_requires_trait(item) && bounds_include(item.supertraits.iter(), trait_name)
    });
    let impl_has = file.items.iter().any(|item| {
        let Item::Impl(item) = item else { return false };
        is_requires_impl(item) && impl_bounds_include(item, trait_name)
    });
    trait_has && impl_has
}

fn bounds_include<'b>(mut bounds: impl Iterator<Item = &'b TypeParamBound>, name: &str) -> bool {
    bounds.any(
        |bound| matches!(bound, TypeParamBound::Trait(bound) if path_ends(&bound.path, &[name])),
    )
}

fn impl_bounds_include(item: &ItemImpl, name: &str) -> bool {
    let inline = item.generics.type_params().any(|param| bounds_include(param.bounds.iter(), name));
    let clauses =
        item.generics.where_clause.iter().flat_map(|clause| &clause.predicates).any(|predicate| {
            match predicate {
                WherePredicate::Type(ty) => bounds_include(ty.bounds.iter(), name),
                _ => false,
            }
        });
    inline || clauses
}

// Byte and line spans of `pub trait Requires` and its blanket `impl` in an
// entry file, with its current supertrait spellings.
pub struct RequiresSpans {
    pub trait_bytes: Range<usize>,
    pub trait_lines: RangeInclusive<usize>,
    pub impl_bytes: Range<usize>,
    pub impl_lines: RangeInclusive<usize>,
    pub bounds: Vec<String>,
}

pub fn requires_spans(text: &str, file: &syn::File) -> Option<RequiresSpans> {
    let mut traits = file.items.iter().filter_map(|item| match item {
        Item::Trait(item) if is_requires_trait(item) => Some(item),
        _ => None,
    });
    let trait_item = traits.next().filter(|_| traits.next().is_none())?;
    let Visibility::Public(pub_token) = &trait_item.vis else { return None };

    let mut impls = file.items.iter().filter_map(|item| match item {
        Item::Impl(item) if is_blanket_requires_impl(item) => Some(item),
        _ => None,
    });
    let impl_item = impls.next().filter(|_| impls.next().is_none())?;

    let (trait_start, trait_end) =
        (pub_token.span.start(), trait_item.brace_token.span.close().end());
    let (impl_start, impl_end) =
        (impl_item.impl_token.span.start(), impl_item.brace_token.span.close().end());

    let trait_bytes = byte_offset(text, trait_start.line, trait_start.column)?
        ..byte_offset(text, trait_end.line, trait_end.column)?;
    let impl_bytes = byte_offset(text, impl_start.line, impl_start.column)?
        ..byte_offset(text, impl_end.line, impl_end.column)?;
    let bounds = trait_item.supertraits.iter().map(bound_spelling).collect::<Option<Vec<_>>>()?;

    Some(RequiresSpans {
        trait_bytes,
        trait_lines: trait_start.line..=trait_end.line,
        impl_bytes,
        impl_lines: impl_start.line..=impl_end.line,
        bounds,
    })
}

fn bound_spelling(bound: &TypeParamBound) -> Option<String> {
    match bound {
        TypeParamBound::Trait(bound) => {
            if bound.lifetimes.is_some()
                || bound.maybe.is_some()
                || bound.modifiers != syn::TraitBoundModifiers::default()
                || bound.path.segments.iter().any(|segment| !segment.arguments.is_empty())
            {
                return None;
            }
            let segments: Vec<String> =
                bound.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            Some(segments.join("::"))
        }
        TypeParamBound::Lifetime(lifetime) => Some(format!("'{}", lifetime.ident)),
        _ => None,
    }
}

// Top-level `use` block span in `text` and its `module -> names` bindings,
// when every `use` is public/private without attributes or `as` renames.
pub struct UseBlock {
    pub bytes: Range<usize>,
    pub entries: Vec<(String, Vec<String>)>,
}

pub fn use_block(text: &str, file: &syn::File) -> Option<UseBlock> {
    let mut first_span = None;
    let mut last_span = None;
    let mut seen_non_use = false;
    let mut entries = Vec::new();
    for item in &file.items {
        let Item::Use(item) = item else {
            if first_span.is_some() {
                seen_non_use = true;
            }
            continue;
        };
        if seen_non_use || !matches!(item.vis, Visibility::Inherited) || !item.attrs.is_empty() {
            return None;
        }
        first_span.get_or_insert_with(|| item.use_token.span.start());
        last_span = Some(item.semi_token.span.end());
        collect_use_entries(&item.tree, &mut Vec::new(), &mut entries)?;
    }
    let (start, end) = (first_span?, last_span?);
    let bytes =
        byte_offset(text, start.line, start.column)?..byte_offset(text, end.line, end.column)?;
    Some(UseBlock { bytes, entries })
}

fn collect_use_entries(
    tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<(String, Vec<String>)>,
) -> Option<()> {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_entries(&path.tree, prefix, out)?;
            prefix.pop();
            Some(())
        }
        UseTree::Name(name) if !prefix.is_empty() => {
            out.push((prefix.join("::"), vec![name.ident.to_string()]));
            Some(())
        }
        UseTree::Group(group) if !prefix.is_empty() => {
            let mut leaves = Vec::new();
            for sub in &group.items {
                match sub {
                    UseTree::Name(name) => leaves.push(name.ident.to_string()),
                    _ => collect_use_entries(sub, prefix, out)?,
                }
            }
            if !leaves.is_empty() {
                out.push((prefix.join("::"), leaves));
            }
            Some(())
        }
        _ => None,
    }
}

// Line ranges of root `use omnia_sdk::..` capability imports (excluding
// `use omnia_sdk::api::..`), which a whole write of an entry may widen.
pub fn sdk_capability_use_lines(file: &syn::File) -> Vec<RangeInclusive<usize>> {
    file.items
        .iter()
        .filter_map(|item| {
            let Item::Use(item) = item else { return None };
            let UseTree::Path(path) = &item.tree else { return None };
            let is_api = matches!(&*path.tree, UseTree::Path(sub) if sub.ident == "api");
            (path.ident == "omnia_sdk" && !is_api)
                .then(|| line(item.use_token.span)..=line(item.semi_token.span))
        })
        .collect()
}

// The methods a part's `handlers()` chains over `Handlers::new()`.
const REGISTRATIONS: [&str; 4] = ["route", "topic", "topic_with", "command"];

// The byte offset where the registration chain in the top-level `fn
// <name>` ends: the closing `)` of the last `.route(..)` or `.topic(..)`
// call, or of the `Handlers::new()` receiver, so a new call appended there
// extends the chain.
pub fn chain_end(text: &str, file: &syn::File, name: &str) -> Option<usize> {
    let item = file.items.iter().find_map(|item| match item {
        Item::Fn(item) if item.sig.ident == name => Some(item),
        _ => None,
    })?;
    let syn::Stmt::Expr(expr, None) = item.block.stmts.last()? else {
        return None;
    };
    let span = match expr {
        Expr::MethodCall(call) if is_registration_chain(expr) => call.paren_token.span.close(),
        Expr::Call(call) if is_handlers_new(call) => call.paren_token.span.close(),
        _ => return None,
    };
    let end = span.end();
    byte_offset(text, end.line, end.column)
}

fn is_registration_chain(mut expr: &Expr) -> bool {
    while let Expr::MethodCall(call) = expr {
        if !REGISTRATIONS.iter().any(|method| call.method == method) {
            return false;
        }
        expr = &call.receiver;
    }
    matches!(expr, Expr::Call(call) if is_handlers_new(call))
}

fn is_handlers_new(call: &syn::ExprCall) -> bool {
    call.args.is_empty()
        && matches!(&*call.func, Expr::Path(path) if path_ends(&path.path, &["Handlers", "new"]))
}

fn byte_offset(text: &str, line: usize, column: usize) -> Option<usize> {
    let mut current_line = 1;
    let mut offset = 0;
    for segment in text.split_inclusive('\n') {
        if current_line == line {
            let col_bytes: usize = segment.chars().take(column).map(char::len_utf8).sum();
            return (col_bytes <= segment.len()).then_some(offset + col_bytes);
        }
        offset += segment.len();
        current_line += 1;
    }
    (current_line == line && column == 0).then_some(offset)
}

// Every `.topic(..)` registration of the top-level `fn handlers` in `file`
// whose handler is filled, at its line: the registration that leaves the
// decoder the SDK's, JSON. One whose handler still holds its marker is a
// stub the scaffold laid, left to the turn.
pub fn json_registrations(file: &syn::File) -> Vec<usize> {
    let stubs: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(item) if holds_marker(&item.block) => Some(item.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    registrations(file, &["topic"], &stubs)
}

// The lines of the calls of `methods` in the top-level `fn handlers`, less
// those registering a handler of `stubs`.
fn registrations(file: &syn::File, methods: &[&str], stubs: &BTreeSet<String>) -> Vec<usize> {
    let mut found = Registrations {
        methods,
        stubs,
        lines: Vec::new(),
    };
    for item in &file.items {
        if let Item::Fn(item) = item
            && item.sig.ident == "handlers"
        {
            found.visit_block(&item.block);
        }
    }
    found.lines
}

struct Registrations<'s> {
    methods: &'s [&'s str],
    stubs: &'s BTreeSet<String>,
    lines: Vec<usize>,
}

impl<'ast> Visit<'ast> for Registrations<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
        if self.methods.iter().any(|method| call.method == method) {
            let handler = call.args.iter().nth(1).and_then(|arg| match arg {
                Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
                _ => None,
            });
            if !handler.is_some_and(|handler| self.stubs.contains(&handler)) {
                self.lines.push(line(call.method.span()));
            }
        }
        visit::visit_expr_method_call(self, call);
    }
}

// Every `.topic(..)` and `.topic_with(..)` registration of the top-level
// `fn handlers` in `file`, at its line, a stub's among them: the part
// consumes a stream whatever decodes it.
pub fn topic_registrations(file: &syn::File) -> Vec<usize> {
    registrations(file, &["topic", "topic_with"], &BTreeSet::new())
}

// The line of the `Handlers::from(..)` fold in the top-level `fn handlers`
// of an entry, where a part's `handlers()` is listed.
pub fn handlers_from(file: &syn::File) -> Option<usize> {
    struct Fold(Option<usize>);

    impl<'ast> Visit<'ast> for Fold {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if self.0.is_none()
                && let Expr::Path(path) = &*call.func
                && path_ends(&path.path, &["Handlers", "from"])
            {
                self.0 = Some(line(call.paren_token.span.open()));
            }
            visit::visit_expr_call(self, call);
        }
    }

    let mut fold = Fold(None);
    for item in &file.items {
        if let Item::Fn(item) = item
            && item.sig.ident == "handlers"
        {
            fold.visit_block(&item.block);
        }
    }
    fold.0
}

// Whether `block` holds a `todo!("REQ-..")` marker, so the item is a stub
// the scaffold laid and the turn has not filled.
pub fn holds_marker(block: &syn::Block) -> bool {
    struct Markers(bool);

    impl<'ast> Visit<'ast> for Markers {
        fn visit_macro(&mut self, node: &'ast Macro) {
            self.0 |= todo_marker(node).is_some();
            visit::visit_macro(self, node);
        }
    }

    let mut markers = Markers(false);
    markers.visit_block(block);
    markers.0
}

// Whether a block still holds a `todo!` of any text: the gate's stub,
// whose marker names no requirement.
pub fn holds_todo(block: &syn::Block) -> bool {
    struct Todos(bool);

    impl<'ast> Visit<'ast> for Todos {
        fn visit_macro(&mut self, node: &'ast Macro) {
            self.0 |= path_ends(&node.path, &["todo"]);
            visit::visit_macro(self, node);
        }
    }

    let mut todos = Todos(false);
    todos.visit_block(block);
    todos.0
}

// Every `pub(crate) async fn(<context>: Context<..>, ..)` handler in
// `file` that `handlers()` names in no `.route(..)`, `.topic(..)`, or
// `.topic_with(..)`, at its line.
pub fn unregistered_handlers(file: &syn::File) -> Vec<(usize, String)> {
    let registered: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(item) if item.sig.ident == "handlers" => Some(Idents::of(&item.block)),
            _ => None,
        })
        .flatten()
        .collect();
    file.items
        .iter()
        .filter_map(|item| {
            let Item::Fn(item) = item else { return None };
            let name = item.sig.ident.to_string();
            (is_handler_fn(item) && !registered.contains(&name))
                .then(|| (line(item.sig.fn_token.span), name))
        })
        .collect()
}

#[derive(Default)]
struct Idents(BTreeSet<String>);

impl Idents {
    fn of(block: &syn::Block) -> BTreeSet<String> {
        let mut idents = Self::default();
        idents.visit_block(block);
        idents.0
    }
}

impl<'ast> Visit<'ast> for Idents {
    fn visit_ident(&mut self, node: &'ast Ident) {
        self.0.insert(node.to_string());
    }
}

// A handler takes the context first: a `pub(crate) async fn` whose first
// parameter is a `Context<..>`.
pub fn is_handler_fn(item: &ItemFn) -> bool {
    if !is_crate(&item.vis) || item.sig.asyncness.is_none() {
        return false;
    }
    let Some(FnArg::Typed(first)) = item.sig.inputs.first() else {
        return false;
    };
    matches!(&*first.ty, Type::Path(ty) if path_ends(&ty.path, &["Context"]))
}

// Whether a `#[derive(..)]` among `attrs` names `name`.
pub fn derives(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().filter(|attr| attr.path().is_ident("derive")).any(|attr| {
        attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
            .is_ok_and(|paths| paths.iter().any(|path| path_ends(path, &[name])))
    })
}

// A setting a file declares, at its line: a `const KEY: &str = "KEY"` or a
// field of a `#[derive(Settings)]` struct, keyed as the derive spells it.
pub struct SettingKey {
    pub line: usize,
    pub key: String,
}

pub fn setting_keys(file: &syn::File) -> Vec<SettingKey> {
    #[derive(Default)]
    struct Keys {
        found: Vec<SettingKey>,
    }

    impl Keys {
        fn constant(&mut self, at: usize, name: &Ident, expr: &Expr) {
            let Expr::Lit(ExprLit {
                lit: Lit::Str(value), ..
            }) = expr
            else {
                return;
            };
            let key = name.to_string();
            if value.value() == key {
                self.found.push(SettingKey { line: at, key });
            }
        }
    }

    impl<'ast> Visit<'ast> for Keys {
        fn visit_item_const(&mut self, node: &'ast ItemConst) {
            self.constant(line(node.const_token.span), &node.ident, &node.expr);
            visit::visit_item_const(self, node);
        }

        fn visit_impl_item_const(&mut self, node: &'ast ImplItemConst) {
            self.constant(line(node.const_token.span), &node.ident, &node.expr);
            visit::visit_impl_item_const(self, node);
        }

        fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
            if !derives(&node.attrs, "Settings") {
                return;
            }
            let prefix = setting_attr(&node.attrs, "prefix").unwrap_or_default();
            let Fields::Named(fields) = &node.fields else { return };
            for field in &fields.named {
                let Some(ident) = &field.ident else { continue };
                let key = setting_attr(&field.attrs, "key").unwrap_or_else(|| {
                    let raw = ident.to_string();
                    let stem = raw.strip_prefix("r#").unwrap_or(&raw);
                    format!("{prefix}{}", stem.to_ascii_uppercase())
                });
                self.found.push(SettingKey {
                    line: line(ident.span()),
                    key,
                });
            }
        }
    }

    let mut keys = Keys::default();
    keys.visit_file(file);
    keys.found
}

fn setting_attr(attrs: &[Attribute], name: &str) -> Option<String> {
    attrs.iter().filter(|attr| attr.path().is_ident("setting")).find_map(|attr| {
        let mut value = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(name) {
                let lit: LitStr = meta.value()?.parse()?;
                value = Some(lit.value());
            } else if meta.input.peek(Token![=]) {
                let _: Expr = meta.value()?.parse()?;
            }
            Ok(())
        });
        value
    })
}

// Every string literal `file` spells outside doc comments, including inside
// macro and attribute token streams.
pub fn string_literals(file: &syn::File) -> Vec<String> {
    #[derive(Default)]
    struct Literals {
        found: Vec<String>,
    }

    impl Literals {
        fn tokens(&mut self, stream: TokenStream) {
            for token in stream {
                match token {
                    TokenTree::Literal(literal) => {
                        if let Lit::Str(lit) = Lit::new(literal) {
                            self.found.push(lit.value());
                        }
                    }
                    TokenTree::Group(group) => self.tokens(group.stream()),
                    TokenTree::Ident(_) | TokenTree::Punct(_) => {}
                }
            }
        }
    }

    impl<'ast> Visit<'ast> for Literals {
        fn visit_attribute(&mut self, node: &'ast Attribute) {
            if node.path().is_ident("doc") {
                return;
            }
            visit::visit_attribute(self, node);
        }

        fn visit_lit_str(&mut self, node: &'ast LitStr) {
            self.found.push(node.value());
        }

        fn visit_macro(&mut self, node: &'ast Macro) {
            self.tokens(node.tokens.clone());
            visit::visit_macro(self, node);
        }

        fn visit_meta_list(&mut self, node: &'ast MetaList) {
            self.tokens(node.tokens.clone());
            visit::visit_meta_list(self, node);
        }
    }

    let mut literals = Literals::default();
    literals.visit_file(file);
    literals.found
}

// Every string literal `file` spells outside attributes, at its opening line,
// including inside macro token streams.
pub fn spanned_literals(file: &syn::File) -> Vec<(usize, String)> {
    #[derive(Default)]
    struct Literals {
        found: Vec<(usize, String)>,
    }

    impl Literals {
        fn tokens(&mut self, stream: TokenStream) {
            for token in stream {
                match token {
                    TokenTree::Literal(literal) => {
                        let at = line(literal.span());
                        if let Lit::Str(lit) = Lit::new(literal) {
                            self.found.push((at, lit.value()));
                        }
                    }
                    TokenTree::Group(group) => self.tokens(group.stream()),
                    TokenTree::Ident(_) | TokenTree::Punct(_) => {}
                }
            }
        }
    }

    impl<'ast> Visit<'ast> for Literals {
        fn visit_attribute(&mut self, _: &'ast Attribute) {}

        fn visit_lit_str(&mut self, node: &'ast LitStr) {
            self.found.push((line(node.span()), node.value()));
        }

        fn visit_macro(&mut self, node: &'ast Macro) {
            self.tokens(node.tokens.clone());
            visit::visit_macro(self, node);
        }
    }

    let mut literals = Literals::default();
    literals.visit_file(file);
    literals.found
}
