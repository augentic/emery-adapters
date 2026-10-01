//! Reads one module into the facts the survey decides from.
//!
//! A `Module` is plain data, so the rules in `surface` and `skeleton` never
//! name a syntax-tree type; the parser is confined to `walk`, and a parser
//! swap touches that file alone. A module the parser cannot read is still a
//! `Module` — its imports and exports as far as the parser got, and `parsed`
//! false — so one broken file never fails a run. Where each import leads is
//! settled once the tree is read (`Resolver::settle`) and carried on the
//! import, so every rule reads the target rather than resolving again.

use std::fmt::{self, Display, Formatter};

use super::resolve::Target;
use super::unique;

mod walk;

// The methods whose function arguments are structure rather than handlers —
// iteration, wrapping, scheduling onto the loop, mounting, dependency
// declaration — so a call through one never registers a surface, and a
// function passed to one runs at its caller's depth. Matched against a
// call's method, or its head when it has none.
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
    "patch",
    "raises",
    "fixture",
    "parametrize",
    "field",
    "Field",
    "Column",
    "mapped_column",
    "relationship",
];

// The calls that hook the process, a connection, or the application's
// lifecycle rather than register a surface for a caller, by the tail of
// their dotted spelling.
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
];

// Decorators that shape what they decorate — a property, a dataclass, a
// cache, a guard on a view another decorator registers — rather than
// register it with a framework: a `def` under one of these alone is no
// handler, and one of these never names a surface.
const DECORATOR_NOISE: &[&str] = &[
    "dataclass",
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
    "total_ordering",
    "unique",
    "fixture",
    "parametrize",
    "mark",
    "skip",
    "skipif",
];

// The module stems that name a role rather than a thing, which take their
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

/// Whether a decorator shapes what it decorates rather than register it.
pub(super) fn shapes(decorator: &str) -> bool {
    DECORATOR_NOISE.contains(&decorator)
}

// The bases a class declares data under beside an enumeration — a typed
// mapping or tuple, a protocol — rather than behaviour a caller calls.
const DATA_BASES: &[&str] = &["TypedDict", "NamedTuple", "Protocol"];

/// Whether a class with `bases` declares data by what it extends: an
/// enumeration, a typed mapping or tuple, a protocol — a type the caller
/// copies, not an export it calls.
pub(super) fn declares_data(bases: &[String]) -> bool {
    bases.iter().any(|base| {
        base.ends_with("Enum") || base.ends_with("Flag") || DATA_BASES.contains(&base.as_str())
    })
}

/// Whether a dotted spelling hooks a lifecycle rather than register a
/// surface: `app.on_event`, `atexit.register`, `admin.site.register`.
pub(super) fn hooks(dotted: &str) -> bool {
    LIFECYCLE.iter().any(|tail| dotted == *tail || dotted.ends_with(&format!(".{tail}")))
}

/// A 1-based, inclusive range of lines; the default holds none.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lines {
    pub start: u32,
    pub end: u32,
}

impl Lines {
    pub const fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }

    pub const fn holds(self, line: u32) -> bool {
        self.start <= line && line <= self.end
    }

    /// The range in the claim anchor grammar: `L3`, or `L3-L5`.
    pub fn anchor(self) -> String {
        if self.start == self.end {
            format!("L{}", self.start)
        } else {
            format!("L{}-L{}", self.start, self.end)
        }
    }
}

/// The range a claim anchor cites, saturated to what a module can hold.
impl From<(u64, u64)> for Lines {
    fn from((start, end): (u64, u64)) -> Self {
        let line = |cited: u64| u32::try_from(cited).unwrap_or(u32::MAX);
        Self {
            start: line(start),
            end: line(end),
        }
    }
}

// The range as prose, with an en dash; `anchor` is the claim grammar.
impl Display for Lines {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "L{}", self.start)
        } else {
            write!(f, "L{}–L{}", self.start, self.end)
        }
    }
}

/// One production module, read into the facts the survey decides from.
#[derive(Debug, Default)]
pub struct Module {
    pub path: String,
    pub text: String,
    /// The whole file: its first line to its last, one line at the least.
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
    /// The lines of every `return` or `yield` that yields a value: where a
    /// function's result is decided, which a requirement about what it
    /// computes anchors at.
    pub returns: Vec<Lines>,
    /// The lines of every module loaded by a computed name — `import_module`
    /// or `__import__` handed an expression, a package walked, entry points
    /// read — that no resolver can follow.
    pub dynamic: Vec<Lines>,
    /// The `if __name__ == "__main__":` guard at module level, when the
    /// module has one: what runs when the file is run as a script.
    pub main_guard: Option<Lines>,
    /// The tests the module declares — a `def test_*`, at module level or
    /// in a class — for a test file read for what it states.
    pub tests: Vec<TestDef>,
    /// The names `__all__` declares, when it is declared.
    all: Option<Vec<String>>,
    references: Vec<Reference>,
}

