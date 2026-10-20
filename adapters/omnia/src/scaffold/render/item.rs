//! The Rust items a scaffold writes and the one printer over them, laid as the layout's formatter lays them.

use std::fmt::{self, Display, Write as _};

// The template's `rustfmt.toml` width: a line the formatter would break
// is written broken, so a scaffolded file passes its check unchanged.
pub const MAX_WIDTH: usize = 100;
// The formatter's block indent.
pub const TAB: usize = 4;

// A source file's items in order, printed one blank line apart.
#[derive(Default)]
pub struct Source {
    items: Vec<Item>,
}

pub enum Item {
    // The `//!` lines a file opens with.
    ModuleDoc(Doc),
    // One `pub mod <name>;` line per name.
    Mods(Vec<String>),
    Uses(Uses),
    Consts(Consts),
    Fn(Fn),
    Struct(Struct),
    Enum(Enum),
}

// The lines of a doc comment, an empty one its marker alone.
#[derive(Default)]
pub struct Doc(Vec<String>);

// The `use` lines of a file, one per module with the names taken from it.
#[derive(Default)]
pub struct Uses(Vec<(String, Vec<String>)>);

// Constants under one `//` comment.
#[derive(Default)]
pub struct Consts {
    comments: Vec<String>,
    consts: Vec<Const>,
}

pub struct Const {
    docs: Doc,
    vis: Vis,
    name: String,
    ty: String,
    value: String,
}

// A function: the `//` lines above it, its docs and attributes, its
// signature, and its body as lines already indented.
#[derive(Default)]
pub struct Fn {
    comments: Vec<String>,
    docs: Doc,
    attrs: Vec<String>,
    vis: Vis,
    is_const: bool,
    is_async: bool,
    name: String,
    generics: String,
    params: Vec<String>,
    ret: String,
    body: Vec<String>,
}

// A struct: its generics, `<T>`, printed after the name where it has any.
#[derive(Default)]
pub struct Struct {
    docs: Doc,
    attrs: Vec<String>,
    vis: Vis,
    name: String,
    generics: String,
    fields: Vec<Field>,
}

pub struct Field {
    docs: Doc,
    attrs: Vec<String>,
    vis: Vis,
    name: String,
    ty: String,
}

#[derive(Default)]
pub struct Enum {
    docs: Doc,
    attrs: Vec<String>,
    vis: Vis,
    name: String,
    variants: Vec<Variant>,
}

// A unit variant under its docs and attributes.
pub struct Variant {
    docs: Doc,
    attrs: Vec<String>,
    name: String,
}

#[derive(Clone, Copy, Default)]
pub enum Vis {
    #[default]
    Private,
    Crate,
    Pub,
}

// An expression as the printer lays it: text it never breaks, a call it
// breaks by its arguments, or a macro call, broken the same way with no
// comma after its last argument.
pub enum Expr {
    Text(String),
    Call(Call),
    Macro(Call),
}

// A call: `callee(a, b)` where it fits, else one argument per line, or a
// sole call argument broken in place behind the callee's parenthesis.
pub struct Call {
    callee: String,
    args: Vec<Expr>,
}

// Method calls chained on a receiver, `.await` after the last where the
// chain is awaited. A receiver shorter than the indent, which the formatter
// extends the first call onto, is none the scaffold chains on.
pub struct Chain {
    receiver: Expr,
    calls: Vec<Call>,
    awaited: bool,
}

// A Markdown fence longer than any run of backticks in the text it holds.
pub struct Fence(String);

impl Source {
    pub fn push(&mut self, item: impl Into<Item>) {
        self.items.push(item.into());
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            item.render(&mut out);
        }
        out
    }
}

impl Item {
    fn render(&self, out: &mut String) {
        match self {
            Self::ModuleDoc(doc) => doc.render("//!", out),
            Self::Mods(names) => {
                for name in names {
                    let _ = writeln!(out, "pub mod {name};");
                }
            }
            Self::Uses(uses) => uses.render(out),
            Self::Consts(consts) => consts.render(out),
            Self::Fn(item) => item.render(out),
            Self::Struct(item) => item.render(out),
            Self::Enum(item) => item.render(out),
        }
    }
}

impl From<Uses> for Item {
    fn from(uses: Uses) -> Self {
        Self::Uses(uses)
    }
}

