//! Checks for empty cache misses on constant `StateStore` keys, single-field
//! `data` response wrappers, refusals answered `Ok(())`, and format settings.

use syn::visit::{self, Visit};
use syn::{
    Arm, Expr, ExprCall, ExprIf, ExprMethodCall, ExprReturn, Fields, ImplItemConst, ImplItemFn,
    Item, ItemConst, ItemFn, ItemStruct, Lit, Local, Pat, Stmt, TraitItemFn,
};

use super::syntax::{self, calls, derives, is_unit_ok, pat_leads, pat_names, path_ends};
use super::{Module, Tree};

// The calls that answer an empty collection, each with its spelling.
const NOTHING: &[(&[&str], &str)] = &[
    (&["Vec", "new"], "Vec::new()"),
    (&["HashMap", "new"], "HashMap::new()"),
    (&["BTreeMap", "new"], "BTreeMap::new()"),
    (&["HashSet", "new"], "HashSet::new()"),
    (&["BTreeSet", "new"], "BTreeSet::new()"),
    (&["Default", "default"], "Default::default()"),
];

const EMPTY_VEC: &str = "vec![]";

// The arguments of `unwrap_or_else` that default a miss, each with its spelling.
const DEFAULTED_WITH: &[(&[&str], &str)] = &[
    (&["Vec", "new"], ".unwrap_or_else(Vec::new)"),
    (&["HashMap", "new"], ".unwrap_or_else(HashMap::new)"),
    (&["Default", "default"], ".unwrap_or_else(Default::default)"),
];

impl Tree<'_> {
    pub(super) fn empty_misses(&self) -> Option<String> {
        let mut lines: Vec<String> = self.modules.iter().flat_map(Module::empty_misses).collect();
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        lines.dedup();
        Some(format!(
            "a table read from the store under one key answers empty on a miss, so the table is \
             never loaded: on `Ok(None)` the function fetches the table from its upstream, as the \
             design spells the call, stores it under the same key with a TTL, and answers what it \
             fetched, never an empty collection; repair each read named through `edit_files`, \
             writing the fetch in the same function:\n{}",
            lines.join("\n")
        ))
    }

    pub(super) fn response_wrappers(&self) -> Option<String> {
        let mut lines = Vec::new();
        for module in &self.modules {
            if module.is_common() {
                continue;
            }
            for (number, name) in module.response_wrappers() {
                lines.push(format!(
                    "{}:{number}: response wrapper struct `{name}` wraps a single `data` field",
                    module.path
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a struct wraps an HTTP response in a single `data` field, which is the source's \
             HTTP client property (`response.data`) and never a field on the wire: remove each \
             wrapper struct named through `edit_files` and deserialize `response.body()` \
             directly into the body's own shape, a top-level `Vec<T>` for a JSON array or a \
             struct with the body's fields at its root:\n{}",
            lines.join("\n")
        ))
    }

    pub(super) fn swallowed_rejections(&self) -> Option<String> {
        let mut lines: Vec<String> =
            self.modules.iter().flat_map(Module::swallowed_rejections).collect();
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a delivery or a call the code refused is answered `Ok(())`, as processed, so the \
             refusal is lost: return the error with `?` — `bad_request!` for a validation or \
             timeliness rejection, `bad_gateway!` for an upstream that refused — and answer \
             `Ok(())` only for a valid message the rule filters out; repair each line named \
             through `edit_files`:\n{}",
            lines.join("\n")
        ))
    }

    pub(super) fn format_settings(&self) -> Option<String> {
        let mut lines = Vec::new();
        for module in &self.modules {
            for (number, name) in module.format_settings() {
                lines.push(format!("{}:{number}: setting `{name}` names a format", module.path));
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a format is the parser's syntax and never a deployment value: a date or time format \
             a criterion spells (`DD/MM/YYYY`) is a `const` of the module that parses with it, \
             holding the parser's own syntax (`\"%d/%m/%Y\"`), and no setting read through \
             `Config::get`; replace each setting named with that constant through \
             `edit_files`:\n{}",
            lines.join("\n")
        ))
    }
}

impl Module {
    fn swallowed_rejections(&self) -> Vec<String> {
        let mut swallowed = Swallowed {
            module: self,
            open: Vec::new(),
            lines: Vec::new(),
        };
        if let Some(file) = self.file() {
            swallowed.visit_file(file);
        }
        swallowed.lines
    }

