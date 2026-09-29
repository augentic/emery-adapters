//! Reads one module into the facts the survey decides from.
//!
//! The parser is confined here: a `Module` is plain data, so the rules in
//! `surface` never name a syntax-tree type, and a parser swap touches this
//! file alone. A module the parser cannot read is still a `Module` — its
//! imports and exports as far as the parser got, and `parsed` false — so one
//! broken file never fails a run.

use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, CatchClause, ChainElement, Class, ClassElement,
    ComputedMemberExpression, ConditionalExpression, Declaration, Decorator, ExportAllDeclaration,
    ExportDeclaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
    ExportFromDeclaration, ExportNamedDeclaration, Expression, ExpressionStatement,
    FormalParameters, IdentifierReference, IfStatement, ImportDeclaration,
    ImportDeclarationSpecifier, ImportExpression, ImportOrExportKind, MethodDefinition,
    MethodDefinitionKind, NewExpression, ObjectPropertyKind, PropertyDefinition, PropertyKey,
    ReturnStatement, StaticMemberExpression, SwitchStatement, TSAccessibility, TSEnumDeclaration,
    TSInterfaceDeclaration, TSType, TSTypeAliasDeclaration, TSTypeName, ThrowStatement,
    UnaryOperator, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

// The methods whose function arguments are structure rather than handlers —
// lifecycle, promise, iteration, and mounting — so a call through one never
// registers a surface, and a function passed to one runs at its caller's
// depth. Matched against a call's method, or its head when it has none.
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

// The listener methods whose event decides whether a handler is a surface.
const LISTENERS: &[&str] = &["on", "once", "addListener", "addEventListener", "prependListener"];

// The events a listener method hears about the process, a connection, or a
// stream, not about a caller: a handler for one is lifecycle, not a surface.
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

// A binding's initializer is kept as its head: its first line, cut to this
// many characters — a literal, a default, a small expression a criterion
// could quote. A decision's text is cut the same way.
const INIT_TEXT: usize = 160;

// The calls that defer or repeat what they are handed: a decision about
// when the code runs.
const TIMERS: &[&str] =
    &["setTimeout", "setInterval", "setImmediate", "schedule", "scheduleJob", "cron", "every"];

// The calls that return their literal argument as a value, so `Number(3000)`
// spells a value as `3000` does.
const WRAPPERS: &[&str] = &["Number", "String", "Boolean", "BigInt", "parseInt", "parseFloat"];

/// A 1-based, inclusive range of lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lines {
    pub start: u32,
    pub end: u32,
}

impl Lines {
    pub const fn contains(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
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
    /// The lines of every `return` that yields a value: where a function's
    /// result is decided, which a requirement about what it computes anchors
    /// at.
    pub returns: Vec<Lines>,
    references: Vec<Reference>,
}

impl Module {
    /// The file's stem, or its directory's for an `index` or `main` module
    /// beneath a directory that names something.
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

    /// The import binding `local` names, if any.
    pub fn import(&self, local: &str) -> Option<&Import> {
        self.imports.iter().find(|import| import.local == local)
    }

    /// The binding `name` resolves to from `scope`: the innermost enclosing
    /// function that binds it, else the module.
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
        self.bindings.iter().find(|b| b.name == name && b.scope == Scope::Class(class.to_owned()))
    }

    /// The export `name` names, if any.
    pub fn export(&self, name: &str) -> Option<&Export> {
        self.exports.iter().find(|export| export.name == name)
    }

    /// The names referenced within `lines`, in order, once each.
    pub fn referenced(&self, lines: Lines) -> Vec<&str> {
        let mut names: Vec<&str> = Vec::new();
        for reference in &self.references {
            if reference.line >= lines.start
                && reference.line <= lines.end
                && !names.contains(&reference.name.as_str())
            {
                names.push(&reference.name);
            }
        }
        names
    }

    /// The names referenced outside every range of `lines`, in order, once
    /// each.
    pub fn referenced_outside(&self, lines: &[Lines]) -> Vec<&str> {
        let mut names: Vec<&str> = Vec::new();
        for reference in &self.references {
            let inside = lines.iter().any(|l| reference.line >= l.start && reference.line <= l.end);
            if !inside && !names.contains(&reference.name.as_str()) {
                names.push(&reference.name);
            }
        }
        names
    }

    /// The names loading the module or constructing its classes reach: the
    /// head of the declared type and of the initializer of every class field
    /// and module-level value binding, in order, once each.
    pub fn constructed(&self) -> Vec<&str> {
        let mut names: Vec<&str> = Vec::new();
        for binding in &self.bindings {
            let ((Scope::Class(_), BindingKind::Field { type_path, root, .. })
            | (Scope::Module, BindingKind::Value { type_path, root, .. })) =
                (&binding.scope, &binding.kind)
            else {
                continue;
            };
            for head in type_path.iter().chain(root).filter_map(|path| path.first()) {
                if !names.contains(&head.as_str()) {
                    names.push(head);
                }
            }
        }
        names
    }
}