impl From<Consts> for Item {
    fn from(consts: Consts) -> Self {
        Self::Consts(consts)
    }
}

impl From<Fn> for Item {
    fn from(item: Fn) -> Self {
        Self::Fn(item)
    }
}

impl From<Struct> for Item {
    fn from(item: Struct) -> Self {
        Self::Struct(item)
    }
}

impl From<Enum> for Item {
    fn from(item: Enum) -> Self {
        Self::Enum(item)
    }
}

impl Doc {
    pub fn new<L: Into<String>>(lines: impl IntoIterator<Item = L>) -> Self {
        Self(lines.into_iter().map(Into::into).collect())
    }

    pub fn push(&mut self, line: impl Into<String>) {
        self.0.push(line.into());
    }

    fn render(&self, marker: &str, out: &mut String) {
        marked(marker, &self.0, out);
    }
}

impl<L: Into<String>, const N: usize> From<[L; N]> for Doc {
    fn from(lines: [L; N]) -> Self {
        Self::new(lines)
    }
}

impl From<Vec<String>> for Doc {
    fn from(lines: Vec<String>) -> Self {
        Self(lines)
    }
}

impl Uses {
    // The names taken from `path`, each once in sorted order.
    pub fn add(&mut self, path: &str, names: &[&str]) {
        let mut names = names.to_vec();
        names.sort_unstable();
        names.dedup();
        if !names.is_empty() {
            self.0.push((path.to_owned(), names.into_iter().map(str::to_owned).collect()));
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    // `use <path>::<name>;` for one name, `use <path>::{<names>};` for
    // several, the names compressed beneath the brace where the line is
    // too long, and the standard library's group a blank line apart from
    // the crates', as the template's `group_imports` lays them.
    fn render(&self, out: &mut String) {
        for (at, (path, names)) in self.0.iter().enumerate() {
            if at > 0 && is_std(&self.0[at - 1].0) && !is_std(path) {
                out.push('\n');
            }
            if let [name] = names.as_slice() {
                let _ = writeln!(out, "use {path}::{name};");
                continue;
            }
            let joined = format!("use {path}::{{{}}};", names.join(", "));
            if width(&joined) <= MAX_WIDTH {
                let _ = writeln!(out, "{joined}");
            } else {
                let _ = writeln!(out, "use {path}::{{");
                compressed(names, TAB, out);
                out.push_str("};\n");
            }
        }
    }
}

fn is_std(path: &str) -> bool {
    ["std", "core", "alloc"]
        .iter()
        .any(|root| path == *root || path.starts_with(&format!("{root}::")))
}

impl Consts {
    pub fn new(consts: impl IntoIterator<Item = Const>) -> Self {
        Self {
            comments: Vec::new(),
            consts: consts.into_iter().collect(),
        }
    }

    pub fn comment(mut self, line: impl Into<String>) -> Self {
        self.comments.push(line.into());
        self
    }

    fn render(&self, out: &mut String) {
        marked("//", &self.comments, out);
        for item in &self.consts {
            item.render(out);
        }
    }
}

impl Const {
    pub fn new(name: impl Into<String>, ty: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            docs: Doc::default(),
            vis: Vis::Private,
            name: name.into(),
            ty: ty.into(),
            value: value.into(),
        }
    }

    pub fn docs(mut self, docs: impl Into<Doc>) -> Self {
        self.docs = docs.into();
        self
    }

    pub const fn vis(mut self, vis: Vis) -> Self {
        self.vis = vis;
        self
    }

    fn render(&self, out: &mut String) {
        self.docs.render("///", out);
        let decl = format!("{}const {}: {} =", self.vis.prefix(), self.name, self.ty);
        for line in assigned(&decl, &self.value, TAB) {
            let _ = writeln!(out, "{line}");
        }
    }
}

impl Fn {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    // The `//` lines above the fn, after any it holds.
    pub fn comments<L: Into<String>>(mut self, lines: impl IntoIterator<Item = L>) -> Self {
        self.comments.extend(lines.into_iter().map(Into::into));
        self
    }

    pub fn docs(mut self, docs: impl Into<Doc>) -> Self {
        self.docs = docs.into();
        self
    }

    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }

