//! Reads one module into the facts the survey decides from.
//!
//! A `Module` is plain data. The parser is confined to `walk`, so the rules
//! in `surface` and `skeleton` never name a syntax-tree type and a parser
//! swap touches one file. A module the parser cannot read is still a
//! `Module`, read as far as the parser got with `parsed` false, so one
//! broken file never fails a run. Where each import leads is settled once
//! the tree is read (`Resolver::settle`) and carried on the import.

use emery_sdk::survey::Lines;
use emery_sdk::survey::resolve::Target;

use super::unique;

mod walk;

// Methods whose function arguments are structure, not handlers: iteration,
// wrapping, scheduling onto the loop, mounting, dependency declaration. A
// call through one registers nothing, and a function passed to one runs at
// its caller's depth. Matched against a call's method, or its head when it
// has none.
const STRUCTURAL: &[&str] = &[
    "map",
    "filter",
    "sorted",
    "sort",
    "reduce",
    "partial",
    "wraps",
    "run",
    "run_until_complete",
    "create_task",
    "gather",
    "ensure_future",
    "run_in_executor",
    "to_thread",
    "register_blueprint",
    "include_router",
    "add_middleware",
    "mount",
    "setdefault",
    "Depends",
    "Security",
    "raises",
    "fixture",
    "parametrize",
    "field",
    "Field",
    "Column",
    "mapped_column",
    "relationship",
];

// `unittest.mock`'s `patch` and pytest-mock's, by their spelling whole. The
// same name on an application, a router, or an HTTP client is a verb.
const MOCKING: &[&str] = &["patch", "mock.patch", "unittest.mock.patch", "mocker.patch"];

// Hooks on the process, a connection, or the application's lifecycle, not
// registrations — Django's admin site among them, which serves what it
// registers on the source's behalf, by call or by decorator. Matched by the
// tail of the dotted spelling.
const LIFECYCLE: &[&str] = &[
    "signal.signal",
    "atexit.register",
    "add_signal_handler",
    "on_event",
    "add_event_handler",
    "add_exception_handler",
    "register_error_handler",
    "teardown_appcontext",
    "teardown_request",
    "lifespan",
    "site.register",
    "admin.register",
    "admin.action",
    "admin.display",
];

// Decorators that shape what they decorate rather than register it with a
// framework. A `def` under one of these alone is no handler.
const DECORATOR_NOISE: &[&str] = &[
    "dataclass",
    "define",
    "frozen",
    "property",
    "setter",
    "getter",
    "deleter",
    "staticmethod",
    "classmethod",
    "cached_property",
    "lru_cache",
    "cache",
    "wraps",
    "overload",
    "abstractmethod",
    "override",
    "final",
    "contextmanager",
    "asynccontextmanager",
    "login_required",
    "permission_required",
    "csrf_exempt",
    "require_http_methods",
    "require_POST",
    "require_GET",
    "atomic",
    "retry",
    "validator",
    "field_validator",
    "model_validator",
    "root_validator",
    "computed_field",
    "total_ordering",
    "unique",
    "fixture",
    "parametrize",
    "mark",
    "skip",
    "skipif",
];

// Decorators that hook what they decorate onto an application's or a
// signal's lifecycle rather than register a surface for a caller.
const DECORATOR_HOOKS: &[&str] = &[
    "connect",
    "receiver",
    "listens_for",
    "exception_handler",
    "errorhandler",
    "error_handler",
    "before_request",
    "after_request",
    "before_first_request",
    "middleware",
    "context_processor",
    "template_filter",
    "url_value_preprocessor",
    "url_defaults",
];

// Keywords a function is handed under as a hook on the thing being built,
// not as the handler it registers.
const HOOK_KEYWORDS: &[&str] = &[
    "lifespan",
    "on_startup",
    "on_shutdown",
    "exception_handlers",
    "default_factory",
    "default",
    "key",
    "callback",
    "dependencies",
    "middleware",
    "result_callback",
];

// Stems that name a role rather than a thing; the module takes its
// directory's name instead.
const GENERIC_STEMS: &[&str] = &[
    "__init__",
    "__main__",
    "main",
    "app",
    "views",
    "urls",
    "routes",
    "api",
    "handlers",
    "endpoints",
    "tasks",
];

pub(super) fn shapes(decorator: &str) -> bool {
    DECORATOR_NOISE.contains(&decorator)
}

// Bases that declare data, beside an enumeration.
const DATA_BASES: &[&str] = &["TypedDict", "NamedTuple", "Protocol"];

