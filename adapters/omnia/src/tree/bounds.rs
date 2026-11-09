//! Holds a filled handler's `P` bound to the traits its body reaches, and a
//! deployment setting to the part's `#[derive(Settings)]` struct.

use std::collections::BTreeSet;

use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, FnArg, ItemConst, ItemFn, Lit, Pat, TypeParamBound};

use super::{Module, Tree};
use crate::syntax::{self, path_ends};

// Each accessor of the context, with the trait it borrows the provider as.
const ACCESSORS: &[(&str, &str)] = &[
    ("blobs", "BlobStore"),
    ("broadcast", "Broadcast"),
    ("config", "Config"),
    ("docs", "DocumentStore"),
    ("http", "HttpRequest"),
    ("identity", "Identity"),
    ("model", "Model"),
    ("plugins", "Plugins"),
    ("publish", "Publish"),
    ("state", "StateStore"),
    ("tables", "TableStore"),
    ("vault", "Vault"),
    ("vcs", "Vcs"),
];

impl Tree<'_> {
    pub fn broad_bounds(&self) -> Option<String> {
        let mut lines: Vec<String> = self.modules.iter().flat_map(Module::broad_bounds).collect();
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a filled handler is bounded on the traits its body reaches through the context, and \
             no wider, since `P: Capabilities` grants every capability and hides which the \
             handler uses: replace `P: Capabilities` on each handler named with the traits \
             listed, `where P: StateStore + Config` for `context.state()` and \
             `context.config()`, adding the bounds of any helper the body hands \
             `context.provider()` to, through `edit_files`:\n{}",
            lines.join("\n")
        ))
    }

    pub fn old_settings(&self) -> Option<String> {
        let mut lines: Vec<String> = self.modules.iter().flat_map(Module::old_settings).collect();
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a deployment setting a part requires is a field of its `#[derive(Settings)]` \
             struct, loaded once with `<Part>Settings::load(context.config()).await?`, and never \
             a `pub(crate) const` key read through `Config::get` or a key spelled at the read: \
             declare the struct, `#[setting(prefix = \"<PART>_\")]` over the shared prefix or \
             `#[setting(key = \"..\")]` per field, typed as its value parses, and replace each \
             read named through `edit_files`:\n{}",
            lines.join("\n")
        ))
    }
}

impl Module {
    fn broad_bounds(&self) -> Vec<String> {
        let Some(file) = self.file() else { return Vec::new() };
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(item) if syntax::is_handler_fn(item) => Some(item),
                _ => None,
            })
            .filter(|item| !syntax::holds_marker(&item.block) && names_capabilities(item))
            .map(|item| {
                let line = syntax::line(item.sig.fn_token.span);
                let reached = reached_traits(item);
                let traits = if reached.is_empty() {
                    "no capability through the context".to_owned()
                } else {
                    reached.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ")
                };
                format!(
                    "{}:{line}: handler `{}` is bounded `P: Capabilities` and reaches {traits}",
                    self.path, item.sig.ident
                )
            })
            .collect()
    }

    fn old_settings(&self) -> Vec<String> {
        let Some(file) = self.file() else { return Vec::new() };
        let mut reads = Reads {
            module: self,
            constants: file.items.iter().filter_map(setting_constant).collect(),
            lines: Vec::new(),
        };
        reads.visit_file(file);
        reads.lines
    }
}

// Whether the fn's generics bound a parameter on `Capabilities`, inline or
// in its `where` clause.
fn names_capabilities(item: &ItemFn) -> bool {
    struct Bounds(bool);

    impl<'ast> Visit<'ast> for Bounds {
        fn visit_type_param_bound(&mut self, node: &'ast TypeParamBound) {
            if let TypeParamBound::Trait(bound) = node {
                self.0 |= path_ends(&bound.path, &["Capabilities"]);
            }
            visit::visit_type_param_bound(self, node);
        }
    }

    let mut bounds = Bounds(false);
    bounds.visit_generics(&item.sig.generics);
    bounds.0
}

// The traits the body borrows the provider as, by the accessors it calls on
// the context parameter.
fn reached_traits(item: &ItemFn) -> BTreeSet<&'static str> {
    struct Reached<'c> {
        context: &'c str,
        traits: BTreeSet<&'static str>,
    }

    impl<'ast> Visit<'ast> for Reached<'_> {
        fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
            if node.args.is_empty()
                && matches!(&*node.receiver, Expr::Path(path) if path.path.is_ident(self.context))
                && let Some((_, name)) = ACCESSORS.iter().find(|(name, _)| node.method == name)
            {
                self.traits.insert(name);
            }
            visit::visit_expr_method_call(self, node);
        }
    }

    let context = match item.sig.inputs.first() {
        Some(FnArg::Typed(first)) => match &*first.pat {
            Pat::Ident(ident) => ident.ident.to_string(),
            _ => return BTreeSet::new(),
        },
        _ => return BTreeSet::new(),
    };
    let mut reached = Reached {
        context: &context,
        traits: BTreeSet::new(),
    };
    reached.visit_block(&item.block);
    reached.traits
}

// A `pub(crate) const KEY: &str = "KEY"` setting constant, by name.
fn setting_constant(item: &syn::Item) -> Option<String> {
    let syn::Item::Const(ItemConst { ident, expr, .. }) = item else { return None };
    let Expr::Lit(literal) = &**expr else { return None };
    let Lit::Str(value) = &literal.lit else { return None };
    let name = ident.to_string();
    let screaming = name.contains('_')
        && name.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_');
    (screaming && value.value() == name).then_some(name)
}

// Each required read of a setting, `Config::get(.., KEY).await?` and
// `context.config().get(KEY).await?`, under a setting constant or a key
// spelled at the read; a read whose failure the body handles itself, as
// `.await.is_ok_and(..)` or `.await.ok()`, is an optional setting and no
// finding.
struct Reads<'m> {
    module: &'m Module,
    constants: Vec<String>,
    lines: Vec<String>,
}

impl Reads<'_> {
    fn read(&mut self, line: usize, key: &Expr) {
        let spelled = match key {
            Expr::Lit(literal) => match &literal.lit {
                Lit::Str(value) => format!("`\"{}\"` spelled at the read", value.value()),
                _ => return,
            },
            Expr::Path(path) => {
                let Some(name) = path.path.get_ident().map(ToString::to_string) else { return };
                if !self.constants.contains(&name) {
                    return;
                }
                format!("the `pub(crate) const {name}` key")
            }
            Expr::Reference(reference) => return self.read(line, &reference.expr),
            _ => return,
        };
        self.lines.push(format!(
            "{}:{line}: a required setting is read under {spelled}",
            self.module.path
        ));
    }
}

impl<'ast> Visit<'ast> for Reads<'_> {
    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        let mut inner = &*node.expr;
        while let Expr::Await(awaited) = inner {
            inner = &awaited.base;
        }
        match inner {
            Expr::Call(ExprCall { func, args, .. }) => {
                if let Expr::Path(path) = &**func
                    && path_ends(&path.path, &["Config", "get"])
                    && let Some(key) = args.iter().nth(1)
                {
                    self.read(syntax::head_line(func).unwrap_or_default(), key);
                }
            }
            Expr::MethodCall(call) if call.method == "get" && call.args.len() == 1 => {
                if let Expr::MethodCall(accessor) = &*call.receiver
                    && accessor.method == "config"
                    && accessor.args.is_empty()
                    && let Some(key) = call.args.first()
                {
                    self.read(syntax::head_line(&call.receiver).unwrap_or_default(), key);
                }
            }
            _ => {}
        }
        visit::visit_expr_try(self, node);
    }
}
