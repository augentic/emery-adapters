//! Walks one module's syntax tree into its `Module`.
//!
//! The one file that names a parser type.

use ruff_python_ast::visitor::{self, Visitor};
use ruff_python_ast::{
    Arguments, CmpOp, Decorator, ElifElseClause, ExceptHandler, Expr, ExprCall, ExprContext,
    FStringPartRef, InterpolatedStringElement, Operator, Parameters, PySourceType, Stmt,
    StmtAnnAssign, StmtAssign, StmtAugAssign, StmtClassDef, StmtFunctionDef, StmtIf, StmtImport,
    StmtImportFrom, StmtTypeAlias, UnaryOp,
};
use ruff_python_parser::parse_unchecked_source;
use ruff_text_size::{Ranged, TextRange, TextSize};

use super::{
    Arg, Binding, BindingKind, Call, Callee, ClassDecl, Decision, Decorated, EnvRead, Export,
    ExportKind, Import, Imported, Init, Invocation, Lines, Link, Member, MemberKind, Module,
    Reference, Scope, TestDef, TypeDecl, TypeKind, Use,
};

// An initializer is kept as its head: its first line, cut to this many
// characters, enough for a literal or a default a criterion could quote. A
// decision's text is cut the same way.
const INIT_TEXT: usize = 160;

// Calls that defer or repeat what they are handed: a decision about when
// the code runs.
const TIMERS: &[&str] = &[
    "sleep",
    "call_later",
    "call_at",
    "Timer",
    "every",
    "cron",
    "crontab",
    "apply_async",
    "periodic_task",
    "add_job",
    "schedule",
    "wait_for",
    "timeout",
];

// Calls that return their literal argument as a value, so `int("3000")`
// spells a value as `3000` does.
const WRAPPERS: &[&str] = &[
    "int",
    "float",
    "str",
    "bool",
    "bytes",
    "Decimal",
    "timedelta",
    "Path",
    "frozenset",
    "set",
    "tuple",
    "list",
    "dict",
    "Fraction",
];

// Modules whose imports are declarations', followed nowhere, as a
// `TYPE_CHECKING` import is.
const TYPING_MODULES: &[&str] = &[
    "typing",
    "typing_extensions",
    "collections.abc",
    "types",
    "__future__",
    "abc",
    "dataclasses",
    "enum",
];

// Calls that declare a type by a name they are handed.
const TYPE_ALIASES: &[&str] = &["TypeVar", "NewType", "ParamSpec", "TypeVarTuple"];
const TYPE_FUNCTIONAL: &[&str] =
    &["TypedDict", "NamedTuple", "namedtuple", "Enum", "IntEnum", "StrEnum", "Flag", "IntFlag"];

// Heads a module-level subscript is a type alias under.
const TYPING_HEADS: &[&str] = &[
    "Literal",
    "Union",
    "Optional",
    "Callable",
    "Annotated",
    "Mapping",
    "MutableMapping",
    "Sequence",
    "Iterable",
    "Iterator",
    "Awaitable",
    "Coroutine",
    "Type",
    "dict",
    "list",
    "tuple",
    "set",
    "frozenset",
    "type",
];

// Calls that load a module by name: a literal name is an import, a computed
// one is dynamic.
const LOADERS: &[&str] = &["import_module", "__import__", "load_entry_point"];

// Calls that walk packages or entry points: dynamic, whatever they are
// handed.
const WALKERS: &[&str] = &["iter_modules", "walk_packages", "entry_points", "autodiscover_modules"];

// Mirrors the survey's `is_data`.
const DATA_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "csv", "ini"];

// Exports unsettled and `text` unset; `parse` finishes both.
pub(super) fn read(path: &str, text: &str) -> Module {
    let parsed = parse_unchecked_source(text, PySourceType::Python);
    if let Some(first) = parsed.errors().first() {
        emery_sdk::tracing::warn!(
            path,
            error = %first,
            "module did not parse cleanly; read as far as the parser got"
        );
    }

    let mut walker = Walker {
        text,
        starts: std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| u32::try_from(i + 1).unwrap_or(u32::MAX)))
            .collect(),
        module: Module {
            path: path.to_owned(),
            parsed: parsed.errors().is_empty(),
            ..Module::default()
        },
        ..Walker::default()
    };
    walker.visit_body(&parsed.syntax().body);
    walker.module
}

#[derive(Default)]
struct Walker<'s> {
    text: &'s str,
    starts: Vec<u32>,
    module: Module,
    frames: Vec<Frame>,
    classes: Vec<ClassContext>,
    next_frame: u32,
    // How many argument lists enclose the node being walked, within the
    // current function body.
    arg_nesting: usize,
    // For each call being walked, whether a function among its arguments is
    // a handler, so its body runs one depth in.
    positions: Vec<bool>,
    // The call an expression statement discards the value of.
    discarded: Option<TextRange>,
    // The call an `await` waits on.
    awaited: Option<TextRange>,
    // The calls a further, non-structural call in their chain is made on.
    chained: Vec<TextRange>,
    // The name the next lambda frame takes: the variable it is assigned to.
    pending_name: Option<String>,
    // Within `if TYPE_CHECKING:`, where imports are declarations'.
    type_checking: bool,
}

struct Frame {
    id: u32,
    name: Option<String>,
    depth: usize,
    saved_nesting: usize,
}

struct ClassContext {
    name: String,
    // How many frames enclosed the class: a `def` at that many is a method.
    frames: usize,
}

impl<'s> Walker<'s> {
    fn line(&self, offset: u32) -> u32 {
        u32::try_from(self.starts.partition_point(|&start| start <= offset)).unwrap_or(u32::MAX)
    }

