//! Walk the parsed modules and build `behaviour.json`.
//!
//! Everything here is a fixed rule: a row of the mapping table, or one of
//! the few language rules spelled out on `Ctx`. Nothing is guessed. A
//! value the code does not fix (a computed href, a handler passed through a
//! prop) is left out and listed in `behaviour.unmapped.json`.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;

use serde_json::{Map, Value, json};
use ui_ir_behaviour::{Mappings, Row, sha256_hex};

use crate::syntax::{self, Attr, Func, JName, Jsx, Lines, Module, Pat, S, V};

/// What to read.
#[derive(Debug, Clone)]
pub struct Options {
    /// The source root; evidence paths are relative to it.
    pub source: PathBuf,
    /// The entry module, relative to `source`.
    pub entry: String,
    /// The screen slug.
    pub screen: String,
    /// Import aliases (`@/` to `src/`), longest first.
    pub aliases: Vec<(String, String)>,
}

/// The two documents written.
#[derive(Debug, Clone)]
pub struct Output {
    /// `behaviour.json`.
    pub behaviour: Value,
    /// `behaviour.unmapped.json`.
    pub unmapped: Value,
}

const EXTS: [&str; 8] = ["", ".tsx", ".ts", ".jsx", ".js", "/index.tsx", "/index.ts", "/index.js"];
const GLOBALS: [&str; 6] =
    ["localStorage", "sessionStorage", "window", "document", "fetch", "console"];
const MAX_DEPTH: usize = 24;

#[derive(Clone)]
struct Env {
    file: Rc<str>,
    frames: Vec<Rc<Frame>>,
    this: Option<Rc<Thunk>>,
    owner: Rc<str>,
}

#[derive(Default)]
struct Frame {
    vars: RefCell<BTreeMap<String, Rc<Thunk>>>,
}

#[derive(Clone)]
enum Step {
    Prop(String),
    Index(usize),
}

struct Thunk {
    value: V,
    env: Env,
    path: Vec<Step>,
    name: String,
    pair: Option<String>,
    cache: RefCell<Option<Sem>>,
}

#[derive(Clone)]
enum Sem {
    Lit(Value),
    Func(Rc<Func>, Env, String),
    Obj(Vec<(String, V)>, Env),
    Arr(Vec<V>, Env),
    Pkg(String),
    Class(String, String),
    Ns(String),
    StateInit(Value, Rc<str>, Lines),
    State(String, u8),
    Navigator,
    Mutation(Vec<(String, V)>, Env),
    Form(Vec<String>),
    Zod(V, Env),
    Route(String, Lines),
    RouteDef(String, Vec<(String, V)>, Env, Lines),
    Local(String),
    Unknown,
}

#[derive(Default)]
struct Flow {
    does: Vec<Value>,
    guard: Vec<Value>,
}

impl Flow {
    fn extend(&mut self, other: Self) {
        self.does.extend(other.does);
        self.guard.extend(other.guard);
    }
}

#[derive(Default)]
struct Gesture {
    does: Vec<Value>,
    guard: Vec<Value>,
    evidence: Vec<String>,
}

struct Site {
    seed: String,
    anchor: String,
    role: String,
    text: Option<String>,
    /// When the text is one of a few literals (`cond ? "Hide" : "Show"`).
    text_in: Vec<String>,
    href: Option<String>,
    name: Option<String>,
    in_overlay: bool,
    adopted: bool,
    gestures: BTreeMap<String, Gesture>,
    x_web: Map<String, Value>,
    submitters: Vec<Value>,
}

impl Site {
    fn tuple(&self) -> Value {
        let mut tuple = json!({
            "role": self.role, "text": self.text, "href": self.href,
            "name": self.name, "in_overlay": self.in_overlay,
        });
        if !self.text_in.is_empty() {
            tuple["text_in"] = json!(self.text_in);
        }
        tuple
    }
}

#[derive(Clone, Default)]
struct Jcx {
    in_overlay: bool,
    nearest: Option<usize>,
    form: Option<usize>,
    overlay: Option<(String, Option<String>)>,
    field: Option<String>,
    adopt: Option<usize>,
    seed: String,
    root: bool,
}

struct Ctx {
    table: Mappings,
    root: PathBuf,
    aliases: Vec<(String, String)>,
    modules: RefCell<BTreeMap<String, Option<Rc<Module>>>>,
    envs: RefCell<BTreeMap<String, Env>>,
    depth: Cell<usize>,
    sites_stack: RefCell<Vec<String>>,
    running: RefCell<BTreeSet<String>>,
    sites: RefCell<Vec<Site>>,
    state: RefCell<BTreeMap<String, Map<String, Value>>>,
    rules: RefCell<BTreeMap<String, Value>>,
    routes: RefCell<BTreeMap<String, (bool, BTreeSet<String>)>>,
    guards: RefCell<Vec<Value>>,
    unmapped: RefCell<BTreeSet<(String, String)>>,
    unconverted: RefCell<BTreeSet<(String, String, String)>>,
    pending: RefCell<Vec<(Rc<Func>, Env, String)>>,
    walked: RefCell<BTreeSet<String>>,
}

const TEXT_ROLES: [&str; 7] =
    ["button", "link", "menuitem", "menuitemcheckbox", "menuitemradio", "option", "tab"];

fn anchor(file: &str, lines: Lines) -> String {
    if lines.1 > lines.0 {
        format!("{file}#L{}-L{}", lines.0, lines.1)
    } else {
        format!("{file}#L{}", lines.0)
    }
}

fn evidence(anchors: &[String]) -> Value {
    let mut seen = BTreeSet::new();
    Value::Array(
        anchors
            .iter()
            .filter(|a| seen.insert((*a).clone()))
            .map(|a| json!({ "path": a }))
            .collect(),
    )
}

fn ev1(file: &str, lines: Lines) -> Value {
    evidence(&[anchor(file, lines)])
}

fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 9.0e15 {
        #[expect(clippy::cast_possible_truncation, reason = "checked integral and in range")]
        let i = n as i64;
        json!(i)
    } else {
        json!(n)
    }
}

fn prop<'v>(props: &'v [(String, V)], name: &str) -> Option<&'v V> {
    props.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

const fn is_exit(stmts: &[S]) -> bool {
    matches!(stmts, [S::Return(None | Some(V::Null | V::Bool(_) | V::Str(_)), _)])
}

fn normalize(path: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    out.join("/")
}

impl Ctx {
    fn new(options: &Options) -> Self {
        Self {
            table: Mappings::embedded(),
            root: options.source.clone(),
            aliases: options.aliases.clone(),
            modules: RefCell::default(),
            envs: RefCell::default(),
            depth: Cell::new(0),
            sites_stack: RefCell::default(),
            running: RefCell::default(),
            sites: RefCell::default(),
            state: RefCell::default(),
            rules: RefCell::default(),
            routes: RefCell::default(),
            guards: RefCell::default(),
            unmapped: RefCell::default(),
            unconverted: RefCell::default(),
            pending: RefCell::default(),
            walked: RefCell::default(),
        }
    }

    fn row(&self, key: &str, value: Option<&str>) -> Option<Row> {
        self.table.lookup(key, value).cloned()
    }

    fn note_unmapped(&self, key: &str, at: &str) {
        self.unmapped.borrow_mut().insert((key.to_string(), at.to_string()));
    }

    fn note_unconverted(&self, key: &str, reason: &str, at: &str) {
        self.unconverted.borrow_mut().insert((key.to_string(), reason.to_string(), at.to_string()));
    }

    fn note_route(&self, path: &str, declared: bool, at: &str) {
        let mut routes = self.routes.borrow_mut();
        let entry = routes.entry(path.to_string()).or_default();
        entry.0 |= declared;
        entry.1.insert(at.to_string());
    }

    // ---- modules -------------------------------------------------------

