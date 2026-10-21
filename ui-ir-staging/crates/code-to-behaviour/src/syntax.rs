//! One module read with oxc into an owned, lifetime-free tree.
//!
//! Only the shapes behaviour extraction looks at are kept: calls, members,
//! literals, functions, objects, JSX, and the statements around them, each
//! with its lines.

use std::collections::BTreeMap;
use std::rc::Rc;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionBody, BindingPattern, ChainElement, Class,
    ClassElement, Declaration, ExportDefaultDeclarationKind, Expression, FormalParameters,
    Function, ImportDeclarationSpecifier, JSXAttributeItem, JSXAttributeName, JSXAttributeValue,
    JSXChild, JSXElement, JSXElementName, JSXExpression, JSXMemberExpression,
    JSXMemberExpressionObject, ModuleExportName, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

/// First and last line, 1-based.
pub type Lines = (u32, u32);

/// An expression, reduced.
#[derive(Debug, Clone)]
pub enum V {
    /// A string literal (or a template literal with no expressions).
    Str(String),
    /// A number literal.
    Num(f64),
    /// A boolean literal.
    Bool(bool),
    /// `null`.
    Null,
    /// A name.
    Ident(String),
    /// `this`.
    This,
    /// `object.property`.
    Member(Box<Self>, String),
    /// A call.
    Call(Box<Self>, Vec<Self>, Lines),
    /// A function or arrow.
    Func(Rc<Func>),
    /// An object literal; methods are `Func` values.
    Obj(Vec<(String, Self)>),
    /// An array literal.
    Arr(Vec<Self>),
    /// A JSX element.
    Jsx(Rc<Jsx>),
    /// A JSX fragment's children.
    Frag(Vec<Self>),
    /// `test ? yes : no`.
    Cond(Box<Self>, Box<Self>, Box<Self>),
    /// `a && b`, `a || b`, `a ?? b`.
    Logical(Box<Self>, Box<Self>),
    /// `await x`.
    Await(Box<Self>),
    /// A binary or unary operation: its operands, for walking only.
    Ops(Vec<Self>),
    /// Anything else, as written.
    Other(String, Lines),
}

/// A binding pattern.
#[derive(Debug, Clone)]
pub enum Pat {
    /// `x`.
    Name(String),
    /// `{ a, b: c }` as `(key, pattern)`.
    Obj(Vec<(String, Self)>),
    /// `[a, , b]`.
    Arr(Vec<Option<Self>>),
    /// Anything else.
    Other,
}

/// A function body.
#[derive(Debug)]
pub struct Func {
    /// The parameters.
    pub params: Vec<Pat>,
    /// The statements (an expression body is one `Return`).
    pub body: Vec<S>,
    /// Its lines.
    pub lines: Lines,
}

/// A statement, reduced.
#[derive(Debug, Clone)]
pub enum S {
    /// An expression statement.
    Expr(V, Lines),
    /// `if`: the test, its text as written, then and else.
    If(V, String, Vec<Self>, Vec<Self>, Lines),
    /// `return`.
    Return(Option<V>, Lines),
    /// `throw`.
    Throw(V, Lines),
    /// `const pat = init`.
    Decl(Pat, Option<V>, Lines),
}

/// A JSX element.
#[derive(Debug)]
pub struct Jsx {
    /// The element name.
    pub name: JName,
    /// The attributes in order.
    pub attrs: Vec<Attr>,
    /// The children.
    pub children: Vec<V>,
    /// Its lines.
    pub lines: Lines,
}

/// A JSX element name.
#[derive(Debug, Clone)]
pub enum JName {
    /// A host tag (`a`, `form`).
    Tag(String),
    /// A component reference (`Button`, `SheetPrimitive.Trigger`).
    Ref(V),
}

/// A JSX attribute.
#[derive(Debug, Clone)]
pub enum Attr {
    /// `name={value}`; `None` is a bare `name`.
    Named(String, Option<V>, Lines),
    /// `{...value}`.
    Spread(V),
}

/// One import binding.
#[derive(Debug, Clone)]
pub struct Import {
    /// The module specifier.
    pub source: String,
    /// The exported name; `default`, or `*` for a namespace.
    pub name: String,
}

/// One parsed module.
#[derive(Debug, Default)]
pub struct Module {
    /// Path relative to the source root.
    pub path: String,
    /// Local name to import.
    pub imports: BTreeMap<String, Import>,
    /// Top-level declarations by name (`Class.method` for static methods).
    pub decls: BTreeMap<String, (Pat, V, Lines)>,
    /// Exported name to local name.
    pub exports: BTreeMap<String, String>,
    /// `export * from` specifiers.
    pub reexport_all: Vec<String>,
    /// Exported name to `(source, imported name)` for `export { a } from`.
    pub reexports: BTreeMap<String, (String, String)>,
    /// Parse errors, as text.
    pub errors: Vec<String>,
}