    fn format_settings(&self) -> Vec<(usize, String)> {
        let mut settings = Settings::default();
        if let Some(file) = self.file() {
            settings.visit_file(file);
        }
        settings.found
    }

    fn empty_misses(&self) -> Vec<String> {
        let mut functions = Functions {
            module: self,
            lines: Vec::new(),
        };
        if let Some(file) = self.file() {
            functions.visit_file(file);
        }
        functions.lines
    }

    fn response_wrappers(&self) -> Vec<(usize, String)> {
        let mut wrappers = Wrappers::default();
        if let Some(file) = self.file() {
            wrappers.visit_file(file);
        }
        wrappers.found
    }
}

// A refusal is caught under one of the shapes `Catch::of_*` name; each
// `Ok(())` within what the catch runs answers it as processed, once per
// catch standing open around it.
struct Swallowed<'m> {
    module: &'m Module,
    open: Vec<Catch>,
    lines: Vec<String>,
}

struct Catch {
    opened: &'static str,
    line: usize,
}

impl Swallowed<'_> {
    fn caught(&mut self, catch: Catch, body: impl FnOnce(&mut Self)) {
        self.open.push(catch);
        body(self);
        self.open.pop();
    }
}

impl<'ast> Visit<'ast> for Swallowed<'_> {
    fn visit_item(&mut self, node: &'ast Item) {
        let open = std::mem::take(&mut self.open);
        visit::visit_item(self, node);
        self.open = open;
    }

    fn visit_local(&mut self, node: &'ast Local) {
        let Some(init) = &node.init else { return };
        self.visit_expr(&init.expr);
        let Some((_, diverge)) = &init.diverge else { return };
        let opened = if pat_leads(&node.pat, "Ok") {
            "`let Ok(..) = .. else` block"
        } else if pat_leads(&node.pat, "Some") && calls(&init.expr, "ok") {
            "`.ok() else` block"
        } else {
            return self.visit_expr(diverge);
        };
        let catch = Catch {
            opened,
            line: syntax::line(node.let_token.span),
        };
        self.caught(catch, |this| this.visit_expr(diverge));
    }

    fn visit_expr_if(&mut self, node: &'ast ExprIf) {
        self.visit_expr(&node.cond);
        let opened = match &*node.cond {
            Expr::Let(bound) if pat_leads(&bound.pat, "Err") => Some("`if let Err(..)` branch"),
            Expr::Let(_) => None,
            cond if calls(cond, "is_err") => Some("`is_err()` branch"),
            _ => None,
        };
        match opened {
            Some(opened) => {
                let catch = Catch {
                    opened,
                    line: syntax::line(node.if_token.span),
                };
                self.caught(catch, |this| this.visit_block(&node.then_branch));
            }
            None => self.visit_block(&node.then_branch),
        }
        if let Some((_, otherwise)) = &node.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_arm(&mut self, node: &'ast Arm) {
        if !pat_leads(&node.pat, "Err") {
            return visit::visit_arm(self, node);
        }
        let catch = Catch {
            opened: "`Err(..) =>` arm",
            line: syntax::pat_line(&node.pat)
                .unwrap_or_else(|| syntax::line(node.fat_arrow_token.spans[0])),
        };
        self.caught(catch, |this| this.visit_expr(&node.body));
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if is_unit_ok(node) {
            let line = syntax::head_line(&node.func).unwrap_or_default();
            for catch in &self.open {
                let opener = if catch.line == line {
                    String::new()
                } else {
                    format!(" at line {}", catch.line)
                };
                self.lines.push(format!(
                    "{}:{line}: `Ok(())` answers the refusal the {}{opener} caught",
                    self.module.path, catch.opened
                ));
            }
        }
        visit::visit_expr_call(self, node);
    }
}

#[derive(Default)]
struct Settings {
    found: Vec<(usize, String)>,
}

impl Settings {
    fn setting(&mut self, line: usize, name: &syn::Ident, expr: &Expr) {
        let Expr::Lit(literal) = expr else { return };
        let Lit::Str(value) = &literal.lit else { return };
        let name = name.to_string();
        if value.value() == name && name.split('_').any(|segment| segment == "FORMAT") {
            self.found.push((line, name));
        }
    }
}