// A type the caller copies, not an export it calls.
pub(super) fn declares_data(bases: &[String]) -> bool {
    bases.iter().any(|base| {
        base.ends_with("Enum") || base.ends_with("Flag") || DATA_BASES.contains(&base.as_str())
    })
}

fn hooks(dotted: &str) -> bool {
    LIFECYCLE.iter().any(|tail| dotted == *tail || dotted.ends_with(&format!(".{tail}")))
}

#[derive(Debug, Default)]
pub struct Module {
    pub path: String,
    pub text: String,
    // First line to last, one line at the least.
    pub span: Lines,
    pub parsed: bool,
    pub imports: Vec<Import>,
    pub reexports: Vec<Reexport>,
    pub exports: Vec<Export>,
    pub bindings: Vec<Binding>,
    pub calls: Vec<Call>,
    pub decorated: Vec<Decorated>,
    pub types: Vec<TypeDecl>,
    pub classes: Vec<ClassDecl>,
    pub env: Vec<EnvRead>,
    pub decisions: Vec<Decision>,
    // Every `return` or `yield` of a value.
    pub returns: Vec<Lines>,
    // Every load of a module by a computed name, which no resolver can
    // follow.
    pub dynamic: Vec<Lines>,
    // The module-level `if __name__ == "__main__":`, if any.
    pub main_guard: Option<Lines>,
    // Every `def test_*`, at module level or in a class.
    pub tests: Vec<TestDef>,
    // `__all__`, where declared.
    all: Option<Vec<String>>,
    references: Vec<Reference>,
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
        let Self {
            exports,
            imports,
            reexports,
            types,
            classes,
            all,
            ..
        } = self;

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

    pub fn stem(&self) -> &str {
        let (dir, file) = self.path.rsplit_once('/').unwrap_or(("", &self.path));
        let stem = file.split_once('.').map_or(file, |(stem, _)| stem);
        let parent = dir.rsplit_once('/').map_or(dir, |(_, last)| last);
        if GENERIC_STEMS.contains(&stem) && !matches!(parent, "" | "src" | "app" | "lib") {
            parent
        } else {
            stem
        }
    }

    pub fn is_init(&self) -> bool {
        self.path.ends_with("__init__.py")
    }

    pub fn import(&self, local: &str) -> Option<&Import> {
        self.imports.iter().find(|import| !import.local.is_empty() && import.local == local)
    }

    pub fn package(&self, local: &str) -> Option<&str> {
        self.import(local)?.target.as_ref()?.package()
    }

    pub fn imported(&self, local: &str) -> Option<&str> {
        self.import(local)?.target.as_ref()?.module()
    }

    // `frames` are the enclosing functions, outermost first. The innermost
    // binding wins, else the module's.
    pub fn binding(&self, name: &str, frames: &[u32]) -> Option<&Binding> {
        frames
            .iter()
            .rev()
            .find_map(|frame| {
                self.bindings.iter().find(|b| b.name == name && b.scope == Scope::Function(*frame))
            })
            .or_else(|| self.bindings.iter().find(|b| b.name == name && b.scope == Scope::Module))
    }

    pub fn field(&self, class: &str, name: &str) -> Option<&Binding> {
        self.bindings
            .iter()
            .find(|b| b.name == name && matches!(&b.scope, Scope::Class(c) if c == class))
    }