    fn resolve_path(&self, from: &str, spec: &str) -> Option<String> {
        let base = if spec.starts_with('.') {
            Path::new(from).parent().unwrap_or_else(|| Path::new("")).join(spec)
        } else {
            let (alias, target) =
                self.aliases.iter().find(|(a, _)| spec.starts_with(a.as_str()))?;
            Path::new(target).join(&spec[alias.len()..])
        };
        let base = normalize(&base);
        EXTS.iter().map(|ext| format!("{base}{ext}")).find(|c| self.root.join(c).is_file())
    }

    fn load(&self, rel: &str) -> Option<Rc<Module>> {
        if let Some(m) = self.modules.borrow().get(rel) {
            return m.clone();
        }
        let module = std::fs::read_to_string(self.root.join(rel)).ok().map(|text| {
            let module = syntax::parse(rel, &text);
            for error in &module.errors {
                self.note_unconverted("parse", error, &format!("{rel}#L1"));
            }
            Rc::new(module)
        });
        self.modules.borrow_mut().insert(rel.to_string(), module.clone());
        module
    }

    fn module_env(&self, rel: &str) -> Option<Env> {
        if let Some(env) = self.envs.borrow().get(rel) {
            return Some(env.clone());
        }
        let module = self.load(rel)?;
        let frame = Rc::new(Frame::default());
        let env = Env {
            file: rel.into(),
            frames: vec![frame.clone()],
            this: None,
            owner: "".into(),
        };
        for (name, (pat, value, _)) in &module.decls {
            if !name.contains('.') {
                Self::bind(pat, value, &env, &[], &frame, None);
            }
        }
        self.envs.borrow_mut().insert(rel.to_string(), env.clone());
        Some(env)
    }