    fn lines(&self, range: TextRange) -> Lines {
        let start = range.start().to_u32();
        let last = range.end().to_u32().saturating_sub(1).max(start);
        Lines {
            start: self.line(start),
            end: self.line(last),
        }
    }

    fn slice(&self, start: u32, end: u32) -> &'s str {
        self.text.get(start as usize..end as usize).unwrap_or("")
    }

    fn text_of(&self, range: TextRange) -> &'s str {
        self.slice(range.start().to_u32(), range.end().to_u32())
    }

    // The first line of `range`, cut as an initializer's head is.
    fn excerpt(&self, range: TextRange) -> String {
        first_line(self.text_of(range))
    }

    fn initializer(&self, value: Option<&Expr>) -> (Init, Option<String>) {
        value.map_or((Init::Other, None), |value| {
            (classify(value), Some(self.excerpt(value.range())))
        })
    }

    fn scope(&self) -> Scope {
        self.frames.last().map_or(Scope::Module, |frame| Scope::Function(frame.id))
    }

    const fn at_module(&self) -> bool {
        self.frames.is_empty() && self.classes.is_empty()
    }

    // Directly within a class body: a `def` here is a method, an assignment
    // a field.
    fn in_class_body(&self) -> Option<&str> {
        self.classes
            .last()
            .filter(|class| class.frames == self.frames.len())
            .map(|class| class.name.as_str())
    }

    fn function_name(&self) -> Option<String> {
        self.frames.iter().rev().find_map(|frame| frame.name.clone())
    }

    fn class_name(&self) -> Option<String> {
        self.classes.last().map(|class| class.name.clone())
    }

    fn bind(&mut self, name: String, scope: Scope, kind: BindingKind, range: TextRange) {
        let lines = self.lines(range);
        self.module.bindings.push(Binding {
            name,
            scope,
            kind,
            lines,
        });
    }

    fn decide(&mut self, range: TextRange, text: String) {
        let lines = self.lines(range);
        self.module.decisions.push(Decision {
            lines,
            function: self.function_name(),
            class: self.class_name(),
            text,
        });
    }

    // A function positioned as a handler argument, or decorated as one, runs
    // one depth in.
    fn enter_function(&mut self, name: Option<String>, params: Option<&Parameters>, handler: bool) {
        let positioned = self.arg_nesting > 0 && self.positions.last().copied().unwrap_or(false);
        let raise = positioned || handler;
        let depth = self.frames.last().map_or(0, |frame| frame.depth) + usize::from(raise);
        let id = self.next_frame;
        self.next_frame += 1;

        if let Some(params) = params {
            for param in params {
                let parameter = param.as_parameter();
                let type_path = parameter.annotation.as_deref().and_then(head_path);
                self.bind(
                    parameter.name.to_string(),
                    Scope::Function(id),
                    BindingKind::Param { type_path },
                    parameter.range(),
                );
            }
        }

        self.frames.push(Frame {
            id,
            name,
            depth,
            saved_nesting: self.arg_nesting,
        });
        self.arg_nesting = 0;
    }

    fn leave_function(&mut self) {
        if let Some(frame) = self.frames.pop() {
            self.arg_nesting = frame.saved_nesting;
        }
    }

    fn function_def(&mut self, def: &'s StmtFunctionDef) {
        let name = def.name.to_string();
        let range = def.range();
        let lines = self.lines(range);
        let class = self.in_class_body().map(str::to_owned);
        let decorators: Vec<String> =
            def.decorator_list.iter().filter_map(|d| decorator_name(&d.expression)).collect();
        let handler = decorators.iter().any(|decorator| !super::shapes(decorator));

        // a method is its class's member; a function a binding, and an export at module level
        if let Some(class) = &class {
            let end = def.body.first().map_or_else(|| range.end(), Ranged::start);
            let def_start = def
                .decorator_list
                .last()
                .map_or_else(|| range.start(), |last| self.after(last.range().end()));
            let signature = self
                .slice(def_start.to_u32(), end.to_u32())
                .trim_end()
                .trim_end_matches(':')
                .trim_end()
                .to_owned();
            let kind = if name == "__init__" {
                MemberKind::Constructor
            } else if decorators.iter().any(|d| d == "property" || d == "cached_property") {
                MemberKind::Getter
            } else if decorators.iter().any(|d| d == "setter") {
                MemberKind::Setter
            } else {
                MemberKind::Method
            };
            if let Some(decl) = self.module.classes.iter_mut().rev().find(|c| &c.name == class) {
                decl.members.push(Member {
                    name: name.clone(),
                    kind,
                    private: hidden(&name),
                    lines,
                    signature,
                });
            }
        } else {
            self.bind(name.clone(), self.scope(), BindingKind::Function, range);
            if self.at_module() {
                self.export(&name, ExportKind::Function, range);
            }
        }

        // record the decorations and the test
        for decorator in &def.decorator_list {
            self.decorate(class.as_deref(), Some(&name), decorator, lines);
        }
        if let Some(test) = self.test_def(def, class.as_deref(), &decorators) {
            self.module.tests.push(test);
        }

        // walk the decorators, the signature, and the body
        for decorator in &def.decorator_list {
            self.decorator_parts(decorator);
        }
        if let Some(returns) = &def.returns {
            self.visit_annotation(returns);
        }
        self.enter_function(Some(name), Some(&def.parameters), handler);
        self.visit_parameters(&def.parameters);
        self.visit_body(&def.body);
        self.leave_function();
    }

    // A suite is a class named `Test*` or extending a `TestCase`. A skipped
    // test states nothing.
    fn test_def(
        &self, def: &StmtFunctionDef, class: Option<&str>, decorators: &[String],
    ) -> Option<TestDef> {
        let name = def.name.as_str();
        let in_suite = class.is_some_and(|class| {
            class.starts_with("Test")
                || self
                    .module
                    .class(class)
                    .is_some_and(|decl| decl.bases.iter().any(|base| base.ends_with("TestCase")))
        });
        let skipped = decorators.iter().any(|d| d == "skip" || d == "skipif");
        if !(name.starts_with("test") && (class.is_none() || in_suite)) || skipped {
            return None;
        }
        Some(TestDef {
            name: name.to_owned(),
            class: class.map(str::to_owned),
            doc: docstring(&def.body).map(first_line),
            parametrized: decorators.iter().any(|d| d == "parametrize"),
            lines: self.lines(def.range()),
        })
    }

    // The offset of the first non-blank character after `offset`.
    fn after(&self, offset: TextSize) -> TextSize {
        let start = offset.to_u32();
        let skipped =
            self.text.get(start as usize..).map_or(0, |rest| rest.len() - rest.trim_start().len());
        TextSize::from(start.saturating_add(u32::try_from(skipped).unwrap_or(u32::MAX)))
    }

    fn class_def(&mut self, class: &'s StmtClassDef) {
        let name = class.name.to_string();
        let range = class.range();
        let lines = self.lines(range);
        let bases: Vec<String> = class
            .arguments
            .as_deref()
            .map(|arguments| {
                arguments
                    .args
                    .iter()
                    .filter_map(|base| head_path(base).and_then(|mut path| path.pop()))
                    .collect()
            })
            .unwrap_or_default();

        let header_start = class
            .decorator_list
            .last()
            .map_or_else(|| range.start(), |last| self.after(last.range().end()));
        let header_end = class.body.first().map_or_else(|| range.end(), Ranged::start);
        let header = self.slice(header_start.to_u32(), header_end.to_u32()).trim_end().to_owned();

        // a nested class is a member of the outer class, not a class of the module
        if let Some(outer) = self.in_class_body().map(str::to_owned) {
            if let Some(decl) = self.module.classes.iter_mut().rev().find(|c| c.name == outer) {
                decl.members.push(Member {
                    name: name.clone(),
                    kind: MemberKind::Nested,
                    private: hidden(&name),
                    lines,
                    signature: header.trim_end_matches(':').to_owned(),
                });
            }
        } else {
            self.bind(
                name.clone(),
                self.scope(),
                BindingKind::Class { bases: bases.clone() },
                range,
            );
            if self.at_module() {
                let kind =
                    if super::declares_data(&bases) { ExportKind::Type } else { ExportKind::Class };
                self.export(&name, kind, range);
            }
            for decorator in &class.decorator_list {
                self.decorate(Some(&name), None, decorator, lines);
            }
            self.module.classes.push(ClassDecl {
                name: name.clone(),
                exported: false,
                lines,
                header,
                bases,
                members: Vec::new(),
            });
        }

        for decorator in &class.decorator_list {
            self.decorator_parts(decorator);
        }
        if let Some(arguments) = &class.arguments {
            self.visit_arguments(arguments);
        }
        self.classes.push(ClassContext {
            name,
            frames: self.frames.len(),
        });
        self.visit_body(&class.body);
        self.classes.pop();
    }

    fn decorate(
        &mut self, class: Option<&str>, member: Option<&str>, decorator: &Decorator, lines: Lines,
    ) {
        let Some(callee) = callee(&decorator.expression) else { return };
        let arguments = match &decorator.expression {
            Expr::Call(call) => Some(&call.arguments),
            _ => None,
        };
        let literal = arguments.and_then(|arguments| {
            arguments.args.first().and_then(string_value).or_else(|| {
                arguments.keywords.iter().find_map(|keyword| {
                    matches!(keyword.arg.as_deref(), Some("path" | "rule" | "name" | "route"))
                        .then(|| string_value(&keyword.value))
                        .flatten()
                })
            })
        });
        let keywords = arguments.map_or_else(Vec::new, |arguments| {
            arguments
                .keywords
                .iter()
                .filter_map(|keyword| {
                    let key = keyword.arg.as_ref()?.as_str().to_owned();
                    Some((key, self.excerpt(keyword.value.range())))
                })
                .collect()
        });
        self.module.decorated.push(Decorated {
            class: class.map(str::to_owned),
            member: member.map(str::to_owned),
            name: callee.path(),
            literal,
            keywords,
            lines,
        });
    }

    // Walks the decorator's head and arguments without recording its call:
    // `decorate` has recorded it as the decoration, and
    // `@router.get("", response_model=Page)` registers nothing beyond the
    // `def`, however many classes it is handed.
    fn decorator_parts(&mut self, decorator: &'s Decorator) {
        match &decorator.expression {
            Expr::Call(call) => {
                self.visit_expr(&call.func);
                self.visit_arguments(&call.arguments);
            }
            other => self.visit_expr(other),
        }
    }

    fn record_call(&mut self, call: &ExprCall) -> Option<&Call> {
        let range = call.range();
        let callee = callee(&call.func)?;
        if !callee.structural()
            && let Some(inner) = inner_call(&call.func)
        {
            self.chained.push(inner);
        }
        let args = self.args(&call.arguments);
        let lines = self.lines(range);
        if TIMERS.contains(&callee.method()) {
            let text = self.excerpt(range);
            self.decide(range, text);
        }
        let constructs = callee.method().chars().next().is_some_and(char::is_uppercase);
        self.module.calls.push(Call {
            callee,
            constructs,
            args,
            depth: self.frames.last().map_or(0, |frame| frame.depth),
            value: if self.discarded == Some(range) {
                Use::Discarded
            } else if self.awaited == Some(range) {
                Use::Awaited
            } else {
                Use::Consumed
            },
            inner: self.chained.contains(&range),
            frames: self.frames.iter().map(|frame| frame.id).collect(),
            function: self.function_name(),
            class: self.class_name(),
            lines,
        });
        self.module.calls.last()
    }

    fn args(&self, arguments: &Arguments) -> Vec<Arg> {
        let positional = arguments.args.iter().map(|expr| self.arg(None, expr));
        let keywords = arguments.keywords.iter().map(|keyword| {
            self.arg(keyword.arg.as_ref().map(|arg| arg.as_str().to_owned()), &keyword.value)
        });
        positional.chain(keywords).collect()
    }

    fn arg(&self, keyword: Option<String>, expr: &Expr) -> Arg {
        let inner = match core(expr) {
            Core::Call(call) => call.arguments.args.first().and_then(string_value),
            Core::Other(_) => None,
        };
        Arg {
            keyword,
            literal: string_value(expr),
            root: head_path(expr),
            called: matches!(core(expr), Core::Call(_)),
            inner,
            function: fn_valued(expr),
            head: self.excerpt(expr.range()),
            lines: self.lines(expr.range()),
        }
    }

    fn import(&mut self, local: &str, specifier: &str, imported: Imported, type_only: bool) {
        self.module.imports.push(Import {
            local: local.to_owned(),
            specifier: specifier.to_owned(),
            imported,
            type_only,
            target: None,
        });
    }

    fn export(&mut self, name: &str, kind: ExportKind, range: TextRange) {
        let lines = self.lines(range);
        self.module.exports.push(Export {
            name: name.to_owned(),
            kind,
            lines,
        });
    }

    fn type_decl(&mut self, name: &str, kind: TypeKind, range: TextRange) {
        let lines = self.lines(range);
        self.module.types.push(TypeDecl {
            name: name.to_owned(),
            kind,
            exported: false,
            lines,
            text: self.text_of(range).to_owned(),
        });
    }

    fn env_read(&mut self, key: &str, range: TextRange) {
        let lines = self.lines(range);
        self.module.env.push(EnvRead {
            key: key.to_owned(),
            lines,
        });
    }

    fn import_stmt(&mut self, import: &StmtImport) {
        for alias in &import.names {
            let specifier = alias.name.as_str();
            let local = alias
                .asname
                .as_ref()
                .map_or_else(|| specifier.split('.').next().unwrap_or(specifier), |n| n.as_str());
            let type_only = self.type_checking || typing_module(specifier);
            self.import(local, specifier, Imported::Module, type_only);
        }
    }

    fn import_from(&mut self, import: &StmtImportFrom) {
        let dots = ".".repeat(usize::try_from(import.level).unwrap_or(0));
        let module = import.module.as_ref().map_or("", |m| m.as_str());
        let specifier = format!("{dots}{module}");
        let type_only = self.type_checking || typing_module(&specifier);
        for alias in &import.names {
            let name = alias.name.as_str();
            if name == "*" {
                self.import("", &specifier, Imported::Star, type_only);
                continue;
            }
            let local = alias.asname.as_ref().map_or(name, |n| n.as_str());
            self.import(local, &specifier, Imported::Named(name.to_owned()), type_only);
        }
    }

    fn declare_all(&mut self, target: &Expr, value: &Expr) {
        if name_of(target) == Some("__all__") && self.at_module() {
            self.module.all = Some(listed(value));
        }
    }

    fn assign(&mut self, assign: &'s StmtAssign) {
        let value = &*assign.value;
        for target in &assign.targets {
            self.declare_all(target, value);
        }
        for target in &assign.targets {
            self.target(target, None, Some(value), assign.range());
        }
        if classify(value) == Init::Function {
            self.pending_name = assign.targets.first().and_then(name_of).map(str::to_owned);
        }
        self.visit_expr(value);
        self.pending_name = None;
        for target in &assign.targets {
            self.visit_expr(target);
        }
    }

    fn ann_assign(&mut self, assign: &'s StmtAnnAssign) {
        let value = assign.value.as_deref();
        let annotation = &*assign.annotation;
        if let Some(value) = value {
            self.declare_all(&assign.target, value);
        }
        let type_alias =
            matches!(head_path(annotation).as_deref(), Some([.., last]) if last == "TypeAlias");
        if let (Some(name), true, true) = (name_of(&assign.target), type_alias, self.at_module()) {
            self.type_decl(name, TypeKind::Alias, assign.range());
            self.export(name, ExportKind::Type, assign.range());
        } else {
            self.target(&assign.target, Some(annotation), value, assign.range());
        }
        if let Some(value) = value {
            if classify(value) == Init::Function {
                self.pending_name = name_of(&assign.target).map(str::to_owned);
            }
            self.visit_expr(value);
            self.pending_name = None;
        }
        self.visit_annotation(annotation);
        self.visit_expr(&assign.target);
    }

    // A `self.<name>` target within a method is a field of the class.
    fn target(
        &mut self, target: &Expr, annotation: Option<&Expr>, value: Option<&Expr>, range: TextRange,
    ) {
        let type_path = annotation.and_then(head_path);
        let root = value.and_then(head_path);
        let (init, head) = self.initializer(value);
        match target {
            Expr::Name(name) => {
                let name = name.id.as_str().to_owned();
                if let Some(class) = self.in_class_body().map(str::to_owned) {
                    let signature = self.excerpt(range);
                    let lines = self.lines(range);
                    if let Some(decl) =
                        self.module.classes.iter_mut().rev().find(|c| c.name == class)
                    {
                        decl.members.push(Member {
                            name: name.clone(),
                            kind: MemberKind::Field,
                            private: hidden(&name),
                            lines,
                            signature,
                        });
                    }
                    self.bind(
                        name,
                        Scope::Class(class),
                        BindingKind::Field {
                            type_path,
                            root,
                            init,
                            head,
                        },
                        range,
                    );
                    return;
                }
                if self.at_module() {
                    match type_kind(value) {
                        Some(kind) => {
                            self.type_decl(&name, kind, range);
                            self.export(&name, ExportKind::Type, range);
                        }
                        None if name != "__all__" => self.export(&name, ExportKind::Value, range),
                        None => {}
                    }
                }
                let call = value
                    .is_some_and(|value| matches!(core(value), Core::Call(_)))
                    .then_some(self.module.calls.len());
                let string = value.and_then(string_value);
                self.bind(
                    name,
                    self.scope(),
                    BindingKind::Value {
                        root,
                        type_path,
                        call,
                        string,
                        init,
                        head,
                    },
                    range,
                );
            }
            Expr::Attribute(attribute) => {
                if !matches!(&*attribute.value, Expr::Name(object) if object.id == "self") {
                    return;
                }
                let Some(class) = self.class_name() else { return };
                self.bind(
                    attribute.attr.as_str().to_owned(),
                    Scope::Class(class),
                    BindingKind::Field {
                        type_path,
                        root,
                        init,
                        head,
                    },
                    range,
                );
            }
            Expr::Tuple(tuple) => {
                for element in &tuple.elts {
                    self.target(element, None, None, range);
                }
            }
            _ => {}
        }
    }

    fn if_stmt(&mut self, stmt: &'s StmtIf) {
        let at_module = self.at_module();
        if at_module && is_main_guard(&stmt.test) {
            self.module.main_guard = Some(self.lines(stmt.range()));
        }
        let type_checking = is_type_checking(&stmt.test);
        if !type_checking {
            let text = format!("if {}", self.excerpt(stmt.test.range()));
            self.decide(stmt.range(), text);
        }
        self.visit_expr(&stmt.test);
        let saved = self.type_checking;
        self.type_checking |= type_checking;
        self.visit_body(&stmt.body);
        self.type_checking = saved;
        for clause in &stmt.elif_else_clauses {
            self.visit_elif_else_clause(clause);
        }
    }

    fn call_expr(&mut self, call: &'s ExprCall) {
        let method = callee(&call.func).map(|callee| callee.method().to_owned());
        if let Some(method) = &method {
            let first = call.arguments.args.first();
            let loads_computed =
                LOADERS.contains(&method.as_str()) && first.and_then(string_value).is_none();
            if loads_computed || WALKERS.contains(&method.as_str()) {
                let lines = self.lines(call.range());
                self.module.dynamic.push(lines);
            }
            if let Some(key) = env_key(call) {
                self.env_read(&key, call.range());
            }
        }
        let handler = self.record_call(call).is_some_and(|call| !call.structural());
        self.positions.push(handler);
        self.visit_expr(&call.func);
        self.visit_arguments(&call.arguments);
        self.positions.pop();
    }
}

