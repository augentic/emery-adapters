//! Parses one Python module into the SDK's code model.

use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};

use emery_sdk::survey::Lines;
use emery_sdk::survey::code::{self, ClassDecl, Imported, MemberKind, Reexport};
use emery_sdk::survey::tests::Statement;

use super::DIALECT;

mod walk;

const DATA_BASES: &[&str] = &["TypedDict", "NamedTuple", "Protocol"];

pub fn data_bases(bases: &[String]) -> bool {
    DIALECT.enumerates(bases) || bases.iter().any(|base| DATA_BASES.contains(&base.as_str()))
}

pub fn declares_data(class: &ClassDecl) -> bool {
    data_bases(&class.bases)
        || (!class.members.is_empty()
            && class.members.iter().all(|member| match member.kind {
                MemberKind::Field
                | MemberKind::Getter
                | MemberKind::Setter
                | MemberKind::Nested => true,
                MemberKind::Method => member.name.starts_with("__") && member.name.ends_with("__"),
                MemberKind::Constructor => false,
            }))
}

#[derive(Debug, Default)]
pub struct Module {
    core: code::Module,
    pub main_guard: Option<Lines>,
    tests: Vec<TestDef>,
    all: Option<Vec<String>>,
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

    fn settle_exports(&mut self) {
        let is_init = self.is_init();
        let Self { core, all, .. } = self;
        let code::Module {
            exports,
            imports,
            reexports,
            types,
            classes,
            ..
        } = core;

        if let Some(all) = all {
            exports.retain(|export| all.contains(&export.name));
        } else {
            exports.retain(|export| !export.name.starts_with('_'));
        }

        let mut grouped: Vec<Reexport> = Vec::new();
        for import in imports.iter().filter(|import| !import.local.is_empty()) {
            let relative = import.specifier.starts_with('.');
            let listed = all.as_ref().is_some_and(|all| all.contains(&import.local));
            if !(listed || (is_init && relative)) {
                continue;
            }
            let Imported::Named(name) = &import.imported else { continue };
            let pair = (name.clone(), import.local.clone());
            match grouped.iter_mut().find(|re| re.specifier == import.specifier) {
                Some(Reexport {
                    names: Some(names), ..
                }) => names.push(pair),
                Some(_) => {}
                None => grouped.push(Reexport {
                    specifier: import.specifier.clone(),
                    names: Some(vec![pair]),
                    type_only: import.type_only,
                    target: None,
                }),
            }
        }
        if is_init {
            for import in imports.iter().filter(|import| import.imported == Imported::Star) {
                if !grouped.iter().any(|re| re.specifier == import.specifier) {
                    grouped.push(Reexport {
                        specifier: import.specifier.clone(),
                        names: None,
                        type_only: import.type_only,
                        target: None,
                    });
                }
            }
        }
        *reexports = grouped;

        let exported: BTreeSet<&str> = exports.iter().map(|export| export.name.as_str()).collect();
        for decl in types {
            decl.exported = exported.contains(decl.name.as_str());
        }
        for class in classes {
            class.exported = exported.contains(class.name.as_str());
        }
    }

    fn is_init(&self) -> bool {
        self.path.ends_with("__init__.py")
    }

    pub fn statements(&self) -> Vec<Statement> {
        self.tests
            .iter()
            .map(|test| {
                let title = test.doc.clone().unwrap_or_else(|| {
                    test.name.trim_start_matches("test").trim_start_matches('_').replace('_', " ")
                });
                let title = if test.parametrized {
                    format!("{title} (over several values)")
                } else {
                    title
                };
                let text = match &test.class {
                    Some(class) => format!("{} › {title}", class.trim_start_matches("Test")),
                    None => title,
                };
                Statement {
                    text,
                    line: test.lines.start,
                }
            })
            .collect()
    }
}

#[derive(Debug)]
struct TestDef {
    name: String,
    class: Option<String>,
    doc: Option<String>,
    parametrized: bool,
    lines: Lines,
}