impl<'ast> Visit<'ast> for Settings {
    fn visit_item_const(&mut self, node: &'ast ItemConst) {
        self.setting(syntax::line(node.const_token.span), &node.ident, &node.expr);
        visit::visit_item_const(self, node);
    }

    fn visit_impl_item_const(&mut self, node: &'ast ImplItemConst) {
        self.setting(syntax::line(node.const_token.span), &node.ident, &node.expr);
        visit::visit_impl_item_const(self, node);
    }
}

#[derive(Default)]
struct Wrappers {
    found: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for Wrappers {
    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let Fields::Named(fields) = &node.fields else { return };
        let data = fields.named.len() == 1
            && fields
                .named
                .first()
                .and_then(|field| field.ident.as_ref())
                .is_some_and(|ident| ident == "data");
        if data && derives(&node.attrs, "Deserialize") {
            self.found.push((syntax::line(node.struct_token.span), node.ident.to_string()));
        }
    }
}

// Each function body read for its misses on its own, nested functions
// among them.
struct Functions<'m> {
    module: &'m Module,
    lines: Vec<String>,
}

impl Functions<'_> {
    fn body(&mut self, block: &syn::Block) {
        let mut misses = Misses {
            module: self.module,
            read: None,
            closed: false,
            lines: Vec::new(),
        };
        misses.visit_block(block);
        self.lines.append(&mut misses.lines);
    }
}

impl<'ast> Visit<'ast> for Functions<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.body(&node.block);
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.body(&node.block);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast TraitItemFn) {
        if let Some(block) = &node.default {
            self.body(block);
        }
        visit::visit_trait_item_fn(self, node);
    }
}

// From a read of the store under a constant key to the end of the function,
// skipping what a hit runs — a `Some(..)` or `Ok(Some(..))` arm, an
// `if let Some(..)` branch — and ending at a `let Some(..) = .. else`, past
// which the hit alone runs, what answers a miss empty is a finding.
struct Misses<'m> {
    module: &'m Module,
    read: Option<usize>,
    closed: bool,
    lines: Vec<String>,
}

impl Misses<'_> {
    fn found(&mut self, line: usize, what: &str, spelling: &str) {
        let Some(read) = self.read else { return };
        self.lines.push(format!(
            "{}:{line}: {what} (`{spelling}`) after the read at line {read}",
            self.module.path
        ));
    }

    fn answered(&mut self, line: usize, spelling: &str) {
        self.found(line, "a miss answered as no entries", spelling);
    }

    fn defaulted(&mut self, line: usize, spelling: &str) {
        self.found(line, "a miss defaulted", spelling);
    }
}