impl<'s> Visitor<'s> for Walker<'s> {
    fn visit_stmt(&mut self, stmt: &'s Stmt) {
        match stmt {
            Stmt::FunctionDef(def) => self.function_def(def),
            Stmt::ClassDef(class) => self.class_def(class),
            Stmt::Import(import) => self.import_stmt(import),
            Stmt::ImportFrom(import) => self.import_from(import),
            Stmt::Assign(assign) => self.assign(assign),
            Stmt::AnnAssign(assign) => self.ann_assign(assign),
            Stmt::AugAssign(StmtAugAssign { target, value, .. }) => {
                self.visit_expr(value);
                self.visit_expr(target);
            }
            Stmt::TypeAlias(StmtTypeAlias { name, .. }) => {
                if let Some(name) = name_of(name)
                    && self.at_module()
                {
                    self.type_decl(name, TypeKind::Alias, stmt.range());
                    self.export(name, ExportKind::Type, stmt.range());
                }
                visitor::walk_stmt(self, stmt);
            }
            Stmt::If(if_) => self.if_stmt(if_),
            Stmt::Match(match_) => {
                let text = format!("match {}", self.excerpt(match_.subject.range()));
                self.decide(stmt.range(), text);
                visitor::walk_stmt(self, stmt);
            }
            Stmt::Raise(_) => {
                let text = self.excerpt(stmt.range());
                self.decide(stmt.range(), text);
                visitor::walk_stmt(self, stmt);
            }
            Stmt::While(while_) => {
                let text = format!("while {}", self.excerpt(while_.test.range()));
                self.decide(stmt.range(), text);
                visitor::walk_stmt(self, stmt);
            }
            Stmt::Assert(assert) => {
                let text = format!("assert {}", self.excerpt(assert.test.range()));
                self.decide(stmt.range(), text);
                visitor::walk_stmt(self, stmt);
            }
            Stmt::Return(ret) => {
                if ret.value.is_some() {
                    let lines = self.lines(stmt.range());
                    self.module.returns.push(lines);
                }
                visitor::walk_stmt(self, stmt);
            }
            Stmt::Expr(expr) => {
                self.discarded = match core(&expr.value) {
                    Core::Call(call) => Some(call.range()),
                    Core::Other(_) => None,
                };
                visitor::walk_stmt(self, stmt);
            }
            _ => visitor::walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &'s Expr) {
        match expr {
            Expr::Call(call) => self.call_expr(call),
            Expr::Await(await_) => {
                self.awaited = match core(&await_.value) {
                    Core::Call(call) => Some(call.range()),
                    Core::Other(_) => None,
                };
                visitor::walk_expr(self, expr);
            }
            Expr::If(ternary) => {
                let text = format!(
                    "{} if {} else …",
                    self.excerpt(ternary.body.range()),
                    self.excerpt(ternary.test.range())
                );
                self.decide(expr.range(), text);
                visitor::walk_expr(self, expr);
            }
            Expr::Lambda(lambda) => {
                let name = self.pending_name.take();
                self.enter_function(name, lambda.parameters.as_deref(), false);
                if let Some(parameters) = &lambda.parameters {
                    self.visit_parameters(parameters);
                }
                self.visit_expr(&lambda.body);
                self.leave_function();
            }
            Expr::Name(name) => {
                if name.ctx == ExprContext::Load {
                    let line = self.line(name.range().start().to_u32());
                    self.module.references.push(Reference {
                        name: name.id.to_string(),
                        line,
                    });
                }
            }
            Expr::Subscript(subscript) => {
                if is_environ(&subscript.value)
                    && let Some(key) = string_value(&subscript.slice)
                {
                    self.env_read(&key, expr.range());
                }
                visitor::walk_expr(self, expr);
            }
            Expr::Yield(yielded) => {
                if yielded.value.is_some() {
                    let lines = self.lines(expr.range());
                    self.module.returns.push(lines);
                }
                visitor::walk_expr(self, expr);
            }
            Expr::YieldFrom(_) => {
                let lines = self.lines(expr.range());
                self.module.returns.push(lines);
                visitor::walk_expr(self, expr);
            }
            Expr::StringLiteral(literal) => {
                let value = literal.value.to_str();
                if let Some(specifier) = module_literal(value) {
                    self.import("", specifier, Imported::Literal, false);
                } else if is_data_path(value) {
                    self.import("", value, Imported::Literal, false);
                }
            }
            _ => visitor::walk_expr(self, expr),
        }
    }

    fn visit_arguments(&mut self, arguments: &'s Arguments) {
        self.arg_nesting += 1;
        visitor::walk_arguments(self, arguments);
        self.arg_nesting -= 1;
    }

    fn visit_except_handler(&mut self, except_handler: &'s ExceptHandler) {
        let ExceptHandler::ExceptHandler(handler) = except_handler;
        let text = handler.type_.as_deref().map_or_else(
            || "except".to_owned(),
            |type_| format!("except {}", self.excerpt(type_.range())),
        );
        self.decide(handler.range(), text);
        if let Some(type_) = &handler.type_ {
            self.visit_expr(type_);
        }
        self.visit_body(&handler.body);
    }

    fn visit_elif_else_clause(&mut self, elif_else_clause: &'s ElifElseClause) {
        if let Some(test) = &elif_else_clause.test {
            let text = format!("elif {}", self.excerpt(test.range()));
            self.decide(elif_else_clause.range(), text);
        }
        visitor::walk_elif_else_clause(self, elif_else_clause);
    }
}

