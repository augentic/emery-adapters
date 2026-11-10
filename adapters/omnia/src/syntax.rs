//! What the scaffold and the checks read of Rust through `syn`: the parse, the lines, and the shapes several rules share.

use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    Expr, ExprLit, ExprMethodCall, FnArg, Item, ItemFn, ItemUse, Lit, LitStr, Macro, MetaList,
    Path, Type, UseTree, Visibility,
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
    joined.trim_end_matches('{').trim_end().to_owned()
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