impl Module {
    /// The file's stem, or its directory's for a module whose own stem
    /// names a role — `__init__`, `main`, `views`, `urls`, `tasks` — beneath
    /// a directory that names something.
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

    /// Whether the module is a package's `__init__.py`.
    pub fn is_init(&self) -> bool {
        self.path.ends_with("__init__.py")
    }

    /// The import binding `local` names, if any.
    pub fn import(&self, local: &str) -> Option<&Import> {
        self.imports.iter().find(|import| !import.local.is_empty() && import.local == local)
    }

    /// The package `local` is imported from, if it is an import of one.
    pub fn package(&self, local: &str) -> Option<&str> {
        match self.import(local)?.target.as_ref()? {
            Target::Package(package) => Some(package),
            Target::Module(_) | Target::Data(_) | Target::Unresolved(_) => None,
        }
    }

    /// The module of the tree `local` is imported from, if it is an import
    /// of one.
    pub fn imported(&self, local: &str) -> Option<&str> {
        self.import(local)?.target.as_ref()?.module()
    }

    /// The binding `name` resolves to from within `frames`, the enclosing
    /// functions outermost first: the innermost that binds it, else the
    /// module.
    pub fn binding(&self, name: &str, frames: &[u32]) -> Option<&Binding> {
        frames
            .iter()
            .rev()
            .find_map(|frame| {
                self.bindings.iter().find(|b| b.name == name && b.scope == Scope::Function(*frame))
            })
            .or_else(|| self.bindings.iter().find(|b| b.name == name && b.scope == Scope::Module))
    }

    /// The field `name` of `class`, if the class declares or constructs it.
    pub fn field(&self, class: &str, name: &str) -> Option<&Binding> {
        self.bindings
            .iter()
            .find(|b| b.name == name && matches!(&b.scope, Scope::Class(c) if c == class))
    }

    /// The export `name` names, if any.
    pub fn export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|export| export.name == name)
    }

    /// The class `name` declares, if any.
    pub fn class(&self, name: &str) -> Option<&ClassDecl> {
        self.classes.iter().find(|class| class.name == name)
    }

    /// The decorators on the module-level function or the method `name`,
    /// in order.
    pub fn decorators_of(&self, class: Option<&str>, name: &str) -> Vec<&Decorated> {
        self.decorated
            .iter()
            .filter(|d| d.class.as_deref() == class && d.member.as_deref() == Some(name))
            .collect()
    }

    /// The names referenced within `lines`, in order, once each.
    pub fn referenced(&self, lines: Lines) -> Vec<&str> {
        self.names(|line| lines.holds(line))
    }

    /// The names referenced outside every range of `lines`, in order, once
    /// each.
    pub fn referenced_outside(&self, lines: &[Lines]) -> Vec<&str> {
        self.names(|line| !lines.iter().any(|l| l.holds(line)))
    }

    // the names referenced at the lines `keep` admits, in order, once each
    fn names(&self, keep: impl Fn(u32) -> bool) -> Vec<&str> {
        unique(self.references.iter().filter(|r| keep(r.line)).map(|r| r.name.as_str()))
    }

    /// The lines at which a name `keep` admits is referenced, in order, once
    /// each.
    pub fn referencing(&self, keep: impl Fn(&str) -> bool) -> Vec<u32> {
        unique(self.references.iter().filter(|r| keep(&r.name)).map(|r| r.line))
    }

    /// The names loading the module or constructing its classes reach: the
    /// head of the declared type and of the initializer of every class field
    /// and module-level value binding, in order, once each.
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

    /// Where the module's imports and re-exports lead, in order, once the
    /// tree is read.
    pub fn targets(&self) -> impl Iterator<Item = &Target> {
        self.imports
            .iter()
            .map(|import| import.target.as_ref())
            .chain(self.reexports.iter().map(|reexport| reexport.target.as_ref()))
            .flatten()
    }

    /// The modules of the tree the module imports, names by a string, or
    /// re-exports from, in order, once each. A type-only import is a
    /// declaration's, followed nowhere.
    pub fn reached(&self) -> Vec<&str> {
        let imports = self.imports.iter().filter(|import| !import.type_only).map(|i| &i.target);
        let reexports = self.reexports.iter().filter(|re| !re.type_only).map(|re| &re.target);
        unique(imports.chain(reexports).flatten().filter_map(Target::module))
    }

    /// The data files the module names by path, root-relative, in order,
    /// once each.
    pub fn data(&self) -> Vec<&str> {
        unique(self.targets().filter_map(|target| match target {
            Target::Data(path) => Some(path.as_str()),
            _ => None,
        }))
    }

    /// The modules the module imports for a value that the tree should hold
    /// and does not, as written, in order, once each. A type-only import is
    /// a declaration's, followed nowhere, so it never counts.
    pub fn unresolved(&self) -> Vec<&str> {
        let imports = self.imports.iter().filter(|import| !import.type_only).map(|i| &i.target);
        let reexports = self.reexports.iter().filter(|re| !re.type_only).map(|re| &re.target);
        unique(imports.chain(reexports).filter_map(|target| match target {
            Some(Target::Unresolved(specifier)) => Some(specifier.as_str()),
            _ => None,
        }))
    }
}