struct Reader<'s> {
    text: &'s str,
    starts: Vec<u32>,
}

impl Reader<'_> {
    fn line(&self, offset: u32) -> u32 {
        let index = self.starts.partition_point(|&start| start <= offset);
        u32::try_from(index).unwrap_or(u32::MAX).max(1)
    }

    fn lines(&self, span: Span) -> Lines {
        (self.line(span.start), self.line(span.end.saturating_sub(1).max(span.start)))
    }

    fn text(&self, span: Span) -> String {
        self.text
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn other(&self, span: Span) -> V {
        V::Other(self.text(span), self.lines(span))
    }

    #[expect(clippy::too_many_lines, reason = "one arm per expression shape, kept in one place")]
    fn expr(&self, e: &Expression<'_>) -> V {
        match e {
            Expression::StringLiteral(s) => V::Str(s.value.to_string()),
            Expression::NumericLiteral(n) => V::Num(n.value),
            Expression::BooleanLiteral(b) => V::Bool(b.value),
            Expression::NullLiteral(_) => V::Null,
            Expression::TemplateLiteral(t) if t.expressions.is_empty() => V::Str(
                t.quasis
                    .iter()
                    .map(|q| {
                        q.value
                            .cooked
                            .as_ref()
                            .map_or_else(|| q.value.raw.to_string(), ToString::to_string)
                    })
                    .collect(),
            ),
            Expression::Identifier(id) => V::Ident(id.name.to_string()),
            Expression::ThisExpression(_) => V::This,
            Expression::StaticMemberExpression(m) => {
                V::Member(Box::new(self.expr(&m.object)), m.property.name.to_string())
            }
            Expression::ComputedMemberExpression(m) => match &m.expression {
                Expression::StringLiteral(s) => {
                    V::Member(Box::new(self.expr(&m.object)), s.value.to_string())
                }
                _ => self.other(m.span),
            },
            Expression::CallExpression(c) => V::Call(
                Box::new(self.expr(&c.callee)),
                c.arguments.iter().map(|a| self.arg(a)).collect(),
                self.lines(c.span),
            ),
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => V::Call(
                    Box::new(self.expr(&call.callee)),
                    call.arguments.iter().map(|a| self.arg(a)).collect(),
                    self.lines(call.span),
                ),
                other => other.as_member_expression().map_or_else(
                    || self.other(c.span),
                    |m| {
                        m.static_property_name().map_or_else(
                            || self.other(c.span),
                            |name| V::Member(Box::new(self.expr(m.object())), name.to_string()),
                        )
                    },
                ),
            },
            Expression::ArrowFunctionExpression(a) => {
                let body = match &a.body {
                    ArrowFunctionBody::FunctionBody(b) => self.stmts(&b.statements),
                    other => other.as_expression().map_or_else(Vec::new, |e| {
                        vec![S::Return(Some(self.expr(e)), self.lines(e.span()))]
                    }),
                };
                V::Func(Rc::new(Func {
                    params: Self::params(&a.params),
                    body,
                    lines: self.lines(a.span),
                }))
            }
            Expression::FunctionExpression(f) => V::Func(Rc::new(self.function(f))),
            Expression::ObjectExpression(o) => V::Obj(
                o.properties
                    .iter()
                    .filter_map(|p| match p {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            Some((key_name(&p.key)?, self.expr(&p.value)))
                        }
                        ObjectPropertyKind::SpreadProperty(_) => None,
                    })
                    .collect(),
            ),
            Expression::ArrayExpression(a) => V::Arr(
                a.elements
                    .iter()
                    .map(|el| match el {
                        ArrayExpressionElement::SpreadElement(s) => self.other(s.span),
                        ArrayExpressionElement::Elision(s) => self.other(s.span),
                        other => other.as_expression().map_or(V::Null, |e| self.expr(e)),
                    })
                    .collect(),
            ),
            Expression::JSXElement(el) => V::Jsx(Rc::new(self.jsx(el))),
            Expression::JSXFragment(f) => V::Frag(self.children(&f.children)),
            Expression::ConditionalExpression(c) => V::Cond(
                Box::new(self.expr(&c.test)),
                Box::new(self.expr(&c.consequent)),
                Box::new(self.expr(&c.alternate)),
            ),
            Expression::LogicalExpression(l) => {
                V::Logical(Box::new(self.expr(&l.left)), Box::new(self.expr(&l.right)))
            }
            Expression::BinaryExpression(b) => {
                V::Ops(vec![self.expr(&b.left), self.expr(&b.right)])
            }
            Expression::UnaryExpression(u) => V::Ops(vec![self.expr(&u.argument)]),
            Expression::AwaitExpression(a) => V::Await(Box::new(self.expr(&a.argument))),
            Expression::ParenthesizedExpression(p) => self.expr(&p.expression),
            Expression::TSAsExpression(t) => self.expr(&t.expression),
            Expression::TSSatisfiesExpression(t) => self.expr(&t.expression),
            Expression::TSNonNullExpression(t) => self.expr(&t.expression),
            Expression::TSInstantiationExpression(t) => self.expr(&t.expression),
            other => self.other(other.span()),
        }
    }

    fn arg(&self, a: &Argument<'_>) -> V {
        a.as_expression().map_or_else(|| self.other(a.span()), |e| self.expr(e))
    }

    fn function(&self, f: &Function<'_>) -> Func {
        Func {
            params: Self::params(&f.params),
            body: f.body.as_ref().map_or_else(Vec::new, |b| self.stmts(&b.statements)),
            lines: self.lines(f.span),
        }
    }

    fn params(p: &FormalParameters<'_>) -> Vec<Pat> {
        p.items.iter().map(|item| Self::pat(&item.pattern)).collect()
    }

    fn pat(p: &BindingPattern<'_>) -> Pat {
        match p {
            BindingPattern::BindingIdentifier(id) => Pat::Name(id.name.to_string()),
            BindingPattern::ObjectPattern(o) => Pat::Obj(
                o.properties
                    .iter()
                    .filter_map(|prop| Some((key_name(&prop.key)?, Self::pat(&prop.value))))
                    .collect(),
            ),
            BindingPattern::ArrayPattern(a) => {
                Pat::Arr(a.elements.iter().map(|el| el.as_ref().map(|p| Self::pat(p))).collect())
            }
            BindingPattern::AssignmentPattern(a) => Self::pat(&a.left),
        }
    }

    fn stmts(&self, list: &[Statement<'_>]) -> Vec<S> {
        list.iter().flat_map(|s| self.stmt(s)).collect()
    }

    fn stmt(&self, s: &Statement<'_>) -> Vec<S> {
        match s {
            Statement::ExpressionStatement(e) => {
                vec![S::Expr(self.expr(&e.expression), self.lines(e.span))]
            }
            Statement::IfStatement(i) => vec![S::If(
                self.expr(&i.test),
                self.text(i.test.span()),
                self.stmt(&i.consequent),
                i.alternate.as_ref().map_or_else(Vec::new, |a| self.stmt(a)),
                self.lines(i.span),
            )],
            Statement::ReturnStatement(r) => {
                vec![S::Return(r.argument.as_ref().map(|a| self.expr(a)), self.lines(r.span))]
            }
            Statement::ThrowStatement(t) => {
                vec![S::Throw(self.expr(&t.argument), self.lines(t.span))]
            }
            Statement::BlockStatement(b) => self.stmts(&b.body),
            Statement::TryStatement(t) => {
                let mut out = self.stmts(&t.block.body);
                if let Some(handler) = &t.handler {
                    out.extend(self.stmts(&handler.body.body));
                }
                out
            }
            Statement::VariableDeclaration(v) => v
                .declarations
                .iter()
                .map(|d| {
                    S::Decl(
                        Self::pat(&d.id),
                        d.init.as_ref().map(|i| self.expr(i)),
                        self.lines(d.span),
                    )
                })
                .collect(),
            Statement::FunctionDeclaration(f) => f.id.as_ref().map_or_else(Vec::new, |id| {
                vec![S::Decl(
                    Pat::Name(id.name.to_string()),
                    Some(V::Func(Rc::new(self.function(f)))),
                    self.lines(f.span),
                )]
            }),
            _ => Vec::new(),
        }
    }

    fn jsx(&self, el: &JSXElement<'_>) -> Jsx {
        let opening = &el.opening_element;
        let name = match &opening.name {
            JSXElementName::Identifier(id) => JName::Tag(id.name.to_string()),
            JSXElementName::IdentifierReference(id) => JName::Ref(V::Ident(id.name.to_string())),
            JSXElementName::MemberExpression(m) => JName::Ref(jsx_member(m)),
            JSXElementName::NamespacedName(n) => {
                JName::Tag(format!("{}:{}", n.namespace.name, n.name.name))
            }
            JSXElementName::ThisExpression(_) => JName::Ref(V::This),
        };
        let attrs = opening
            .attributes
            .iter()
            .map(|item| match item {
                JSXAttributeItem::Attribute(a) => {
                    let name = match &a.name {
                        JSXAttributeName::Identifier(id) => id.name.to_string(),
                        JSXAttributeName::NamespacedName(n) => {
                            format!("{}:{}", n.namespace.name, n.name.name)
                        }
                    };
                    let value = a.value.as_ref().map(|v| match v {
                        JSXAttributeValue::StringLiteral(s) => V::Str(s.value.to_string()),
                        JSXAttributeValue::ExpressionContainer(c) => {
                            self.jsx_expr(&c.expression, c.span)
                        }
                        JSXAttributeValue::Element(e) => V::Jsx(Rc::new(self.jsx(e))),
                        JSXAttributeValue::Fragment(f) => V::Frag(self.children(&f.children)),
                    });
                    Attr::Named(name, value, self.lines(a.span))
                }
                JSXAttributeItem::SpreadAttribute(s) => Attr::Spread(self.expr(&s.argument)),
            })
            .collect();
        Jsx {
            name,
            attrs,
            children: self.children(&el.children),
            lines: self.lines(el.span),
        }
    }

    fn jsx_expr(&self, e: &JSXExpression<'_>, span: Span) -> V {
        e.as_expression().map_or_else(|| self.other(span), |e| self.expr(e))
    }

    fn children(&self, list: &[JSXChild<'_>]) -> Vec<V> {
        list.iter()
            .filter_map(|child| match child {
                JSXChild::Text(t) => {
                    let text = jsx_text(t.value.as_str());
                    (!text.is_empty()).then_some(V::Str(text))
                }
                JSXChild::Element(e) => Some(V::Jsx(Rc::new(self.jsx(e)))),
                JSXChild::Fragment(f) => Some(V::Frag(self.children(&f.children))),
                JSXChild::ExpressionContainer(c) => match &c.expression {
                    JSXExpression::EmptyExpression(_) => None,
                    other => Some(self.jsx_expr(other, c.span)),
                },
                JSXChild::Spread(s) => Some(self.other(s.span)),
            })
            .collect()
    }

    fn class(&self, class: &Class<'_>, name: &str, module: &mut Module) {
        for element in &class.body.body {
            if let ClassElement::MethodDefinition(m) = element
                && let Some(method) = m.key.static_name()
            {
                module.decls.insert(
                    format!("{name}.{method}"),
                    (Pat::Other, V::Func(Rc::new(self.function(&m.value))), self.lines(m.span)),
                );
            }
        }
    }

    fn declaration(&self, d: &Declaration<'_>, module: &mut Module, exported: bool) {
        match d {
            Declaration::VariableDeclaration(v) => {
                for decl in &v.declarations {
                    let pat = Self::pat(&decl.id);
                    if let Pat::Name(name) = &pat {
                        let init = decl.init.as_ref().map_or(V::Null, |i| self.expr(i));
                        if exported {
                            module.exports.insert(name.clone(), name.clone());
                        }
                        module
                            .decls
                            .insert(name.clone(), (pat.clone(), init, self.lines(decl.span)));
                    }
                }
            }
            Declaration::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    let name = id.name.to_string();
                    if exported {
                        module.exports.insert(name.clone(), name.clone());
                    }
                    module.decls.insert(
                        name.clone(),
                        (Pat::Name(name), V::Func(Rc::new(self.function(f))), self.lines(f.span)),
                    );
                }
            }
            Declaration::ClassDeclaration(c) => {
                if let Some(id) = &c.id {
                    let name = id.name.to_string();
                    if exported {
                        module.exports.insert(name.clone(), name.clone());
                    }
                    self.class(c, &name, module);
                }
            }
            _ => {}
        }
    }
}