fn hidden(name: &str) -> bool {
    name.starts_with('_') && !(name.starts_with("__") && name.ends_with("__"))
}

fn listed(value: &Expr) -> Vec<String> {
    let elements = match value {
        Expr::List(list) => &list.elts,
        Expr::Tuple(tuple) => &tuple.elts,
        _ => return Vec::new(),
    };
    elements.iter().filter_map(string_value).collect()
}

fn name_of(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}

fn decorator_name(expr: &Expr) -> Option<String> {
    callee(expr).map(|callee| callee.method().to_owned())
}

// Over the specifier as written: a relative one is led by its dots, so
// `.types` is the tree's own module however its last segment reads.
fn typing_module(module: &str) -> bool {
    TYPING_MODULES
        .iter()
        .any(|typing| module == *typing || module.starts_with(&format!("{typing}.")))
}

fn is_main_guard(test: &Expr) -> bool {
    let Expr::Compare(compare) = test else { return false };
    let [left, right] = compare.operands.as_ref() else { return false };
    let [CmpOp::Eq] = compare.ops.as_ref() else { return false };
    let dunder = |expr: &Expr| name_of(expr) == Some("__name__");
    let main = |expr: &Expr| string_value(expr).as_deref() == Some("__main__");
    (dunder(left) && main(right)) || (dunder(right) && main(left))
}