#[derive(Debug)]
pub struct Import {
    /// The name the import binds; empty for a string that names a module.
    pub local: String,
    /// The module as written: `a.b.c`, `.sibling`, `..` — the dots of a
    /// relative import leading.
    pub specifier: String,
    pub imported: Imported,
    /// Under `if TYPE_CHECKING:`, or from the typing modules: a
    /// declaration's, followed nowhere.
    pub type_only: bool,
    /// Where the specifier leads, settled once the tree is read; `None`
    /// before then.
    pub target: Option<Target>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Imported {
    /// `import a.b` or `import a.b as m`, or a `from` import whose name
    /// settled as a submodule: the module itself is bound.
    Module,
    /// `from m import name`: one name of the module.
    Named(String),
    /// `from m import *`.
    Star,
    /// A string spelling a dotted module path — `"shop.urls"`,
    /// `"orders.apps.OrdersConfig"`, `"app.main:app"` — or a data file's
    /// path: reached, binding nothing.
    Literal,
}

#[derive(Debug)]
pub struct Reexport {
    pub specifier: String,
    /// `(imported, exported)` pairs, or `None` for `from m import *`.
    pub names: Option<Vec<(String, String)>>,
    pub type_only: bool,
    /// Where the specifier leads, as an [`Import`]'s.
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
        /// The last name of each base, in order.
        bases: Vec<String>,
    },
    Value {
        /// The identifier path at the head of the initializer: `FastAPI`
        /// for `FastAPI()`, `click.group` for `click.group()`, `create_app`
        /// for `create_app()`.
        root: Option<Vec<String>>,
        /// The annotated type's path, `Session` for `db: Session = ..`.
        type_path: Option<Vec<String>>,
        /// The index in `Module::calls` of the call that initializes it.
        call: Option<usize>,
        /// The string a plain string literal initializer holds.
        string: Option<String>,
        init: Init,
        /// The initializer's head, when it has one.
        head: Option<String>,
    },
    Field {
        type_path: Option<Vec<String>>,
        root: Option<Vec<String>>,
        init: Init,
        /// The initializer's head, when it has one.
        head: Option<String>,
    },
    Param {
        type_path: Option<Vec<String>>,
    },
}

/// What a binding's initializer is, read from its syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Init {
    /// A literal, or an expression over literals: `5 * 1000`, `["a", "b"]`,
    /// `int("3")`, `timedelta(hours=2)`.
    Literal,
    /// A compiled pattern: `re.compile(..)` over a literal.
    Pattern,
    /// An expression reading the environment, whatever else it does with it.
    Env,
    /// A call handed something spelled in place — a literal, a dict, a list
    /// — and no function: `Field(default=..)`, `FastAPI(title="Shop")`.
    Definition,
    /// A call handed nothing spelled in place, or an `await`ed one:
    /// `FastAPI()`, `create_app()`, `get_wsgi_application()`.
    Construction,
    /// A lambda.
    Function,
    /// A reference, an attribute read, a comprehension, or no initializer
    /// at all.
    Other,
}

impl Init {
    /// Whether the initializer spells a value of its own — one a criterion
    /// could cite — rather than building, referencing, or defining behaviour.
    pub const fn is_value(self) -> bool {
        matches!(self, Self::Literal | Self::Pattern | Self::Env | Self::Definition)
    }
}

