// SPDX-License-Identifier: AGPL-3.0-or-later
//! The syntax tree. Every node keeps its span so that tools can point back into the source.

use crate::source::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

/// A dotted path such as `converges.eps_n`.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub segments: Vec<Name>,
    pub span: Span,
}

impl Path {
    pub fn joined(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(".")
    }
}

/// Text in one language, e.g. `label de "konvergiert gegen"`.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub lang: Name,
    pub value: String,
    pub span: Span,
}

/// Lean 4 source kept as a string; `span` covers the string literal.
#[derive(Clone, Debug, PartialEq)]
pub struct Lean {
    pub code: String,
    pub span: Span,
}

/// Labels and sources that any declaration may carry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meta {
    pub labels: Vec<Text>,
    pub sources: Vec<(String, Span)>,
}

#[derive(Clone, Debug, Default)]
pub struct File {
    pub module: Option<Path>,
    /// The languages a module is written in (`module m original de, en`); other languages belong in translations.
    pub originals: Vec<Name>,
    /// A translation file names the module it translates (`translation m`) instead of being one.
    pub translation: Option<Path>,
    /// What a translation file translates.
    pub translated: Vec<Translated>,
    pub items: Vec<Item>,
}

/// The texts of one declaration in other languages: `converges.eps_n { label fr "…" }`.
#[derive(Clone, Debug)]
pub struct Translated {
    /// The declaration, relative to the module.
    pub target: Path,
    pub labels: Vec<Text>,
    pub texts: Vec<Text>,
    pub criteria: Vec<Text>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Item {
    Concept(Concept),
    Theorem(Theorem),
    Remark(Remark),
    Norm(Norm),
    Path(PathDecl),
    Enum(EnumDecl),
    Unit(UnitDecl),
    Record(RecordDecl),
}

impl Item {
    pub fn name(&self) -> &Name {
        match self {
            Item::Concept(x) => &x.name,
            Item::Theorem(x) => &x.name,
            Item::Remark(x) => &x.name,
            Item::Norm(x) => &x.name,
            Item::Path(x) => &x.name,
            Item::Enum(x) => &x.name,
            Item::Unit(x) => &x.name,
            Item::Record(x) => &x.name,
        }
    }

    pub fn meta(&self) -> &Meta {
        match self {
            Item::Concept(x) => &x.meta,
            Item::Theorem(x) => &x.meta,
            Item::Remark(x) => &x.meta,
            Item::Norm(x) => &x.meta,
            Item::Path(x) => &x.meta,
            Item::Enum(x) => &x.meta,
            Item::Unit(x) => &x.meta,
            Item::Record(x) => &x.meta,
        }
    }
}

/// A notion with one or more variants of its definition and bridges between them. Without a type a concept is a
/// statement about its parameters (a `Prop`); with one it defines an object of that type.
#[derive(Clone, Debug)]
pub struct Concept {
    pub name: Name,
    pub meta: Meta,
    pub params: Option<Lean>,
    /// The type of the object the concept defines (`type lean "ℝ"`); none for a statement.
    pub ty: Option<Lean>,
    pub variants: Vec<Variant>,
    pub bridges: Vec<Bridge>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub name: Name,
    pub meta: Meta,
    pub def: Option<Lean>,
    pub span: Span,
}

/// An equivalence (`<->`, between statements) or an equality (`=`, between objects) of two variants of the same
/// concept.
#[derive(Clone, Debug)]
pub struct Bridge {
    pub left: Name,
    pub right: Name,
    /// Written with `=` rather than `<->`.
    pub eq: bool,
    pub proof: Option<Lean>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TheoremKind {
    Theorem,
    Example,
}

#[derive(Clone, Debug)]
pub struct Theorem {
    pub kind: TheoremKind,
    pub name: Name,
    pub meta: Meta,
    pub params: Option<Lean>,
    /// Variants the statement and proof are written against (`using converges.filter`); the first variant of
    /// every other concept is assumed.
    pub using: Vec<Path>,
    pub statement: Option<Lean>,
    pub proof: Option<Lean>,
    pub span: Span,
}

/// Informal text attached to formal declarations.
#[derive(Clone, Debug)]
pub struct Remark {
    pub name: Name,
    pub meta: Meta,
    pub about: Vec<Path>,
    pub texts: Vec<Text>,
    pub span: Span,
}

/// A legal provision: inputs, constants and rules with exceptions.
#[derive(Clone, Debug)]
pub struct Norm {
    pub name: Name,
    pub meta: Meta,
    pub valid_from: Option<(String, Span)>,
    pub valid_to: Option<(String, Span)>,
    /// The earlier version of the same provision that this one replaces (`replaces small_business_2020`).
    pub replaces: Option<Path>,
    pub members: Vec<Member>,
    pub span: Span,
}

/// The type of an input, judgement, constant, rule or field: a built-in type (`money`), an enumeration, a unit or a
/// record declared in some module (`residence`, `de.ustg.p1.supply`), or a list of records (`list supply`).
#[derive(Clone, Debug, PartialEq)]
pub struct TypeRef {
    pub list: bool,
    pub name: Path,
    pub span: Span,
}

impl TypeRef {
    /// A built-in type at `span`, such as the `bool` of an obligation.
    pub fn builtin(name: &str, span: Span) -> TypeRef {
        let name = Name { text: name.into(), span };
        TypeRef { list: false, name: Path { segments: vec![name], span }, span }
    }