fn is_type_checking(test: &Expr) -> bool {
    matches!(head_path(test).as_deref(), Some([.., last]) if last == "TYPE_CHECKING")
}

fn type_kind(value: Option<&Expr>) -> Option<TypeKind> {
    let value = value?;
    match value {
        Expr::Call(call) => {
            let method = callee(&call.func)?.method().to_owned();
            if TYPE_ALIASES.contains(&method.as_str()) {
                Some(TypeKind::Alias)
            } else if TYPE_FUNCTIONAL.contains(&method.as_str()) {
                Some(TypeKind::Functional)
            } else {
                None
            }
        }
        Expr::Subscript(subscript) => {
            let head = head_path(&subscript.value)?;
            TYPING_HEADS.contains(&head.last()?.as_str()).then_some(TypeKind::Alias)
        }
        Expr::BinOp(binop) if binop.op == Operator::BitOr => {
            (type_like(&binop.left) && type_like(&binop.right)).then_some(TypeKind::Alias)
        }
        _ => None,
    }
}

// What a type alias is written over; a string is a forward reference.
fn type_like(expr: &Expr) -> bool {
    match expr {
        Expr::Name(_) | Expr::Attribute(_) | Expr::NoneLiteral(_) | Expr::StringLiteral(_) => true,
        Expr::Subscript(subscript) => type_like(&subscript.value),
        Expr::BinOp(binop) if binop.op == Operator::BitOr => {
            type_like(&binop.left) && type_like(&binop.right)
        }
        _ => false,
    }
}

