//! Reads one module into the facts the survey decides from.
//!
//! A `Module` is plain data. The parser is confined to `walk`, so the rules
//! in `surface` and `skeleton` never name a syntax-tree type and a parser
//! swap touches one file. A module the parser cannot read is still a
//! `Module`, read as far as the parser got with `parsed` false, so one
//! broken file never fails a run. Where each import leads is settled once
//! the tree is read (`Resolver::settle`) and carried on the import.

use std::fmt::{self, Display, Formatter};

use super::resolve::Target;
use super::unique;

mod walk;

// Methods whose function arguments are structure, not handlers: lifecycle,
// promise, iteration, and mounting. A call through one registers nothing,
// and a function passed to one runs at its caller's depth. Matched against
// a call's method, or its head when it has none.
const STRUCTURAL: &[&str] = &[
    "use",
    "register",
    "mount",
    "plugin",
    "decorate",
    "addHook",
    "hook",
    "listen",
    "connect",
    "disconnect",
    "close",
    "end",
    "then",
    "catch",
    "finally",
    "start",
    "stop",
    "init",
    "initialize",
    "forEach",
    "map",
    "filter",
    "reduce",
    "find",
    "findIndex",
    "some",
    "every",
    "sort",
    "flatMap",
    "Promise",
    "setTimeout",
    "setInterval",
    "setImmediate",
    "nextTick",
    "queueMicrotask",
    "describe",
    "it",
    "test",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
];

// Listener methods whose event decides whether a handler is a surface.
const LISTENERS: &[&str] = &["on", "once", "addListener", "addEventListener", "prependListener"];

// Events a listener hears about the process, a connection, or a stream, not
// about a caller. A handler for one is lifecycle, not a surface.
const LIFECYCLE_EVENTS: &[&str] = &[
    "error",
    "close",
    "connect",
    "connecting",
    "disconnect",
    "end",
    "ready",
    "open",
    "listening",
    "exit",
    "warning",
    "drain",
    "finish",
    "timeout",
    "reconnecting",
    "SIGINT",
    "SIGTERM",
    "SIGHUP",
    "unhandledRejection",
    "uncaughtException",
    "beforeExit",
];

// Method decorators that shape a handler another decorator registers.
const DECORATOR_NOISE: &[&str] = &[
    "UseGuards",
    "UseInterceptors",
    "UsePipes",
    "UseFilters",
    "HttpCode",
    "Header",
    "Redirect",
    "Bind",
    "SetMetadata",
    "Roles",
    "Public",
    "Version",
    "Transactional",
    "Injectable",
    "Inject",
    "SerializeOptions",
];

// The noise list, and the documentation decorators (`@ApiTags`,
// `@ApiOkResponse`).
fn shapes(decorator: &str) -> bool {
    DECORATOR_NOISE.contains(&decorator) || decorator.starts_with("Api")
}

// 1-based and inclusive. The default holds no line.
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

    // The claim anchor grammar: `L3`, or `L3-L5`.
    pub fn anchor(self) -> String {
        if self.start == self.end {
            format!("L{}", self.start)
        } else {
            format!("L{}-L{}", self.start, self.end)
        }
    }
}

// A cited line past `u32::MAX` saturates.
impl From<(u64, u64)> for Lines {
    fn from((start, end): (u64, u64)) -> Self {
        let line = |cited: u64| u32::try_from(cited).unwrap_or(u32::MAX);
        Self {
            start: line(start),
            end: line(end),
        }
    }
}