impl<'ast> Visit<'ast> for Misses<'_> {
    fn visit_item(&mut self, _: &'ast Item) {}

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        if self.closed {
            return;
        }
        if self.read.is_some()
            && let Stmt::Expr(expr, _) = node
            && let Some(spelling) = empty_answer(expr)
        {
            self.answered(syntax::head_line(expr).unwrap_or_default(), &spelling);
            return;
        }
        visit::visit_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        if !self.closed {
            visit::visit_expr(self, node);
        }
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(line) = constant_keyed_read(node) {
            self.read = Some(line);
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_return(&mut self, node: &'ast ExprReturn) {
        if self.read.is_some()
            && let Some(spelling) = node.expr.as_deref().and_then(empty_answer)
        {
            self.answered(syntax::line(node.return_token.span), &format!("return {spelling}"));
            return;
        }
        visit::visit_expr_return(self, node);
    }

    // The receiver first, since a read defaulted where it is made,
    // `StateStore::get(..).await?.unwrap_or_default()`, is the receiver.
    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        visit::visit_expr_method_call(self, node);
        if self.read.is_some()
            && let Some(spelling) = defaulted(node)
        {
            self.defaulted(syntax::line(node.method.span()), spelling);
        }
    }

    fn visit_arm(&mut self, node: &'ast Arm) {
        if self.read.is_none() {
            return visit::visit_arm(self, node);
        }
        if is_hit(&node.pat) {
            return;
        }
        if pat_names(&node.pat, "None")
            && let Some(spelling) = empty_answer(&node.body)
        {
            let arrow = syntax::line(node.fat_arrow_token.spans[0]);
            let line = syntax::head_line(&node.body).unwrap_or(arrow);
            let spelled = if line == arrow { self.module.spelled(line) } else { spelling };
            self.answered(line, &spelled);
            return;
        }
        visit::visit_arm(self, node);
    }

    fn visit_expr_if(&mut self, node: &'ast ExprIf) {
        self.visit_expr(&node.cond);
        let hit =
            self.read.is_some() && matches!(&*node.cond, Expr::Let(bound) if is_hit(&bound.pat));
        if !hit {
            self.visit_block(&node.then_branch);
        }
        if let Some((_, otherwise)) = &node.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_local(&mut self, node: &'ast Local) {
        let Some(init) = &node.init else { return };
        self.visit_expr(&init.expr);
        let Some((_, diverge)) = &init.diverge else { return };
        self.visit_expr(diverge);
        if self.read.is_some() && pat_leads(&node.pat, "Some") {
            self.closed = true;
        }
    }
}

// A `StateStore::get(.., KEY)` under a constant key, and the line it reads at.
fn constant_keyed_read(call: &ExprCall) -> Option<usize> {
    let Expr::Path(func) = &*call.func else { return None };
    if !path_ends(&func.path, &["StateStore", "get"]) {
        return None;
    }
    let mut key = call.args.iter().nth(1)?;
    while let Expr::Reference(reference) = key {
        key = &reference.expr;
    }
    let Expr::Path(key) = key else { return None };
    let name = key.path.segments.last()?.ident.to_string();
    let constant = name.chars().any(|ch| ch.is_ascii_uppercase())
        && name.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
    constant.then(|| syntax::head_line(&call.func)).flatten()
}

// A `Some(..)` or `Ok(Some(..))` pattern, which the hit runs under.
fn is_hit(pat: &Pat) -> bool {
    if pat_leads(pat, "Some") {
        return true;
    }
    match pat {
        Pat::TupleStruct(tuple) if path_ends(&tuple.path, &["Ok"]) => {
            tuple.elems.len() == 1
                && tuple.elems.first().is_some_and(|inner| pat_leads(inner, "Some"))
        }
        Pat::Or(or) => or.cases.first().is_some_and(is_hit),
        Pat::Paren(paren) => is_hit(&paren.pat),
        _ => false,
    }
}

// An empty collection, spelled as `NOTHING` has it.
fn nothing(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::Call(call) if call.args.is_empty() => {
            let Expr::Path(func) = &*call.func else { return None };
            NOTHING
                .iter()
                .find(|(tail, _)| path_ends(&func.path, tail))
                .map(|(_, spelling)| *spelling)
        }
        Expr::Macro(mac) if path_ends(&mac.mac.path, &["vec"]) && mac.mac.tokens.is_empty() => {
            Some(EMPTY_VEC)
        }
        _ => None,
    }
}

// An empty collection answered bare or under `Ok(..)`, spelled.
fn empty_answer(expr: &Expr) -> Option<String> {
    if let Some(spelling) = nothing(expr) {
        return Some(spelling.to_owned());
    }
    let Expr::Call(call) = expr else { return None };
    let Expr::Path(func) = &*call.func else { return None };
    if !path_ends(&func.path, &["Ok"]) || call.args.len() != 1 {
        return None;
    }
    call.args.first().and_then(nothing).map(|spelling| format!("Ok({spelling})"))
}

// A method call that defaults a miss, spelled as `DEFAULTED_WITH` has it.
fn defaulted(call: &ExprMethodCall) -> Option<&'static str> {
    let arg = match call.args.len() {
        0 => None,
        1 => call.args.first(),
        _ => return None,
    };
    if call.method == "unwrap_or_default" && arg.is_none() {
        return Some(".unwrap_or_default()");
    }
    if call.method == "unwrap_or_else"
        && let Some(Expr::Path(with)) = arg
    {
        return DEFAULTED_WITH
            .iter()
            .find(|(tail, _)| path_ends(&with.path, tail))
            .map(|(_, spelling)| *spelling);
    }
    if call.method == "unwrap_or"
        && let Some(with) = arg
        && nothing(with) == Some("Vec::new()")
    {
        return Some(".unwrap_or(Vec::new())");
    }
    None
}