#[derive(Debug)]
pub struct Import {
    pub local: String,
    pub specifier: String,
    pub imported: Imported,
    pub type_only: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Imported {
    Default,
    Namespace,
    Named(String),
    // `import "./polyfill"` or `import("./x")`: reached, binding nothing
    Effect,
}

#[derive(Debug)]
pub struct Reexport {
    pub specifier: String,
    // `(imported, exported)` pairs, or `None` for `export * from`
    pub names: Option<Vec<(String, String)>>,
    pub type_only: bool,
}

#[derive(Debug)]
pub struct Export {
    pub name: String,
    // the local binding an `export { local as name }` list names
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
        // the identifier path at the head of the initializer: `express` for
        // `express()`, `Worker` for `new Worker(..)`, `ApmCommon.APMService`
        // for `const { APMService } = ApmCommon`
        root: Option<Vec<String>>,
        // the declared type's path, `Kafka.Consumer` for `x: Kafka.Consumer`
        type_path: Option<Vec<String>>,
        // the index in `Module::calls` of the call or construction that
        // initializes it
        call: Option<usize>,
        // the string a plain string literal initializer holds
        string: Option<String>,
        init: Init,
        // the initializer's head, when it has one
        head: Option<String>,
    },
    Field {
        type_path: Option<Vec<String>>,
        root: Option<Vec<String>>,
        init: Init,
        // the initializer's head, when it has one
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
    /// `Number("3")`.
    Literal,
    /// A regular expression: a literal, or `new RegExp` over a literal.
    Pattern,
    /// An expression reading `process.env`, whatever else it does with it.
    Env,
    /// A call or construction handed something spelled in place — a literal,
    /// an object, an array — and no function: `z.object({ .. })`.
    Definition,
    /// A call or construction handed nothing spelled in place, a module
    /// loaded, or an `await`ed one: `express()`, `require("pg")`.
    Construction,
    /// A function.
    Function,
    /// A reference, a member read, or no initializer at all.
    Other,
}

impl Init {
    /// Whether the initializer spells a value of its own — one a criterion
    /// could cite — rather than building, referencing, or defining behaviour.
    pub const fn is_value(self) -> bool {
        matches!(self, Self::Literal | Self::Pattern | Self::Env | Self::Definition)
    }
}

#[derive(Debug)]
pub struct Call {
    pub callee: Callee,
    pub is_new: bool,
    pub args: Vec<Arg>,
    // how many handler bodies enclose the call
    pub depth: usize,
    // the call is an expression statement, awaited or not
    pub discarded: bool,
    // the call's value is what a further call in its chain is made on:
    // `a.b()` in `a.b().c()`, unless that further call is structural
    pub inner: bool,
    // the enclosing function frames, outermost first
    pub frames: Vec<u32>,
    // the innermost named enclosing function
    pub function: Option<String>,
    pub class: Option<String>,
    pub lines: Lines,
}

impl Call {
    /// The method the call invokes: the last link, or the head when there
    /// is none.
    pub fn method(&self) -> &str {
        self.callee.links.last().map_or(&self.callee.head, |link| &link.name)
    }

    /// The string literal leading the arguments, or the one a chain's first
    /// call led with.
    pub fn literal(&self) -> Option<&str> {
        self.args.first().and_then(|arg| arg.literal.as_deref())
    }

    /// Whether the call is structure rather than a registration: a lifecycle,
    /// promise, iteration, or mounting method, or a listener for a lifecycle
    /// event.
    pub fn structural(&self) -> bool {
        structural(&self.callee, self.args.first().and_then(|arg| arg.literal.as_deref()))
    }
}

fn structural(callee: &Callee, literal: Option<&str>) -> bool {
    let method = callee.links.last().map_or(callee.head.as_str(), |link| link.name.as_str());
    STRUCTURAL.contains(&method)
        || (LISTENERS.contains(&method)
            && literal.is_none_or(|event| LIFECYCLE_EVENTS.contains(&event)))
}

#[derive(Debug)]
pub struct Callee {
    // the identifier the callee starts from, or `this`
    pub head: String,
    // present when the head itself is called first (`Router().get`)
    pub head_call: Option<Invocation>,
    pub links: Vec<Link>,
}

#[derive(Debug)]
pub struct Link {
    pub name: String,
    // present when this member is called within the chain
    pub call: Option<Invocation>,
}

/// A call made along a chain, with the string literal leading its arguments.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub literal: Option<String>,
}

#[derive(Debug)]
pub struct Arg {
    pub literal: Option<String>,
    // the identifier path the argument starts from, `healthRouter` for
    // `healthRouter(pool)` and `controller.list` for the member
    pub root: Option<Vec<String>>,
    pub called: bool,
    // a function, an object holding one, or a call passing one
    pub function: bool,
    pub lines: Lines,
}

#[derive(Debug)]
pub struct Decorated {
    pub class: String,
    // the method decorated, or `None` for the class itself
    pub member: Option<String>,
    pub name: Vec<String>,
    pub literal: Option<String>,
    pub lines: Lines,
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
    // the class header as written, `export class Main`
    pub header: String,
    pub members: Vec<Member>,
}

#[derive(Debug)]
pub struct Member {
    pub name: String,
    pub kind: MemberKind,
    pub private: bool,
    pub lines: Lines,
    // the declaration up to its body or initializer
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

/// A point where the code decides: a guard, a switch, a throw, a catch, or
/// a timer — where a behaviour a caller observes starts.
#[derive(Debug)]
pub struct Decision {
    pub lines: Lines,
    // the innermost named enclosing function
    pub function: Option<String>,
    pub class: Option<String>,
    // `if (<test>)`, `switch (<discriminant>)`, the throw statement, `catch
    // (<parameter>)`, or the timer call: its first line, cut as an
    // initializer is
    pub text: String,
}

#[derive(Debug)]
struct Reference {
    name: String,
    line: u32,
}

/// Parses `text`, the module at `path` relative to the source root.
pub fn parse(path: &str, text: String) -> Module {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::ts());
    let parsed = Parser::new(&allocator, &text, source_type).parse();
    if parsed.diagnostics.has_errors() {
        let first = parsed.diagnostics.errors().next().map(|d| d.message.to_string());
        emery_sdk::tracing::warn!(
            path,
            error = first.as_deref().unwrap_or("unknown"),
            "module did not parse cleanly; read as far as the parser got"
        );
    }

    let mut walker = Walker {
        text: &text,
        starts: std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| u32::try_from(i + 1).unwrap_or(u32::MAX)))
            .collect(),
        module: Module {
            path: path.to_owned(),
            parsed: !parsed.fatal_error,
            ..Module::default()
        },
        frames: Vec::new(),
        classes: Vec::new(),
        next_frame: 0,
        arg_nesting: 0,
        positions: Vec::new(),
        discarded: None,
        chained: Vec::new(),
        pending_name: None,
        constructor: false,
    };
    walker.visit_program(&parsed.program);

    let mut module = walker.module;
    settle_exports(&mut module);
    module.text = text;
    module
}