    pub const fn vis(mut self, vis: Vis) -> Self {
        self.vis = vis;
        self
    }

    pub const fn constant(mut self) -> Self {
        self.is_const = true;
        self
    }

    pub const fn asynchronous(mut self) -> Self {
        self.is_async = true;
        self
    }

    pub fn generics(mut self, generics: impl Into<String>) -> Self {
        self.generics = generics.into();
        self
    }

    pub fn params<P: Into<String>>(mut self, params: impl IntoIterator<Item = P>) -> Self {
        self.params = params.into_iter().map(Into::into).collect();
        self
    }

    pub fn returns(mut self, ret: impl Into<String>) -> Self {
        self.ret = ret.into();
        self
    }

    // The body as lines already indented.
    pub fn body<L: Into<String>>(mut self, lines: impl IntoIterator<Item = L>) -> Self {
        self.body = lines.into_iter().map(Into::into).collect();
        self
    }

    fn render(&self, out: &mut String) {
        marked("//", &self.comments, out);
        self.docs.render("///", out);
        attributes(&self.attrs, out);
        self.head(out);
        for line in &self.body {
            let _ = writeln!(out, "{line}");
        }
        out.push_str("}\n");
    }

    // The signature on one line where it fits. Else the generics vertical
    // where the opener has no room for `) {` after it, the parameters
    // compressed beneath the opener and the return on the closer's line
    // where there are parameters, and otherwise the brace alone on the next
    // line where the signature fits before it, three columns past the width
    // at most, and the return on the next line past that.
    fn head(&self, out: &mut String) {
        let lead = format!(
            "{}{}{}fn {}",
            self.vis.prefix(),
            if self.is_const { "const " } else { "" },
            if self.is_async { "async " } else { "" },
            self.name
        );
        let opener = format!("{lead}{}(", self.generics);
        let arrow = if self.ret.is_empty() { String::new() } else { format!(" -> {}", self.ret) };
        let joined = format!("{opener}{}){arrow} {{", self.params.join(", "));
        let bounds = self.generics.strip_prefix('<').and_then(|rest| rest.strip_suffix('>'));
        if width(&joined) <= MAX_WIDTH {
            let _ = writeln!(out, "{joined}");
        } else if let Some(bounds) = bounds.filter(|_| width(&opener) + 3 > MAX_WIDTH) {
            let _ = writeln!(out, "{lead}<\n{}{bounds},", pad(TAB));
            if self.params.is_empty() {
                let _ = writeln!(out, ">(){arrow} {{");
            } else {
                let _ = writeln!(out, ">(");
                compressed(&self.params, TAB, out);
                let _ = writeln!(out, "){arrow} {{");
            }
        } else if !self.params.is_empty() {
            let _ = writeln!(out, "{opener}");
            compressed(&self.params, TAB, out);
            let _ = writeln!(out, "){arrow} {{");
        } else if width(&joined) <= MAX_WIDTH + 3 || arrow.is_empty() {
            let _ = writeln!(out, "{opener}){arrow}\n{{");
        } else {
            let _ = writeln!(out, "{opener})\n-> {} {{", self.ret);
        }
    }
}

impl Struct {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn docs(mut self, docs: impl Into<Doc>) -> Self {
        self.docs = docs.into();
        self
    }

    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }

    pub fn attrs<A: Into<String>>(mut self, attrs: impl IntoIterator<Item = A>) -> Self {
        self.attrs.extend(attrs.into_iter().map(Into::into));
        self
    }

    pub const fn vis(mut self, vis: Vis) -> Self {
        self.vis = vis;
        self
    }

    pub fn generics(mut self, generics: impl Into<String>) -> Self {
        self.generics = generics.into();
        self
    }

    pub fn fields(mut self, fields: impl IntoIterator<Item = Field>) -> Self {
        self.fields = fields.into_iter().collect();
        self
    }

    fn render(&self, out: &mut String) {
        self.docs.render("///", out);
        attributes(&self.attrs, out);
        let decl = format!("{}struct {}{}", self.vis.prefix(), self.name, self.generics);
        if self.fields.is_empty() {
            let _ = writeln!(out, "{decl} {{}}");
            return;
        }
        let _ = writeln!(out, "{decl} {{");
        for field in &self.fields {
            field.render(out);
        }
        out.push_str("}\n");
    }
}