fn docstring(body: &[Stmt]) -> Option<&str> {
    let Stmt::Expr(expr) = body.first()? else { return None };
    let Expr::StringLiteral(literal) = &*expr.value else { return None };
    let text = literal.value.to_str().trim();
    (!text.is_empty()).then_some(text)
}

// The expression under `await` and `not`, which change nothing about it.
enum Core<'b> {
    Call(&'b ExprCall),
    Other(&'b Expr),
}

fn core(expr: &Expr) -> Core<'_> {
    match expr {
        Expr::Await(await_) => core(&await_.value),
        Expr::UnaryOp(unary) if unary.op == UnaryOp::Not => core(&unary.operand),
        Expr::Call(call) => Core::Call(call),
        other => Core::Other(other),
    }
}

// Precedence: a function whatever it reads, then an environment read
// whatever it does with it, then the rest.
fn classify(expr: &Expr) -> Init {
    if matches!(expr, Expr::Lambda(_)) {
        return Init::Function;
    }
    if reads_env(expr) {
        return Init::Env;
    }
    match core(expr) {
        Core::Call(call) if is_compile(&call.func) && all_literal(&call.arguments) => Init::Pattern,
        Core::Other(other) if literal(other) || settings(other) => Init::Literal,
        Core::Call(call) if wraps(call) => Init::Literal,
        Core::Call(call) => invoked(&call.arguments),
        Core::Other(_) => Init::Other,
    }
}