// An `export { local as name }` list names a binding declared elsewhere in
// the module; its kind is that binding's. Declared types and classes learn
// whether they are exported.
fn settle_exports(module: &mut Module) {
    for index in 0..module.exports.len() {
        if module.exports[index].kind != ExportKind::Unknown {
            continue;
        }
        let local = module.exports[index].local.clone().unwrap_or_default();
        let kind = module
            .bindings
            .iter()
            .find(|b| b.name == local && b.scope == Scope::Module)
            .map(|b| match &b.kind {
                BindingKind::Function => ExportKind::Function,
                BindingKind::Class { extends } => ExportKind::Class {
                    extends: extends.clone(),
                },
                _ => ExportKind::Value,
            })
            .or_else(|| module.types.iter().any(|t| t.name == local).then_some(ExportKind::Type));
        if let Some(kind) = kind {
            module.exports[index].kind = kind;
        }
    }

    let exported: Vec<String> = module
        .exports
        .iter()
        .map(|export| export.local.clone().unwrap_or_else(|| export.name.clone()))
        .collect();
    for decl in &mut module.types {
        decl.exported = exported.contains(&decl.name);
    }
    for class in &mut module.classes {
        class.exported = exported.contains(&class.name);
    }
}

struct Walker<'s> {
    text: &'s str,
    starts: Vec<u32>,
    module: Module,
    frames: Vec<Frame>,
    classes: Vec<String>,
    next_frame: u32,
    // how many argument lists enclose the node being walked, within the
    // current function body
    arg_nesting: usize,
    // for each call being walked, whether a function among its arguments is
    // a handler, so its body runs one depth in
    positions: Vec<bool>,
    // the call an expression statement discards the value of
    discarded: Option<Span>,
    // the calls a further, non-structural call in their chain is made on
    chained: Vec<Span>,
    // the name the next function frame takes: a variable's, a method's
    pending_name: Option<String>,
    constructor: bool,
}

struct Frame {
    id: u32,
    name: Option<String>,
    depth: usize,
    saved_nesting: usize,
    saved_constructor: bool,
}

impl<'s> Walker<'s> {
    fn line(&self, offset: u32) -> u32 {
        u32::try_from(self.starts.partition_point(|&start| start <= offset)).unwrap_or(u32::MAX)
    }

    fn lines(&self, span: Span) -> Lines {
        let last = span.end.saturating_sub(1).max(span.start);
        Lines {
            start: self.line(span.start),
            end: self.line(last),
        }
    }

