//! Parses one TypeScript or JavaScript module into the SDK's code model.

use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};

use emery_sdk::survey::Lines;
use emery_sdk::survey::code::{self, BindingKind, Call, ExportKind, Scope};
use emery_sdk::survey::tests::Statement;

mod walk;

const SUITES: &[&str] = &["describe", "context", "suite"];
const CASES: &[&str] = &["it", "test", "specify"];
const RUNS: &[&str] = &["only", "concurrent", "sequential"];

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

    pub fn statements(&self) -> Vec<Statement> {
        let runs = |call: &Call| {
            call.callee.head_call.is_none()
                && call
                    .callee
                    .links
                    .iter()
                    .all(|link| link.call.is_none() && RUNS.contains(&link.name.as_str()))
        };
        let mut suites: Vec<(&str, Lines)> = Vec::new();
        let mut cases: Vec<(&str, Lines)> = Vec::new();
        for call in self.calls.iter().filter(|call| runs(call)) {
            let Some(title) = call.literal() else { continue };
            let head = call.callee.head.as_str();
            if SUITES.contains(&head) {
                if let Some(body) = call.args.iter().find(|arg| arg.function) {
                    suites.push((title, body.lines));
                }
            } else if CASES.contains(&head) {
                cases.push((title, call.lines));
            }
        }
        cases
            .into_iter()
            .map(|(title, lines)| {
                let mut titles: Vec<&str> = suites
                    .iter()
                    .filter(|(_, body)| body.contains(lines))
                    .map(|(t, _)| *t)
                    .collect();
                titles.push(title);
                Statement {
                    text: titles.join(" › "),
                    line: lines.start,
                }
            })
            .collect()
    }

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

        let exported: BTreeSet<&str> =
            exports.iter().map(|export| export.local.as_deref().unwrap_or(&export.name)).collect();
        for decl in types {
            decl.exported = exported.contains(decl.name.as_str());
        }
        for class in classes {
            class.exported = exported.contains(class.name.as_str());
        }
    }
}
