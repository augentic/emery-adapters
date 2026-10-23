//! Rejects `crates/start/` files and process-lifecycle functions or types in stem crates.

use emery_sdk::Error;
use syn::visit::{self, Visit};
use syn::{Ident, ImplItemFn, ItemFn, ItemStruct, TraitItemFn};

use super::Tree;
use crate::layout::start_files;
use crate::syntax;

const LIFECYCLE_FNS: &[(&str, &str)] = &[
    ("start", "a start of its own"),
    ("stop", "a stop of its own"),
    ("run", "a run of its own"),
    ("listen", "a listener of its own"),
    ("serve", "a server of its own"),
    ("bootstrap", "a bootstrap of its own"),
    ("shutdown", "a shutdown of its own"),
];

const LIFECYCLE_TYPES: &[&str] = &["Main", "App", "Server", "Application"];

impl Tree<'_> {
    pub fn simulated_process(&self) -> Result<Vec<String>, Error> {
        let start = start_files(self.root)?;
        let mut lifecycle = Vec::new();
        for module in &self.modules {
            let mut found = Lifecycle::default();
            if let Some(file) = module.file() {
                found.visit_file(file);
            }
            for (number, what, spelled) in found.found {
                lifecycle.push(format!("{}:{number}: {what} (`{spelled}`)", module.path));
            }
        }
        let mut findings = Vec::new();
        if !start.is_empty() {
            findings.push(format!(
                "`crates/start/` is written for `start`, which the seed and the host are and no \
                 crate of the tree ever is: a deployment setting a handler reads is a `pub(crate) \
                 const` of the part that reads it, a cross-cutting rule is a `pub fn` of the \
                 stem that owns it, a stated health route is its own stem's, and how the source \
                 readied itself, loaded its credentials or tables, or shut down is written \
                 nowhere; \
                 remove every file named under `crates/start` through `delete` and drop any \
                 `start` line from the root `Cargo.toml` through `edit_files`:\n{}",
                start.join("\n")
            ));
        }
        if !lifecycle.is_empty() {
            lifecycle.sort();
            findings.push(format!(
                "a module is the process the source was, which the host is and the tree never \
                 simulates; remove each module named, with its `pub mod` line and its test, \
                 through `delete` rather than rename it, and leave what it stood for to the \
                 report:\n{}",
                lifecycle.join("\n")
            ));
        }
        Ok(findings)
    }
}

#[derive(Default)]
struct Lifecycle {
    found: Vec<(usize, &'static str, String)>,
}

impl Lifecycle {
    fn function(&mut self, line: usize, ident: &Ident) {
        if let Some((name, what)) = LIFECYCLE_FNS.iter().find(|(name, _)| ident == name) {
            self.found.push((line, what, format!("fn {name}")));
        }
    }
}

impl<'ast> Visit<'ast> for Lifecycle {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.function(syntax::line(node.sig.fn_token.span), &node.sig.ident);
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.function(syntax::line(node.sig.fn_token.span), &node.sig.ident);
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast TraitItemFn) {
        self.function(syntax::line(node.sig.fn_token.span), &node.sig.ident);
        visit::visit_trait_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        if let Some(name) = LIFECYCLE_TYPES.iter().find(|name| node.ident == name) {
            let line = syntax::line(node.struct_token.span);
            self.found.push((line, "a lifecycle type of its own", format!("struct {name}")));
        }
        visit::visit_item_struct(self, node);
    }
}