    fn slice(&self, start: u32, end: u32) -> &'s str {
        self.text.get(start as usize..end as usize).unwrap_or("")
    }

    fn scope(&self) -> Scope {
        self.frames.last().map_or(Scope::Module, |frame| Scope::Function(frame.id))
    }

    fn bind(&mut self, name: String, scope: Scope, kind: BindingKind, span: Span) {
        let lines = self.lines(span);
        self.module.bindings.push(Binding {
            name,
            scope,
            kind,
            lines,
        });
    }

    fn decide(&mut self, span: Span, text: String) {
        let lines = self.lines(span);
        self.module.decisions.push(Decision {
            lines,
            function: self.frames.iter().rev().find_map(|frame| frame.name.clone()),
            class: self.classes.last().cloned(),
            text,
        });
    }

    fn enter_function(&mut self, name: Option<String>, params: &FormalParameters<'_>) {
        let raise = self.arg_nesting > 0 && self.positions.last().copied().unwrap_or(false);
        let depth = self.frames.last().map_or(0, |frame| frame.depth) + usize::from(raise);
        let id = self.next_frame;
        self.next_frame += 1;

        let class = self.classes.last().cloned();
        for param in &params.items {
            let Some(name) = pattern_name(&param.pattern) else { continue };
            let type_path =
                param.type_annotation.as_ref().and_then(|t| type_path(&t.type_annotation));
            let property = param.accessibility.is_some() || param.readonly;
            match (&class, self.constructor && property) {
                (Some(class), true) => self.bind(
                    name,
                    Scope::Class(class.clone()),
                    BindingKind::Field {
                        type_path,
                        root: None,
                        init: Init::Other,
                        head: None,
                    },
                    param.span,
                ),
                _ => self.bind(
                    name,
                    Scope::Function(id),
                    BindingKind::Param { type_path },
                    param.span,
                ),
            }
        }

        self.frames.push(Frame {
            id,
            name,
            depth,
            saved_nesting: self.arg_nesting,
            saved_constructor: self.constructor,
        });
        self.arg_nesting = 0;
        self.constructor = false;
    }

    fn leave_function(&mut self) {
        if let Some(frame) = self.frames.pop() {
            self.arg_nesting = frame.saved_nesting;
            self.constructor = frame.saved_constructor;
        }
    }

    fn enter_class(&mut self, class: &Class<'_>) {
        let name =
            class.id.as_ref().map_or_else(|| "<anonymous>".to_owned(), |id| id.name.to_string());
        let extends = class
            .heritage
            .as_ref()
            .and_then(|heritage| head_path(&heritage.expression))
            .and_then(|path| path.last().cloned());
        let lines = self.lines(class.span);
        if class.id.is_some() {
            self.bind(name.clone(), self.scope(), BindingKind::Class { extends }, class.span);
        }
        for decorator in &class.decorators {
            self.decorate(&name, None, decorator, lines);
        }

        let header = self.slice(class.span.start, class.body.span.start).trim_end().to_owned();
        let mut members = Vec::new();
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    let Some(key) = key_name(&method.key) else { continue };
                    let end =
                        method.value.body.as_ref().map_or(method.span.end, |body| body.span.start);
                    let member_lines = self.lines(method.span);
                    members.push(Member {
                        name: key.clone(),
                        kind: match method.kind {
                            MethodDefinitionKind::Constructor => MemberKind::Constructor,
                            MethodDefinitionKind::Method => MemberKind::Method,
                            MethodDefinitionKind::Get => MemberKind::Getter,
                            MethodDefinitionKind::Set => MemberKind::Setter,
                        },
                        private: hidden(method.accessibility, &key),
                        lines: member_lines,
                        signature: self.slice(method.span.start, end).trim_end().to_owned(),
                    });
                    for decorator in &method.decorators {
                        self.decorate(&name, Some(&key), decorator, member_lines);
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    let Some(key) = key_name(&property.key) else { continue };
                    let end = property
                        .value
                        .as_ref()
                        .map_or(property.span.end, |value| value.span().start);
                    let signature = self
                        .slice(property.span.start, end)
                        .trim_end()
                        .trim_end_matches(['=', ';'])
                        .trim_end()
                        .to_owned();
                    let (init, head) =
                        property.value.as_ref().map_or((Init::Other, None), |value| {
                            (
                                classify(value),
                                Some(first_line(self.slice(value.span().start, value.span().end))),
                            )
                        });
                    members.push(Member {
                        name: key.clone(),
                        kind: MemberKind::Field,
                        private: hidden(property.accessibility, &key),
                        lines: self.lines(property.span),
                        signature,
                    });
                    self.bind(
                        key,
                        Scope::Class(name.clone()),
                        BindingKind::Field {
                            type_path: property
                                .type_annotation
                                .as_ref()
                                .and_then(|t| type_path(&t.type_annotation)),
                            root: property.value.as_ref().and_then(head_path),
                            init,
                            head,
                        },
                        property.span,
                    );
                }
                _ => {}
            }
        }

        self.module.classes.push(ClassDecl {
            name: name.clone(),
            exported: false,
            lines,
            header,
            members,
        });
        self.classes.push(name);
    }

    fn decorate(
        &mut self, class: &str, member: Option<&str>, decorator: &Decorator<'_>, lines: Lines,
    ) {
        let Some(callee) = callee(&decorator.expression) else { return };
        let literal = callee
            .links
            .last()
            .and_then(|link| link.call.as_ref())
            .or(callee.head_call.as_ref())
            .and_then(|invocation| invocation.literal.clone());
        let name = std::iter::once(callee.head)
            .chain(callee.links.into_iter().map(|link| link.name))
            .collect();
        self.module.decorated.push(Decorated {
            class: class.to_owned(),
            member: member.map(str::to_owned),
            name,
            literal,
            lines,
        });
    }

    fn record_call(
        &mut self, callee_expr: &Expression<'_>, arguments: &[Argument<'_>], is_new: bool,
        span: Span,
    ) {
        let Some(callee) = callee(callee_expr) else { return };
        let literal = arguments.first().and_then(|a| a.as_expression()).and_then(string_value);
        if !structural(&callee, literal.as_deref())
            && let Some(inner) = inner_call(callee_expr)
        {
            self.chained.push(inner);
        }
        let args = arguments.iter().map(|argument| self.arg(argument)).collect();
        let lines = self.lines(span);
        let method = callee.links.last().map_or(callee.head.as_str(), |link| link.name.as_str());
        if TIMERS.contains(&method) {
            let text = first_line(self.slice(span.start, span.end));
            self.decide(span, text);
        }
        self.module.calls.push(Call {
            callee,
            is_new,
            args,
            depth: self.frames.last().map_or(0, |frame| frame.depth),
            discarded: self.discarded == Some(span),
            inner: self.chained.contains(&span),
            frames: self.frames.iter().map(|frame| frame.id).collect(),
            function: self.frames.iter().rev().find_map(|frame| frame.name.clone()),
            class: self.classes.last().cloned(),
            lines,
        });
    }

    fn arg(&self, argument: &Argument<'_>) -> Arg {
        let lines = self.lines(argument.span());
        let Some(expr) = argument.as_expression() else {
            return Arg {
                literal: None,
                root: None,
                called: false,
                function: false,
                lines,
            };
        };
        Arg {
            literal: string_value(expr),
            root: head_path(expr),
            called: matches!(core(expr), Core::Call(_) | Core::New(_)),
            function: fn_valued(expr),
            lines,
        }
    }

    fn import(&mut self, local: &str, specifier: &str, imported: Imported, type_only: bool) {
        self.module.imports.push(Import {
            local: local.to_owned(),
            specifier: specifier.to_owned(),
            imported,
            type_only,
        });
    }

    fn export(&mut self, name: &str, local: Option<&str>, kind: ExportKind, span: Span) {
        let lines = self.lines(span);
        self.module.exports.push(Export {
            name: name.to_owned(),
            local: local.map(str::to_owned),
            kind,
            lines,
        });
    }

    fn declared(&mut self, declaration: &Declaration<'_>, span: Span) {
        match declaration {
            Declaration::FunctionDeclaration(function) => {
                if let Some(id) = &function.id {
                    self.export(&id.name, None, ExportKind::Function, span);
                }
            }
            Declaration::ClassDeclaration(class) => {
                if let Some(id) = &class.id {
                    let extends = class
                        .heritage
                        .as_ref()
                        .and_then(|heritage| head_path(&heritage.expression))
                        .and_then(|path| path.last().cloned());
                    self.export(&id.name, None, ExportKind::Class { extends }, span);
                }
            }
            Declaration::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let Some(name) = pattern_name(&declarator.id) {
                        self.export(&name, None, ExportKind::Value, declarator.span);
                    }
                }
            }
            Declaration::TSTypeAliasDeclaration(alias) => {
                self.export(&alias.id.name, None, ExportKind::Type, span);
            }
            Declaration::TSInterfaceDeclaration(interface) => {
                self.export(&interface.id.name, None, ExportKind::Type, span);
            }
            Declaration::TSEnumDeclaration(declaration) => {
                self.export(&declaration.id.name, None, ExportKind::Type, span);
            }
            _ => {}
        }
    }

    fn type_decl(&mut self, name: &str, kind: TypeKind, span: Span) {
        let lines = self.lines(span);
        self.module.types.push(TypeDecl {
            name: name.to_owned(),
            kind,
            exported: false,
            lines,
            text: self.slice(span.start, span.end).to_owned(),
        });
    }

    fn env_read(&mut self, key: &str, span: Span) {
        let lines = self.lines(span);
        self.module.env.push(EnvRead {
            key: key.to_owned(),
            lines,
        });
    }
}