fn export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(n) => n.name.to_string(),
        ModuleExportName::IdentifierReference(n) => n.name.to_string(),
        ModuleExportName::StringLiteral(s) => s.value.to_string(),
    }
}

fn jsx_member(m: &JSXMemberExpression<'_>) -> V {
    let object = match &m.object {
        JSXMemberExpressionObject::IdentifierReference(id) => V::Ident(id.name.to_string()),
        JSXMemberExpressionObject::MemberExpression(inner) => jsx_member(inner),
        JSXMemberExpressionObject::ThisExpression(_) => V::This,
    };
    V::Member(Box::new(object), m.property.name.to_string())
}

fn key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.to_string()),
        _ => None,
    }
}

/// JSX text the way React renders it: lines trimmed, blank lines dropped,
/// the rest joined by one space.
#[must_use]
pub fn jsx_text(raw: &str) -> String {
    let lines: Vec<&str> = raw.split('\n').collect();
    let last = lines.len().saturating_sub(1);
    let mut parts: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut piece = line.replace('\t', " ");
        if i != 0 {
            piece = piece.trim_start().to_string();
        }
        if i != last {
            piece = piece.trim_end().to_string();
        }
        if !piece.is_empty() {
            parts.push(piece);
        }
    }
    parts.join(" ")
}

