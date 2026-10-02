//! Reads one module into the facts the survey decides from.
//!
//! A `Module` is the SDK's [`code::Module`] with what Python alone declares
//! — the `__main__` guard, the tests, `__all__` — filled by the parser
//! confined to `walk`, so the rules in `surface` and `skeleton` never name a
//! syntax-tree type and a parser swap touches one file. A module the parser
//! cannot read is still a `Module`, read as far as the parser got with
//! `parsed` false, so one broken file never fails a run. Where each import
//! leads is settled once the tree is read (`Resolver::settle`) and carried
//! on the import.

use std::ops::{Deref, DerefMut};

use emery_sdk::survey::Lines;
use emery_sdk::survey::code::{self, ClassDecl, Imported, MemberKind, Reexport};

mod walk;

// Bases that declare data, beside an enumeration.
const DATA_BASES: &[&str] = &["TypedDict", "NamedTuple", "Protocol"];

// A type the caller copies, not an export it calls.
pub(super) fn data_bases(bases: &[String]) -> bool {
    bases.iter().any(|base| {
        base.ends_with("Enum") || base.ends_with("Flag") || DATA_BASES.contains(&base.as_str())
    })
}

pub(super) fn is_enum(class: &ClassDecl) -> bool {
    class.bases.iter().any(|base| base.ends_with("Enum") || base.ends_with("Flag"))
}

// A dunder method is a dataclass's or a model's, not a caller's. An empty
// body declares nothing either way.
pub(super) fn declares_data(class: &ClassDecl) -> bool {
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
    // The module-level `if __name__ == "__main__":`, if any.
    pub main_guard: Option<Lines>,
    // Every `def test_*`, at module level or in a class.
    pub tests: Vec<TestDef>,
    // `__all__`, where declared.
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

    // The exports are the names `__all__` lists, else every module-level
    // declaration not led by an underscore. The re-exports are:
    // - a listed name the module imports rather than declares
    // - every relative `from` import of an `__init__.py`
    // - a star import of an `__init__.py`, which re-exports everything
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

        // group the re-exports by specifier, in first-seen order
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

        // mark the declared types and classes exported
        let exported: Vec<&str> = exports.iter().map(|export| export.name.as_str()).collect();
        for decl in types {
            decl.exported = exported.contains(&decl.name.as_str());
        }
        for class in classes {
            class.exported = exported.contains(&class.name.as_str());
        }
    }

    pub fn is_init(&self) -> bool {
        self.path.ends_with("__init__.py")
    }
}

#[derive(Debug)]
pub struct TestDef {
    pub name: String,
    pub class: Option<String>,
    // The first line of the docstring, if any.
    pub doc: Option<String>,
    pub parametrized: bool,
    pub lines: Lines,
}