    pub fn export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|export| export.name == name)
    }

    pub fn exported_binding(&self, name: &str) -> Option<&Binding> {
        let export = self.export(name)?;
        self.binding(&export.name, &[])
    }

    pub fn declared_at(&self, export: &Export) -> Lines {
        self.binding(&export.name, &[]).map_or(export.lines, |binding| binding.lines)
    }

    pub fn class(&self, name: &str) -> Option<&ClassDecl> {
        self.classes.iter().find(|class| class.name == name)
    }

    pub fn decorators_of(&self, class: Option<&str>, name: &str) -> Vec<&Decorated> {
        self.decorated
            .iter()
            .filter(|d| d.class.as_deref() == class && d.member.as_deref() == Some(name))
            .collect()
    }

    pub fn referenced(&self, lines: Lines) -> Vec<&str> {
        self.names(|line| lines.holds(line))
    }

    pub fn referenced_outside(&self, lines: &[Lines]) -> Vec<&str> {
        self.names(|line| !lines.iter().any(|l| l.holds(line)))
    }

    fn names(&self, keep: impl Fn(u32) -> bool) -> Vec<&str> {
        unique(self.references.iter().filter(|r| keep(r.line)).map(|r| r.name.as_str()))
    }

    pub fn referencing(&self, keep: impl Fn(&str) -> bool) -> Vec<u32> {
        unique(self.references.iter().filter(|r| keep(&r.name)).map(|r| r.line))
    }

    // What loading the module or constructing its classes reaches: the head
    // of every class field's and module-level binding's type and
    // initializer.
    pub fn constructed(&self) -> Vec<&str> {
        unique(self.bindings.iter().flat_map(|binding| {
            let ((Scope::Class(_), BindingKind::Field { type_path, root, .. })
            | (Scope::Module, BindingKind::Value { type_path, root, .. })) =
                (&binding.scope, &binding.kind)
            else {
                return Vec::new();
            };
            type_path
                .iter()
                .chain(root)
                .filter_map(|path| path.first().map(String::as_str))
                .collect()
        }))
    }

    pub fn targets(&self) -> impl Iterator<Item = &Target> {
        self.imports
            .iter()
            .map(|import| import.target.as_ref())
            .chain(self.reexports.iter().map(|reexport| reexport.target.as_ref()))
            .flatten()
    }

    // A type-only import is followed nowhere.
    pub fn reached(&self) -> Vec<&str> {
        let imports = self.imports.iter().filter(|import| !import.type_only).map(|i| &i.target);
        let reexports = self.reexports.iter().filter(|re| !re.type_only).map(|re| &re.target);
        unique(imports.chain(reexports).flatten().filter_map(Target::module))
    }

    pub fn data(&self) -> Vec<&str> {
        unique(self.targets().filter_map(Target::data))
    }

    // A type-only import never counts.
    pub fn unresolved(&self) -> Vec<&str> {
        let imports = self.imports.iter().filter(|import| !import.type_only).map(|i| &i.target);
        let reexports = self.reexports.iter().filter(|re| !re.type_only).map(|re| &re.target);
        unique(imports.chain(reexports).flatten().filter_map(Target::unresolved))
    }
}

#[derive(Debug)]
pub struct Import {
    // Empty for a literal, which binds nothing.
    pub local: String,
    // As written, a relative import's dots leading: `a.b.c`, `.sibling`,
    // `..`.
    pub specifier: String,
    pub imported: Imported,
    // Under `if TYPE_CHECKING:` or from a typing module; followed nowhere.
    pub type_only: bool,
    // `None` until the resolver settles it.
    pub target: Option<Target>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Imported {
    // The module itself is bound: `import a.b`, `import a.b as m`, or a
    // `from` import whose name settled as a submodule.
    Module,
    Named(String),
    Star,
    // A string spelling a dotted module path (`"shop.urls"`,
    // `"app.main:app"`) or a data file's path: reached, binding nothing.
    Literal,
}

#[derive(Debug)]
pub struct Reexport {
    pub specifier: String,
    // `(imported, exported)` pairs, or `None` for `from m import *`.
    pub names: Option<Vec<(String, String)>>,
    pub type_only: bool,
    pub target: Option<Target>,
}

#[derive(Debug)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub lines: Lines,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportKind {
    Function,
    Class,
    Value,
    Type,
}

impl ExportKind {
    pub const fn callable(&self) -> bool {
        matches!(self, Self::Function | Self::Class)
    }

    pub const fn valued(&self) -> bool {
        matches!(self, Self::Value)
    }
}