fn invoked(arguments: &Arguments) -> Init {
    let mut values = values(arguments);
    let spelled = values.clone().any(spelled);
    if spelled && !values.any(fn_valued) { Init::Definition } else { Init::Construction }
}

fn values(arguments: &Arguments) -> impl Iterator<Item = &Expr> + Clone {
    arguments.args.iter().chain(arguments.keywords.iter().map(|keyword| &keyword.value))
}

fn literal(expr: &Expr) -> bool {
    match expr {
        Expr::BooleanLiteral(_)
        | Expr::NoneLiteral(_)
        | Expr::NumberLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::BytesLiteral(_)
        | Expr::EllipsisLiteral(_) => true,
        Expr::FString(fstring) => fstring.value.elements().all(|element| match element {
            InterpolatedStringElement::Literal(_) => true,
            InterpolatedStringElement::Interpolation(element) => literal(&element.expression),
        }),
        Expr::List(list) => list.elts.iter().all(literal),
        Expr::Tuple(tuple) => tuple.elts.iter().all(literal),
        Expr::Set(set) => set.elts.iter().all(literal),
        Expr::Dict(dict) => dict
            .items
            .iter()
            .all(|item| item.key.as_ref().is_some_and(literal) && literal(&item.value)),
        Expr::UnaryOp(unary) => literal(&unary.operand),
        Expr::BinOp(binop) => literal(&binop.left) && literal(&binop.right),
        Expr::BoolOp(boolop) => boolop.values.iter().all(literal),
        Expr::If(ternary) => {
            literal(&ternary.test) && literal(&ternary.body) && literal(&ternary.orelse)
        }
        Expr::Call(call) => wraps(call),
        _ => false,
    }
}

// Written out as settings are: at least one value a literal, every other a
// name, an attribute path, a lambda, or settings again. A call, a
// comprehension, or a splat makes it a construction or a derivation.
fn settings(expr: &Expr) -> bool {
    fn plain(expr: &Expr) -> bool {
        match expr {
            Expr::Name(_) | Expr::Attribute(_) | Expr::Lambda(_) => true,
            other => literal(other) || settings(other),
        }
    }
    let (mut any, mut all) = (false, true);
    match expr {
        Expr::Dict(dict) => {
            for item in &dict.items {
                match &item.key {
                    Some(_) => {
                        any |= literal(&item.value);
                        all &= plain(&item.value);
                    }
                    None => all = false,
                }
            }
        }
        Expr::List(list) => {
            for element in &list.elts {
                any |= literal(element);
                all &= plain(element);
            }
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                any |= literal(element);
                all &= plain(element);
            }
        }
        _ => return false,
    }
    any && all
}

fn wraps(call: &ExprCall) -> bool {
    callee(&call.func).is_some_and(|callee| WRAPPERS.contains(&callee.method()))
        && !(call.arguments.args.is_empty() && call.arguments.keywords.is_empty())
        && all_literal(&call.arguments)
}

fn all_literal(arguments: &Arguments) -> bool {
    values(arguments).all(literal)
}

// Spelled in place, whatever a container holds, rather than passed by name.
fn spelled(expr: &Expr) -> bool {
    match expr {
        Expr::List(_) | Expr::Tuple(_) | Expr::Set(_) | Expr::Dict(_) | Expr::FString(_) => true,
        other => literal(other),
    }
}

fn is_compile(func: &Expr) -> bool {
    callee(func).is_some_and(|callee| callee.method() == "compile" && callee.head == "re")
}