/// What becomes of a call's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// An expression statement's, awaited or not: dropped.
    Discarded,
    /// What an `await` waits on, then used.
    Awaited,
    /// Assigned, passed, returned, or chained on as written.
    Consumed,
}

#[derive(Debug)]
pub struct Call {
    pub callee: Callee,
    /// The callee's last name is spelled as a class is: the call constructs.
    pub constructs: bool,
    /// The positional arguments, then the keyword arguments.
    pub args: Vec<Arg>,
    /// How many handler bodies enclose the call.
    pub depth: usize,
    /// What becomes of the call's value.
    pub value: Use,
    /// The call's value is what a further call in its chain is made on:
    /// `a.b()` in `a.b().c()`, unless that further call is structural.
    pub inner: bool,
    /// The enclosing function frames, outermost first.
    pub frames: Vec<u32>,
    /// The innermost named enclosing function.
    pub function: Option<String>,
    pub class: Option<String>,
    pub lines: Lines,
}

impl Call {
    /// The method the call invokes: the last link, or the head when there
    /// is none.
    pub fn method(&self) -> &str {
        self.callee.method()
    }

    /// The call is an expression statement, awaited or not.
    pub fn discarded(&self) -> bool {
        self.value == Use::Discarded
    }

    /// The string literal leading the positional arguments.
    pub fn literal(&self) -> Option<&str> {
        self.args.iter().find(|arg| arg.keyword.is_none()).and_then(|arg| arg.literal.as_deref())
    }

    /// The keyword argument `name`, if the call passes one.
    pub fn keyword(&self, name: &str) -> Option<&Arg> {
        self.args.iter().find(|arg| arg.keyword.as_deref() == Some(name))
    }

    /// Whether the call is structure rather than a registration: iteration,
    /// wrapping, scheduling, mounting, or a hook on the lifecycle.
    pub fn structural(&self) -> bool {
        self.callee.structural()
    }
}

#[derive(Debug)]
pub struct Callee {
    /// The identifier the callee starts from, or `self`.
    pub head: String,
    /// Present when the head itself is called first (`Typer().command`).
    pub head_call: Option<Invocation>,
    pub links: Vec<Link>,
}

impl Callee {
    /// The method: the last link, or the head when there is none.
    pub fn method(&self) -> &str {
        self.links.last().map_or(&self.head, |link| &link.name)
    }

    /// Whether a call through the callee is structure rather than a
    /// registration: iteration, wrapping, scheduling onto the loop, mounting,
    /// or a hook on the process or the application's lifecycle.
    pub fn structural(&self) -> bool {
        STRUCTURAL.contains(&self.method()) || hooks(&self.dotted())
    }

    /// The identifier path: the head, then each link's name.
    pub fn path(self) -> Vec<String> {
        std::iter::once(self.head).chain(self.links.into_iter().map(|link| link.name)).collect()
    }

    /// The path as the code spells it, dotted.
    pub fn dotted(&self) -> String {
        std::iter::once(self.head.as_str())
            .chain(self.links.iter().map(|link| link.name.as_str()))
            .collect::<Vec<_>>()
            .join(".")
    }

    /// The receiver as the code spells it: the head, or the first link's
    /// name under `self`.
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
    /// Present when this member is called within the chain.
    pub call: Option<Invocation>,
}

/// A call made along a chain, with the string literal leading its arguments.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub literal: Option<String>,
}

#[derive(Debug)]
pub struct Arg {
    /// The keyword the argument is passed by, `None` for a positional one.
    pub keyword: Option<String>,
    pub literal: Option<String>,
    /// The identifier path the argument starts from, `health` for `health`,
    /// `views.list_orders` for the attribute, and the class for
    /// `OrderList.as_view()`.
    pub root: Option<Vec<String>>,
    pub called: bool,
    /// The string literal leading a called argument's own arguments:
    /// `orders.urls` for `include("orders.urls")`.
    pub inner: Option<String>,
    /// A lambda, a class handed by `as_view()`, or a call passing one.
    pub function: bool,
    /// The argument's first line, cut as an initializer's head is.
    pub head: String,
    pub lines: Lines,
}