#[derive(Debug)]
pub struct Binding {
    pub name: String,
    pub scope: Scope,
    pub kind: BindingKind,
    pub lines: Lines,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Scope {
    Module,
    Function(u32),
    Class(String),
}

#[derive(Debug)]
pub enum BindingKind {
    Function,
    Class {
        // The last name of each base, in order.
        bases: Vec<String>,
    },
    Value {
        // The identifier path at the head of the initializer: `FastAPI` for
        // `FastAPI()`, `click.group` for `click.group()`.
        root: Option<Vec<String>>,
        // The annotated type's path: `Session` for `db: Session = ..`.
        type_path: Option<Vec<String>>,
        // The index in `Module::calls` of the initializing call.
        call: Option<usize>,
        // The value of a plain string literal initializer.
        string: Option<String>,
        init: Init,
        // The initializer's first line, cut as `walk` cuts it.
        head: Option<String>,
    },
    Field {
        type_path: Option<Vec<String>>,
        root: Option<Vec<String>>,
        init: Init,
        head: Option<String>,
    },
    Param {
        type_path: Option<Vec<String>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Init {
    // A literal, or an expression over literals: `5 * 1000`, `["a", "b"]`,
    // `int("3")`, `timedelta(hours=2)`.
    Literal,
    // `re.compile(..)` over a literal.
    Pattern,
    // Reads the environment, whatever else it does with it.
    Env,
    // A call handed something spelled in place and no function:
    // `Field(default=..)`, `FastAPI(title="Shop")`.
    Definition,
    // A call handed nothing spelled in place, or an awaited one:
    // `FastAPI()`, `create_app()`.
    Construction,
    // A lambda.
    Function,
    // A reference, an attribute read, a comprehension, or no initializer.
    Other,
}

impl Init {
    // A value a criterion could cite.
    pub const fn is_value(self) -> bool {
        matches!(self, Self::Literal | Self::Pattern | Self::Env | Self::Definition)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    // An expression statement's, awaited or not.
    Discarded,
    // What an `await` waits on, then used.
    Awaited,
    // Assigned, passed, returned, or chained on.
    Consumed,
}

#[derive(Debug)]
pub struct Call {
    pub callee: Callee,
    // The callee's last name is capitalised.
    pub constructs: bool,
    // Positional, then keyword.
    pub args: Vec<Arg>,
    // How many handler bodies enclose the call.
    pub depth: usize,
    pub value: Use,
    // A further, non-structural call in the chain is made on this one's
    // value: `a.b()` in `a.b().c()`.
    pub inner: bool,
    // The enclosing function frames, outermost first.
    pub frames: Vec<u32>,
    // The innermost named enclosing function.
    pub function: Option<String>,
    pub class: Option<String>,
    pub lines: Lines,
}

impl Call {
    pub fn method(&self) -> &str {
        self.callee.method()
    }

    pub fn discarded(&self) -> bool {
        self.value == Use::Discarded
    }

    // The string literal leading the positional arguments.
    pub fn literal(&self) -> Option<&str> {
        self.args.iter().find(|arg| arg.keyword.is_none()).and_then(|arg| arg.literal.as_deref())
    }

    pub fn keyword(&self, name: &str) -> Option<&Arg> {
        self.args.iter().find(|arg| arg.keyword.as_deref() == Some(name))
    }

    pub fn structural(&self) -> bool {
        self.callee.structural()
    }

    // A chain names itself at its first call with a literal (`command("x")`),
    // else the call's own method and lead stand.
    pub fn registered(&self, led: Option<String>) -> (&str, Option<String>) {
        let chained = self.callee.links.iter().find_map(|link| {
            let literal = link.call.as_ref()?.literal.clone()?;
            Some((link.name.as_str(), literal))
        });
        chained.map_or_else(|| (self.method(), led), |(method, literal)| (method, Some(literal)))
    }
}

#[derive(Debug)]
pub struct Callee {
    // The identifier the callee starts from, or `self`.
    pub head: String,
    // Present when the head itself is called first: `Typer().command`.
    pub head_call: Option<Invocation>,
    pub links: Vec<Link>,
}

impl Callee {
    // The last link, or the head when there is none.
    pub fn method(&self) -> &str {
        self.links.last().map_or(&self.head, |link| &link.name)
    }

    pub fn structural(&self) -> bool {
        let dotted = self.dotted();
        STRUCTURAL.contains(&self.method()) || MOCKING.contains(&dotted.as_str()) || hooks(&dotted)
    }

    pub fn path(self) -> Vec<String> {
        std::iter::once(self.head).chain(self.links.into_iter().map(|link| link.name)).collect()
    }

    pub fn dotted(&self) -> String {
        std::iter::once(self.head.as_str())
            .chain(self.links.iter().map(|link| link.name.as_str()))
            .collect::<Vec<_>>()
            .join(".")
    }

    // The head, or the first link's name under `self`.
    pub fn receiver(&self) -> &str {
        if self.head == "self" {
            self.links.first().map_or("self", |link| link.name.as_str())
        } else {
            &self.head
        }
    }
}

#[derive(Debug)]
pub struct Link {
    pub name: String,
    // Present when this member is called within the chain.
    pub call: Option<Invocation>,
}

// The string literal leading a chained call's arguments.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub literal: Option<String>,
}

#[derive(Debug)]
pub struct Arg {
    // `None` for a positional argument.
    pub keyword: Option<String>,
    pub literal: Option<String>,
    // The identifier path the argument starts from: `views.list_orders`,
    // `OrderList.as_view`.
    pub root: Option<Vec<String>>,
    pub called: bool,
    // The string literal leading a called argument's own arguments, or a
    // list's elements: `orders.urls` for `include("orders.urls")`, `login`
    // for `["login"]`.
    pub inner: Option<String>,
    // A lambda, a class handed by `as_view()`, or a call passing one.
    pub function: bool,
    // The argument's first line, cut as an initializer's head is.
    pub head: String,
    pub lines: Lines,
}

impl Arg {
    pub fn is_hook(&self) -> bool {
        self.keyword.as_deref().is_some_and(|keyword| HOOK_KEYWORDS.contains(&keyword))
    }
}

#[derive(Debug)]
pub struct Decorated {
    // The class a decorated `def` is a method of; for a class decorator,
    // the class itself.
    pub class: Option<String>,
    // `None` for a class decorator.
    pub member: Option<String>,
    pub name: Vec<String>,
    // The first positional string, else the `path`, `rule`, `name`, or
    // `route` keyword's.
    pub literal: Option<String>,
    // Each keyword argument with its value's head: `methods` and
    // `["GET", "POST"]`.
    pub keywords: Vec<(String, String)>,
    // From the first decorator through the `def` or `class` line.
    pub head: Lines,
    // From the first decorator to the end of what it decorates.
    pub lines: Lines,
}

impl Decorated {
    pub fn keyword(&self, name: &str) -> Option<&str> {
        self.keywords.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    // Registers what it decorates, rather than shaping it or hooking it on a
    // lifecycle.
    pub fn registering(&self) -> bool {
        self.name
            .last()
            .is_some_and(|name| !shapes(name) && !DECORATOR_HOOKS.contains(&name.as_str()))
            && !hooks(&self.name.join("."))
    }
}

#[derive(Debug)]
pub struct TypeDecl {
    pub name: String,
    pub kind: TypeKind,
    pub exported: bool,
    pub lines: Lines,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    // `type X = ..`, `X: TypeAlias = ..`, a typing subscript bound at module
    // level, a `TypeVar` or `NewType`.
    Alias,
    // A functional `TypedDict`, `NamedTuple`, or `Enum` call.
    Functional,
}

#[derive(Debug)]
pub struct ClassDecl {
    pub name: String,
    pub exported: bool,
    pub lines: Lines,
    // As written: `class Money:`.
    pub header: String,
    // The last name of each base, in order.
    pub bases: Vec<String>,
    pub members: Vec<Member>,
}

impl ClassDecl {
    pub fn is_enum(&self) -> bool {
        self.bases.iter().any(|base| base.ends_with("Enum") || base.ends_with("Flag"))
    }

    // A dunder method is a dataclass's or a model's, not a caller's. An
    // empty body declares nothing either way.
    pub fn declares_data(&self) -> bool {
        declares_data(&self.bases)
            || (!self.members.is_empty()
                && self.members.iter().all(|member| match member.kind {
                    MemberKind::Field
                    | MemberKind::Getter
                    | MemberKind::Setter
                    | MemberKind::Nested => true,
                    MemberKind::Method => {
                        member.name.starts_with("__") && member.name.ends_with("__")
                    }
                    MemberKind::Constructor => false,
                }))
    }
}

#[derive(Debug)]
pub struct Member {
    pub name: String,
    pub kind: MemberKind,
    pub private: bool,
    pub lines: Lines,
    // A `def`'s declaration up to its body; a field's whole statement, cut
    // as an initializer's head is.
    pub signature: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberKind {
    Constructor,
    Method,
    Getter,
    Setter,
    Field,
    // A class nested in the body: a `Meta`, a `Config`.
    Nested,
}

#[derive(Debug)]
pub struct EnvRead {
    pub key: String,
    pub lines: Lines,
}

// Where the code decides: a guard, a match, a conditional, a raise, an
// except, a loop condition, an assertion, or a timer.
#[derive(Debug)]
pub struct Decision {
    pub lines: Lines,
    // The innermost named enclosing function.
    pub function: Option<String>,
    pub class: Option<String>,
    // The first line, cut as an initializer's head is: `if <test>`,
    // `match <subject>`, `except <type>`, the raise statement.
    pub text: String,
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

#[derive(Debug)]
struct Reference {
    name: String,
    line: u32,
}
