//! Reads one module into the facts the survey decides from.
//!
//! A `Module` is the SDK's [`code::Module`], filled by the parser confined
//! to `walk`, so the rules in `surface` and `skeleton` never name a
//! syntax-tree type and a parser swap touches one file. A module the parser
//! cannot read is still a `Module`, read as far as the parser got with
//! `parsed` false, so one broken file never fails a run. Where each import
//! leads is settled once the tree is read (`Resolver::settle`) and carried
//! on the import.

use std::ops::{Deref, DerefMut};

use emery_sdk::survey::Lines;
use emery_sdk::survey::code::{self, BindingKind, ExportKind, Scope};

mod walk;

#[derive(Debug, Default)]
pub struct Module {
    core: code::Module,
}

impl Deref for Module {
    type Target = code::Module;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for Module {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl Module {
    // `path` is root-relative. Imports are the resolver's to settle.
    pub fn parse(path: &str, text: String) -> Self {
        let mut module = walk::read(path, &text);
        module.settle_exports();
        module.span = Lines {
            start: 1,
            end: u32::try_from(text.lines().count()).unwrap_or(u32::MAX).max(1),
        };
        module.text = text;
        module
    }

    // An `export { local as name }` list names a binding declared elsewhere
    // in the module; its kind is that binding's.
    fn settle_exports(&mut self) {
        let code::Module {
            exports,
            bindings,
            types,
            classes,
            ..
        } = &mut self.core;
        for export in exports.iter_mut().filter(|export| export.kind == ExportKind::Unknown) {
            let Some(local) = &export.local else { continue };
            let kind = bindings
                .iter()
                .find(|b| &b.name == local && b.scope == Scope::Module)
                .map(|b| match &b.kind {
                    BindingKind::Function => ExportKind::Function,
                    BindingKind::Class { .. } => ExportKind::Class,
                    _ => ExportKind::Value,
                })
                .or_else(|| types.iter().any(|t| &t.name == local).then_some(ExportKind::Type));
            if let Some(kind) = kind {
                export.kind = kind;
            }
        }

        // mark the declared types and classes exported
        let exported: Vec<&str> =
            exports.iter().map(|export| export.local.as_deref().unwrap_or(&export.name)).collect();
        for decl in types {
            decl.exported = exported.contains(&decl.name.as_str());
        }
        for class in classes {
            class.exported = exported.contains(&class.name.as_str());
        }
    }
}