/// Parse one module.
#[must_use]
pub fn parse(path: &str, text: &str) -> Module {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_else(|_| SourceType::tsx());
    let parsed = Parser::new(&allocator, text, source_type).parse();
    let mut starts = vec![0_u32];
    for (i, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(u32::try_from(i + 1).unwrap_or(u32::MAX));
        }
    }
    let reader = Reader { text, starts };
    let mut module = Module {
        path: path.to_string(),
        errors: parsed.diagnostics.iter().map(ToString::to_string).collect(),
        ..Module::default()
    };
    for statement in &parsed.program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                let source = import.source.value.to_string();
                for spec in import.specifiers.iter().flatten() {
                    let (local, name) = match spec {
                        ImportDeclarationSpecifier::ImportSpecifier(s) => {
                            (s.local.name.to_string(), export_name(&s.imported))
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                            (s.local.name.to_string(), "default".to_string())
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                            (s.local.name.to_string(), "*".to_string())
                        }
                    };
                    module.imports.insert(
                        local,
                        Import {
                            source: source.clone(),
                            name,
                        },
                    );
                }
            }
            Statement::ExportDeclaration(export) => {
                reader.declaration(&export.declaration, &mut module, true);
            }
            Statement::ExportNamedDeclaration(export) => {
                for spec in &export.specifiers {
                    module.exports.insert(export_name(&spec.exported), export_name(&spec.local));
                }
            }
            Statement::ExportFromDeclaration(export) => {
                for spec in &export.specifiers {
                    module.reexports.insert(
                        export_name(&spec.exported),
                        (export.source.value.to_string(), export_name(&spec.local)),
                    );
                }
            }
            Statement::ExportAllDeclaration(all) if all.exported.is_none() => {
                module.reexport_all.push(all.source.value.to_string());
            }
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    let name =
                        f.id.as_ref()
                            .map_or_else(|| "default".to_string(), |id| id.name.to_string());
                    module.exports.insert("default".into(), name.clone());
                    module.decls.insert(
                        name.clone(),
                        (
                            Pat::Name(name),
                            V::Func(Rc::new(reader.function(f))),
                            reader.lines(f.span),
                        ),
                    );
                }
                other => {
                    if let Some(e) = other.as_expression() {
                        match reader.expr(e) {
                            V::Ident(name) => {
                                module.exports.insert("default".into(), name);
                            }
                            value => {
                                module.exports.insert("default".into(), "default".into());
                                module.decls.insert(
                                    "default".into(),
                                    (Pat::Name("default".into()), value, reader.lines(export.span)),
                                );
                            }
                        }
                    }
                }
            },
            other => {
                if let Some(d) = other.as_declaration() {
                    reader.declaration(d, &mut module, false);
                }
            }
        }
    }
    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsx_text_follows_react_whitespace() {
        assert_eq!(jsx_text("\n   Forgot your password?\n  "), "Forgot your password?");
        assert_eq!(jsx_text("Log In"), "Log In");
        assert_eq!(jsx_text("\n  a\n  b  \n"), "a b");
    }

    #[test]
    fn reads_imports_decls_and_lines() {
        let m = parse(
            "x.tsx",
            "import { a } from \"pkg\"\nimport * as N from \"ns\"\nexport const f = () => a(1)\nexport class C { static g() { return 2 } }\nexport * from \"./y\"\n",
        );
        assert_eq!(m.imports.get("a").map(|i| i.source.as_str()), Some("pkg"));
        assert_eq!(m.imports.get("N").map(|i| i.name.as_str()), Some("*"));
        assert_eq!(m.decls.get("f").map(|d| d.2), Some((3, 3)));
        assert!(m.decls.contains_key("C.g"));
        assert_eq!(m.reexport_all, vec!["./y".to_string()]);
        assert_eq!(m.errors, [] as [std::string::String; 0]);
    }
}