impl Field {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        Self {
            docs: Doc::default(),
            attrs: Vec::new(),
            vis: Vis::Private,
            name: name.into(),
            ty: ty.into(),
        }
    }

    pub fn docs(mut self, docs: impl Into<Doc>) -> Self {
        self.docs = docs.into();
        self
    }

    pub fn attrs<A: Into<String>>(mut self, attrs: impl IntoIterator<Item = A>) -> Self {
        self.attrs.extend(attrs.into_iter().map(Into::into));
        self
    }

    pub const fn vis(mut self, vis: Vis) -> Self {
        self.vis = vis;
        self
    }

    fn render(&self, out: &mut String) {
        let indent = pad(TAB);
        self.docs.render(&format!("{indent}///"), out);
        for attr in &self.attrs {
            let _ = writeln!(out, "{indent}{attr}");
        }
        let _ = writeln!(out, "{indent}{}{}: {},", self.vis.prefix(), self.name, self.ty);
    }
}

impl Enum {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn docs(mut self, docs: impl Into<Doc>) -> Self {
        self.docs = docs.into();
        self
    }

    pub fn attr(mut self, attr: impl Into<String>) -> Self {
        self.attrs.push(attr.into());
        self
    }

    pub const fn vis(mut self, vis: Vis) -> Self {
        self.vis = vis;
        self
    }

    pub fn variants(mut self, variants: impl IntoIterator<Item = Variant>) -> Self {
        self.variants = variants.into_iter().collect();
        self
    }

    fn render(&self, out: &mut String) {
        self.docs.render("///", out);
        attributes(&self.attrs, out);
        let decl = format!("{}enum {}", self.vis.prefix(), self.name);
        if self.variants.is_empty() {
            let _ = writeln!(out, "{decl} {{}}");
            return;
        }
        let _ = writeln!(out, "{decl} {{");
        for variant in &self.variants {
            variant.render(out);
        }
        out.push_str("}\n");
    }
}

impl Variant {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            docs: Doc::default(),
            attrs: Vec::new(),
            name: name.into(),
        }
    }

    pub fn docs(mut self, docs: impl Into<Doc>) -> Self {
        self.docs = docs.into();
        self
    }

    pub fn attrs<A: Into<String>>(mut self, attrs: impl IntoIterator<Item = A>) -> Self {
        self.attrs.extend(attrs.into_iter().map(Into::into));
        self
    }

    fn render(&self, out: &mut String) {
        let indent = pad(TAB);
        self.docs.render(&format!("{indent}///"), out);
        for attr in &self.attrs {
            let _ = writeln!(out, "{indent}{attr}");
        }
        let _ = writeln!(out, "{indent}{},", self.name);
    }
}

impl Vis {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Private => "",
            Self::Crate => "pub(crate) ",
            Self::Pub => "pub ",
        }
    }
}

impl From<String> for Expr {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<&str> for Expr {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<Call> for Expr {
    fn from(call: Call) -> Self {
        Self::Call(call)
    }
}

impl Expr {
    // The expression as the tail of a block at `indent`.
    pub fn tail(&self, indent: usize) -> Vec<String> {
        indented(self.within(MAX_WIDTH - indent, indent), indent, "")
    }

    fn inline(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Call(call) | Self::Macro(call) => call.inline(),
        }
    }

