//! What the scaffold and the checks read of Rust through `syn`: the parse, the lines, and the shapes several rules share.

use std::collections::BTreeMap;
use std::mem;

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprCall, ExprLit, ExprMethodCall, Item, ItemUse, Lit, LitStr, Macro,
    MetaList, Pat, PatGuard, PatIdent, Path, Token, UseTree, Visibility,
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

    pub fn names(&self, name: &str) -> bool {
        self.idents.contains_key(name)
    }

    // The first line `name` stands on as an identifier.
    pub fn first(&self, name: &str) -> Option<usize> {
        self.idents.get(name).copied()
    }

    // Whether a path leads with `head` and goes on, `<head>::<item>`.
    pub fn leads(&self, head: &str) -> bool {
        self.paths.iter().any(|path| path.first().is_some_and(|first| first == head))
    }

    // Whether a path ends with `tail`, `<stem>::router`.
    pub fn ends(&self, tail: &[&str]) -> bool {
        self.paths.iter().any(|path| {
            path.len() >= tail.len()
                && path.iter().rev().zip(tail.iter().rev()).all(|(segment, name)| segment == name)
        })
    }

    pub fn paths(&self) -> impl Iterator<Item = &[String]> {
        self.paths.iter().map(Vec::as_slice)
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

// The line of the bare `pub fn <name>` among the items of `file`, the `pub`
// bare: a `pub(crate)` one nothing calls is the compiler's to refuse.
pub fn pub_fn(file: &syn::File, name: &str) -> Option<usize> {
    file.items.iter().find_map(|item| {
        let Item::Fn(item) = item else { return None };
        (is_pub(&item.vis) && item.sig.ident == name).then(|| line(item.sig.fn_token.span))
    })
}

const fn is_pub(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
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
            let name = match &*item.self_ty {
                syn::Type::Path(path) => {
                    path.path.segments.last().map(|segment| segment.ident.to_string())
                }
                _ => None,
            };
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

// Whether `pat` leads with the tuple struct `name`: `Ok(..)`, `Some(..) | None`.
pub fn pat_leads(pat: &Pat, name: &str) -> bool {
    match pat {
        Pat::TupleStruct(tuple) => path_ends(&tuple.path, &[name]),
        Pat::Or(or) => or.cases.first().is_some_and(|case| pat_leads(case, name)),
        Pat::Paren(paren) => pat_leads(&paren.pat, name),
        Pat::Guard(guard) => pat_leads(&guard.pat, name),
        _ => false,
    }
}

// Whether `pat` names `name` anywhere within it: `None`, `Ok(None)`, `Some(..) | None`.
pub fn pat_names(pat: &Pat, name: &str) -> bool {
    struct Names<'n> {
        name: &'n str,
        found: bool,
    }

    impl<'ast> Visit<'ast> for Names<'_> {
        fn visit_pat_ident(&mut self, node: &'ast PatIdent) {
            self.found |= node.ident == self.name;
            visit::visit_pat_ident(self, node);
        }

        fn visit_path(&mut self, node: &'ast Path) {
            self.found |= path_ends(node, &[self.name]);
            visit::visit_path(self, node);
        }

        fn visit_pat_guard(&mut self, node: &'ast PatGuard) {
            self.visit_pat(&node.pat);
        }
    }

    let mut names = Names { name, found: false };
    names.visit_pat(pat);
    names.found
}

// Whether `call` is `Ok(())`.
pub fn is_unit_ok(call: &ExprCall) -> bool {
    let Expr::Path(func) = &*call.func else { return false };
    path_ends(&func.path, &["Ok"])
        && call.args.len() == 1
        && matches!(call.args.first(), Some(Expr::Tuple(tuple)) if tuple.elems.is_empty())
}

// Whether `expr` calls the method `name` with no arguments anywhere within it.
pub fn calls(expr: &Expr, name: &str) -> bool {
    struct Calls<'n> {
        name: &'n str,
        found: bool,
    }

    impl<'ast> Visit<'ast> for Calls<'_> {
        fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
            self.found |= node.method == self.name && node.args.is_empty();
            visit::visit_expr_method_call(self, node);
        }
    }

    let mut calls = Calls { name, found: false };
    calls.visit_expr(expr);
    calls.found
}

// The line a pattern starts on, for the shapes a catch opens with.
pub fn pat_line(pat: &Pat) -> Option<usize> {
    match pat {
        Pat::TupleStruct(tuple) => {
            tuple.path.segments.first().map(|segment| line(segment.ident.span()))
        }
        Pat::Or(or) => or.cases.first().and_then(pat_line),
        Pat::Paren(paren) => Some(line(paren.paren_token.span.open())),
        Pat::Guard(guard) => pat_line(&guard.pat),
        _ => None,
    }
}

// Whether a `#[derive(..)]` among `attrs` names `name`.
pub fn derives(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().filter(|attr| attr.path().is_ident("derive")).any(|attr| {
        attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
            .is_ok_and(|paths| paths.iter().any(|path| path_ends(path, &[name])))
    })
}

// The marker a `todo!("REQ-..")` carries: the line of the call and its text.
pub fn todo_marker(mac: &Macro) -> Option<(usize, String)> {
    if !path_ends(&mac.path, &["todo"]) {
        return None;
    }
    let text = mac.parse_body::<LitStr>().ok()?.value();
    text.starts_with("REQ-").then(|| (line(mac.bang_token.span), text))
}