#[derive(Debug)]
pub struct Decorated {
    /// The class the decorated `def` is a method of; for a class decorator,
    /// the class itself.
    pub class: Option<String>,
    /// The `def` decorated, or `None` for the class itself.
    pub member: Option<String>,
    pub name: Vec<String>,
    /// The first positional string, else the `path`, `rule`, or `name`
    /// keyword's.
    pub literal: Option<String>,
    /// Each keyword argument with its value's head: `methods` and
    /// `["GET", "POST"]`.
    pub keywords: Vec<(String, String)>,
    /// From the first decorator to the end of what it decorates.
    pub lines: Lines,
}

impl Decorated {
    /// The keyword argument `name`'s value head, if the decorator passes one.
    pub fn keyword(&self, name: &str) -> Option<&str> {
        self.keywords.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
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
    /// `type X = ..`, `X: TypeAlias = ..`, a typing subscript bound at
    /// module level, a `TypeVar` or `NewType`.
    Alias,
    /// A functional `TypedDict`, `NamedTuple`, or `Enum` call.
    Functional,
}

#[derive(Debug)]
pub struct ClassDecl {
    pub name: String,
    pub exported: bool,
    pub lines: Lines,
    /// The class header as written, `class Money:`.
    pub header: String,
    /// The last name of each base, in order.
    pub bases: Vec<String>,
    pub members: Vec<Member>,
}

impl ClassDecl {
    /// Whether the class is an enumeration, by its bases.
    pub fn is_enum(&self) -> bool {
        self.bases.iter().any(|base| base.ends_with("Enum") || base.ends_with("Flag"))
    }

    /// Whether the class declares data alone — by a base that does, or by
    /// members that are fields, properties, nested classes, and the dunder
    /// methods a dataclass or a model spells, with no method of its own for
    /// a caller to call. An empty body declares nothing either way.
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
    /// The declaration up to its body or initializer.
    pub signature: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberKind {
    Constructor,
    Method,
    Getter,
    Setter,
    Field,
    /// A class nested in the body: a `Meta`, a `Config`.
    Nested,
}

#[derive(Debug)]
pub struct EnvRead {
    pub key: String,
    pub lines: Lines,
}

/// A point where the code decides: a guard, a match, a conditional, a raise,
/// an except, a loop condition, an assertion, or a timer — where a behaviour
/// a caller observes starts.
#[derive(Debug)]
pub struct Decision {
    pub lines: Lines,
    /// The innermost named enclosing function.
    pub function: Option<String>,
    pub class: Option<String>,
    /// `if <test>`, `match <subject>`, `<body> if <test> else …`, the raise
    /// statement, `except <type>`, `while <test>`, `assert <test>`, or the
    /// timer call: its first line, cut as an initializer is.
    pub text: String,
}

/// A test the module declares, for a test file read for what it states.
#[derive(Debug)]
pub struct TestDef {
    pub name: String,
    /// The class the test is a method of, if any.
    pub class: Option<String>,
    /// The first line of its docstring, when it has one.
    pub doc: Option<String>,
    /// Decorated with a parametrization.
    pub parametrized: bool,
    pub lines: Lines,
}

#[derive(Debug)]
struct Reference {
    name: String,
    line: u32,
}

/// Parses `text`, the module at `path` relative to the source root. Where
/// its imports lead is the resolver's to settle once the tree is read.
pub fn parse(path: &str, text: String) -> Module {
    let mut module = walk::read(path, &text);
    settle_exports(&mut module);
    module.span = Lines {
        start: 1,
        end: u32::try_from(text.lines().count()).unwrap_or(u32::MAX).max(1),
    };
    module.text = text;
    module
}

// What the module exports: under a declared `__all__`, the names it lists;
// else every module-level function, class, type, and built value whose name
// is not led by an underscore. A listed name the module imports rather than
// declares is a re-export from that import's module — as every relative
// `from` import of an `__init__.py` is — and a star import of an
// `__init__.py` re-exports everything. Declared types and classes learn
// whether they are exported.
fn settle_exports(module: &mut Module) {
    let is_init = module.is_init();
    let Module {
        exports,
        imports,
        reexports,
        types,
        classes,
        all,
        ..
    } = module;

    if let Some(all) = all {
        exports.retain(|export| all.contains(&export.name));
    } else {
        exports.retain(|export| !export.name.starts_with('_'));
    }

    // re-exports: by specifier, in the order first seen
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
                    type_only: false,
                    target: None,
                });
            }
        }
    }
    *reexports = grouped;

    let exported: Vec<&str> = exports.iter().map(|export| export.name.as_str()).collect();
    for decl in types {
        decl.exported = exported.contains(&decl.name.as_str());
    }
    for class in classes {
        class.exported = exported.contains(&class.name.as_str());
    }
}