    // Inline where that fits within `width`, else, for a call, broken
    // beneath `at`.
    fn within(&self, width: usize, at: usize) -> Vec<String> {
        match self {
            Self::Text(text) => vec![text.clone()],
            Self::Call(call) => call.within(width, at, true),
            Self::Macro(call) => call.within(width, at, false),
        }
    }
}

impl Call {
    pub fn new<A: Into<Expr>>(
        callee: impl Into<String>, args: impl IntoIterator<Item = A>,
    ) -> Self {
        Self {
            callee: callee.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    fn inline(&self) -> String {
        let args: Vec<String> = self.args.iter().map(Expr::inline).collect();
        format!("{}({})", self.callee, args.join(", "))
    }

    // The call opened on its first line, one argument per line beneath
    // `at`, each within what its line has left before its comma, and closed
    // at `at`; the last argument's comma only where `trailing`.
    fn vertical(&self, at: usize, trailing: bool) -> Vec<String> {
        let inner = at + TAB;
        let mut lines = vec![format!("{}(", self.callee)];
        for (i, arg) in self.args.iter().enumerate() {
            let close = if trailing || i + 1 < self.args.len() { "," } else { "" };
            lines.extend(indented(
                arg.within(MAX_WIDTH - inner - close.len(), inner),
                inner,
                close,
            ));
        }
        lines.push(format!("{})", pad(at)));
        lines
    }

    // Inline where that fits within `width`, else broken beneath `at`.
    fn within(&self, width: usize, at: usize, trailing: bool) -> Vec<String> {
        let inline = self.inline();
        if self::width(&inline) <= width || self.args.is_empty() {
            vec![inline]
        } else {
            self.broken(at, trailing)
        }
    }

    // A sole call argument broken behind the callee's parenthesis, its own
    // closed with it; the arguments vertical beneath `at` otherwise.
    fn broken(&self, at: usize, trailing: bool) -> Vec<String> {
        let overflowed = match self.args.as_slice() {
            [Expr::Call(sole)] => Some(sole.vertical(at, true)),
            [Expr::Macro(sole)] => Some(sole.vertical(at, false)),
            _ => None,
        };
        let Some(mut lines) = overflowed else { return self.vertical(at, trailing) };
        if let Some(first) = lines.first_mut() {
            first.insert_str(0, &format!("{}(", self.callee));
        }
        if let Some(last) = lines.last_mut() {
            last.push(')');
        }
        lines
    }
}

impl Chain {
    pub fn new(receiver: impl Into<Expr>, calls: Vec<Call>) -> Self {
        Self {
            receiver: receiver.into(),
            calls,
            awaited: false,
        }
    }

    pub const fn awaited(mut self) -> Self {
        self.awaited = true;
        self
    }

    // The chain as the tail expression of a block at `indent`.
    pub fn tail(&self, indent: usize) -> Vec<String> {
        indented(self.lay(indent, 0, 0), indent, "")
    }

    // `let <pattern> = <chain>;` at `indent`: on one line where it fits,
    // else after the binding or beneath it, beneath where the chain is
    // whole there, where it takes two lines fewer, or where only after the
    // binding does its first line open a call.
    pub fn bound(&self, indent: usize, pattern: &str) -> Vec<String> {
        let lead = format!("{}let {pattern} = ", pad(indent));
        let after = self.lay(indent, width(&lead) - indent, 1);
        let beneath = self.lay(indent + TAB, 0, 1);
        let opens = |lines: &[String]| lines.first().is_some_and(|first| first.ends_with('('));
        if after.len() > 1
            && (beneath.len() == 1
                || after.len() > beneath.len() + 1
                || opens(&after) && !opens(&beneath))
        {
            let mut lines = vec![format!("{}let {pattern} =", pad(indent))];
            lines.extend(indented(beneath, indent + TAB, ";"));
            return lines;
        }
        let mut lines = indented(after, 0, ";");
        if let Some(first) = lines.first_mut() {
            first.insert_str(0, &lead);
        }
        lines
    }

    // The chain's lines, the first unindented, within what a line at
    // `indent` has left after `offset` columns and before `overhead` more:
    // one line where it fits, else every call on its own line, a level in
    // from a receiver that fits its line and level with one broken over
    // lines, except a last call that fits on none, whose arguments the
    // formatter breaks from the first line when every call before it fits
    // there.
    fn lay(&self, indent: usize, offset: usize, overhead: usize) -> Vec<String> {
        let width = MAX_WIDTH.saturating_sub(indent + offset + overhead);
        let suffix = if self.awaited { ".await" } else { "" };
        let inline: Vec<String> = self.calls.iter().map(Call::inline).collect();
        let joined = format!("{}{}{suffix}", self.receiver.inline(), inline.concat());
        if self::width(&joined) <= width {
            return vec![joined];
        }

        // the receiver, and the calls laid where they go after it
        let mut lines = self.receiver.within(width, indent);
        let at = if lines.len() > 1 { indent } else { indent + TAB };
        let laid: Vec<Vec<String>> = self
            .calls
            .iter()
            .enumerate()
            .map(|(i, call)| {
                let last = i + 1 == self.calls.len() && !self.awaited;
                call.within(MAX_WIDTH - at - if last { overhead } else { 0 }, at, true)
            })
            .collect();

        // the last call's arguments broken from the first line
        if let Some((own, before)) = laid.split_last()
            && let Some(last) = self.calls.last()
            && lines.len() == 1
            && !self.awaited
            && own.len() > 1
            && before.iter().all(|call| call.len() == 1)
        {
            let led = format!("{}{}", lines.concat(), inline[..before.len()].concat());
            if self::width(&led) + self::width(&last.callee) < width {
                let mut lines = last.broken(indent, true);
                if let Some(first) = lines.first_mut() {
                    first.insert_str(0, &led);
                }
                return lines;
            }
        }

        // every call on its own line
        for call in laid {
            lines.extend(indented(call, at, ""));
        }
        if self.awaited {
            lines.push(format!("{}.await", pad(at)));
        }
        lines
    }
}

impl Fence {
    pub fn around(text: &str) -> Self {
        let longest = text.lines().map(backticks).max().unwrap_or(0);
        Self("`".repeat(longest.max(2) + 1))
    }
}

impl Display for Fence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// A closure at `indent`, `close` after it: `<head> <body>` on one line
// where it fits, else the body in a block.
pub fn closure(indent: usize, head: &str, body: &str, close: &str) -> Vec<String> {
    let joined = format!("{}{head} {body}{close}", pad(indent));
    if width(&joined) <= MAX_WIDTH {
        return vec![joined];
    }
    vec![
        format!("{}{head} {{", pad(indent)),
        format!("{}{body}", pad(indent + TAB)),
        format!("{}}}{close}", pad(indent)),
    ]
}

// `<lead> <value>;` on one line where it fits, else the value alone on the
// line beneath at `indent` where it fits there, else the one line as it is.
fn assigned(lead: &str, value: &str, indent: usize) -> Vec<String> {
    let joined = format!("{lead} {value};");
    if width(&joined) <= MAX_WIDTH || indent + width(value) + 1 > MAX_WIDTH {
        vec![joined]
    } else {
        vec![lead.to_owned(), format!("{}{value};", pad(indent))]
    }
}

// Items laid as the formatter compresses a list: as many per line as fit
// beneath `indent`, each line closed by its comma. The formatter counts an
// item's comma where another follows it on the line and, once a line has
// wrapped, the last one's too, so a first line may end at the width and
// every other one column short of it.
fn compressed(items: &[String], indent: usize, out: &mut String) {
    let room = MAX_WIDTH - indent - 1;
    let mut line = String::new();
    let mut wrapped = false;
    for (i, item) in items.iter().enumerate() {
        let last = i + 1 == items.len();
        let counted = width(item) + usize::from(!last || wrapped);
        if !line.is_empty() && width(&line) + 1 + counted > room {
            let _ = writeln!(out, "{}{line}", pad(indent));
            line.clear();
            wrapped = true;
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(item);
        line.push(',');
    }
    let _ = writeln!(out, "{}{line}", pad(indent));
}

// The lines of one expression, the first indented at `at` and the last
// closed by `close`.
fn indented(mut lines: Vec<String>, at: usize, close: &str) -> Vec<String> {
    if let Some(first) = lines.first_mut() {
        first.insert_str(0, &pad(at));
    }
    if let Some(last) = lines.last_mut() {
        last.push_str(close);
    }
    lines
}

// Each line under `marker`, an empty line the marker alone.
fn marked(marker: &str, lines: &[String], out: &mut String) {
    for line in lines {
        if line.is_empty() {
            let _ = writeln!(out, "{marker}");
        } else {
            let _ = writeln!(out, "{marker} {line}");
        }
    }
}

fn attributes(attrs: &[String], out: &mut String) {
    for attr in attrs {
        let _ = writeln!(out, "{attr}");
    }
}

fn pad(width: usize) -> String {
    " ".repeat(width)
}

// The columns a line takes, as the formatter counts them: one per
// character, so a dash or an accented letter is no wider than its bytes
// would make it.
fn width(text: &str) -> usize {
    text.chars().count()
}

fn backticks(line: &str) -> usize {
    line.split(|ch| ch != '`').map(str::len).max().unwrap_or(0)
}