impl<'a> Visit<'a> for Walker<'_> {
    fn enter_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(function) => {
                let declared = function.id.as_ref().map(|id| id.name.to_string());
                if let Some(name) = &declared {
                    self.bind(name.clone(), self.scope(), BindingKind::Function, function.span);
                }
                let name = declared.or_else(|| self.pending_name.take());
                self.enter_function(name, &function.params);
            }
            AstKind::ArrowFunctionExpression(arrow) => {
                let name = self.pending_name.take();
                self.enter_function(name, &arrow.params);
            }
            AstKind::Class(class) => self.enter_class(class),
            _ => {}
        }
    }

    fn leave_node(&mut self, kind: AstKind<'a>) {
        match kind {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => self.leave_function(),
            AstKind::Class(_) => {
                self.classes.pop();
            }
            _ => {}
        }
    }

    fn visit_arguments(&mut self, it: &oxc_allocator::Vec<'a, Argument<'a>>) {
        self.arg_nesting += 1;
        walk::walk_arguments(self, it);
        self.arg_nesting -= 1;
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        self.record_call(&it.callee, &it.arguments, false, it.span);
        let handler = callee(&it.callee).is_some_and(|callee| {
            !structural(
                &callee,
                it.arguments
                    .first()
                    .and_then(|a| a.as_expression())
                    .and_then(string_value)
                    .as_deref(),
            )
        });
        self.positions.push(handler);
        walk::walk_call_expression(self, it);
        self.positions.pop();
    }

    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        self.record_call(&it.callee, &it.arguments, true, it.span);
        let handler = callee(&it.callee).is_some_and(|callee| !structural(&callee, None));
        self.positions.push(handler);
        walk::walk_new_expression(self, it);
        self.positions.pop();
    }

    fn visit_expression_statement(&mut self, it: &ExpressionStatement<'a>) {
        self.discarded = match core(&it.expression) {
            Core::Call(call) => Some(call.span),
            Core::New(new) => Some(new.span),
            Core::Other(_) => None,
        };
        walk::walk_expression_statement(self, it);
    }

    fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
        let guard = it.test.span();
        let text = format!("if ({})", first_line(self.slice(guard.start, guard.end)));
        self.decide(it.span, text);
        walk::walk_if_statement(self, it);
    }

    fn visit_switch_statement(&mut self, it: &SwitchStatement<'a>) {
        let discriminant = it.discriminant.span();
        let text =
            format!("switch ({})", first_line(self.slice(discriminant.start, discriminant.end)));
        self.decide(it.span, text);
        walk::walk_switch_statement(self, it);
    }

    fn visit_conditional_expression(&mut self, it: &ConditionalExpression<'a>) {
        let condition = it.test.span();
        let text = format!("{} ? …", first_line(self.slice(condition.start, condition.end)));
        self.decide(it.span, text);
        walk::walk_conditional_expression(self, it);
    }

    fn visit_throw_statement(&mut self, it: &ThrowStatement<'a>) {
        let text = first_line(self.slice(it.span.start, it.span.end));
        self.decide(it.span, text);
        walk::walk_throw_statement(self, it);
    }

    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        if it.argument.is_some() {
            let lines = self.lines(it.span);
            self.module.returns.push(lines);
        }
        walk::walk_return_statement(self, it);
    }

    fn visit_catch_clause(&mut self, it: &CatchClause<'a>) {
        let text = it.param.as_ref().map_or_else(
            || "catch".to_owned(),
            |param| {
                let span = param.pattern.span();
                format!("catch ({})", first_line(self.slice(span.start, span.end)))
            },
        );
        self.decide(it.span, text);
        walk::walk_catch_clause(self, it);
    }

    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        let init = it.init.as_ref();
        let call = init
            .is_some_and(|init| matches!(core(init), Core::Call(_) | Core::New(_)))
            .then_some(self.module.calls.len());
        let root = init.and_then(head_path);
        let string = init.and_then(string_value);
        let type_path = it.type_annotation.as_ref().and_then(|t| type_path(&t.type_annotation));
        let (kind, head) = init.map_or((Init::Other, None), |init| {
            (classify(init), Some(first_line(self.slice(init.span().start, init.span().end))))
        });
        let required = init.and_then(|init| match core(init) {
            Core::Call(call) if is_require(&call.callee) => {
                call.arguments.first().and_then(|a| a.as_expression()).and_then(string_value)
            }
            _ => None,
        });
        if kind == Init::Function {
            self.pending_name = pattern_name(&it.id);
        }

        let scope = self.scope();
        match &it.id {
            BindingPattern::BindingIdentifier(id) => {
                if let Some(specifier) = &required {
                    self.import(&id.name, specifier, Imported::Namespace, false);
                }
                self.bind(
                    id.name.to_string(),
                    scope,
                    BindingKind::Value {
                        root,
                        type_path,
                        call,
                        string,
                        init: kind,
                        head,
                    },
                    it.span,
                );
            }
            BindingPattern::ObjectPattern(pattern) => {
                for property in &pattern.properties {
                    let Some(key) = key_name(&property.key) else { continue };
                    let Some(local) = pattern_name(&property.value) else { continue };
                    if let Some(specifier) = &required {
                        self.import(&local, specifier, Imported::Named(key.clone()), false);
                    }
                    let root = root.as_ref().map(|root| {
                        let mut path = root.clone();
                        path.push(key);
                        path
                    });
                    self.bind(
                        local,
                        scope.clone(),
                        BindingKind::Value {
                            root,
                            type_path: None,
                            call: None,
                            string: None,
                            init: Init::Other,
                            head: None,
                        },
                        property.span,
                    );
                }
            }
            _ => {}
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_method_definition(&mut self, it: &MethodDefinition<'a>) {
        self.pending_name = key_name(&it.key);
        self.constructor = it.kind == MethodDefinitionKind::Constructor;
        walk::walk_method_definition(self, it);
        self.constructor = false;
        self.pending_name = None;
    }

    fn visit_property_definition(&mut self, it: &PropertyDefinition<'a>) {
        if it.value.as_ref().is_some_and(|value| {
            matches!(
                core(value),
                Core::Other(
                    Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                )
            )
        }) {
            self.pending_name = key_name(&it.key);
        }
        walk::walk_property_definition(self, it);
        self.pending_name = None;
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        let line = self.line(it.span.start);
        self.module.references.push(Reference {
            name: it.name.to_string(),
            line,
        });
    }

    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        if is_process_env(&it.object) {
            self.env_read(&it.property.name, it.span);
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        if is_process_env(&it.object)
            && let Some(key) = string_value(&it.expression)
        {
            self.env_read(&key, it.span);
        }
        walk::walk_computed_member_expression(self, it);
    }

    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        let specifier = it.source.value.to_string();
        let type_only = it.import_kind == ImportOrExportKind::Type;
        match &it.specifiers {
            None => self.import("", &specifier, Imported::Effect, false),
            Some(specifiers) => {
                for specifier_node in specifiers {
                    match specifier_node {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => self.import(
                            &s.local.name,
                            &specifier,
                            Imported::Named(s.imported.name().to_string()),
                            type_only || s.import_kind == ImportOrExportKind::Type,
                        ),
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            self.import(&s.local.name, &specifier, Imported::Default, type_only);
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            self.import(&s.local.name, &specifier, Imported::Namespace, type_only);
                        }
                    }
                }
            }
        }
        walk::walk_import_declaration(self, it);
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        if let Some(specifier) = string_value(&it.source) {
            self.import("", &specifier, Imported::Effect, false);
        }
        walk::walk_import_expression(self, it);
    }

    fn visit_export_declaration(&mut self, it: &ExportDeclaration<'a>) {
        self.declared(&it.declaration, it.span);
        walk::walk_export_declaration(self, it);
    }

    fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'a>) {
        let type_only = it.export_kind == ImportOrExportKind::Type;
        for specifier in &it.specifiers {
            let kind = if type_only || specifier.export_kind == ImportOrExportKind::Type {
                ExportKind::Type
            } else {
                ExportKind::Unknown
            };
            let local = specifier.local.name().to_string();
            self.export(&specifier.exported.name(), Some(&local), kind, specifier.span);
        }
        walk::walk_export_named_declaration(self, it);
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        let names = it
            .specifiers
            .iter()
            .map(|s| (s.local.name().to_string(), s.exported.name().to_string()))
            .collect();
        self.module.reexports.push(Reexport {
            specifier: it.source.value.to_string(),
            names: Some(names),
            type_only: it.export_kind == ImportOrExportKind::Type,
        });
        walk::walk_export_from_declaration(self, it);
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.module.reexports.push(Reexport {
            specifier: it.source.value.to_string(),
            names: None,
            type_only: it.export_kind == ImportOrExportKind::Type,
        });
        walk::walk_export_all_declaration(self, it);
    }

    fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'a>) {
        match &it.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                let local = function.id.as_ref().map(|id| id.name.to_string());
                self.export("default", local.as_deref(), ExportKind::Function, it.span);
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                let local = class.id.as_ref().map(|id| id.name.to_string());
                let extends = class
                    .heritage
                    .as_ref()
                    .and_then(|heritage| head_path(&heritage.expression))
                    .and_then(|path| path.last().cloned());
                self.export("default", local.as_deref(), ExportKind::Class { extends }, it.span);
            }
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
                self.export("default", None, ExportKind::Type, it.span);
            }
            other => match other.as_expression() {
                Some(Expression::Identifier(id)) => {
                    self.export("default", Some(&id.name), ExportKind::Unknown, it.span);
                }
                _ => self.export("default", None, ExportKind::Value, it.span),
            },
        }
        walk::walk_export_default_declaration(self, it);
    }

    fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'a>) {
        self.type_decl(&it.id.name, TypeKind::Interface, it.span);
        walk::walk_ts_interface_declaration(self, it);
    }

    fn visit_ts_type_alias_declaration(&mut self, it: &TSTypeAliasDeclaration<'a>) {
        self.type_decl(&it.id.name, TypeKind::Alias, it.span);
        walk::walk_ts_type_alias_declaration(self, it);
    }

    fn visit_ts_enum_declaration(&mut self, it: &TSEnumDeclaration<'a>) {
        self.type_decl(&it.id.name, TypeKind::Enum, it.span);
        walk::walk_ts_enum_declaration(self, it);
    }
}