fn is_environ(expr: &Expr) -> bool {
    matches!(head_path(expr).as_deref(), Some([head]) if head == "environ")
        || matches!(head_path(expr).as_deref(), Some([os, environ]) if os == "os" && environ == "environ")
}

// `os.getenv("K")`, `os.environ.get("K")`, `environ.get("K")`, `getenv("K")`.
fn env_key(call: &ExprCall) -> Option<String> {
    let callee = callee(&call.func)?;
    let reads = match callee.method() {
        "getenv" => matches!(callee.head.as_str(), "os" | "getenv") && callee.links.len() <= 1,
        "get" => match &*call.func {
            Expr::Attribute(attribute) => is_environ(&attribute.value),
            _ => false,
        },
        _ => false,
    };
    reads.then(|| call.arguments.args.first().and_then(string_value)).flatten()
}

// Anywhere within the expression.
fn reads_env(expr: &Expr) -> bool {
    struct Finder(bool);

    impl<'a> Visitor<'a> for Finder {
        fn visit_expr(&mut self, expr: &'a Expr) {
            match expr {
                Expr::Subscript(subscript) => self.0 |= is_environ(&subscript.value),
                Expr::Call(call) => self.0 |= env_key(call).is_some(),
                _ => {}
            }
            visitor::walk_expr(self, expr);
        }
    }

    let mut finder = Finder(false);
    finder.visit_expr(expr);
    finder.0
}

// Cut to `INIT_TEXT` characters, `…` marking what is cut or continues
// below.
fn first_line(text: &str) -> String {
    let (first, more) = text.split_once('\n').map_or((text, false), |(first, _)| (first, true));
    let first = first.trim_end();
    match first.char_indices().nth(INIT_TEXT) {
        Some((cut, _)) => format!("{}…", first[..cut].trim_end()),
        None if more => format!("{first}…"),
        None => first.to_owned(),
    }
}

// The call a chain continues from: for `a.b().c()`, the range of `a.b()`.
fn inner_call(func: &Expr) -> Option<TextRange> {
    let Expr::Attribute(attribute) = func else { return None };
    match core(&attribute.value) {
        Core::Call(call) => Some(call.range()),
        Core::Other(_) => None,
    }
}

// A subscript or an `await` along the way is read through.
fn callee(expr: &Expr) -> Option<Callee> {
    match expr {
        Expr::Name(name) => Some(Callee {
            head: name.id.to_string(),
            head_call: None,
            links: Vec::new(),
        }),
        Expr::Attribute(attribute) => {
            let mut callee = callee(&attribute.value)?;
            callee.links.push(Link {
                name: attribute.attr.to_string(),
                call: None,
            });
            Some(callee)
        }
        Expr::Call(call) => Some(called(callee(&call.func)?, &call.arguments)),
        Expr::Subscript(subscript) => callee(&subscript.value),
        Expr::Await(await_) => callee(&await_.value),
        _ => None,
    }
}

fn called(mut callee: Callee, arguments: &Arguments) -> Callee {
    let literal = arguments.args.first().and_then(string_value);
    let invocation = Invocation { literal };
    match callee.links.last_mut() {
        Some(link) => link.call = Some(invocation),
        None => callee.head_call = Some(invocation),
    }
    callee
}

// Calls and subscripts stripped.
fn head_path(expr: &Expr) -> Option<Vec<String>> {
    callee(expr).map(Callee::path)
}

// An f-string over literals included.
fn string_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(literal) => Some(literal.value.to_str().to_owned()),
        Expr::FString(fstring) => {
            let mut out = String::new();
            for part in &fstring.value {
                match part {
                    FStringPartRef::Literal(literal) => out.push_str(&literal.value),
                    FStringPartRef::FString(fstring) => {
                        for element in &fstring.elements {
                            match element {
                                InterpolatedStringElement::Literal(literal) => {
                                    out.push_str(&literal.value);
                                }
                                InterpolatedStringElement::Interpolation(_) => return None,
                            }
                        }
                    }
                }
            }
            Some(out)
        }
        _ => None,
    }
}

// `shop.urls` as itself; `app.main:app` as `app.main`.
fn module_literal(value: &str) -> Option<&str> {
    let path = value
        .split_once(':')
        .map_or(value, |(path, object)| if is_identifier(object) { path } else { value });
    let mut segments = path.split('.');
    let first = segments.next()?;
    let mut count = 1;
    if !is_identifier(first) {
        return None;
    }
    for segment in segments {
        if !is_identifier(segment) {
            return None;
        }
        count += 1;
    }
    (count >= 2 && !path.contains(':')).then_some(path)
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_data_path(value: &str) -> bool {
    let Some((stem, extension)) = value.rsplit_once('.') else { return false };
    DATA_EXTENSIONS.contains(&extension)
        && !stem.is_empty()
        && !value.starts_with('/')
        && !value.contains("://")
        && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
}

// A structural call passing one (`sorted(items, key=f)`) is a value, not a
// function.
fn fn_valued(expr: &Expr) -> bool {
    match expr {
        Expr::Lambda(_) => true,
        Expr::List(list) => list.elts.iter().any(fn_valued),
        Expr::Tuple(tuple) => tuple.elts.iter().any(fn_valued),
        Expr::Dict(dict) => dict.items.iter().any(|item| fn_valued(&item.value)),
        Expr::Call(call) => callee(&call.func).is_some_and(|callee| {
            callee.method() == "as_view"
                || (!callee.structural() && values(&call.arguments).any(fn_valued))
        }),
        Expr::Await(await_) => fn_valued(&await_.value),
        _ => false,
    }
}