    /// Every module reachable from `entry` through local imports, sorted.
    fn reachable(&self, entry: &str) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut todo = vec![entry.to_string()];
        while let Some(rel) = todo.pop() {
            if !seen.insert(rel.clone()) {
                continue;
            }
            let Some(module) = self.load(&rel) else { continue };
            let specs = module
                .imports
                .values()
                .map(|i| i.source.clone())
                .chain(module.reexport_all.iter().cloned())
                .chain(module.reexports.values().map(|(s, _)| s.clone()));
            for spec in specs {
                if let Some(target) = self.resolve_path(&rel, &spec) {
                    todo.push(target);
                }
            }
        }
        seen.into_iter().collect()
    }

    // ---- binding and evaluation -----------------------------------------

    fn bind(pat: &Pat, value: &V, env: &Env, path: &[Step], frame: &Frame, pair: Option<&str>) {
        match pat {
            Pat::Name(name) => {
                frame.vars.borrow_mut().insert(
                    name.clone(),
                    Rc::new(Thunk {
                        value: value.clone(),
                        env: env.clone(),
                        path: path.to_vec(),
                        name: name.clone(),
                        pair: pair.map(str::to_string),
                        cache: RefCell::new(None),
                    }),
                );
            }
            Pat::Obj(props) => {
                for (key, p) in props {
                    let mut next = path.to_vec();
                    next.push(Step::Prop(key.clone()));
                    Self::bind(p, value, env, &next, frame, pair);
                }
            }
            Pat::Arr(items) => {
                let first = match items.first() {
                    Some(Some(Pat::Name(n))) => Some(n.as_str()),
                    _ => pair,
                };
                for (i, item) in items.iter().enumerate() {
                    if let Some(p) = item {
                        let mut next = path.to_vec();
                        next.push(Step::Index(i));
                        Self::bind(p, value, env, &next, frame, first);
                    }
                }
            }
            Pat::Other => {}
        }
    }

    fn enter(&self) -> bool {
        let d = self.depth.get();
        if d >= MAX_DEPTH {
            return false;
        }
        self.depth.set(d + 1);
        true
    }

    fn leave(&self) {
        self.depth.set(self.depth.get().saturating_sub(1));
    }

    fn force(&self, thunk: &Thunk) -> Sem {
        if let Some(s) = thunk.cache.borrow().as_ref() {
            return s.clone();
        }
        if !self.enter() {
            return Sem::Unknown;
        }
        let mut sem = match &thunk.value {
            V::Func(f) if thunk.path.is_empty() => {
                let owner = if thunk.env.owner.is_empty() || thunk.env.frames.len() == 1 {
                    thunk.name.clone()
                } else {
                    format!("{}.{}", thunk.env.owner, thunk.name)
                };
                Sem::Func(f.clone(), thunk.env.clone(), owner)
            }
            other => match self.eval(other, &thunk.env) {
                Sem::Func(f, e, q) if q.is_empty() => Sem::Func(f, e, thunk.name.clone()),
                s => s,
            },
        };
        for step in &thunk.path {
            sem = match (step, sem) {
                (Step::Index(i), Sem::StateInit(initial, file, lines)) => {
                    let key = format!(
                        "{file}:{}.{}",
                        thunk.env.owner,
                        thunk.pair.clone().unwrap_or_else(|| thunk.name.clone())
                    );
                    let kind = if initial.is_boolean() { "flag" } else { "value" };
                    self.state.borrow_mut().entry(key.clone()).or_insert_with(|| {
                        let mut m = Map::new();
                        m.insert("kind".into(), json!(kind));
                        m.insert("initial".into(), initial.clone());
                        m.insert("evidence".into(), ev1(&file, lines));
                        m
                    });
                    Sem::State(key, if *i == 0 { 1 } else { 2 })
                }
                (Step::Index(i), Sem::Arr(items, env)) => {
                    items.get(*i).map_or(Sem::Unknown, |v| self.eval(v, &env))
                }
                (Step::Prop(p), s) => self.member(s, p),
                _ => Sem::Unknown,
            };
        }
        self.leave();
        *thunk.cache.borrow_mut() = Some(sem.clone());
        sem
    }

    fn thunk_of(env: &Env, name: &str) -> Option<Rc<Thunk>> {
        env.frames.iter().rev().find_map(|f| f.vars.borrow().get(name).cloned())
    }

    fn lookup(&self, name: &str, env: &Env) -> Sem {
        if let Some(t) = Self::thunk_of(env, name) {
            return self.force(&t);
        }
        let Some(module) = self.load(&env.file) else { return Sem::Unknown };
        let class_prefix = format!("{name}.");
        if module.decls.keys().any(|k| k.starts_with(&class_prefix)) {
            return Sem::Class(env.file.to_string(), name.to_string());
        }
        if let Some(import) = module.imports.get(name) {
            return match self.resolve_path(&env.file, &import.source) {
                Some(target) if import.name == "*" => Sem::Ns(target),
                Some(target) => self.export(&target, &import.name, 0),
                None if import.name == "*" || import.name == "default" => {
                    Sem::Pkg(format!("{}#", import.source))
                }
                None => Sem::Pkg(format!("{}#{}", import.source, import.name)),
            };
        }
        if GLOBALS.contains(&name) {
            return Sem::Pkg(format!("global#{name}"));
        }
        Sem::Unknown
    }

    fn export(&self, file: &str, name: &str, depth: usize) -> Sem {
        if depth > 8 {
            return Sem::Unknown;
        }
        let (Some(module), Some(env)) = (self.load(file), self.module_env(file)) else {
            return Sem::Unknown;
        };
        if let Some(local) = module.exports.get(name) {
            return self.lookup(local, &env);
        }
        if let Some((source, original)) = module.reexports.get(name) {
            return self.resolve_path(file, source).map_or_else(
                || Sem::Pkg(format!("{source}#{original}")),
                |target| self.export(&target, original, depth + 1),
            );
        }
        for source in &module.reexport_all {
            if let Some(target) = self.resolve_path(file, source) {
                let sem = self.export(&target, name, depth + 1);
                if !matches!(sem, Sem::Unknown) {
                    return sem;
                }
            }
        }
        Sem::Unknown
    }

    fn literal(&self, v: &V, env: &Env) -> Option<Value> {
        match self.eval(v, env) {
            Sem::Lit(value) => Some(value),
            _ => None,
        }
    }

    fn literal_str(&self, v: &V, env: &Env) -> Option<String> {
        self.literal(v, env).and_then(|v| v.as_str().map(str::to_string))
    }

    fn eval(&self, v: &V, env: &Env) -> Sem {
        if !self.enter() {
            return Sem::Unknown;
        }
        let sem = match v {
            V::Str(s) => Sem::Lit(json!(s)),
            V::Num(n) => Sem::Lit(num(*n)),
            V::Bool(b) => Sem::Lit(json!(b)),
            V::Null => Sem::Lit(Value::Null),
            V::Ident(name) => self.lookup(name, env),
            V::This => env.this.as_ref().map_or(Sem::Unknown, |t| self.force(t)),
            V::Member(object, name) => {
                let object = self.eval(object, env);
                self.member(object, name)
            }
            V::Call(callee, args, lines) => self.call_value(callee, args, *lines, env),
            V::Func(f) => Sem::Func(f.clone(), env.clone(), env.owner.to_string()),
            V::Obj(props) => Sem::Obj(props.clone(), env.clone()),
            V::Arr(items) => Sem::Arr(items.clone(), env.clone()),
            V::Await(inner) => self.eval(inner, env),
            _ => Sem::Unknown,
        };
        self.leave();
        sem
    }

    fn member(&self, object: Sem, name: &str) -> Sem {
        match object {
            Sem::Pkg(key) if key.ends_with('#') => Sem::Pkg(format!("{key}{name}")),
            Sem::Pkg(key) => Sem::Pkg(format!("{key}.{name}")),
            Sem::Obj(props, env) => prop(&props, name).map_or(Sem::Unknown, |v| match v {
                V::Func(f) => Sem::Func(f.clone(), env.clone(), format!("{}.{name}", env.owner)),
                other => self.eval(other, &env),
            }),
            Sem::Class(file, class) => {
                let key = format!("{class}.{name}");
                match (self.load(&file), self.module_env(&file)) {
                    (Some(module), Some(env)) => match module.decls.get(&key) {
                        Some((_, V::Func(f), _)) => Sem::Func(f.clone(), env, key),
                        _ => Sem::Unknown,
                    },
                    _ => Sem::Unknown,
                }
            }
            Sem::Ns(file) => self.export(&file, name, 0),
            // a route object's hooks (`Route.useNavigate()`): rows keyed `call:route#<hook>`
            Sem::RouteDef(..) => Sem::Pkg(format!("route#{name}")),
            Sem::Local(q) => Sem::Local(format!("{q}.{name}")),
            _ => Sem::Unknown,
        }
    }

    fn call_value(&self, callee: &V, args: &[V], lines: Lines, env: &Env) -> Sem {
        match self.eval(callee, env) {
            Sem::Pkg(key) => {
                let Some(row) = self.row(&format!("call:{key}"), None) else { return Sem::Unknown };
                let first = args.first();
                match row.converter.as_str() {
                    "use_state" => Sem::StateInit(
                        first.and_then(|a| self.literal(a, env)).unwrap_or(Value::Null),
                        env.file.clone(),
                        lines,
                    ),
                    "navigator" => Sem::Navigator,
                    "mutation" => match first.map(|a| self.eval(a, env)) {
                        Some(Sem::Obj(props, menv)) => Sem::Mutation(props, menv),
                        _ => Sem::Unknown,
                    },
                    "form" => self.form(first, env, lines),
                    "zod_resolver" => {
                        first.map_or(Sem::Unknown, |a| Sem::Zod(a.clone(), env.clone()))
                    }
                    "file_route" => first
                        .and_then(|a| self.literal_str(a, env))
                        .map_or(Sem::Unknown, |path| Sem::Route(path, lines)),
                    "wrapper" => first.map_or(Sem::Unknown, |a| self.eval(a, env)),
                    _ => Sem::Unknown,
                }
            }
            Sem::Route(path, route_lines) => match args.first().map(|a| self.eval(a, env)) {
                Some(Sem::Obj(props, oenv)) => Sem::RouteDef(path, props, oenv, route_lines),
                _ => Sem::Unknown,
            },
            Sem::Func(f, fenv, q) => {
                let (benv, _) = Self::enter_func(&f, &fenv, &q, args, env);
                let ret = first_return(&f.body).map_or(Sem::Unknown, |r| self.eval(r, &benv));
                match ret {
                    Sem::Unknown => Sem::Local(format!("{}:{q}()", fenv.file)),
                    other => other,
                }
            }
            Sem::Local(q) => Sem::Local(format!("{q}()")),
            _ => Sem::Unknown,
        }
    }

    /// The environment of a function body: parameters bound to the call's
    /// arguments (evaluated in the caller), then the body's declarations.
    fn enter_func(f: &Func, fenv: &Env, q: &str, args: &[V], caller: &Env) -> (Env, Rc<Frame>) {
        let frame = Rc::new(Frame::default());
        for (i, param) in f.params.iter().enumerate() {
            let value = args.get(i).cloned().unwrap_or(V::Other(String::new(), (0, 0)));
            Self::bind(param, &value, caller, &[], &frame, None);
        }
        let mut benv = fenv.clone();
        benv.frames.push(frame);
        benv.owner = q.into();
        let body_frame = Rc::new(Frame::default());
        benv.frames.push(body_frame.clone());
        for stmt in &f.body {
            if let S::Decl(pat, Some(init), _) = stmt {
                Self::bind(pat, init, &benv, &[], &body_frame, None);
            }
        }
        (benv, body_frame)
    }

    // ---- forms and zod ---------------------------------------------------

    fn raw(&self, v: &V, env: &Env) -> (V, Env) {
        let mut v = v.clone();
        let mut env = env.clone();
        for _ in 0..8 {
            let V::Ident(name) = &v else { break };
            let t = Self::thunk_of(&env, name).or_else(|| {
                let module = self.load(&env.file)?;
                let import = module.imports.get(name)?;
                let target = self.resolve_path(&env.file, &import.source)?;
                let tmodule = self.load(&target)?;
                let local = tmodule.exports.get(&import.name)?;
                Self::thunk_of(&self.module_env(&target)?, local)
            });
            match t {
                Some(t) if t.path.is_empty() => {
                    v = t.value.clone();
                    env = t.env.clone();
                }
                _ => break,
            }
        }
        (v, env)
    }

    fn form(&self, opts: Option<&V>, env: &Env, lines: Lines) -> Sem {
        let Some(Sem::Obj(props, oenv)) = opts.map(|o| self.eval(o, env)) else {
            return Sem::Form(Vec::new());
        };
        let mut fields = Vec::new();
        if let Some(Sem::Zod(schema, zenv)) = prop(&props, "resolver").map(|r| self.eval(r, &oenv))
        {
            let (schema, zenv) = self.raw(&schema, &zenv);
            if let V::Call(_, args, _) = &schema
                && let Some(V::Obj(shape)) = args.first()
            {
                for (field, chain) in shape {
                    fields.push(field.clone());
                    self.zod_rules(field, chain, &zenv);
                }
            }
        }
        if let Some(Sem::Obj(defaults, denv)) =
            prop(&props, "defaultValues").map(|d| self.eval(d, &oenv))
        {
            for (field, value) in &defaults {
                if !fields.contains(field) {
                    fields.push(field.clone());
                }
                let key = format!("{}:{}.{field}", env.file, env.owner);
                let initial = self.literal(value, &denv).unwrap_or(Value::Null);
                self.state.borrow_mut().entry(key).or_insert_with(|| {
                    let mut m = Map::new();
                    m.insert("kind".into(), json!("field"));
                    m.insert("initial".into(), initial);
                    m.insert("evidence".into(), ev1(&env.file, lines));
                    m
                });
            }
        }
        Sem::Form(fields)
    }

    fn zod_rules(&self, field: &str, chain: &V, env: &Env) {
        let mut calls = Vec::new();
        let mut cur = chain;
        while let V::Call(callee, args, lines) = cur {
            let V::Member(object, method) = &**callee else { break };
            calls.push((method.clone(), args.clone(), *lines));
            cur = object;
        }
        calls.reverse();
        for (method, args, lines) in calls {
            let at = anchor(&env.file, (lines.1, lines.1));
            let key = format!("zod:{method}");
            match self.row(&key, None) {
                Some(row) if row.converter == "zod_rule" => {
                    let mut rule = Map::new();
                    rule.insert("field".into(), json!(field));
                    rule.insert("rule".into(), json!(row.param("rule")));
                    // zod: a check with no value (`email`, `url`, `nonempty`) takes the
                    // message first; one with a value (`min(n, message)`) second. The
                    // message is a string, or `{ message }` / `{ error }` with a string.
                    let no_value = matches!(method.as_str(), "email" | "url" | "nonempty");
                    if let Some(value) = args.first().and_then(|a| self.literal(a, env))
                        && (value.is_number() || value.is_string())
                        && !no_value
                    {
                        rule.insert("value".into(), value);
                    }
                    let message =
                        args.iter().enumerate().find_map(|(i, a)| match self.eval(a, env) {
                            Sem::Obj(p, e) => prop(&p, "message")
                                .or_else(|| prop(&p, "error"))
                                .and_then(|m| self.literal_str(m, &e)),
                            Sem::Lit(Value::String(s)) if i == usize::from(!no_value) => Some(s),
                            _ => None,
                        });
                    rule.insert("message".into(), json!(message));
                    rule.insert("nodes".into(), json!([]));
                    rule.insert("evidence".into(), evidence(std::slice::from_ref(&at)));
                    let key = format!("{field}\u{0}{at}\u{0}{method}");
                    self.rules.borrow_mut().insert(key, Value::Object(rule));
                }
                Some(_) => {}
                None => self.note_unmapped(&key, &at),
            }
        }
    }

    // ---- effects ---------------------------------------------------------

    fn handler(&self, v: &V, env: &Env) -> Flow {
        if let V::Call(callee, args, _) = v
            && let V::Member(target, method) = &**callee
            && method == "bind"
        {
            if let Sem::Func(f, mut fenv, q) = self.eval(target, env) {
                fenv.this = args.first().map(|a| {
                    Rc::new(Thunk {
                        value: a.clone(),
                        env: env.clone(),
                        path: Vec::new(),
                        name: "this".into(),
                        pair: None,
                        cache: RefCell::new(None),
                    })
                });
                return self.run(&f, &fenv, &q, &[], env);
            }
            return Flow::default();
        }
        if matches!(v, V::Call(..)) {
            return self.call_fx(v, env);
        }
        match self.eval(v, env) {
            Sem::Func(f, fenv, q) => self.run(&f, &fenv, &q, &[], env),
            _ => Flow::default(),
        }
    }

    fn run(&self, f: &Func, fenv: &Env, q: &str, args: &[V], caller: &Env) -> Flow {
        let key = format!("{}:{q}:{}", fenv.file, f.lines.0);
        if !self.running.borrow_mut().insert(key.clone()) || !self.enter() {
            return Flow::default();
        }
        let (benv, _) = Self::enter_func(f, fenv, q, args, caller);
        let flow = self.stmts_fx(&f.body, &benv);
        self.leave();
        self.running.borrow_mut().remove(&key);
        flow
    }

    fn stmts_fx(&self, stmts: &[S], env: &Env) -> Flow {
        let mut flow = Flow::default();
        for stmt in stmts {
            match stmt {
                S::Decl(_, Some(init), _) | S::Expr(init, _) => {
                    flow.extend(self.expr_fx(init, env));
                }
                S::Decl(_, None, _) => {}
                S::If(_, text, then, els, lines) => {
                    let at = anchor(&env.file, (lines.0, lines.0));
                    if is_exit(then) && els.is_empty() {
                        flow.guard.push(json!({ "expr": text, "evidence": evidence(&[at]) }));
                        continue;
                    }
                    let mut then_flow = self.stmts_fx(then, env);
                    let mut else_flow = self.stmts_fx(els, env);
                    for e in &mut then_flow.does {
                        e.as_object_mut().map(|o| {
                            o.entry("when").or_insert(
                                json!({ "expr": text, "evidence": evidence(std::slice::from_ref(&at)) }),
                            )
                        });
                    }
                    for e in &mut else_flow.does {
                        e.as_object_mut().map(|o| o.entry("when").or_insert(json!({ "expr": format!("!({text})"), "evidence": evidence(std::slice::from_ref(&at)) })));
                    }
                    flow.extend(then_flow);
                    flow.extend(else_flow);
                }
                S::Return(value, _) => {
                    if let Some(v) = value {
                        flow.extend(self.expr_fx(v, env));
                    }
                    break;
                }
                S::Throw(..) => break,
            }
        }
        flow
    }

    fn expr_fx(&self, v: &V, env: &Env) -> Flow {
        match v {
            V::Call(..) => self.call_fx(v, env),
            V::Await(inner) => self.expr_fx(inner, env),
            V::Logical(a, b) | V::Cond(_, a, b) => {
                let mut flow = self.expr_fx(a, env);
                flow.extend(self.expr_fx(b, env));
                flow
            }
            V::Ops(items) => {
                let mut flow = Flow::default();
                for item in items {
                    flow.extend(self.expr_fx(item, env));
                }
                flow
            }
            _ => Flow::default(),
        }
    }

    fn site_anchor(&self, env: &Env, lines: Lines) -> String {
        self.sites_stack.borrow().last().cloned().unwrap_or_else(|| anchor(&env.file, lines))
    }

    fn call_fx(&self, v: &V, env: &Env) -> Flow {
        let V::Call(callee, args, lines) = v else { return Flow::default() };
        let lines = *lines;
        let at = anchor(&env.file, lines);
        let mut flow = Flow::default();
        if let V::Member(object, method) = &**callee {
            if let Some(row) = self.row(&format!("call:http#{method}"), None)
                && row.converter == "http_operation"
                && let Some(V::Obj(props)) = args.first()
                && let Some(url) = prop(props, "url").and_then(|u| self.literal_str(u, env))
                && url.starts_with('/')
            {
                flow.does.push(json!({
                    "kind": "call",
                    "callee": format!("{}:{}", env.file, env.owner),
                    "args": [],
                    "operation": { "method": row.param("method"), "url": url, "evidence": evidence(&[at]) },
                    "evidence": evidence(&[self.site_anchor(env, lines)]),
                }));
                return flow;
            }
            match self.eval(object, env) {
                Sem::Mutation(props, menv)
                    if self
                        .row("call:@tanstack/react-query#useMutation", None)
                        .is_some_and(|r| r.has_method(method)) =>
                {
                    return self.mutation_fx(&props, &menv, &at);
                }
                Sem::Form(fields)
                    if self
                        .row("call:react-hook-form#useForm", None)
                        .is_some_and(|r| r.has_method(method)) =>
                {
                    flow.does.push(json!({ "kind": "validate", "fields": fields, "evidence": evidence(&[at]) }));
                    if let Some(h) = args.first() {
                        flow.extend(self.handler(h, env));
                    }
                    return flow;
                }
                _ => {}
            }
        }
        match self.eval(callee, env) {
            Sem::Navigator => {
                let to = args.first().and_then(|a| match self.eval(a, env) {
                    Sem::Obj(p, e) => prop(&p, "to").and_then(|t| self.literal_str(t, &e)),
                    Sem::Lit(Value::String(s)) => Some(s),
                    _ => None,
                });
                match to {
                    Some(to) if to.starts_with('/') => {
                        self.note_route(&to, false, &at);
                        flow.does.push(
                            json!({ "kind": "navigate", "to": to, "evidence": evidence(&[at]) }),
                        );
                    }
                    _ => {
                        self.note_unconverted("call:navigate", "target is not a literal path", &at);
                    }
                }
            }
            Sem::State(key, 2) => {
                let value = args
                    .first()
                    .and_then(|a| self.literal(a, env))
                    .filter(|v| !v.is_object() && !v.is_array());
                if value.is_none() {
                    self.note_unconverted("call:setState", "value is not a literal", &at);
                }
                flow.does.push(json!({ "kind": "set", "state": key, "value": value, "evidence": evidence(&[at]) }));
            }
            Sem::Pkg(key) => flow.extend(self.pkg_fx(&key, args, env, &at)),
            Sem::Func(f, fenv, q) => {
                self.sites_stack.borrow_mut().push(at);
                flow.extend(self.run(&f, &fenv, &q, args, env));
                self.sites_stack.borrow_mut().pop();
            }
            Sem::Local(q) => {
                let literal_args: Vec<Value> =
                    args.iter().filter_map(|a| self.literal(a, env)).collect();
                flow.does.push(json!({ "kind": "call", "callee": q, "args": literal_args, "evidence": evidence(&[at]) }));
            }
            _ => {}
        }
        flow
    }

    fn pkg_fx(&self, key: &str, args: &[V], env: &Env, at: &str) -> Flow {
        let mut flow = Flow::default();
        let Some(row) = self.row(&format!("call:{key}"), None) else {
            self.note_unmapped(&format!("call:{key}"), at);
            return flow;
        };
        match row.converter.as_str() {
            "show" => {
                let title = args.first().and_then(|a| self.literal_str(a, env));
                flow.does.push(json!({
                    "kind": "show", "surface": row.param("surface"), "tone": row.param("tone"),
                    "title": title, "evidence": evidence(&[at.to_string()]),
                }));
            }
            "storage" => {
                let storage_key = args.first().and_then(|a| self.literal_str(a, env));
                flow.does.push(json!({
                    "kind": "call",
                    "callee": format!("global:{}", key.trim_start_matches("global#")),
                    "args": storage_key.iter().collect::<Vec<_>>(),
                    "x-web": { "storage": { "op": row.param("op"), "key": storage_key } },
                    "evidence": evidence(&[at.to_string()]),
                }));
            }
            _ => {}
        }
        flow
    }

    fn mutation_fx(&self, props: &[(String, V)], env: &Env, at: &str) -> Flow {
        self.sites_stack.borrow_mut().push(at.to_string());
        let mut flow =
            prop(props, "mutationFn").map_or_else(Flow::default, |m| self.handler(m, env));
        self.sites_stack.borrow_mut().pop();
        let success = prop(props, "onSuccess").map_or_else(Flow::default, |h| self.handler(h, env));
        let failure = prop(props, "onError").map_or_else(Flow::default, |h| self.handler(h, env));
        let index = flow.does.iter().position(|e| e.get("operation").is_some());
        if let Some(i) = index {
            let tail: Vec<Value> = flow.does.split_off(i + 1);
            let call = &mut flow.does[i];
            let mut on_success = tail;
            on_success.extend(success.does);
            call["on_success"] = Value::Array(on_success);
            call["on_failure"] = Value::Array(failure.does);
            let mut ev = vec![json!({ "path": at })];
            if let Some(old) = call["evidence"].as_array() {
                ev.extend(old.iter().filter(|e| e["path"] != at).cloned());
            }
            call["evidence"] = Value::Array(ev);
        } else {
            self.note_unconverted(
                "call:useMutation",
                "mutationFn makes no request the table knows",
                at,
            );
            flow.does.extend(success.does);
        }
        flow
    }

    fn storage_reads(&self, v: &V, env: &Env, out: &mut Vec<Value>, depth: usize) {
        if depth > 6 {
            return;
        }
        match v {
            V::Call(callee, args, _) => {
                match self.eval(callee, env) {
                    Sem::Pkg(key) => {
                        if let Some(row) = self.row(&format!("call:{key}"), None)
                            && row.converter == "storage"
                        {
                            let k = args.first().and_then(|a| self.literal_str(a, env));
                            out.push(json!({ "op": row.param("op"), "key": k }));
                        }
                    }
                    Sem::Func(f, fenv, q) => {
                        let (benv, _) = Self::enter_func(&f, &fenv, &q, args, env);
                        if let Some(r) = first_return(&f.body) {
                            self.storage_reads(r, &benv, out, depth + 1);
                        }
                    }
                    _ => {}
                }
                for a in args {
                    self.storage_reads(a, env, out, depth + 1);
                }
            }
            V::Logical(a, b) | V::Cond(_, a, b) => {
                self.storage_reads(a, env, out, depth + 1);
                self.storage_reads(b, env, out, depth + 1);
            }
            V::Ops(items) => items.iter().for_each(|i| self.storage_reads(i, env, out, depth + 1)),
            V::Member(o, _) | V::Await(o) => self.storage_reads(o, env, out, depth + 1),
            _ => {}
        }
    }

    // ---- routes and guards ----------------------------------------------

    fn route(&self, id: &str, props: &[(String, V)], env: &Env, lines: Lines) {
        let path = route_url(id);
        // a pathless layout (`/_auth`) has no URL of its own
        if !id.rsplit('/').next().is_some_and(|s| s.starts_with('_')) {
            self.note_route(&path, true, &anchor(&env.file, lines));
        }
        if let Some(Sem::Func(f, fenv, q)) = prop(props, "component").map(|c| self.eval(c, env)) {
            self.enqueue(f, fenv, q);
        }
        let Some(Sem::Func(f, fenv, q)) = prop(props, "beforeLoad").map(|b| self.eval(b, env))
        else {
            return;
        };
        let (benv, _) = Self::enter_func(&f, &fenv, &q, &[], env);
        for stmt in &f.body {
            let S::If(cond, text, then, _, if_lines) = stmt else { continue };
            for inner in then {
                let S::Throw(V::Call(callee, args, throw_lines), _) = inner else { continue };
                let is_redirect = matches!(self.eval(callee, &benv), Sem::Pkg(k) if self.row(&format!("call:{k}"), None).is_some_and(|r| r.converter == "redirect"));
                let go = args.first().and_then(|a| match self.eval(a, &benv) {
                    Sem::Obj(p, e) => prop(&p, "to").and_then(|t| self.literal_str(t, &e)),
                    _ => None,
                });
                let at = anchor(&benv.file, *throw_lines);
                match (is_redirect, go) {
                    (true, Some(go)) if go.starts_with('/') => {
                        let mut reads = Vec::new();
                        self.storage_reads(cond, &benv, &mut reads, 0);
                        let mut when = json!({ "expr": text, "evidence": ev1(&benv.file, (if_lines.0, if_lines.0)) });
                        if !reads.is_empty() {
                            when["x-web"] = json!({ "storage": reads });
                        }
                        self.note_route(&go, false, &at);
                        self.guards.borrow_mut().push(json!({
                            "route": path, "when": when, "go": go, "evidence": evidence(&[at]),
                        }));
                    }
                    (true, _) => {
                        self.note_unconverted("call:redirect", "target is not a literal path", &at);
                    }
                    _ => {}
                }
            }
        }
    }

    // ---- JSX -------------------------------------------------------------

    fn element_row(&self, name: &JName, env: &Env, depth: usize) -> Option<Row> {
        match name {
            JName::Tag(tag) => self.row(&format!("jsx:{tag}"), None),
            JName::Ref(v) => self.component_row(v, env, depth),
        }
    }

    fn component_row(&self, v: &V, env: &Env, depth: usize) -> Option<Row> {
        if depth > 8 {
            return None;
        }
        if let V::Ident(name) = v
            && let Some(t) = Self::thunk_of(env, name)
            && t.path.is_empty()
            && let V::Cond(_, a, b) = &t.value
        {
            let pick = |branch: &V| match branch {
                V::Str(tag) => self.row(&format!("jsx:{tag}"), None),
                other => self.component_row(other, &t.env, depth + 1),
            };
            return pick(a).filter(|r| r.converter != "passthrough").or_else(|| pick(b));
        }
        match self.eval(v, env) {
            Sem::Pkg(key) => self.row(&format!("jsx:{key}"), None),
            Sem::Lit(Value::String(tag)) => self.row(&format!("jsx:{tag}"), None),
            Sem::Func(f, fenv, q) => {
                let (benv, _) = Self::enter_func(&f, &fenv, &q, &[], &fenv);
                returns(&f.body).into_iter().find_map(|r| self.first_row(r, &benv, depth + 1))
            }
            _ => None,
        }
    }

    fn first_row(&self, v: &V, env: &Env, depth: usize) -> Option<Row> {
        match v {
            V::Jsx(j) => match self.element_row(&j.name, env, depth) {
                Some(row) if row.converter != "passthrough" => Some(row),
                Some(_) => j.children.iter().find_map(|c| self.first_row(c, env, depth)),
                None => None,
            },
            V::Frag(children) => children.iter().find_map(|c| self.first_row(c, env, depth)),
            V::Cond(_, a, b) => {
                self.first_row(a, env, depth).or_else(|| self.first_row(b, env, depth))
            }
            _ => None,
        }
    }

    /// Every text the children can render, when each is fixed by the code:
    /// one for plain text, several for a conditional between literals.
    /// `None` when any part is not a literal.
    fn texts(&self, children: &[V], env: &Env) -> Option<BTreeSet<String>> {
        const MAX_TEXTS: usize = 8;
        let mut acc = BTreeSet::from([String::new()]);
        for child in children {
            let options: BTreeSet<String> = match child {
                V::Str(s) => BTreeSet::from([s.clone()]),
                V::Jsx(j) => {
                    // A local component may render text of its own that the
                    // call site does not show: the text is then unknown.
                    if let JName::Ref(r) = &j.name
                        && matches!(self.eval(r, env), Sem::Func(..))
                    {
                        return None;
                    }
                    self.texts(&j.children, env)?
                }
                V::Frag(c) => self.texts(c, env)?,
                V::Cond(_, a, b) => {
                    let mut both = self.texts(std::slice::from_ref(&**a), env)?;
                    both.extend(self.texts(std::slice::from_ref(&**b), env)?);
                    both
                }
                V::Logical(_, b) => {
                    if self.texts(std::slice::from_ref(&**b), env)?.iter().any(|t| !t.is_empty()) {
                        return None;
                    }
                    BTreeSet::from([String::new()])
                }
                V::Ident(_) | V::Member(..) => match self.eval(child, env) {
                    Sem::Lit(Value::String(s)) => BTreeSet::from([s]),
                    Sem::Lit(Value::Number(n)) => BTreeSet::from([n.to_string()]),
                    _ => return None,
                },
                _ => return None,
            };
            acc = acc.iter().flat_map(|a| options.iter().map(move |o| format!("{a}{o}"))).collect();
            if acc.len() > MAX_TEXTS {
                return None;
            }
        }
        Some(acc)
    }

    fn enqueue(&self, f: Rc<Func>, fenv: Env, name: String) {
        let key = format!("{}:{name}:{}", fenv.file, f.lines.0);
        if self.walked.borrow_mut().insert(key) {
            self.pending.borrow_mut().push((f, fenv, name));
        }
    }

    fn walk_component(&self, f: &Func, fenv: &Env, name: &str) {
        let (benv, body_frame) = Self::enter_func(f, fenv, name, &[], fenv);
        let thunks: Vec<Rc<Thunk>> = body_frame.vars.borrow().values().cloned().collect();
        for t in thunks {
            self.force(&t);
        }
        let jcx = Jcx {
            root: true,
            ..Jcx::default()
        };
        for r in returns(&f.body) {
            self.walk_node(r, &benv, &jcx);
        }
    }

    fn walk_node(&self, v: &V, env: &Env, jcx: &Jcx) {
        match v {
            V::Jsx(j) => self.walk_el(j, env, jcx),
            V::Frag(children) => children.iter().for_each(|c| self.walk_node(c, env, jcx)),
            V::Cond(_, a, b) => {
                self.walk_node(a, env, jcx);
                self.walk_node(b, env, jcx);
            }
            V::Logical(_, b) => self.walk_node(b, env, jcx),
            V::Call(callee, args, _) => {
                if let V::Member(list, method) = &**callee
                    && method == "map"
                    && let Some(V::Func(cb)) = args.first()
                {
                    if let Sem::Arr(items, ienv) = self.eval(list, env) {
                        for (i, item) in items.iter().enumerate() {
                            let frame = Rc::new(Frame::default());
                            if let Some(p) = cb.params.first() {
                                Self::bind(p, item, &ienv, &[], &frame, None);
                            }
                            if let Some(p) = cb.params.get(1) {
                                Self::bind(
                                    p,
                                    &V::Num(f64::from(u32::try_from(i).unwrap_or(u32::MAX))),
                                    &ienv,
                                    &[],
                                    &frame,
                                    None,
                                );
                            }
                            let mut benv = env.clone();
                            benv.frames.push(frame);
                            let mut child = jcx.clone();
                            child.seed = format!("{}/{i}", jcx.seed);
                            for r in returns(&cb.body) {
                                self.walk_node(r, &benv, &child);
                            }
                        }
                    } else {
                        let frame = Rc::new(Frame::default());
                        let mut benv = env.clone();
                        benv.frames.push(frame);
                        for r in returns(&cb.body) {
                            self.walk_node(r, &benv, jcx);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn add_effects(&self, site: usize, gesture: &str, flow: Flow, at: String) {
        let mut sites = self.sites.borrow_mut();
        let g = sites[site].gestures.entry(gesture.to_string()).or_default();
        g.does.extend(flow.does);
        g.guard.extend(flow.guard);
        g.evidence.push(at);
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one pass over an element's attributes, kept together"
    )]
    fn walk_el(&self, j: &Jsx, env: &Env, jcx: &Jcx) {
        if let JName::Ref(v) = &j.name {
            match self.eval(v, env) {
                Sem::Func(f, fenv, q) => self.enqueue(f, fenv, q),
                Sem::Pkg(key) if self.row(&format!("jsx:{key}"), None).is_none() => {
                    self.note_unmapped(
                        &format!("jsx:{key}"),
                        &anchor(&env.file, (j.lines.0, j.lines.0)),
                    );
                }
                _ => {}
            }
        }
        let row = self.element_row(&j.name, env, 0);
        let conv = row.as_ref().map_or("", |r| r.converter.as_str());
        let attr = |name: &str| {
            j.attrs.iter().find_map(|a| match a {
                Attr::Named(n, v, l) if n == name => Some((v.clone(), *l)),
                _ => None,
            })
        };
        let attr_lit =
            |name: &str| attr(name).and_then(|(v, _)| v.and_then(|v| self.literal_str(&v, env)));
        let elem_at = anchor(&env.file, (j.lines.0, j.lines.0));
        let by_type = row.as_ref().zip(attr_lit("type")).and_then(|(r, t)| {
            r.raw
                .get("role_by_type")
                .and_then(|m| m.get(&t))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        let mut role = match conv {
            "element" | "overlay_trigger" | "overlay_close" => {
                by_type.or_else(|| row.as_ref().and_then(|r| r.param("role")).map(str::to_string))
            }
            _ => None,
        };
        let href_attr = row.as_ref().and_then(|r| r.param("href_attr")).map(str::to_string);
        let mut child = jcx.clone();
        child.adopt = None;
        child.root = jcx.root && conv == "passthrough";
        let wrapper_root = jcx.root && j.attrs.iter().any(|a| matches!(a, Attr::Spread(_)));
        if wrapper_root {
            role = None;
        }
        let mut site_idx = None;
        if let Some(r) = role.clone() {
            let texts = TEXT_ROLES
                .contains(&r.as_str())
                .then(|| self.texts(&j.children, env))
                .flatten()
                .map(|all| {
                    all.iter()
                        .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let (text, text_in) = if texts.len() == 1 {
                (texts.into_iter().next(), Vec::new())
            } else {
                (None, texts.into_iter().collect())
            };
            let href = href_attr.as_deref().and_then(attr_lit);
            let field_role = matches!(r.as_str(), "textbox" | "checkbox" | "radio" | "combobox");
            let name =
                if field_role { attr_lit("name").or_else(|| jcx.field.clone()) } else { None };
            let mut sites = self.sites.borrow_mut();
            if let Some(idx) = jcx.adopt
                && !sites[idx].adopted
            {
                let s = &mut sites[idx];
                s.adopted = true;
                s.role = r;
                if text.is_some() || !text_in.is_empty() {
                    s.text = text;
                    s.text_in = text_in;
                }
                if href.is_some() {
                    s.href = href;
                }
                site_idx = Some(idx);
            } else {
                sites.push(Site {
                    seed: format!("{elem_at}{}", jcx.seed),
                    anchor: elem_at.clone(),
                    role: r,
                    text,
                    text_in,
                    href,
                    name,
                    in_overlay: jcx.in_overlay,
                    adopted: false,
                    gestures: BTreeMap::new(),
                    x_web: Map::new(),
                    submitters: Vec::new(),
                });
                site_idx = Some(sites.len() - 1);
            }
            child.nearest = site_idx;
        }
        let target = site_idx.or(jcx.nearest);
        // HTML: a button in a form with no `type` submits it.
        if let (Some(i), Some(form)) = (site_idx, jcx.form)
            && role.as_deref() == Some("button")
            && conv == "element"
            && attr("type").is_none()
            && row.as_ref().is_some_and(|r| r.source == "jsx:button")
        {
            let tuple = self.sites.borrow()[i].tuple();
            self.sites.borrow_mut()[form].submitters.push(json!({
                "match": tuple, "nodes": [], "match_status": "unresolved", "evidence": evidence(std::slice::from_ref(&elem_at)),
            }));
        }
        for a in &j.attrs {
            let Attr::Named(n, Some(v), alines) = a else { continue };
            let at = anchor(&env.file, *alines);
            let lit = self.literal_str(v, env);
            if let Some(r) = self.row(&format!("attr:{n}"), lit.as_deref()) {
                match r.converter.as_str() {
                    "gesture" => {
                        let gesture = r.param("gesture").unwrap_or("tap").to_string();
                        if let Some(t) = target {
                            let flow = self.handler(v, env);
                            self.add_effects(t, &gesture, flow, at);
                        } else {
                            self.note_unconverted(
                                &format!("attr:{n}"),
                                "no element with a role around the handler",
                                &at,
                            );
                        }
                    }
                    "new_tab" => {
                        if let Some(i) = site_idx {
                            self.sites.borrow_mut()[i].x_web.insert("new_tab".into(), json!(true));
                        }
                    }
                    "submitter" => {
                        if let (Some(i), Some(form)) = (site_idx, jcx.form) {
                            let tuple = self.sites.borrow()[i].tuple();
                            self.sites.borrow_mut()[form].submitters.push(json!({
                            "match": tuple, "nodes": [], "match_status": "unresolved", "evidence": evidence(&[at]),
                        }));
                        }
                    }
                    _ => {}
                }
            } else {
                let handler_like = n.len() > 2
                    && n.starts_with("on")
                    && n.chars().nth(2).is_some_and(char::is_uppercase)
                    && matches!(v, V::Ident(_) | V::Member(..) | V::Func(_) | V::Call(..));
                if handler_like {
                    self.note_unmapped(&format!("attr:{n}"), &at);
                }
            }
        }
        if let (Some(i), Some(ha)) = (site_idx, href_attr.as_deref())
            && let Some((Some(v), alines)) = attr(ha)
        {
            let at = anchor(&env.file, alines);
            match self.literal_str(&v, env) {
                Some(h) => {
                    let effect = if h.starts_with('#') {
                        Some(
                            json!({ "kind": "scroll_to", "anchor": h, "evidence": evidence(std::slice::from_ref(&at)) }),
                        )
                    } else if h.starts_with('/') {
                        self.note_route(h.split(['#', '?']).next().unwrap_or("/"), false, &at);
                        Some(
                            json!({ "kind": "navigate", "to": h, "evidence": evidence(std::slice::from_ref(&at)) }),
                        )
                    } else if h.contains(':')
                        && h.split(':')
                            .next()
                            .is_some_and(|s| s.chars().all(|c| c.is_ascii_lowercase()))
                    {
                        Some(
                            json!({ "kind": "open_url", "url": h, "evidence": evidence(std::slice::from_ref(&at)) }),
                        )
                    } else {
                        None
                    };
                    match effect {
                        Some(e) => {
                            let mut sites = self.sites.borrow_mut();
                            let g = sites[i].gestures.entry("tap".into()).or_default();
                            g.does.push(e);
                            g.evidence.push(at);
                        }
                        None => self.note_unconverted(
                            &format!("attr:{ha}"),
                            "href is not a path, anchor, or URL",
                            &at,
                        ),
                    }
                }
                None => self.note_unconverted(&format!("attr:{ha}"), "href is not a literal", &at),
            }
        }
        if let Some(r) = &row {
            let overlay = r.param("overlay").unwrap_or("dialog").to_string();
            match conv {
                "overlay_trigger" | "overlay_close" => {
                    if let Some(i) = site_idx {
                        let kind = if conv == "overlay_close" {
                            "close"
                        } else {
                            r.param("does").unwrap_or("toggle")
                        };
                        let mut e = json!({ "kind": kind, "overlay": overlay, "evidence": evidence(std::slice::from_ref(&elem_at)) });
                        if let Some((o, Some(state))) = &jcx.overlay
                            && *o == overlay
                        {
                            e["state"] = json!(state);
                        }
                        let mut sites = self.sites.borrow_mut();
                        let g = sites[i].gestures.entry("tap".into()).or_default();
                        g.does.push(e);
                        g.evidence.push(elem_at);
                        let as_child = j.attrs.iter().any(|a| matches!(a, Attr::Named(n, None | Some(V::Bool(true)), _) if n == "asChild"));
                        if as_child {
                            child.adopt = Some(i);
                        }
                    }
                }
                "overlay_root" => {
                    let state = attr("open").and_then(|(v, l)| {
                        let v = v?;
                        let Sem::State(key, 1) = self.eval(&v, env) else { return None };
                        if let Some(entry) = self.state.borrow_mut().get_mut(&key) {
                            entry.insert(
                                "controls".into(),
                                json!({ "overlay": overlay, "evidence": ev1(&env.file, l) }),
                            );
                        }
                        Some(key)
                    });
                    child.overlay = Some((overlay, state));
                }
                "overlay_content" => child.in_overlay = true,
                _ => {}
            }
        }
        if let JName::Tag(tag) = &j.name
            && matches!(tag.as_str(), "input" | "textarea" | "select")
        {
            self.constraints(j, env, attr_lit("name").as_deref());
        }
        if role.as_deref() == Some("form") {
            child.form = site_idx;
        }
        for a in &j.attrs {
            match a {
                Attr::Named(n, Some(V::Func(f)), _) if !n.starts_with("on") => {
                    let mut fcx = child.clone();
                    fcx.field = attr_lit("name").or_else(|| child.field.clone());
                    let (benv, _) = Self::enter_func(f, env, &env.owner, &[], env);
                    for r in returns(&f.body) {
                        self.walk_node(r, &benv, &fcx);
                    }
                }
                Attr::Named(_, Some(v @ (V::Jsx(_) | V::Frag(_))), _) => {
                    self.walk_node(v, env, &child);
                }
                _ => {}
            }
        }
        for c in &j.children {
            self.walk_node(c, env, &child);
        }
    }

    /// HTML constraint validation on a host field (`required`, `minLength`,
    /// `type="email"`, ...): one rule per attribute the table names. The
    /// message is the browser's own, so it is `null`.
    fn constraints(&self, j: &Jsx, env: &Env, field: Option<&str>) {
        for a in &j.attrs {
            let Attr::Named(n, v, alines) = a else { continue };
            let lit = v.as_ref().map_or(Some(Value::Bool(true)), |v| self.literal(v, env));
            let text =
                lit.as_ref().map(|l| l.as_str().map_or_else(|| l.to_string(), str::to_string));
            let Some(row) = self.row(&format!("attr:{n}"), text.as_deref()) else { continue };
            if row.converter != "constraint" || lit == Some(Value::Bool(false)) {
                continue;
            }
            let at = anchor(&env.file, *alines);
            let Some(field) = field else {
                self.note_unconverted(&format!("attr:{n}"), "the field has no literal name", &at);
                continue;
            };
            let mut rule = Map::new();
            rule.insert("field".into(), json!(field));
            rule.insert("rule".into(), json!(row.param("rule")));
            if row.raw.get("value").and_then(Value::as_bool) == Some(true) {
                match lit {
                    Some(Value::String(t)) if t.parse::<f64>().is_ok() => {
                        rule.insert("value".into(), num(t.parse::<f64>().unwrap_or_default()));
                    }
                    Some(l @ (Value::String(_) | Value::Number(_))) => {
                        rule.insert("value".into(), l);
                    }
                    _ => {}
                }
            }
            rule.insert("message".into(), Value::Null);
            rule.insert("nodes".into(), json!([]));
            rule.insert("evidence".into(), evidence(std::slice::from_ref(&at)));
            self.rules
                .borrow_mut()
                .insert(format!("{field}\u{0}{at}\u{0}{n}"), Value::Object(rule));
        }
    }

    // ---- output ------------------------------------------------------------

    fn triggers(&self) -> Vec<Value> {
        let sites = self.sites.borrow();
        let mut used = BTreeSet::new();
        let mut out = Vec::new();
        for site in sites.iter() {
            for (gesture, g) in &site.gestures {
                let mut seed = format!("{}:{gesture}", site.seed);
                let mut n = 1;
                while !used.insert(seed.clone()) {
                    n += 1;
                    seed = format!("{}:{gesture}#{n}", site.seed);
                }
                let id = format!("t-{}", &sha256_hex(seed.as_bytes())[..12]);
                if g.does.is_empty() {
                    self.note_unconverted(
                        &format!("trigger:{gesture}"),
                        "the handler has no effect the table can read",
                        &site.anchor,
                    );
                }
                let mut on = json!({ "gesture": gesture, "match": site.tuple(), "nodes": [], "match_status": "unresolved" });
                if gesture == "submit" {
                    on["submitters"] = Value::Array(site.submitters.clone());
                }
                let mut anchors = vec![site.anchor.clone()];
                anchors.extend(g.evidence.iter().cloned());
                let mut t =
                    json!({ "id": id, "on": on, "does": g.does, "evidence": evidence(&anchors) });
                if !g.guard.is_empty() {
                    t["guard"] = Value::Array(g.guard.clone());
                }
                if !site.x_web.is_empty() {
                    t["x-web"] = Value::Object(site.x_web.clone());
                }
                out.push(t);
            }
        }
        out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        out
    }
}

/// A file route id as its URL path: route groups (`(auth)`) and pathless
/// layouts (`_auth`) are not part of the URL, as `TanStack` Router's file routing has it.
fn route_url(id: &str) -> String {
    let kept: Vec<&str> = id
        .split('/')
        .filter(|s| {
            !(s.is_empty() || s.starts_with('_') || (s.starts_with('(') && s.ends_with(')')))
        })
        .collect();
    format!("/{}", kept.join("/"))
}

fn first_return(body: &[S]) -> Option<&V> {
    body.iter().find_map(|s| match s {
        S::Return(Some(v), _) => Some(v),
        _ => None,
    })
}

fn returns(body: &[S]) -> Vec<&V> {
    let mut out = Vec::new();
    for s in body {
        match s {
            S::Return(Some(v), _) => out.push(v),
            S::If(_, _, then, els, _) => {
                out.extend(returns(then));
                out.extend(returns(els));
            }
            _ => {}
        }
    }
    out
}

fn is_component(name: &str, value: &V) -> bool {
    name.chars().next().is_some_and(char::is_uppercase) && matches!(value, V::Func(_) | V::Call(..))
}

/// Read the source and build the two documents.
#[must_use]
pub fn extract(options: &Options) -> Output {
    let cx = Ctx::new(options);
    let modules = cx.reachable(&options.entry);
    if let (Some(module), Some(env)) = (cx.load(&options.entry), cx.module_env(&options.entry)) {
        for (name, (_, value, _)) in module.decls.iter().rev() {
            if name.contains('.') {
                continue;
            }
            let Some(thunk) = Ctx::thunk_of(&env, name) else { continue };
            match cx.force(&thunk) {
                Sem::RouteDef(path, props, renv, lines) => cx.route(&path, &props, &renv, lines),
                Sem::Func(f, fenv, q) if is_component(name, value) => cx.enqueue(f, fenv, q),
                _ => {}
            }
        }
    }
    loop {
        let next = cx.pending.borrow_mut().pop();
        let Some((f, fenv, name)) = next else { break };
        cx.walk_component(&f, &fenv, &name);
    }
    let routes: Vec<Value> = cx
        .routes
        .borrow()
        .iter()
        .map(|(path, (declared, anchors))| {
            let anchors: Vec<String> = anchors.iter().cloned().collect();
            json!({ "path": path, "declared": declared, "evidence": evidence(&anchors) })
        })
        .collect();
    let mut guards = cx.guards.borrow().clone();
    guards.sort_by_key(ToString::to_string);
    let state: Map<String, Value> =
        cx.state.borrow().iter().map(|(k, v)| (k.clone(), Value::Object(v.clone()))).collect();
    let validation: Vec<Value> = cx.rules.borrow().values().cloned().collect();
    let behaviour = json!({
        "version": 1,
        "screen": options.screen,
        "routes": routes,
        "triggers": cx.triggers(),
        "guards": guards,
        "state": state,
        "validation": validation,
        "transitions": [],
    });
    let mut by_key: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, at) in cx.unmapped.borrow().iter() {
        by_key.entry(key.clone()).or_default().push(at.clone());
    }
    let unmapped: Vec<Value> = by_key
        .into_iter()
        .map(|(key, at)| json!({ "key": key, "count": at.len(), "at": at }))
        .collect();
    let unconverted: Vec<Value> = cx
        .unconverted
        .borrow()
        .iter()
        .map(|(key, reason, at)| json!({ "key": key, "reason": reason, "at": at }))
        .collect();
    let report = json!({
        "version": 1,
        "screen": options.screen,
        "modules": modules,
        "counts": { "unmapped": unmapped.len(), "unconverted": unconverted.len() },
        "unmapped": unmapped,
        "unconverted": unconverted,
    });
    Output {
        behaviour,
        unmapped: report,
    }
}