fn hidden(accessibility: Option<TSAccessibility>, key: &str) -> bool {
    matches!(accessibility, Some(TSAccessibility::Private | TSAccessibility::Protected))
        || key.starts_with('#')
}

// The expression under the wrappers that change nothing about what is
// called: parentheses, `await`, `void`, `!`, and TypeScript's casts.
enum Core<'a, 'b> {
    Call(&'b CallExpression<'a>),
    New(&'b NewExpression<'a>),
    Other(&'b Expression<'a>),
}

fn core<'a, 'b>(expr: &'b Expression<'a>) -> Core<'a, 'b> {
    let mut expr = expr;
    loop {
        expr = match expr {
            Expression::ParenthesizedExpression(e) => &e.expression,
            Expression::AwaitExpression(e) => &e.argument,
            Expression::UnaryExpression(e)
                if matches!(e.operator, UnaryOperator::Void | UnaryOperator::LogicalNot) =>
            {
                &e.argument
            }
            Expression::TSAsExpression(e) => &e.expression,
            Expression::TSSatisfiesExpression(e) => &e.expression,
            Expression::TSNonNullExpression(e) => &e.expression,
            Expression::TSTypeAssertion(e) => &e.expression,
            Expression::TSInstantiationExpression(e) => &e.expression,
            Expression::ChainExpression(chain) => {
                return match &chain.expression {
                    ChainElement::CallExpression(call) => Core::Call(call),
                    ChainElement::TSNonNullExpression(e) => core(&e.expression),
                    _ => Core::Other(expr),
                };
            }
            Expression::CallExpression(call) => return Core::Call(call),
            Expression::NewExpression(new) => return Core::New(new),
            other => return Core::Other(other),
        };
    }
}