// Prose, with an en dash; `anchor` is the claim grammar.
impl Display for Lines {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "L{}", self.start)
        } else {
            write!(f, "L{}–L{}", self.start, self.end)
        }
    }
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
    // Every `return` of a value.
    pub returns: Vec<Lines>,
    // Every `import()` or `require()` handed a computed name, which no
    // resolver can follow.
    pub dynamic: Vec<Lines>,
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

    // An `export { local as name }` list names a binding declared elsewhere
    // in the module; its kind is that binding's.
    fn settle_exports(&mut self) {
        let Self {
            exports,
            bindings,
            types,
            classes,
            ..
        } = self;
        for export in exports.iter_mut().filter(|export| export.kind == ExportKind::Unknown) {
            let Some(local) = &export.local else { continue };
            let kind = bindings
                .iter()
                .find(|b| &b.name == local && b.scope == Scope::Module)
                .map(|b| match &b.kind {
                    BindingKind::Function => ExportKind::Function,
                    BindingKind::Class { extends } => ExportKind::Class {
                        extends: extends.clone(),
                    },
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

    // An `index` or `main` module takes its directory's name.
    pub fn stem(&self) -> &str {
        let (dir, file) = self.path.rsplit_once('/').unwrap_or(("", &self.path));
        let stem = file.split_once('.').map_or(file, |(stem, _)| stem);
        let parent = dir.rsplit_once('/').map_or(dir, |(_, last)| last);
        if matches!(stem, "index" | "main") && !matches!(parent, "" | "src" | "lib" | "app") {
            parent
        } else {
            stem
        }
    }

    pub fn import(&self, local: &str) -> Option<&Import> {
        self.imports.iter().find(|import| import.local == local)
    }

    pub fn package(&self, local: &str) -> Option<&str> {
        match self.import(local)?.target.as_ref()? {
            Target::Package(package) => Some(package),
            Target::Module(_) | Target::Data(_) | Target::Unresolved(_) => None,
        }
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
        self.binding(export.local.as_deref().unwrap_or(&export.name), &[])
    }

    pub fn declared_at(&self, export: &Export) -> Lines {
        self.binding(export.local.as_deref().unwrap_or(&export.name), &[])
            .map_or(export.lines, |binding| binding.lines)
    }

    pub fn class(&self, name: &str) -> Option<&ClassDecl> {
        self.classes.iter().find(|class| class.name == name)
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

    pub fn reached(&self) -> Vec<&str> {
        unique(self.targets().filter_map(Target::module))
    }

    pub fn data(&self) -> Vec<&str> {
        unique(self.targets().filter_map(|target| match target {
            Target::Data(path) => Some(path.as_str()),
            _ => None,
        }))
    }

    // A type-only import never counts.
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
    pub local: String,
    pub specifier: String,
    pub imported: Imported,
    pub type_only: bool,
    // `None` until the resolver settles it, and for an absolute specifier.
    pub target: Option<Target>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Imported {
    Default,
    Namespace,
    Named(String),
    // `import "./polyfill"` or `import("./x")`: reached, binding nothing.
    Effect,
}

#[derive(Debug)]
pub struct Reexport {
    pub specifier: String,
    // `(imported, exported)` pairs, or `None` for `export * from`.
    pub names: Option<Vec<(String, String)>>,
    pub type_only: bool,
    pub target: Option<Target>,
}

#[derive(Debug)]
pub struct Export {
    pub name: String,
    // The local binding an `export { local as name }` list names.
    pub local: Option<String>,
    pub kind: ExportKind,
    pub lines: Lines,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportKind {
    Function,
    Class { extends: Option<String> },
    Value,
    Type,
    Unknown,
}

impl ExportKind {
    pub const fn callable(&self) -> bool {
        matches!(self, Self::Function | Self::Class { .. })
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
        extends: Option<String>,
    },
    Value {
        // The identifier path at the head of the initializer: `express` for
        // `express()`, `Worker` for `new Worker(..)`, `ApmCommon.APMService`
        // for `const { APMService } = ApmCommon`.
        root: Option<Vec<String>>,
        // The declared type's path: `Kafka.Consumer` for `x: Kafka.Consumer`.
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
    // `Number("3")`.
    Literal,
    // A regular expression literal, or `new RegExp` over a literal.
    Pattern,
    // Reads `process.env`, whatever else it does with it.
    Env,
    // A call or construction handed something spelled in place and no
    // function: `z.object({ .. })`.
    Definition,
    // A call or construction handed nothing spelled in place, a module
    // loaded, or an awaited one: `express()`, `require("pg")`.
    Construction,
    // A function.
    Function,
    // A reference, a member read, or no initializer.
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
    pub is_new: bool,
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

    // The string literal leading the arguments.
    pub fn literal(&self) -> Option<&str> {
        self.args.first().and_then(|arg| arg.literal.as_deref())
    }

    pub fn structural(&self) -> bool {
        self.callee.structural(self.literal())
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
    // The identifier the callee starts from, or `this`.
    pub head: String,
    // Present when the head itself is called first: `Router().get`.
    pub head_call: Option<Invocation>,
    pub links: Vec<Link>,
}

impl Callee {
    // The last link, or the head when there is none.
    pub fn method(&self) -> &str {
        self.links.last().map_or(&self.head, |link| &link.name)
    }

    // A listener is structural unless its event names a caller's.
    pub fn structural(&self, literal: Option<&str>) -> bool {
        let method = self.method();
        STRUCTURAL.contains(&method)
            || (LISTENERS.contains(&method)
                && literal.is_none_or(|event| LIFECYCLE_EVENTS.contains(&event)))
    }

    pub fn path(self) -> Vec<String> {
        std::iter::once(self.head).chain(self.links.into_iter().map(|link| link.name)).collect()
    }

    // The head, or the first link's name under `this`.
    pub fn receiver(&self) -> &str {
        if self.head == "this" {
            self.links.first().map_or("this", |link| link.name.as_str())
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
    pub literal: Option<String>,
    // The identifier path the argument starts from: `healthRouter` for
    // `healthRouter(pool)`, `controller.list` for the member.
    pub root: Option<Vec<String>>,
    pub called: bool,
    // A function, an object holding one, or a call passing one.
    pub function: bool,
    // The string-valued properties of an object written in place, nested
    // objects flattened: `{ dir: join(__dirname, "routes"), options: {
    // prefix: "/api" } }` holds `dir` and `options.prefix`.
    pub properties: Vec<Property>,
    pub lines: Lines,
}

impl Arg {
    // `key` is the dotted path from the object.
    pub fn property(&self, key: &str) -> Option<&Property> {
        self.properties.iter().find(|property| property.key == key)
    }
}

#[derive(Debug)]
pub struct Property {
    // The static keys from the object down to the property, dotted.
    pub key: String,
    // The string the property holds, or the path it spells from the module's
    // own directory.
    pub value: String,
    // Spelled from the module's own directory (`path.join(__dirname,
    // "routes")`, `${import.meta.dirname}/routes`), so it names a directory
    // of the tree relative to the module's.
    pub relative: bool,
}

#[derive(Debug)]
pub struct Decorated {
    pub class: String,
    // `None` for a class decorator.
    pub member: Option<String>,
    pub name: Vec<String>,
    pub literal: Option<String>,
    pub lines: Lines,
}

impl Decorated {
    // Registers what it decorates, rather than shaping it.
    pub fn registering(&self) -> bool {
        self.name.last().is_some_and(|name| !shapes(name))
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
    Interface,
    Alias,
    Enum,
}

#[derive(Debug)]
pub struct ClassDecl {
    pub name: String,
    pub exported: bool,
    pub lines: Lines,
    // As written: `export class Main`.
    pub header: String,
    pub members: Vec<Member>,
}

#[derive(Debug)]
pub struct Member {
    pub name: String,
    pub kind: MemberKind,
    pub private: bool,
    pub lines: Lines,
    // The declaration up to its body or initializer.
    pub signature: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberKind {
    Constructor,
    Method,
    Getter,
    Setter,
    Field,
}

#[derive(Debug)]
pub struct EnvRead {
    pub key: String,
    pub lines: Lines,
}

// Where the code decides: a guard, a switch, a conditional, a throw, a
// catch, or a timer.
#[derive(Debug)]
pub struct Decision {
    pub lines: Lines,
    // The innermost named enclosing function.
    pub function: Option<String>,
    pub class: Option<String>,
    // The first line, cut as an initializer's head is: `if (<test>)`,
    // `switch (<discriminant>)`, `<test> ? …`, `catch (<parameter>)`, the
    // throw statement, or the timer call.
    pub text: String,
}

#[derive(Debug)]
struct Reference {
    name: String,
    line: u32,
}
