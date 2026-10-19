//! What the checks read of a module's syntax tree: the parse, the lines, and the shapes several rules share.

use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprCall, ExprMethodCall, LitStr, Macro, Pat, PatGuard, PatIdent, Path, Token,
};

// Where `syn` refused a file, and why.
pub(super) struct Unparsed {
    pub(super) line: usize,
    pub(super) message: String,
}

pub(super) fn parse(text: &str) -> Result<syn::File, Unparsed> {
    syn::parse_file(text).map_err(|err| Unparsed {
        line: line(err.span()),
        message: err.to_string(),
    })
}

pub(super) fn line(span: Span) -> usize {
    span.start().line
}

// The line an expression starts on, for the shapes a finding is spelled at.
pub(super) fn head_line(expr: &Expr) -> Option<usize> {
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

pub(super) fn path_ends(path: &Path, tail: &[&str]) -> bool {
    path.segments.len() >= tail.len()
        && path
            .segments
            .iter()
            .rev()
            .zip(tail.iter().rev())
            .all(|(segment, name)| segment.ident == name)
}

// Whether `pat` leads with the tuple struct `name`: `Ok(..)`, `Some(..) | None`.
pub(super) fn pat_leads(pat: &Pat, name: &str) -> bool {
    match pat {
        Pat::TupleStruct(tuple) => path_ends(&tuple.path, &[name]),
        Pat::Or(or) => or.cases.first().is_some_and(|case| pat_leads(case, name)),
        Pat::Paren(paren) => pat_leads(&paren.pat, name),
        Pat::Guard(guard) => pat_leads(&guard.pat, name),
        _ => false,
    }
}

// Whether `pat` names `name` anywhere within it: `None`, `Ok(None)`, `Some(..) | None`.
pub(super) fn pat_names(pat: &Pat, name: &str) -> bool {
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
pub(super) fn is_unit_ok(call: &ExprCall) -> bool {
    let Expr::Path(func) = &*call.func else { return false };
    path_ends(&func.path, &["Ok"])
        && call.args.len() == 1
        && matches!(call.args.first(), Some(Expr::Tuple(tuple)) if tuple.elems.is_empty())
}

// Whether `expr` calls the method `name` with no arguments anywhere within it.
pub(super) fn calls(expr: &Expr, name: &str) -> bool {
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
pub(super) fn pat_line(pat: &Pat) -> Option<usize> {
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
pub(super) fn derives(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().filter(|attr| attr.path().is_ident("derive")).any(|attr| {
        attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
            .is_ok_and(|paths| paths.iter().any(|path| path_ends(path, &[name])))
    })
}

// The marker a `todo!("REQ-..")` carries: the line of the call and its text.
pub(super) fn todo_marker(mac: &Macro) -> Option<(usize, String)> {
    if !path_ends(&mac.path, &["todo"]) {
        return None;
    }
    let text = mac.parse_body::<LitStr>().ok()?.value();
    text.starts_with("REQ-").then(|| (line(mac.bang_token.span), text))
}