// What an initializer is: a function first, whatever it reads; then an
// environment read, whatever it does with it; then a pattern, a value over
// literals, a call or construction by what it is handed, a reference.
fn classify(expr: &Expression<'_>) -> Init {
    let core = core(expr);
    if matches!(
        core,
        Core::Other(Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_))
    ) {
        return Init::Function;
    }
    if reads_env(expr) {
        return Init::Env;
    }
    match core {
        Core::Other(Expression::RegExpLiteral(_)) => Init::Pattern,
        Core::New(new) if is_regexp(&new.callee) && all_literal(&new.arguments) => Init::Pattern,
        Core::Other(other) if literal(other) => Init::Literal,
        Core::Call(call) if wraps(call) => Init::Literal,
        Core::Call(call) if is_require(&call.callee) => Init::Construction,
        Core::Call(call) => invoked(&call.arguments),
        Core::New(new) => invoked(&new.arguments),
        Core::Other(Expression::ImportExpression(_)) => Init::Construction,
        Core::Other(_) => Init::Other,
    }
}

// A call or construction is a definition when it is handed something spelled
// in place and no function, a construction otherwise.
fn invoked(arguments: &[Argument<'_>]) -> Init {
    let mut exprs = arguments.iter().filter_map(Argument::as_expression);
    let spelled = exprs.clone().any(spelled);
    if spelled && !exprs.any(fn_valued) { Init::Definition } else { Init::Construction }
}

// A literal, or an expression over literals: arithmetic, an array or object
// holding literals, a template, a wrapper call such as `Number(..)`.
fn literal(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::StringLiteral(_) => true,
        Expression::Identifier(id) => matches!(id.name.as_str(), "undefined" | "Infinity" | "NaN"),
        Expression::TemplateLiteral(template) => template.expressions.iter().all(literal),
        Expression::ArrayExpression(array) => {
            array.elements.iter().all(|element| element.as_expression().is_some_and(literal))
        }
        Expression::ObjectExpression(object) => {
            object.properties.iter().all(|property| match property {
                ObjectPropertyKind::ObjectProperty(p) => !p.method && literal(&p.value),
                ObjectPropertyKind::SpreadProperty(_) => false,
            })
        }
        Expression::UnaryExpression(unary) => {
            matches!(
                unary.operator,
                UnaryOperator::UnaryNegation
                    | UnaryOperator::UnaryPlus
                    | UnaryOperator::LogicalNot
                    | UnaryOperator::BitwiseNot
            ) && literal(&unary.argument)
        }
        Expression::BinaryExpression(binary) => literal(&binary.left) && literal(&binary.right),
        Expression::LogicalExpression(logical) => literal(&logical.left) && literal(&logical.right),
        Expression::ConditionalExpression(conditional) => {
            literal(&conditional.test)
                && literal(&conditional.consequent)
                && literal(&conditional.alternate)
        }
        Expression::CallExpression(call) => wraps(call),
        Expression::ParenthesizedExpression(e) => literal(&e.expression),
        Expression::TSAsExpression(e) => literal(&e.expression),
        Expression::TSSatisfiesExpression(e) => literal(&e.expression),
        Expression::TSNonNullExpression(e) => literal(&e.expression),
        Expression::TSTypeAssertion(e) => literal(&e.expression),
        _ => false,
    }
}

// A wrapper call over literals: `Number(3000)`, `parseInt("10", 10)`.
fn wraps(call: &CallExpression<'_>) -> bool {
    matches!(&call.callee, Expression::Identifier(id) if WRAPPERS.contains(&id.name.as_str()))
        && !call.arguments.is_empty()
        && all_literal(&call.arguments)
}

fn all_literal(arguments: &[Argument<'_>]) -> bool {
    arguments.iter().all(|argument| argument.as_expression().is_some_and(literal))
}

// An argument spelled in place — a literal, or an object, array, or template
// written out whatever it holds — rather than passed by name.
fn spelled(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::TemplateLiteral(_) => true,
        Expression::ParenthesizedExpression(e) => spelled(&e.expression),
        Expression::TSAsExpression(e) => spelled(&e.expression),
        Expression::TSSatisfiesExpression(e) => spelled(&e.expression),
        other => literal(other),
    }
}

fn is_require(callee: &Expression<'_>) -> bool {
    matches!(callee, Expression::Identifier(id) if id.name.as_str() == "require")
}

fn is_regexp(callee: &Expression<'_>) -> bool {
    matches!(callee, Expression::Identifier(id) if id.name.as_str() == "RegExp")
}

// Whether an expression reads `process.env` anywhere within it.
fn reads_env(expr: &Expression<'_>) -> bool {
    struct Finder(bool);

    impl<'a> Visit<'a> for Finder {
        fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
            self.0 |= is_process_env(&it.object);
            walk::walk_static_member_expression(self, it);
        }

        fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
            self.0 |= is_process_env(&it.object) && string_value(&it.expression).is_some();
            walk::walk_computed_member_expression(self, it);
        }
    }

    let mut finder = Finder(false);
    finder.visit_expression(expr);
    finder.0
}

// An initializer's first line, cut to `INIT_TEXT` characters, `…` marking
// what is cut or continues below.
fn first_line(text: &str) -> String {
    let (first, more) = text.split_once('\n').map_or((text, false), |(first, _)| (first, true));
    let first = first.trim_end();
    match first.char_indices().nth(INIT_TEXT) {
        Some((cut, _)) => format!("{}…", first[..cut].trim_end()),
        None if more => format!("{first}…"),
        None => first.to_owned(),
    }
}