    /// As written: `money`, `list supply`.
    pub fn text(&self) -> String {
        if self.list { format!("list {}", self.name.joined()) } else { self.name.joined() }
    }
}

/// An enumeration: a type with a fixed set of values, each with its own labels – `enum residence { value domestic
/// value eu_other value third_country }`.
#[derive(Clone, Debug)]
pub struct EnumDecl {
    pub name: Name,
    pub meta: Meta,
    pub values: Vec<EnumValue>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct EnumValue {
    pub name: Name,
    pub meta: Meta,
    pub span: Span,
}

/// A unit of measurement other than money (`unit km`): amounts of it are written `30 km`, and only amounts of the
/// same unit add up or compare.
#[derive(Clone, Debug)]
pub struct UnitDecl {
    pub name: Name,
    pub meta: Meta,
    pub span: Span,
}

/// A record: several named fields that belong together – one supply with its amount and kind. Inputs may be one
/// record or a list of them.
#[derive(Clone, Debug)]
pub struct RecordDecl {
    pub name: Name,
    pub meta: Meta,
    pub fields: Vec<RecordField>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct RecordField {
    pub name: Name,
    pub ty: TypeRef,
    pub meta: Meta,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub name: Name,
    pub ty: TypeRef,
    pub meta: Meta,
    pub kind: MemberKind,
    /// For a rule written `obligation`, `permission` or `prohibition`: what it says about the act it names. Such a
    /// rule is `bool`: whether the obligation, permission or prohibition applies.
    pub modality: Option<Modality>,
    pub span: Span,
}

/// Deontic modalities: what a rule says about an act.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modality {
    Obligation,
    Permission,
    Prohibition,
}

impl Modality {
    pub fn keyword(self) -> &'static str {
        match self {
            Modality::Obligation => "obligation",
            Modality::Permission => "permission",
            Modality::Prohibition => "prohibition",
        }
    }
}

#[derive(Clone, Debug)]
pub enum MemberKind {
    Input,
    /// An undetermined legal term: an input that is not a fact but a judgement someone makes – by the criteria given,
    /// per language – and has to justify.
    Judgement {
        criteria: Vec<Text>,
    },
    Constant(Expr),
    Rule {
        default: Option<Expr>,
        unless: Vec<Unless>,
    },
}

/// `unless <cond> then <value>`: an exception that overrides the default.
#[derive(Clone, Debug)]
pub struct Unless {
    pub cond: Expr,
    pub value: Expr,
    pub meta: Meta,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    /// A member, a constant of another norm, an enumeration value, a variable of an aggregate – and fields of
    /// records after it (`owner.age`, `s.amount`).
    Ref(Path),
    Number(String),
    /// `19 %`: the number divided by 100.
    Percent(String),
    /// An amount: of money (`5 EUR`) or of a unit (`30 km`).
    Money(String, Name),
    Bool(bool),
    Date(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Paren(Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Path, Vec<Expr>),
    /// `sum(s.amount for s in supplies where s.taxable)`, `count(s in supplies where …)`, `all(…)`, `any(…)`.
    Aggregate(Box<Aggregate>),
    /// A rule or constant of another norm, for inputs given by name: `total_turnover.total(taxable = x, …)`.
    Apply(Path, Vec<(Name, Expr)>),
    /// A table: a value by ranges of a number or by the values of an enumeration.
    Table(Box<Table>),
    Error,
}

/// `<func>(<body> for <var> in <list> where <filter>)`; `count` has no body (`count(s in supplies)`).
#[derive(Clone, Debug)]
pub struct Aggregate {
    pub func: Name,
    pub body: Option<Expr>,
    pub var: Name,
    pub list: Expr,
    pub filter: Option<Expr>,
}

/// The rows of a rule written `by <subject>`: `up to <bound>: <value>` for ranges, ascending, or `<value of an
/// enumeration>: <value>`, and `otherwise: <value>` for the rest.
#[derive(Clone, Debug)]
pub struct Table {
    pub subject: Expr,
    pub rows: Vec<Row>,
    pub otherwise: Option<Expr>,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub key: RowKey,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum RowKey {
    /// `up to <bound>`: the subject at most this.
    UpTo(Expr),
    /// A value of the enumeration the subject is of.
    Value(Path),
}

/// A path through the knowledge: a lecture, a guide, a commentary.
#[derive(Clone, Debug)]
pub struct PathDecl {
    pub name: Name,
    pub meta: Meta,
    pub steps: Vec<Path>,
    pub span: Span,
}