// The call a chain continues from, when a callee is a member of one's
// value: for `a.b().c()`, the span of `a.b()`.
fn inner_call(callee_expr: &Expression<'_>) -> Option<Span> {
    let object = match callee_expr {
        Expression::StaticMemberExpression(member) => &member.object,
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::StaticMemberExpression(member) => &member.object,
            _ => return None,
        },
        Expression::ParenthesizedExpression(e) => return inner_call(&e.expression),
        Expression::TSNonNullExpression(e) => return inner_call(&e.expression),
        Expression::TSAsExpression(e) => return inner_call(&e.expression),
        _ => return None,
    };
    match core(object) {
        Core::Call(call) => Some(call.span),
        Core::New(new) => Some(new.span),
        Core::Other(_) => None,
    }
}

// The callee of a call as a head and the members walked from it, each
// marked when it is itself called along the way.
fn callee(expr: &Expression<'_>) -> Option<Callee> {
    match expr {
        Expression::Identifier(id) => Some(Callee {
            head: id.name.to_string(),
            head_call: None,
            links: Vec::new(),
        }),
        Expression::ThisExpression(_) => Some(Callee {
            head: "this".to_owned(),
            head_call: None,
            links: Vec::new(),
        }),
        Expression::StaticMemberExpression(member) => {
            let mut callee = callee(&member.object)?;
            callee.links.push(Link {
                name: member.property.name.to_string(),
                call: None,
            });
            Some(callee)
        }
        Expression::CallExpression(call) => Some(called(callee(&call.callee)?, &call.arguments)),
        Expression::NewExpression(new) => Some(called(callee(&new.callee)?, &new.arguments)),
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::CallExpression(call) => {
                Some(called(callee(&call.callee)?, &call.arguments))
            }
            ChainElement::TSNonNullExpression(e) => callee(&e.expression),
            other => callee(other.as_member_expression()?.as_expression()),
        },
        Expression::ParenthesizedExpression(e) => callee(&e.expression),
        Expression::AwaitExpression(e) => callee(&e.argument),
        Expression::TSAsExpression(e) => callee(&e.expression),
        Expression::TSSatisfiesExpression(e) => callee(&e.expression),
        Expression::TSNonNullExpression(e) => callee(&e.expression),
        Expression::TSTypeAssertion(e) => callee(&e.expression),
        Expression::TSInstantiationExpression(e) => callee(&e.expression),
        _ => None,
    }
}

fn called(mut callee: Callee, arguments: &[Argument<'_>]) -> Callee {
    let literal = arguments.first().and_then(|a| a.as_expression()).and_then(string_value);
    let invocation = Invocation { literal };
    match callee.links.last_mut() {
        Some(link) => link.call = Some(invocation),
        None => callee.head_call = Some(invocation),
    }
    callee
}

// The identifier path an expression starts from, casts and calls stripped.
fn head_path(expr: &Expression<'_>) -> Option<Vec<String>> {
    let callee = callee(expr)?;
    Some(
        std::iter::once(callee.head)
            .chain(callee.links.into_iter().map(|link| link.name))
            .collect(),
    )
}

fn string_value(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::TemplateLiteral(template) => {
            template.single_quasi().map(|quasi| quasi.to_string())
        }
        Expression::ParenthesizedExpression(e) => string_value(&e.expression),
        Expression::TSAsExpression(e) => string_value(&e.expression),
        Expression::TSSatisfiesExpression(e) => string_value(&e.expression),
        _ => None,
    }
}

// A function, an object with a function-valued property or a method, or a
// call — not a structural one, so `items.map(fn)` is a value — passing one
// of those.
fn fn_valued(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::ObjectExpression(object) => {
            object.properties.iter().any(|property| match property {
                ObjectPropertyKind::ObjectProperty(p) => p.method || fn_valued(&p.value),
                ObjectPropertyKind::SpreadProperty(_) => false,
            })
        }
        Expression::CallExpression(call) => {
            callee(&call.callee).is_some_and(|callee| !structural(&callee, None))
                && call.arguments.iter().any(|a| a.as_expression().is_some_and(fn_valued))
        }
        Expression::ParenthesizedExpression(e) => fn_valued(&e.expression),
        Expression::AwaitExpression(e) => fn_valued(&e.argument),
        Expression::TSAsExpression(e) => fn_valued(&e.expression),
        Expression::TSSatisfiesExpression(e) => fn_valued(&e.expression),
        Expression::TSNonNullExpression(e) => fn_valued(&e.expression),
        _ => false,
    }
}

fn type_path(ty: &TSType<'_>) -> Option<Vec<String>> {
    match ty {
        TSType::TSTypeReference(reference) => type_name_path(&reference.type_name),
        _ => None,
    }
}

fn type_name_path(name: &TSTypeName<'_>) -> Option<Vec<String>> {
    match name {
        TSTypeName::IdentifierReference(id) => Some(vec![id.name.to_string()]),
        TSTypeName::QualifiedName(qualified) => {
            let mut path = type_name_path(&qualified.left)?;
            path.push(qualified.right.name.to_string());
            Some(path)
        }
        TSTypeName::ThisExpression(_) => None,
    }
}

fn pattern_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id.name.to_string()),
        BindingPattern::AssignmentPattern(assignment) => pattern_name(&assignment.left),
        _ => None,
    }
}

fn key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::PrivateIdentifier(id) => Some(format!("#{}", id.name)),
        other => other.static_name().map(Cow::into_owned),
    }
}

fn is_process_env(object: &Expression<'_>) -> bool {
    matches!(
        object,
        Expression::StaticMemberExpression(member)
            if member.property.name.as_str() == "env"
                && matches!(&member.object, Expression::Identifier(id) if id.name.as_str() == "process")
    )
}
