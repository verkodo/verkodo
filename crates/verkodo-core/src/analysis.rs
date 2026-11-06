// SPDX-License-Identifier: AGPL-3.0-or-later
//! Symbols, references, dependencies and the checks on top of them.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;

use verkodo_syntax::ast::*;
use verkodo_syntax::{Diagnostic, Span};

use crate::lean;
use crate::sources::Sources;
use crate::types::{CURRENCIES, Ty};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymKind {
    Concept,
    Variant,
    Theorem,
    Example,
    Remark,
    Norm,
    Input,
    Judgement,
    Constant,
    Rule,
    Path,
    Enum,
    /// A value of an enumeration.
    Value,
    Unit,
    Record,
    /// A field of a record.
    Field,
}

impl SymKind {
    pub fn keyword(self) -> &'static str {
        match self {
            SymKind::Concept => "concept",
            SymKind::Variant => "variant",
            SymKind::Theorem => "theorem",
            SymKind::Example => "example",
            SymKind::Remark => "remark",
            SymKind::Norm => "norm",
            SymKind::Input => "input",
            SymKind::Judgement => "judgement",
            SymKind::Constant => "constant",
            SymKind::Rule => "rule",
            SymKind::Path => "path",
            SymKind::Enum => "enum",
            SymKind::Value => "value",
            SymKind::Unit => "unit",
            SymKind::Record => "record",
            SymKind::Field => "field",
        }
    }

    /// Kinds a path has to introduce before it uses them.
    fn is_ordered(self) -> bool {
        matches!(
            self,
            SymKind::Concept
                | SymKind::Variant
                | SymKind::Theorem
                | SymKind::Example
                | SymKind::Constant
                | SymKind::Rule
        )
    }
}

/// What a translation translates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Label,
    /// The text of a remark.
    Text,
    /// The criteria of a judgement.
    Criteria,
}

impl Field {
    pub fn keyword(self) -> &'static str {
        match self {
            Field::Label => "label",
            Field::Text => "text",
            Field::Criteria => "criteria",
        }
    }
}

/// How many texts a language has, and which are missing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coverage {
    pub translated: usize,
    pub total: usize,
    /// (declaration id, field) without a text in this language.
    pub missing: Vec<(String, Field)>,
}

/// One step of a chain of bridges: from one variant to another through a bridge of their concept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hop {
    pub from: usize,
    pub to: usize,
    /// The bridge, as its index in the concept.
    pub bridge: usize,
    /// Whether the bridge is written `from <-> to` (otherwise `to <-> from`).
    pub forward: bool,
}

/// Where a declaration lives in the syntax trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loc {
    Item { file: usize, item: usize },
    Variant { file: usize, item: usize, variant: usize },
    Member { file: usize, item: usize, member: usize },
}

impl Loc {
    pub fn file(self) -> usize {
        match self {
            Loc::Item { file, .. } | Loc::Variant { file, .. } | Loc::Member { file, .. } => file,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Symbol {
    /// Stable id: `<module>.<item>[.<variant or member>]`.
    pub id: String,
    pub kind: SymKind,
    pub loc: Loc,
    /// Span of the declared name.
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Symbol(usize),
    /// A parameter of the surrounding declaration.
    Local {
        name: String,
        ty: String,
    },
    /// A Lean or Mathlib name; Lean resolves it in milestone M2.
    External(String),
    /// A built-in function of the legal profile.
    Builtin(String),
}

/// A reference inside a Lean snippet; `range` is a byte range of the snippet's code.
#[derive(Clone, Debug)]
pub struct LeanRef {
    pub range: Range<usize>,
    pub target: Target,
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub symbols: Vec<Symbol>,
    pub by_id: HashMap<String, usize>,
    /// Keyed by file and the span of the Lean string literal.
    pub lean_refs: HashMap<(usize, Span), Vec<LeanRef>>,
    /// Paths in expressions, `about`, `step` and bridge sides, keyed by file and span.
    pub refs: HashMap<(usize, Span), Target>,
    /// What each symbol refers to.
    pub deps: Vec<BTreeSet<usize>>,
    /// The types of inputs, judgements, constants, rules and fields of records.
    pub member_types: HashMap<usize, Ty>,
    /// The type of every expression of the legal profile, keyed by file and span – what Lean needs to convert
    /// whole numbers where fractions are computed.
    pub expr_types: HashMap<(usize, Span), Ty>,
    /// Fields read after a value (`owner.age`, `s.amount`), keyed by file and the span of the path: the field
    /// symbols in order. `refs` holds what the path starts from.
    pub fields: HashMap<(usize, Span), Vec<usize>>,
    /// The variables of the aggregates around the expression being checked.
    locals: Vec<(String, Ty)>,
    /// Versions of a provision: a norm → the earlier norm it replaces.
    pub replaces: HashMap<usize, usize>,
    /// The other way round: a norm → the later norm that replaces it.
    pub replaced_by: HashMap<usize, usize>,
    /// Texts from translation files: (declaration, field, language) → text.
    pub translations: HashMap<(usize, Field, String), String>,
    /// The original languages of each module (`module m original de, en`), where it names them.
    pub originals: HashMap<String, Vec<String>>,
    pub diags: Vec<Diagnostic>,
}

const BUILTINS: &[&str] = &[
    "min",
    "max",
    "abs",
    "div",
    "mod",
    "round_down",
    "round_up",
    "round",
    "year",
    "quarter",
    "month",
    "day",
    "weekday",
    "date",
    "last_day",
    "add_months",
    "add_years",
];

/// Functions that take a date and give a whole number: its year, quarter (1–4), month (1–12), day of the month and
/// day of the week (1 Monday … 7 Sunday).
pub const DATE_PARTS: &[&str] = &["year", "quarter", "month", "day", "weekday"];

/// The aggregates over a list.
pub const AGGREGATES: &[&str] = &["sum", "count", "all", "any"];

/// Rounding to a multiple of a step: down, up, or half away from zero ("kaufmännisch").
pub const ROUNDING: &[&str] = &["round_down", "round_up", "round"];

pub fn analyze(ws: &Sources) -> Analysis {
    let mut a = Analysis::default();
    a.collect(ws);
    a.assign_types(ws);
    a.resolve(ws);
    a.check_versions(ws);
    a.check_translations(ws);
    a.check_paths(ws);
    a.diags.sort_by_key(|d| (d.file, d.span.start, d.severity));
    a
}

impl Analysis {
    pub fn get(&self, id: &str) -> Option<&Symbol> {
        self.by_id.get(id).map(|&i| &self.symbols[i])
    }

    pub fn meta<'a>(&self, ws: &'a Sources, sym: usize) -> &'a Meta {
        match self.symbols[sym].loc {
            Loc::Item { file, item } => ws.asts[file].items[item].meta(),
            Loc::Variant { file, item, variant } => match &ws.asts[file].items[item] {
                Item::Concept(c) => &c.variants[variant].meta,
                _ => unreachable!("variant outside a concept"),
            },
            Loc::Member { file, item, member } => match &ws.asts[file].items[item] {
                Item::Norm(n) => &n.members[member].meta,
                Item::Enum(e) => &e.values[member].meta,
                Item::Record(r) => &r.fields[member].meta,
                _ => unreachable!("member outside a norm, enumeration or record"),
            },
        }
    }

    /// The concept a variant belongs to, or the norm a member belongs to.
    pub fn parent(&self, sym: usize) -> Option<usize> {
        match self.symbols[sym].kind {
            SymKind::Variant
            | SymKind::Input
            | SymKind::Judgement
            | SymKind::Constant
            | SymKind::Rule
            | SymKind::Value
            | SymKind::Field => {
                let (parent, _) = self.symbols[sym].id.rsplit_once('.')?;
                self.by_id.get(parent).copied()
            }
            _ => None,
        }
    }

    fn add(&mut self, id: String, kind: SymKind, loc: Loc, span: Span) -> Option<usize> {
        if let Some(&prev) = self.by_id.get(&id) {
            let prev_file = self.symbols[prev].loc.file();
            let msg = if prev_file == loc.file() {
                format!("`{id}` is declared twice")
            } else {
                format!("`{id}` is already declared in another file")
            };
            self.diags.push(Diagnostic::error(loc.file(), span, msg));
            return None;
        }
        let i = self.symbols.len();
        self.symbols.push(Symbol { id: id.clone(), kind, loc, span });
        self.deps.push(BTreeSet::new());
        self.by_id.insert(id, i);
        Some(i)
    }

    fn lookup(&self, name: &str, scopes: &[&str]) -> Option<usize> {
        scopes.iter().find_map(|s| self.by_id.get(&format!("{s}.{name}"))).or_else(|| self.by_id.get(name)).copied()
    }

    // ---- 1. symbols ----

    fn collect(&mut self, ws: &Sources) {
        for (file, ast) in ws.asts.iter().enumerate() {
            let Some(module) = &ast.module else { continue };
            let m = module.joined();
            for (item, it) in ast.items.iter().enumerate() {
                let id = format!("{m}.{}", it.name().text);
                let kind = match it {
                    Item::Concept(_) => SymKind::Concept,
                    Item::Theorem(t) if t.kind == TheoremKind::Example => SymKind::Example,
                    Item::Theorem(_) => SymKind::Theorem,
                    Item::Remark(_) => SymKind::Remark,
                    Item::Norm(_) => SymKind::Norm,
                    Item::Path(_) => SymKind::Path,
                    Item::Enum(_) => SymKind::Enum,
                    Item::Unit(_) => SymKind::Unit,
                    Item::Record(_) => SymKind::Record,
                };
                if self.add(id.clone(), kind, Loc::Item { file, item }, it.name().span).is_none() {
                    continue;
                }
                match it {
                    Item::Concept(c) => {
                        for (variant, v) in c.variants.iter().enumerate() {
                            let loc = Loc::Variant { file, item, variant };
                            self.add(format!("{id}.{}", v.name.text), SymKind::Variant, loc, v.name.span);
                        }
                    }
                    Item::Norm(n) => {
                        for (member, mem) in n.members.iter().enumerate() {
                            let kind = match mem.kind {
                                MemberKind::Input => SymKind::Input,
                                MemberKind::Judgement { .. } => SymKind::Judgement,
                                MemberKind::Constant(_) => SymKind::Constant,
                                MemberKind::Rule { .. } => SymKind::Rule,
                            };
                            let loc = Loc::Member { file, item, member };
                            self.add(format!("{id}.{}", mem.name.text), kind, loc, mem.name.span);
                        }
                    }
                    Item::Enum(e) => {
                        for (member, v) in e.values.iter().enumerate() {
                            let loc = Loc::Member { file, item, member };
                            self.add(format!("{id}.{}", v.name.text), SymKind::Value, loc, v.name.span);
                        }
                        if e.values.is_empty() {
                            let msg = format!("enumeration `{}` has no values", e.name.text);
                            self.diags.push(Diagnostic::error(file, e.name.span, msg));
                        }
                    }
                    Item::Record(r) => {
                        for (member, f) in r.fields.iter().enumerate() {
                            let loc = Loc::Member { file, item, member };
                            self.add(format!("{id}.{}", f.name.text), SymKind::Field, loc, f.name.span);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The types of members and fields, once every type is declared.
    fn assign_types(&mut self, ws: &Sources) {
        for (file, ast) in ws.asts.iter().enumerate() {
            let Some(module) = &ast.module else { continue };
            let m = module.joined();
            for it in &ast.items {
                let id = format!("{m}.{}", it.name().text);
                let typed: Vec<(&Name, &TypeRef)> = match it {
                    Item::Norm(n) => n.members.iter().map(|x| (&x.name, &x.ty)).collect(),
                    Item::Record(r) => r.fields.iter().map(|x| (&x.name, &x.ty)).collect(),
                    _ => continue,
                };
                for (name, ty) in typed {
                    let Some(&s) = self.by_id.get(&format!("{id}.{}", name.text)) else { continue };
                    if self.symbols[s].span != name.span {
                        continue; // a duplicate, already reported
                    }
                    let t = self.resolve_type(file, &m, ty, s);
                    if let (Item::Record(_), Ty::Record(r) | Ty::List(r)) = (it, t)
                        && self.by_id.get(&id) == Some(&r)
                    {
                        let msg = format!("a field of `{}` cannot hold `{}` itself", it.name().text, it.name().text);
                        self.diags.push(Diagnostic::error(file, ty.span, msg));
                    }
                    self.member_types.insert(s, t);
                }
            }
        }
    }

    /// A type as written: built in, or an enumeration, unit or record of some module; `list` only of records.
    fn resolve_type(&mut self, file: usize, module: &str, ty: &TypeRef, owner: usize) -> Ty {
        let name = ty.name.joined();
        if !ty.list
            && let Some(t) = Ty::parse(&name)
        {
            return t;
        }
        let Some(s) = self.lookup(&name, &[module]) else {
            let msg =
                format!("unknown type `{name}` (expected bool, money, int, decimal, date, or an enum, unit or record)");
            self.diags.push(Diagnostic::error(file, ty.name.span, msg));
            return Ty::Unknown;
        };
        self.refs.insert((file, ty.name.span), Target::Symbol(s));
        self.deps[owner].insert(s);
        let t = match self.symbols[s].kind {
            SymKind::Enum => Ty::Enum(s),
            SymKind::Unit => Ty::Quantity(s),
            SymKind::Record if ty.list => return Ty::List(s),
            SymKind::Record => Ty::Record(s),
            other => {
                let msg = format!("`{name}` is a {}, not a type", other.keyword());
                self.diags.push(Diagnostic::error(file, ty.name.span, msg));
                return Ty::Unknown;
            }
        };
        if ty.list {
            let msg = format!("a list holds records, and `{name}` is an {}", t.name());
            self.diags.push(Diagnostic::error(file, ty.span, msg));
            return Ty::Unknown;
        }
        t
    }

    /// The name of a type as written in the source: `money`, `residence`, `list supply`.
    pub fn ty_name(&self, t: Ty) -> String {
        let short = |s: usize| self.symbols[s].id.rsplit('.').next().unwrap_or_default().to_string();
        match t {
            Ty::Enum(s) | Ty::Quantity(s) | Ty::Record(s) => short(s),
            Ty::List(s) => format!("list {}", short(s)),
            other => other.name().to_string(),
        }
    }

    // ---- 2. references ----

    fn resolve(&mut self, ws: &Sources) {
        for (file, ast) in ws.asts.iter().enumerate() {
            let Some(module) = &ast.module else { continue };
            let m = module.joined();
            for it in &ast.items {
                let id = format!("{m}.{}", it.name().text);
                let Some(&me) = self.by_id.get(&id) else {
                    continue;
                };
                if self.symbols[me].span != it.name().span {
                    continue; // a duplicate, already reported
                }
                match it {
                    Item::Concept(c) => self.resolve_concept(file, &m, &id, me, c),
                    Item::Theorem(t) => {
                        let locals = t.params.as_ref().map(|p| lean::binders(&p.code)).unwrap_or_default();
                        for l in [&t.params, &t.statement, &t.proof].into_iter().flatten() {
                            self.scan_lean(file, l, &m, &locals, me);
                        }
                        for u in &t.using {
                            if let Some(s) = self.resolve_path(file, u, &[&m], me)
                                && self.symbols[s].kind != SymKind::Variant
                            {
                                let msg = format!(
                                    "`using` needs a variant, `{}` is a {}",
                                    u.joined(),
                                    self.symbols[s].kind.keyword()
                                );
                                self.diags.push(Diagnostic::error(file, u.span, msg));
                            }
                        }
                        if t.proof.is_none() {
                            let what = if t.kind == TheoremKind::Example { "example" } else { "theorem" };
                            let msg = format!("the proof of {what} `{}` is OPEN", t.name.text);
                            self.diags.push(Diagnostic::warning(file, t.name.span, msg));
                        }
                    }
                    Item::Remark(r) => {
                        for p in &r.about {
                            self.resolve_path(file, p, &[&m], me);
                        }
                        if r.texts.is_empty() {
                            let msg = format!("remark `{}` has no text", r.name.text);
                            self.diags.push(Diagnostic::warning(file, r.name.span, msg));
                        }
                    }
                    Item::Norm(n) => self.resolve_norm(file, &m, &id, n),
                    Item::Path(d) => {
                        for step in &d.steps {
                            self.resolve_path(file, step, &[&m], me);
                        }
                    }
                    Item::Enum(_) | Item::Unit(_) | Item::Record(_) => {}
                }
            }
        }
    }

    fn resolve_concept(&mut self, file: usize, m: &str, id: &str, me: usize, c: &Concept) {
        let locals = c.params.as_ref().map(|p| lean::binders(&p.code)).unwrap_or_default();
        if let Some(p) = &c.params {
            self.scan_lean(file, p, m, &locals, me);
        }
        if let Some(t) = &c.ty {
            self.scan_lean(file, t, m, &locals, me);
        }
        if c.variants.is_empty() {
            let msg = format!("concept `{}` has no variant", c.name.text);
            self.diags.push(Diagnostic::error(file, c.name.span, msg));
        }
        for v in &c.variants {
            let Some(&vs) = self.by_id.get(&format!("{id}.{}", v.name.text)) else {
                continue;
            };
            match &v.def {
                Some(def) => self.scan_lean(file, def, m, &locals, vs),
                None => {
                    let msg = format!("variant `{}` has no definition", v.name.text);
                    self.diags.push(Diagnostic::error(file, v.name.span, msg));
                }
            }
        }
        for b in &c.bridges {
            for side in [&b.left, &b.right] {
                match self.by_id.get(&format!("{id}.{}", side.text)) {
                    Some(&s) => {
                        self.refs.insert((file, side.span), Target::Symbol(s));
                    }
                    None => {
                        let msg = format!("`{}` is not a variant of `{}`", side.text, c.name.text);
                        self.diags.push(Diagnostic::error(file, side.span, msg));
                    }
                }
            }
            if b.left.text == b.right.text {
                self.diags.push(Diagnostic::error(file, b.span, "a bridge needs two different variants"));
            }
            // Statements are equivalent, objects equal.
            match (c.ty.is_some(), b.eq) {
                (true, false) => {
                    let msg = format!(
                        "`{}` defines an object: write the bridge as an equality, `{} = {}`",
                        c.name.text, b.left.text, b.right.text
                    );
                    self.diags.push(Diagnostic::error(file, b.span, msg));
                }
                (false, true) => {
                    let msg = format!(
                        "`{}` is a statement: write the bridge as an equivalence, `{} <-> {}` (or give the concept a type)",
                        c.name.text, b.left.text, b.right.text
                    );
                    self.diags.push(Diagnostic::error(file, b.span, msg));
                }
                _ => {}
            }
            let op = if b.eq { "=" } else { "<->" };
            match &b.proof {
                Some(p) => self.scan_lean(file, p, m, &locals, me),
                None => {
                    let msg = format!("the proof of the bridge {} {op} {} is OPEN", b.left.text, b.right.text);
                    self.diags.push(Diagnostic::warning(file, b.span, msg));
                }
            }
        }
    }

    fn resolve_norm(&mut self, file: usize, m: &str, id: &str, n: &Norm) {
        let from = n.valid_from.as_ref().and_then(|(d, span)| self.check_date(file, *span, d));
        let to = n.valid_to.as_ref().and_then(|(d, span)| self.check_date(file, *span, d));
        if let (Some(f), Some(t), Some((_, span))) = (from, to, &n.valid_to)
            && t < f
        {
            self.diags.push(Diagnostic::error(file, *span, "a norm cannot end before it begins"));
        }
        // An earlier version: a reference, but not a dependency – a path need not introduce the old law first.
        if let Some(p) = &n.replaces
            && let Some(&me) = self.by_id.get(id)
        {
            match self.lookup(&p.joined(), &[m]) {
                Some(old) if self.symbols[old].kind == SymKind::Norm && old != me => {
                    self.refs.insert((file, p.span), Target::Symbol(old));
                    if let Some(&other) = self.replaced_by.get(&old) {
                        let msg = format!(
                            "`{}` is already replaced by `{}`: a version has one successor",
                            p.joined(),
                            self.symbols[other].id
                        );
                        self.diags.push(Diagnostic::error(file, p.span, msg));
                    } else {
                        self.replaces.insert(me, old);
                        self.replaced_by.insert(old, me);
                    }
                }
                Some(_) => {
                    let msg = format!("`{}` is not another norm: only a norm can replace a norm", p.joined());
                    self.diags.push(Diagnostic::error(file, p.span, msg));
                }
                None => {
                    self.diags.push(Diagnostic::error(file, p.span, format!("cannot find `{}`", p.joined())));
                }
            }
        }
        for mem in &n.members {
            let Some(&s) = self.by_id.get(&format!("{id}.{}", mem.name.text)) else {
                continue;
            };
            let ty = self.member_types.get(&s).copied().unwrap_or(Ty::Unknown);
            let scopes = [id, m];
            match &mem.kind {
                MemberKind::Input => {}
                MemberKind::Judgement { criteria } => {
                    if criteria.is_empty() {
                        let msg = format!(
                            "judgement `{}` has no criteria: say by what it is judged (`criteria de \"…\"`)",
                            mem.name.text
                        );
                        self.diags.push(Diagnostic::warning(file, mem.name.span, msg));
                    }
                }
                MemberKind::Constant(e) => {
                    let got = self.expr(file, e, &scopes, s);
                    self.expect(file, e.span, got, ty);
                }
                MemberKind::Rule { default, unless } => {
                    match default {
                        Some(e) => {
                            let got = self.expr(file, e, &scopes, s);
                            self.expect(file, e.span, got, ty);
                        }
                        None => {
                            let msg = format!("rule `{}` has no default", mem.name.text);
                            self.diags.push(Diagnostic::error(file, mem.name.span, msg));
                        }
                    }
                    for u in unless {
                        let c = self.expr(file, &u.cond, &scopes, s);
                        self.expect(file, u.cond.span, c, Ty::Bool);
                        let v = self.expr(file, &u.value, &scopes, s);
                        self.expect(file, u.value.span, v, ty);
                    }
                }
            }
        }
    }

    /// The day number of a date, or an error at `span`.
    fn check_date(&mut self, file: usize, span: Span, d: &str) -> Option<i64> {
        let n = crate::date::day_number(d);
        if n.is_none() {
            self.diags.push(Diagnostic::error(file, span, format!("`{d}` is not a date")));
        }
        n
    }

    /// Versions of a provision follow each other without gap or overlap: the earlier one ends the day before the
    /// later one begins.
    fn check_versions(&mut self, ws: &Sources) {
        let norm = |a: &Analysis, s: usize| match a.symbols[s].loc {
            Loc::Item { file, item } => match &ws.asts[file].items[item] {
                Item::Norm(n) => Some((file, n)),
                _ => None,
            },
            _ => None,
        };
        let day = |d: &Option<(String, Span)>| d.as_ref().and_then(|(d, _)| crate::date::day_number(d));
        let mut pairs: Vec<(usize, usize)> = self.replaces.iter().map(|(&n, &o)| (n, o)).collect();
        pairs.sort();
        for (new, old) in pairs {
            let (Some((file, n)), Some((_, o))) = (norm(self, new), norm(self, old)) else {
                continue;
            };
            let span = n.replaces.as_ref().map(|p| p.span).unwrap_or(n.name.span);
            // A chain that comes back to itself.
            let mut seen = HashSet::from([new]);
            let mut at = old;
            let mut cycle = false;
            while let Some(&next) = self.replaces.get(&at) {
                if !seen.insert(at) || next == new {
                    cycle = true;
                    break;
                }
                at = next;
            }
            if cycle {
                let msg = format!("the versions of `{}` replace each other in a circle", n.name.text);
                self.diags.push(Diagnostic::error(file, span, msg));
                continue;
            }
            let Some(begins) = day(&n.valid_from) else {
                let msg =
                    format!("`{}` replaces `{}`: say from when it applies (`valid from …`)", n.name.text, o.name.text);
                self.diags.push(Diagnostic::error(file, n.name.span, msg));
                continue;
            };
            let expected = crate::date::from_day_number(begins - 1);
            match day(&o.valid_to) {
                None => {
                    let msg = format!(
                        "`{}` replaces `{}`, which has no end: it needs `valid … to {expected}`",
                        n.name.text, o.name.text
                    );
                    self.diags.push(Diagnostic::error(file, span, msg));
                }
                Some(ends) if ends >= begins => {
                    let msg = format!(
                        "`{}` begins before `{}` ends: the versions overlap (the earlier one should end {expected})",
                        n.name.text, o.name.text
                    );
                    self.diags.push(Diagnostic::error(file, span, msg));
                }
                Some(ends) if ends + 1 < begins => {
                    let msg = format!(
                        "between `{}` and `{}` no version applies from {} to {expected}",
                        o.name.text,
                        n.name.text,
                        crate::date::from_day_number(ends + 1)
                    );
                    self.diags.push(Diagnostic::warning(file, span, msg));
                }
                Some(_) => {}
            }
        }
    }

    /// The judgements (undetermined legal terms) a rule or value depends on, directly or through other rules – what
    /// someone has to assess before its result is known. In the order they are declared.
    pub fn judgements_behind(&self, sym: usize) -> Vec<usize> {
        let mut seen = HashSet::from([sym]);
        let mut stack = vec![sym];
        let mut found = Vec::new();
        while let Some(s) = stack.pop() {
            for &d in &self.deps[s] {
                if seen.insert(d) {
                    if self.symbols[d].kind == SymKind::Judgement {
                        found.push(d);
                    } else if matches!(self.symbols[d].kind, SymKind::Rule | SymKind::Constant) {
                        stack.push(d);
                    }
                }
            }
        }
        found.sort_by_key(|&j| (self.symbols[j].loc.file(), self.symbols[j].span.start));
        found
    }

    /// The texts of a declaration as written at it, for a field: labels, the texts of a remark, the criteria of a
    /// judgement.
    pub fn own_texts<'a>(&self, ws: &'a Sources, sym: usize, field: Field) -> &'a [Text] {
        match field {
            Field::Label => &self.meta(ws, sym).labels,
            Field::Text => match self.symbols[sym].loc {
                Loc::Item { file, item } => match &ws.asts[file].items[item] {
                    Item::Remark(r) => &r.texts,
                    _ => &[],
                },
                _ => &[],
            },
            Field::Criteria => match self.symbols[sym].loc {
                Loc::Member { file, item, member } => match &ws.asts[file].items[item] {
                    Item::Norm(n) => match &n.members[member].kind {
                        MemberKind::Judgement { criteria } => criteria,
                        _ => &[],
                    },
                    _ => &[],
                },
                _ => &[],
            },
        }
    }

    /// A text of a declaration in `lang`: as written at it, else from a translation, else in English, else the first
    /// one written.
    pub fn text_in(&self, ws: &Sources, sym: usize, field: Field, lang: &str) -> Option<String> {
        let own = self.own_texts(ws, sym, field);
        let written = |l: &str| own.iter().find(|t| t.lang.text == l).map(|t| t.value.clone());
        let translated = |l: &str| self.translations.get(&(sym, field, l.to_string())).cloned();
        written(lang)
            .or_else(|| translated(lang))
            .or_else(|| written("en"))
            .or_else(|| translated("en"))
            .or_else(|| own.first().map(|t| t.value.clone()))
    }

    /// The label of a declaration in `lang` (see [`Analysis::text_in`]).
    pub fn label(&self, ws: &Sources, sym: usize, lang: &str) -> Option<String> {
        self.text_in(ws, sym, Field::Label, lang)
    }

    /// For each module and language that a translation file names: how many of the module's texts – written at its
    /// declarations – the language has.
    pub fn coverage(&self, ws: &Sources) -> std::collections::BTreeMap<(String, String), Coverage> {
        let mut pairs = BTreeSet::new();
        for ast in &ws.asts {
            let Some(m) = &ast.translation else { continue };
            for t in &ast.translated {
                for text in t.labels.iter().chain(&t.texts).chain(&t.criteria) {
                    pairs.insert((m.joined(), text.lang.text.clone()));
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        for (module, lang) in pairs {
            let mut c = Coverage::default();
            let prefix = format!("{module}.");
            for sym in (0..self.symbols.len()).filter(|&s| self.symbols[s].id.starts_with(&prefix)) {
                for field in [Field::Label, Field::Text, Field::Criteria] {
                    let own = self.own_texts(ws, sym, field);
                    if own.is_empty() {
                        continue;
                    }
                    c.total += 1;
                    if own.iter().any(|t| t.lang.text == lang)
                        || self.translations.contains_key(&(sym, field, lang.clone()))
                    {
                        c.translated += 1;
                    } else {
                        c.missing.push((self.symbols[sym].id.clone(), field));
                    }
                }
            }
            out.insert((module, lang), c);
        }
        out
    }

    /// Original languages and translation files: texts at declarations in the original languages only (where a
    /// module names them); translations of declarations that exist, of fields they have, once per language, not in
    /// an original language.
    fn check_translations(&mut self, ws: &Sources) {
        for ast in &ws.asts {
            if let Some(m) = &ast.module
                && !ast.originals.is_empty()
            {
                self.originals.insert(m.joined(), ast.originals.iter().map(|n| n.text.clone()).collect());
            }
        }
        // Texts at declarations in a language that is not an original one.
        let mut stray = Vec::new();
        for sym in 0..self.symbols.len() {
            let file = self.symbols[sym].loc.file();
            let Some(module) = &ws.asts[file].module else { continue };
            let Some(originals) = self.originals.get(&module.joined()) else { continue };
            for field in [Field::Label, Field::Text, Field::Criteria] {
                for t in self.own_texts(ws, sym, field) {
                    if !originals.contains(&t.lang.text) {
                        let msg = format!(
                            "`{}` is not an original language of `{}` (original {}): this text belongs in a translation",
                            t.lang.text,
                            module.joined(),
                            originals.join(", ")
                        );
                        stray.push(Diagnostic::warning(file, t.lang.span, msg));
                    }
                }
            }
        }
        self.diags.extend(stray);
        // Translation files.
        for (file, ast) in ws.asts.iter().enumerate() {
            let Some(of) = &ast.translation else { continue };
            let m = of.joined();
            if !ws.asts.iter().any(|a| a.module.as_ref().is_some_and(|x| x.joined() == m)) {
                self.diags.push(Diagnostic::error(file, of.span, format!("there is no module `{m}` to translate")));
                continue;
            }
            let originals = self.originals.get(&m).cloned().unwrap_or_default();
            for t in &ast.translated {
                let id = format!("{m}.{}", t.target.joined());
                let Some(&sym) = self.by_id.get(&id) else {
                    let msg = format!("there is no `{}` in `{m}`", t.target.joined());
                    self.diags.push(Diagnostic::error(file, t.target.span, msg));
                    continue;
                };
                self.refs.insert((file, t.target.span), Target::Symbol(sym));
                for (field, texts) in
                    [(Field::Label, &t.labels), (Field::Text, &t.texts), (Field::Criteria, &t.criteria)]
                {
                    for text in texts {
                        if field != Field::Label && self.own_texts(ws, sym, field).is_empty() {
                            let msg = format!(
                                "`{id}` has no {} to translate ({} belongs to {})",
                                field.keyword(),
                                field.keyword(),
                                if field == Field::Text { "remarks" } else { "judgements" }
                            );
                            self.diags.push(Diagnostic::error(file, text.lang.span, msg));
                            continue;
                        }
                        if originals.contains(&text.lang.text) {
                            let msg = format!(
                                "`{}` is an original language of `{m}`: write this text at the declaration",
                                text.lang.text
                            );
                            self.diags.push(Diagnostic::warning(file, text.lang.span, msg));
                        }
                        let key = (sym, field, text.lang.text.clone());
                        if self.translations.insert(key, text.value.clone()).is_some() {
                            let msg = format!("`{id}` has its {} in `{}` twice", field.keyword(), text.lang.text);
                            self.diags.push(Diagnostic::error(file, text.lang.span, msg));
                        }
                    }
                }
            }
        }
    }

    /// Every version of the provision a norm belongs to, the earliest first.
    pub fn versions(&self, norm: usize) -> Vec<usize> {
        let mut first = norm;
        let mut seen = HashSet::from([norm]);
        while let Some(&old) = self.replaces.get(&first) {
            if !seen.insert(old) {
                break;
            }
            first = old;
        }
        let mut out = vec![first];
        while let Some(&new) = self.replaced_by.get(out.last().unwrap()) {
            if out.contains(&new) {
                break;
            }
            out.push(new);
        }
        out
    }

    /// The version of the provision a norm belongs to that applies on a date (`YYYY-MM-DD`): the one whose
    /// `valid from … to …` contains it (an open bound counts as unbounded).
    pub fn in_force(&self, ws: &Sources, norm: usize, date: &str) -> Option<usize> {
        let d = crate::date::day_number(date)?;
        let day = |v: &Option<(String, Span)>| v.as_ref().and_then(|(s, _)| crate::date::day_number(s));
        self.versions(norm).into_iter().find(|&v| {
            let Loc::Item { file, item } = self.symbols[v].loc else { return false };
            let Item::Norm(n) = &ws.asts[file].items[item] else { return false };
            day(&n.valid_from).is_none_or(|f| f <= d) && day(&n.valid_to).is_none_or(|t| d <= t)
        })
    }

    fn scan_lean(&mut self, file: usize, l: &Lean, module: &str, locals: &[(String, String)], owner: usize) {
        let mut refs = Vec::new();
        for (range, word) in lean::idents(&l.code) {
            let target = if let Some((name, ty)) = locals.iter().find(|(n, _)| n == word) {
                Target::Local { name: name.clone(), ty: ty.clone() }
            } else if let Some(s) = self.lookup(word, &[module]) {
                self.deps[owner].insert(s);
                Target::Symbol(s)
            } else if word.contains('.') && word.starts_with(char::is_uppercase) {
                Target::External(word.to_string())
            } else {
                continue;
            };
            refs.push(LeanRef { range, target });
        }
        self.lean_refs.insert((file, l.span), refs);
    }

    fn resolve_path(&mut self, file: usize, p: &Path, scopes: &[&str], owner: usize) -> Option<usize> {
        let joined = p.joined();
        match self.lookup(&joined, scopes) {
            Some(s) => {
                self.refs.insert((file, p.span), Target::Symbol(s));
                self.deps[owner].insert(s);
                Some(s)
            }
            None => {
                self.diags.push(Diagnostic::error(file, p.span, format!("cannot find `{joined}`")));
                None
            }
        }
    }

    // ---- types of the legal profile ----

    fn expect(&mut self, file: usize, span: Span, got: Ty, want: Ty) {
        if got != Ty::Unknown && want != Ty::Unknown && Ty::unify(got, want) != Some(want) {
            let msg = format!("expected {}, found {}", self.ty_name(want), self.ty_name(got));
            self.diags.push(Diagnostic::error(file, span, msg));
        }
    }

    fn mismatch(&mut self, file: usize, span: Span, msg: String) -> Ty {
        self.diags.push(Diagnostic::error(file, span, msg));
        Ty::Unknown
    }

    /// The type of an expression – recorded, for Lean.
    fn expr(&mut self, file: usize, e: &Expr, scopes: &[&str], owner: usize) -> Ty {
        let t = self.expr_kind(file, e, scopes, owner);
        self.expr_types.insert((file, e.span), t);
        t
    }

    fn expr_kind(&mut self, file: usize, e: &Expr, scopes: &[&str], owner: usize) -> Ty {
        match &e.kind {
            ExprKind::Number(_) => Ty::Num,
            ExprKind::Percent(_) => Ty::Decimal,
            ExprKind::Money(_, unit) => {
                if CURRENCIES.contains(&unit.text.as_str()) {
                    return Ty::Money;
                }
                // A unit by its name here, or – units being few – by its short name in any module, if only one has it.
                let units: Vec<usize> = self
                    .lookup(&unit.text, scopes)
                    .filter(|&s| self.symbols[s].kind == SymKind::Unit)
                    .map(|s| vec![s])
                    .unwrap_or_else(|| {
                        let suffix = format!(".{}", unit.text);
                        (0..self.symbols.len())
                            .filter(|&s| self.symbols[s].kind == SymKind::Unit && self.symbols[s].id.ends_with(&suffix))
                            .collect()
                    });
                if units.len() > 1 {
                    let ids: Vec<&str> = units.iter().map(|&s| self.symbols[s].id.as_str()).collect();
                    let msg = format!("`{}` is a unit of several modules: {}", unit.text, ids.join(", "));
                    return self.mismatch(file, unit.span, msg);
                }
                match units.first().copied() {
                    Some(s) => {
                        self.refs.insert((file, unit.span), Target::Symbol(s));
                        self.deps[owner].insert(s);
                        Ty::Quantity(s)
                    }
                    _ => {
                        let msg = format!(
                            "unknown currency or unit `{}` (currencies: {}; units are declared: `unit {}`)",
                            unit.text,
                            CURRENCIES.join(", "),
                            unit.text
                        );
                        self.mismatch(file, unit.span, msg)
                    }
                }
            }
            ExprKind::Bool(_) => Ty::Bool,
            ExprKind::Date(d) => {
                self.check_date(file, e.span, d);
                Ty::Date
            }
            ExprKind::Error => Ty::Unknown,
            ExprKind::Paren(inner) => self.expr(file, inner, scopes, owner),
            ExprKind::Ref(p) => self.reference(file, p, scopes, owner),
            ExprKind::Call(p, args) => {
                let tys: Vec<Ty> = args.iter().map(|x| self.expr(file, x, scopes, owner)).collect();
                let name = p.joined();
                if !BUILTINS.contains(&name.as_str()) {
                    let msg = if AGGREGATES.contains(&name.as_str()) {
                        format!("`{name}` goes over a list: `{name}(x.amount for x in list)`")
                    } else {
                        format!("unknown function `{name}` (known: {})", BUILTINS.join(", "))
                    };
                    return self.mismatch(file, p.span, msg);
                }
                self.refs.insert((file, p.span), Target::Builtin(name.clone()));
                self.call(file, e, &name, args, &tys)
            }
            ExprKind::Aggregate(agg) => self.aggregate(file, agg, scopes, owner),
            ExprKind::Apply(p, args) => self.apply(file, e, p, args, scopes, owner),
            ExprKind::Table(t) => self.table(file, t, scopes, owner),
            ExprKind::Unary(UnOp::Not, x) => {
                let t = self.expr(file, x, scopes, owner);
                self.expect(file, x.span, t, Ty::Bool);
                Ty::Bool
            }
            ExprKind::Unary(UnOp::Neg, x) => {
                let t = self.expr(file, x, scopes, owner);
                if !t.is_numeric() {
                    return self.mismatch(file, x.span, format!("cannot negate {}", self.ty_name(t)));
                }
                t
            }
            ExprKind::If(c, x, y) => {
                let tc = self.expr(file, c, scopes, owner);
                self.expect(file, c.span, tc, Ty::Bool);
                let tx = self.expr(file, x, scopes, owner);
                let ty = self.expr(file, y, scopes, owner);
                Ty::unify(tx, ty).unwrap_or_else(|| {
                    let msg = format!("the branches differ: {} and {}", self.ty_name(tx), self.ty_name(ty));
                    self.mismatch(file, e.span, msg)
                })
            }
            ExprKind::Binary(op, l, r) => {
                let tl = self.expr(file, l, scopes, owner);
                let tr = self.expr(file, r, scopes, owner);
                self.binary(file, e.span, *op, tl, tr)
            }
        }
    }

    /// A path to a value: a variable of an aggregate, a member of this norm, a constant of another norm or a value of
    /// an enumeration – then fields of records (`owner.age`, `s.amount`).
    fn reference(&mut self, file: usize, p: &Path, scopes: &[&str], owner: usize) -> Ty {
        let segs = &p.segments;
        let first = &segs[0].text;
        // Where the value comes from, and how many segments that takes.
        let (mut ty, used) = if let Some((name, t)) = self.locals.iter().rev().find(|(n, _)| n == first) {
            let target = Target::Local { name: name.clone(), ty: self.ty_name(*t) };
            self.refs.insert((file, p.span), target);
            (*t, 1)
        } else {
            let found = (1..=segs.len()).rev().find_map(|k| {
                let prefix = segs[..k].iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(".");
                self.lookup(&prefix, scopes).map(|s| (s, k))
            });
            let Some((s, k)) = found else {
                return self.mismatch(file, p.span, format!("cannot find `{}`", p.joined()));
            };
            if self.symbols[s].kind == SymKind::Enum && k < segs.len() {
                let msg = format!("enumeration `{}` has no value `{}`", segs[0].text, segs[k].text);
                return self.mismatch(file, segs[k].span, msg);
            }
            self.refs.insert((file, p.span), Target::Symbol(s));
            self.deps[owner].insert(s);
            let sym = &self.symbols[s];
            let t = match sym.kind {
                SymKind::Value => match self.parent(s) {
                    Some(e) => Ty::Enum(e),
                    None => Ty::Unknown,
                },
                SymKind::Input | SymKind::Judgement | SymKind::Rule
                    if self.parent(s).is_some() && self.parent(s) != self.parent(owner) =>
                {
                    let norm = self.parent(s).map(|n| self.symbols[n].id.clone()).unwrap_or_default();
                    let msg = format!(
                        "`{}` belongs to norm `{norm}` and depends on its inputs: give them, as in `{}(input = …)`",
                        segs[..k].iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("."),
                        segs[..k].iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(".")
                    );
                    return self.mismatch(file, p.span, msg);
                }
                _ => match self.member_types.get(&s) {
                    Some(&t) if sym.kind != SymKind::Field => t,
                    _ => {
                        let kw = sym.kind.keyword();
                        let article = if kw.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
                        let msg = format!("`{}` is {article} {kw}, not a value", p.joined());
                        return self.mismatch(file, p.span, msg);
                    }
                },
            };
            (t, k)
        };
        // Fields.
        let mut fields = Vec::new();
        for seg in &segs[used..] {
            let Ty::Record(r) = ty else {
                if ty == Ty::Unknown {
                    return Ty::Unknown;
                }
                let msg = format!("{} has no field `{}`: only records have fields", self.ty_name(ty), seg.text);
                return self.mismatch(file, seg.span, msg);
            };
            let id = format!("{}.{}", self.symbols[r].id, seg.text);
            let Some(&f) = self.by_id.get(&id) else {
                let msg = format!("record `{}` has no field `{}`", self.ty_name(ty), seg.text);
                return self.mismatch(file, seg.span, msg);
            };
            self.refs.insert((file, seg.span), Target::Symbol(f));
            self.deps[owner].insert(f);
            fields.push(f);
            ty = self.member_types.get(&f).copied().unwrap_or(Ty::Unknown);
        }
        if !fields.is_empty() {
            self.fields.insert((file, p.span), fields);
        }
        ty
    }

    /// A built-in function.
    fn call(&mut self, file: usize, e: &Expr, name: &str, args: &[Expr], tys: &[Ty]) -> Ty {
        if ROUNDING.contains(&name) {
            return self.rounding(file, e, name, args, tys);
        }
        let arity = |a: &mut Self, n: usize, how: &str| {
            if tys.len() != n {
                a.mismatch(file, e.span, format!("`{name}` takes {how}"));
                return false;
            }
            true
        };
        let want = |a: &mut Self, i: usize, t: Ty| a.expect(file, args[i].span, tys[i], t);
        match name {
            "min" | "max" => {
                if tys.is_empty() {
                    return self.mismatch(file, e.span, format!("`{name}` needs at least one argument"));
                }
                let mut t = tys[0];
                for &u in &tys[1..] {
                    match Ty::unify(t, u) {
                        Some(x) if x.is_numeric() || x == Ty::Date => t = x,
                        _ => {
                            let msg = format!("`{name}` needs numbers, amounts or dates of one type");
                            return self.mismatch(file, e.span, msg);
                        }
                    }
                }
                t
            }
            "abs" => {
                if !arity(self, 1, "one number or amount") {
                    return Ty::Unknown;
                }
                if !tys[0].is_numeric() {
                    return self.mismatch(file, args[0].span, format!("`abs` of {}", self.ty_name(tys[0])));
                }
                tys[0]
            }
            "div" | "mod" => {
                if !arity(self, 2, "two whole numbers") {
                    return Ty::Unknown;
                }
                want(self, 0, Ty::Int);
                want(self, 1, Ty::Int);
                Ty::Int
            }
            _ if DATE_PARTS.contains(&name) => {
                if arity(self, 1, "a date") {
                    want(self, 0, Ty::Date);
                }
                Ty::Int
            }
            "date" => {
                if arity(self, 3, "a year, a month and a day") {
                    (0..3).for_each(|i| want(self, i, Ty::Int));
                }
                Ty::Date
            }
            "last_day" => {
                if arity(self, 2, "a year and a month") {
                    (0..2).for_each(|i| want(self, i, Ty::Int));
                }
                Ty::Date
            }
            _ => {
                // add_months, add_years
                if arity(self, 2, "a date and a whole number") {
                    want(self, 0, Ty::Date);
                    want(self, 1, Ty::Int);
                }
                Ty::Date
            }
        }
    }

    /// `sum(<body> for <var> in <list> where <filter>)` and its kin.
    fn aggregate(&mut self, file: usize, agg: &Aggregate, scopes: &[&str], owner: usize) -> Ty {
        let func = agg.func.text.as_str();
        if !AGGREGATES.contains(&func) {
            let msg = format!("unknown aggregate `{func}` (known: {})", AGGREGATES.join(", "));
            return self.mismatch(file, agg.func.span, msg);
        }
        self.refs.insert((file, agg.func.span), Target::Builtin(func.to_string()));
        let lt = self.expr(file, &agg.list, scopes, owner);
        let elem = match lt {
            Ty::List(r) => Ty::Record(r),
            Ty::Unknown => Ty::Unknown,
            other => {
                let msg = format!("`{func}` goes over a list, not {}", self.ty_name(other));
                return self.mismatch(file, agg.list.span, msg);
            }
        };
        self.locals.push((agg.var.text.clone(), elem));
        if let Some(f) = &agg.filter {
            let t = self.expr(file, f, scopes, owner);
            self.expect(file, f.span, t, Ty::Bool);
        }
        let body = agg.body.as_ref().map(|b| (b.span, self.expr(file, b, scopes, owner)));
        self.locals.pop();
        match (func, body) {
            ("count", None) => Ty::Int,
            ("count", Some((span, _))) => {
                self.mismatch(file, span, "`count` counts elements: `count(s in supplies where …)`".into())
            }
            (_, None) => {
                let msg = format!("`{func}` needs what to take of each element: `{func}(<…> for s in list)`");
                self.mismatch(file, agg.func.span, msg)
            }
            ("sum", Some((span, t))) => {
                if !t.is_numeric() {
                    return self.mismatch(file, span, format!("cannot sum {}", self.ty_name(t)));
                }
                if t == Ty::Num { Ty::Int } else { t }
            }
            (_, Some((span, t))) => {
                self.expect(file, span, t, Ty::Bool);
                Ty::Bool
            }
        }
    }

    /// A rule or constant of a norm for inputs given by name – every input of that norm, once.
    fn apply(&mut self, file: usize, e: &Expr, p: &Path, args: &[(Name, Expr)], scopes: &[&str], owner: usize) -> Ty {
        let arg_tys: Vec<Ty> = args.iter().map(|(_, x)| self.expr(file, x, scopes, owner)).collect();
        let Some(s) = self.lookup(&p.joined(), scopes) else {
            return self.mismatch(file, p.span, format!("cannot find `{}`", p.joined()));
        };
        let kind = self.symbols[s].kind;
        let Some(norm) = self.parent(s).filter(|_| matches!(kind, SymKind::Rule | SymKind::Constant)) else {
            let msg =
                format!("`{}` is a {}: only a rule or constant of a norm takes inputs", p.joined(), kind.keyword());
            return self.mismatch(file, p.span, msg);
        };
        if s == owner {
            return self.mismatch(file, p.span, format!("`{}` cannot use itself", p.joined()));
        }
        self.refs.insert((file, p.span), Target::Symbol(s));
        self.deps[owner].insert(s);
        let nid = self.symbols[norm].id.clone();
        let inputs: Vec<usize> = (0..self.symbols.len())
            .filter(|&m| {
                self.parent(m) == Some(norm) && matches!(self.symbols[m].kind, SymKind::Input | SymKind::Judgement)
            })
            .collect();
        let mut given = HashSet::new();
        for ((name, x), t) in args.iter().zip(arg_tys) {
            let Some(&i) = self.by_id.get(&format!("{nid}.{}", name.text)).filter(|i| inputs.contains(i)) else {
                let msg = format!("norm `{nid}` has no input `{}`", name.text);
                self.mismatch(file, name.span, msg);
                continue;
            };
            if !given.insert(i) {
                self.mismatch(file, name.span, format!("input `{}` is given twice", name.text));
                continue;
            }
            self.refs.insert((file, name.span), Target::Symbol(i));
            let want = self.member_types.get(&i).copied().unwrap_or(Ty::Unknown);
            self.expect(file, x.span, t, want);
        }
        let missing: Vec<String> = inputs
            .iter()
            .filter(|i| !given.contains(i))
            .map(|&i| self.symbols[i].id.rsplit('.').next().unwrap_or_default().to_string())
            .collect();
        if !missing.is_empty() {
            let msg = format!("`{}` needs every input of `{nid}`; missing: {}", p.joined(), missing.join(", "));
            self.mismatch(file, e.span, msg);
        }
        self.member_types.get(&s).copied().unwrap_or(Ty::Unknown)
    }

    /// A table: rows by the values of an enumeration, or by ranges `up to` ascending bounds with `otherwise` above.
    fn table(&mut self, file: usize, t: &Table, scopes: &[&str], owner: usize) -> Ty {
        let st = self.expr(file, &t.subject, scopes, owner);
        let mut result = Ty::Unknown;
        let mut unify = |a: &mut Self, span: Span, v: Ty| match Ty::unify(result, v) {
            Some(x) => result = x,
            None => {
                let msg = format!("the rows give different types: {} and {}", a.ty_name(result), a.ty_name(v));
                a.mismatch(file, span, msg);
            }
        };
        match st {
            Ty::Enum(en) => {
                let mut seen = HashSet::new();
                for row in &t.rows {
                    let RowKey::Value(key) = &row.key else {
                        self.mismatch(file, row.span, "rows by an enumeration name its values, not `up to`".into());
                        continue;
                    };
                    let enum_id = self.symbols[en].id.clone();
                    let found = self
                        .by_id
                        .get(&format!("{enum_id}.{}", key.joined()))
                        .copied()
                        .or_else(|| self.lookup(&key.joined(), scopes).filter(|&v| self.parent(v) == Some(en)));
                    match found {
                        Some(v) if self.symbols[v].kind == SymKind::Value => {
                            self.refs.insert((file, key.span), Target::Symbol(v));
                            if !seen.insert(v) {
                                self.mismatch(file, key.span, format!("`{}` has a row already", key.joined()));
                            }
                        }
                        _ => {
                            let msg = format!("`{}` is not a value of `{}`", key.joined(), self.ty_name(st));
                            self.mismatch(file, key.span, msg);
                        }
                    }
                }
                let all: Vec<usize> = (0..self.symbols.len())
                    .filter(|&v| self.symbols[v].kind == SymKind::Value && self.parent(v) == Some(en))
                    .collect();
                let missing: Vec<String> = all
                    .iter()
                    .filter(|v| !seen.contains(v))
                    .map(|&v| self.symbols[v].id.rsplit('.').next().unwrap_or_default().to_string())
                    .collect();
                if t.otherwise.is_none() && !missing.is_empty() {
                    let msg = format!("the table has no row for {} – add them or `otherwise`", missing.join(", "));
                    self.mismatch(file, t.subject.span, msg);
                }
                if t.otherwise.is_some() && missing.is_empty() {
                    let msg = "every value has a row: `otherwise` is never used".to_string();
                    self.diags.push(Diagnostic::warning(file, t.subject.span, msg));
                }
            }
            Ty::Unknown => {}
            s if s.is_numeric() || s == Ty::Date => {
                let mut last: Option<(crate::eval::Rat, Span)> = None;
                for row in &t.rows {
                    let RowKey::UpTo(bound) = &row.key else {
                        let msg = format!("rows by {} are ranges: `up to <bound>: <value>`", self.ty_name(s));
                        self.mismatch(file, row.span, msg);
                        continue;
                    };
                    let bt = self.expr(file, bound, scopes, owner);
                    self.expect(file, bound.span, bt, s);
                    if let Some(v) = crate::eval::literal(bound) {
                        if let Some((prev, _)) = last
                            && v <= prev
                        {
                            self.mismatch(file, bound.span, "the bounds of a table go up: each above the last".into());
                        }
                        last = Some((v, bound.span));
                    }
                }
                if t.otherwise.is_none() {
                    let msg = "a table by ranges needs `otherwise` for what lies above the last bound".to_string();
                    self.mismatch(file, t.subject.span, msg);
                }
            }
            other => {
                let msg = format!("a table goes by a number, amount, date or enumeration, not {}", self.ty_name(other));
                self.mismatch(file, t.subject.span, msg);
            }
        }
        for row in &t.rows {
            let v = self.expr(file, &row.value, scopes, owner);
            unify(self, row.value.span, v);
        }
        if let Some(o) = &t.otherwise {
            let v = self.expr(file, o, scopes, owner);
            unify(self, o.span, v);
        }
        result
    }

    fn binary(&mut self, file: usize, span: Span, op: BinOp, l: Ty, r: Ty) -> Ty {
        use BinOp::*;
        let both = |a: &Self| format!("{} and {}", a.ty_name(l), a.ty_name(r));
        match op {
            And | Or => {
                self.expect(file, span, l, Ty::Bool);
                self.expect(file, span, r, Ty::Bool);
                Ty::Bool
            }
            Eq | Ne => {
                match Ty::unify(l, r) {
                    Some(Ty::Record(_) | Ty::List(_)) => {
                        self.mismatch(file, span, "records and lists are compared field by field".into());
                    }
                    Some(_) => {}
                    None => {
                        let msg = format!("cannot compare {}", both(self));
                        self.mismatch(file, span, msg);
                    }
                }
                Ty::Bool
            }
            Lt | Le | Gt | Ge => match Ty::unify(l, r) {
                Some(t) if t.is_numeric() || t == Ty::Date => Ty::Bool,
                _ => {
                    let msg = format!("cannot order {}", both(self));
                    self.mismatch(file, span, msg);
                    Ty::Bool
                }
            },
            // Days after or before a date, and the days between two dates.
            Add if l == Ty::Date && r.is_integral() || r == Ty::Date && l.is_integral() => Ty::Date,
            Sub if l == Ty::Date && r.is_integral() => Ty::Date,
            Sub if l == Ty::Date && r == Ty::Date => Ty::Int,
            Add | Sub => match Ty::unify(l, r) {
                Some(t) if t.is_numeric() => t,
                _ if l.is_amount() != r.is_amount() => {
                    let msg = format!("cannot combine {} – write amounts with their unit, e.g. `5 EUR`", both(self));
                    self.mismatch(file, span, msg)
                }
                _ => {
                    let msg = format!("cannot add or subtract {}", both(self));
                    self.mismatch(file, span, msg)
                }
            },
            Mul => match (l, r) {
                (a, s) | (s, a) if a.is_amount() && s.is_scalar() => a,
                _ if l.is_amount() && r.is_amount() => {
                    let msg = format!("cannot multiply {} – divide an amount by its unit first", both(self));
                    self.mismatch(file, span, msg)
                }
                _ => match Ty::unify(l, r) {
                    Some(t) if t.is_scalar() => t,
                    _ => {
                        let msg = format!("cannot multiply {}", both(self));
                        self.mismatch(file, span, msg)
                    }
                },
            },
            Div => match (l, r) {
                (a, s) if a.is_amount() && s.is_scalar() => a,
                (a, b) if a.is_amount() && a == b => Ty::Decimal,
                _ => match Ty::unify(l, r) {
                    Some(t) if t.is_scalar() => Ty::Decimal,
                    _ => {
                        let msg = format!("cannot divide {}", both(self));
                        self.mismatch(file, span, msg)
                    }
                },
            },
        }
    }

    /// `round_down(x, step)` and its kin: `x` a number or an amount, `step` a positive literal of the same kind
    /// (`1 EUR`, `0.01 EUR`, `1`) – the law rounds to fixed steps.
    fn rounding(&mut self, file: usize, e: &Expr, name: &str, args: &[Expr], tys: &[Ty]) -> Ty {
        let [x, step] = args else {
            return self.mismatch(file, e.span, format!("`{name}` takes a value and a step, e.g. `{name}(x, 1 EUR)`"));
        };
        let positive = crate::eval::literal(step).is_some_and(|v| v > crate::eval::Rat::int(0));
        if !positive {
            let msg = format!("the step of `{name}` must be a positive number or amount written out, e.g. `1 EUR`");
            return self.mismatch(file, step.span, msg);
        }
        match Ty::unify(tys[0], tys[1]) {
            Some(t) if t.is_numeric() => t,
            _ => {
                let msg = format!("`{name}` rounds {} to a step of the same kind", self.ty_name(tys[0]));
                self.mismatch(file, x.span.to(step.span), msg)
            }
        }
    }

    // ---- 3. paths ----

    fn check_paths(&mut self, ws: &Sources) {
        for (file, ast) in ws.asts.iter().enumerate() {
            for it in &ast.items {
                if let Item::Path(d) = it {
                    self.check_path(ws, file, d);
                }
            }
        }
    }

    fn check_path(&mut self, ws: &Sources, file: usize, d: &PathDecl) {
        let mut introduced: HashSet<usize> = HashSet::new();
        for step in &d.steps {
            let Some(Target::Symbol(s)) = self.refs.get(&(file, step.span)).cloned() else {
                continue;
            };
            let parent = self.parent(s);
            let mut missing = Vec::new();
            for &dep in &self.deps[s] {
                if dep == s || Some(dep) == parent || !self.symbols[dep].kind.is_ordered() {
                    continue;
                }
                if !self.is_introduced(ws, dep, &introduced) {
                    missing.push(self.symbols[dep].id.clone());
                }
            }
            for id in missing {
                let msg = format!("`{}` uses `{id}`, which this path has not introduced yet", step.joined());
                self.diags.push(Diagnostic::warning(file, step.span, msg));
            }
            match self.symbols[s].kind {
                SymKind::Variant => {
                    let concept = parent.expect("a variant has a concept");
                    let earlier: Vec<usize> = introduced
                        .iter()
                        .copied()
                        .filter(|&v| self.symbols[v].kind == SymKind::Variant && self.parent(v) == Some(concept))
                        .collect();
                    if !earlier.is_empty() && !earlier.iter().any(|&v| self.bridged(ws, v, s)) {
                        let msg = format!(
                            "`{}` is a second definition of `{}` without a bridge to the one introduced before",
                            step.joined(),
                            self.symbols[concept].id
                        );
                        self.diags.push(Diagnostic::warning(file, step.span, msg));
                    }
                    introduced.insert(concept);
                    introduced.insert(s);
                }
                SymKind::Concept => {
                    let variants = self.variants(s);
                    if variants.len() > 1 {
                        let msg = format!(
                            "`{}` has several variants – choose one, e.g. `{}.{}`",
                            step.joined(),
                            step.joined(),
                            self.symbols[variants[0]].id.rsplit('.').next().unwrap_or_default()
                        );
                        self.diags.push(Diagnostic::warning(file, step.span, msg));
                    }
                    introduced.insert(s);
                    introduced.extend(variants.first());
                }
                // A whole norm brings its inputs, values and rules.
                SymKind::Norm => {
                    introduced.insert(s);
                    introduced.extend((0..self.symbols.len()).filter(|&m| self.parent(m) == Some(s)));
                }
                _ => {
                    introduced.insert(s);
                }
            }
        }
    }

    fn variants(&self, concept: usize) -> Vec<usize> {
        let prefix = format!("{}.", self.symbols[concept].id);
        let mut v: Vec<usize> = (0..self.symbols.len())
            .filter(|&i| self.symbols[i].kind == SymKind::Variant && self.symbols[i].id.starts_with(&prefix))
            .collect();
        v.sort_by_key(|&i| self.symbols[i].span.start);
        v
    }

    fn is_introduced(&self, ws: &Sources, dep: usize, introduced: &HashSet<usize>) -> bool {
        if introduced.contains(&dep) {
            return true;
        }
        // Another variant of the same concept counts if a bridge connects them.
        self.symbols[dep].kind == SymKind::Variant
            && introduced.iter().any(|&v| {
                self.symbols[v].kind == SymKind::Variant
                    && self.parent(v) == self.parent(dep)
                    && self.bridged(ws, v, dep)
            })
    }

    /// Whether a concept – or the concept of a variant – defines an object (has a type) rather than a statement.
    pub fn defines_object(&self, ws: &Sources, sym: usize) -> bool {
        let concept = if self.symbols[sym].kind == SymKind::Variant { self.parent(sym) } else { Some(sym) };
        let Some(Loc::Item { file, item }) = concept.map(|c| self.symbols[c].loc) else {
            return false;
        };
        matches!(&ws.asts[file].items[item], Item::Concept(c) if c.ty.is_some())
    }

    /// Whether bridges connect two different variants of one concept – directly or through other variants.
    pub fn bridged(&self, ws: &Sources, a: usize, b: usize) -> bool {
        self.bridge_route(ws, a, b).is_some_and(|r| !r.is_empty())
    }

    /// The shortest chain of bridges from variant `a` to variant `b` of one concept (empty if `a` is `b`); `None`
    /// if no chain connects them. Among chains of the same length, the bridges written first win.
    pub fn bridge_route(&self, ws: &Sources, a: usize, b: usize) -> Option<Vec<Hop>> {
        let (Loc::Variant { file, item, variant: va }, Loc::Variant { file: fb, item: ib, variant: vb }) =
            (self.symbols[a].loc, self.symbols[b].loc)
        else {
            return None;
        };
        if (file, item) != (fb, ib) {
            return None;
        }
        let Item::Concept(c) = &ws.asts[file].items[item] else {
            return None;
        };
        let index = |name: &str| c.variants.iter().position(|v| v.name.text == name);
        // Breadth first over the variants; `back[v]` is the hop that reached `v`.
        let mut back: Vec<Option<(usize, usize, bool)>> = vec![None; c.variants.len()];
        let mut seen = vec![false; c.variants.len()];
        seen[va] = true;
        let mut queue = std::collections::VecDeque::from([va]);
        while let Some(v) = queue.pop_front() {
            if v == vb {
                break;
            }
            for (bridge, br) in c.bridges.iter().enumerate() {
                let (Some(l), Some(r)) = (index(&br.left.text), index(&br.right.text)) else {
                    continue;
                };
                let next = if l == v {
                    Some((r, true))
                } else if r == v {
                    Some((l, false))
                } else {
                    None
                };
                if let Some((n, forward)) = next
                    && !seen[n]
                {
                    seen[n] = true;
                    back[n] = Some((v, bridge, forward));
                    queue.push_back(n);
                }
            }
        }
        if !seen[vb] {
            return None;
        }
        let sym = |v: usize| {
            self.by_id.get(&format!("{}.{}", self.symbols[a].id.rsplit_once('.')?.0, c.variants[v].name.text)).copied()
        };
        let mut hops = Vec::new();
        let mut v = vb;
        while let Some((prev, bridge, forward)) = back[v] {
            hops.push(Hop { from: sym(prev)?, to: sym(v)?, bridge, forward });
            v = prev;
        }
        hops.reverse();
        Some(hops)
    }

    /// The graph as text: every symbol and what it refers to.
    pub fn graph(&self, ws: &Sources, lang: &str) -> String {
        let mut out = String::new();
        for (i, s) in self.symbols.iter().enumerate() {
            let label = self.label(ws, i, lang).map(|l| format!(" – {l}")).unwrap_or_default();
            out.push_str(&format!("{} {}{label}\n", s.kind.keyword(), s.id));
            for &d in &self.deps[i] {
                out.push_str(&format!("    -> {}\n", self.symbols[d].id));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verkodo_syntax::Severity;

    fn run(src: &str) -> (Sources, Analysis) {
        let ws = Sources::from_sources(vec![("t.vk".into(), src.into())]);
        assert!(ws.diags.is_empty(), "{:?}", ws.diags);
        let a = analyze(&ws);
        (ws, a)
    }

    fn messages(a: &Analysis, sev: Severity) -> Vec<String> {
        a.diags.iter().filter(|d| d.severity == sev).map(|d| d.message.clone()).collect()
    }

    const MATH: &str = r#"module m
        concept conv { params lean "(a : ℕ → ℝ) (L : ℝ)"
          variant e { lean "∀ ε > 0, |a 0 - L| < ε" } variant f { lean "Filter.Tendsto a Filter.atTop (nhds L)" }
          bridge e <-> f { proof lean "by simp" } }
        concept bnd { variant b { lean "True" } }
        theorem t { params lean "(a : ℕ → ℝ) (L : ℝ)" statement lean "conv a L → bnd a" proof lean "by sorry" }"#;

    #[test]
    fn resolves_lean_references_from_the_formula() {
        let (ws, a) = run(MATH);
        let t = a.by_id["m.t"];
        let deps: Vec<_> = a.deps[t].iter().map(|&d| a.symbols[d].id.as_str()).collect();
        assert_eq!(deps, ["m.conv", "m.bnd"]);
        let Item::Theorem(th) = &ws.asts[0].items[2] else { panic!() };
        let refs = &a.lean_refs[&(0, th.statement.as_ref().unwrap().span)];
        assert!(refs.iter().any(|r| matches!(&r.target, Target::Local { name, .. } if name == "L")));
        let Item::Concept(c) = &ws.asts[0].items[0] else { panic!() };
        let frefs = &a.lean_refs[&(0, c.variants[1].def.as_ref().unwrap().span)];
        assert!(frefs.iter().any(|r| r.target == Target::External("Filter.Tendsto".into())));
    }

    #[test]
    fn warns_about_order_in_paths() {
        let (_, a) = run(&format!("{MATH}\npath p {{ step t step conv.e step bnd }}"));
        let w = messages(&a, Severity::Warning);
        assert!(w.iter().any(|m| m.contains("`t` uses `m.bnd`")), "{w:?}");
        assert!(w.iter().any(|m| m.contains("`t` uses `m.conv`")), "{w:?}");
    }

    #[test]
    fn accepts_a_bridged_second_variant() {
        let (_, a) = run(&format!("{MATH}\npath p {{ step conv.f step conv.e step bnd step t }}"));
        assert!(a.diags.is_empty(), "{:?}", a.diags);
    }

    #[test]
    fn finds_chains_of_bridges() {
        let (ws, a) = run(r#"module m
            concept c { variant p { lean "True" } variant q { lean "True" } variant r { lean "True" }
              variant s { lean "True" }
              bridge q <-> p { proof lean "Iff.rfl" } bridge q <-> r { proof lean "Iff.rfl" } }
            path w { step c.p step c.r }"#);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let v = |n: &str| a.by_id[&format!("m.c.{n}")];
        let route = a.bridge_route(&ws, v("p"), v("r")).unwrap();
        let hops: Vec<_> = route.iter().map(|h| (h.from, h.to, h.bridge, h.forward)).collect();
        assert_eq!(hops, [(v("p"), v("q"), 0, false), (v("q"), v("r"), 1, true)]);
        assert!(a.bridged(&ws, v("r"), v("p")));
        assert!(!a.bridged(&ws, v("p"), v("p")));
        assert_eq!(a.bridge_route(&ws, v("p"), v("p")), Some(vec![]));
        assert_eq!(a.bridge_route(&ws, v("p"), v("s")), None);
    }

    #[test]
    fn bridges_of_objects_are_equalities() {
        let (ws, a) = run(r#"module m
            concept d { type lean "ℕ" variant a { lean "1" } variant b { lean "0 + 1" } bridge a <-> b }
            concept p { variant a { lean "True" } variant b { lean "¬False" } bridge a = b }"#);
        let e = messages(&a, Severity::Error);
        assert_eq!(e.len(), 2, "{e:?}");
        assert!(e[0].contains("`d` defines an object: write the bridge as an equality, `a = b`"));
        assert!(e[1].contains("`p` is a statement"));
        assert!(a.defines_object(&ws, a.by_id["m.d"]) && a.defines_object(&ws, a.by_id["m.d.b"]));
        assert!(!a.defines_object(&ws, a.by_id["m.p.a"]));
    }

    #[test]
    fn checks_versions_of_norms() {
        let (ws, a) = run(r#"module m
            norm a { valid from 2020-01-01 to 2024-12-31 input x : bool }
            norm b { valid from 2025-01-01 to 2025-06-30 replaces a input x : bool }
            norm c { valid from 2025-07-02 replaces b input x : bool }"#);
        assert!(messages(&a, Severity::Error).is_empty());
        let w = messages(&a, Severity::Warning);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("between `b` and `c` no version applies from 2025-07-01 to 2025-07-01"), "{w:?}");
        let (na, nb, nc) = (a.by_id["m.a"], a.by_id["m.b"], a.by_id["m.c"]);
        assert_eq!(a.versions(nb), [na, nb, nc]);
        assert_eq!(a.in_force(&ws, nc, "2024-12-31"), Some(na));
        assert_eq!(a.in_force(&ws, na, "2025-01-01"), Some(nb));
        assert_eq!(a.in_force(&ws, na, "2025-07-01"), None);
        assert_eq!(a.in_force(&ws, na, "2030-01-01"), Some(nc));
        assert_eq!(a.in_force(&ws, na, "2019-12-31"), None);

        let (_, a) = run(r#"module m
            norm a { valid from 2020-01-01 input x : bool }
            norm b { valid from 2020-01-01 to 2019-01-01 replaces a input x : bool }
            norm c { valid from 2025-02-30 replaces a input x : bool }
            norm d { replaces d input x : bool }"#);
        let e = messages(&a, Severity::Error);
        assert!(e.iter().any(|m| m.contains("a norm cannot end before it begins")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("which has no end: it needs `valid … to 2019-12-31`")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("`2025-02-30` is not a date")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("`a` is already replaced by `m.b`")), "{e:?}");
        assert!(e.iter().any(|m| m.contains("`d` is not another norm")), "{e:?}");
    }

    #[test]
    fn traces_rules_back_to_judgements() {
        let (ws, a) = run(r#"module m
            norm n {
              input x : money
              judgement fair : bool { label en "fair" criteria en "as a reasonable person would see it" }
              judgement soon : bool
              constant cap : money = 10 EUR
              rule small : bool = x <= cap
              rule ok : bool = small and fair
              rule later : bool = ok or soon
            }"#);
        let w = messages(&a, Severity::Warning);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("judgement `soon` has no criteria"), "{w:?}");
        let id = |n: &str| a.by_id[&format!("m.n.{n}")];
        assert_eq!(a.symbols[id("fair")].kind, SymKind::Judgement);
        assert!(a.judgements_behind(id("small")).is_empty());
        assert_eq!(a.judgements_behind(id("ok")), [id("fair")]);
        assert_eq!(a.judgements_behind(id("later")), [id("fair"), id("soon")]);
        let Item::Norm(n) = &ws.asts[0].items[0] else { panic!() };
        assert!(matches!(&n.members[1].kind, MemberKind::Judgement { criteria } if criteria[0].lang.text == "en"));
    }

    #[test]
    fn checks_rounding() {
        let (_, a) = run(r#"module m
            norm n {
              input x : money
              input k : decimal
              rule ok : money = round_down(x, 1 EUR) + round(x, 0.01 EUR) + round_up(x, 5 EUR)
              rule fine : decimal = round(k, 0.5)
              rule no_step : money = round_down(x)
              rule step_not_written : money = round_down(x, x)
              rule zero_step : money = round(x, 0 EUR)
              rule mixed : money = round_up(x, 0.5)
            }"#);
        let e = messages(&a, Severity::Error);
        assert_eq!(e.len(), 4, "{e:?}");
        assert!(e[0].contains("takes a value and a step"), "{e:?}");
        // An amount rounds to an amount: `0.5 EUR`, not `0.5`.
        assert!(e[3].contains("`round_up` rounds money to a step of the same kind"), "{e:?}");
        assert!(e[1].contains("must be a positive number or amount written out"), "{e:?}");
        assert!(e[2].contains("must be a positive number or amount written out"), "{e:?}");
    }

    #[test]
    fn checks_and_uses_translations() {
        let module = r#"module m original de, en
            concept c { label de "Abstand" label en "distance" label fr "distance (fr)" variant v { lean "True" } }
            remark r { about c text de "Text" text en "Text" }"#;
        let translation = r#"translation m
            c { label es "distancia" }
            r { text es "Texto" criteria es "nada" }
            c { label es "otra" label de "Abstand!" }
            nope { label es "?" }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), module.into()), ("m.es.vk".into(), translation.into())]);
        let a = analyze(&ws);
        let msgs: Vec<String> = a.diags.iter().map(|d| d.message.clone()).collect();
        let has = |s: &str| msgs.iter().any(|m| m.contains(s));
        assert!(has("`fr` is not an original language of `m` (original de, en)"), "{msgs:?}");
        assert!(has("`m.r` has no criteria to translate"), "{msgs:?}");
        assert!(has("`m.c` has its label in `es` twice"), "{msgs:?}");
        assert!(has("`de` is an original language of `m`"), "{msgs:?}");
        assert!(has("there is no `nope` in `m`"), "{msgs:?}");
        let c = a.by_id["m.c"];
        // Written at the declaration first, then the translation, then English.
        assert_eq!(a.label(&ws, c, "de").as_deref(), Some("Abstand"));
        assert_eq!(a.label(&ws, c, "es").as_deref(), Some("otra"));
        assert_eq!(a.label(&ws, c, "it").as_deref(), Some("distance"));
        assert_eq!(a.text_in(&ws, a.by_id["m.r"], Field::Text, "es").as_deref(), Some("Texto"));
        // es: the label and the text of c and r, not the variant without a label.
        let cov = &a.coverage(&ws)[&("m".to_string(), "es".to_string())];
        assert_eq!((cov.translated, cov.total), (2, 2), "{cov:?}");
    }

    #[test]
    fn type_checks_rules() {
        let (_, a) = run(r#"module u
            norm n {
              input x : money
              input w : bool
              constant c : money = 10 EUR
              rule ok : bool { default x <= c and not w unless w then 5 }
              rule bad : money = x + 3
            }"#);
        let e = messages(&a, Severity::Error);
        assert_eq!(e.len(), 2, "{e:?}");
        assert!(e[0].contains("expected bool, found number"));
        assert!(e[1].contains("write amounts with their unit"));
    }

    #[test]
    fn checks_enumerations_units_records_tables_and_norms_using_norms() {
        let src = r#"module u
            enum residence { value domestic value eu value third }
            unit km
            record supply { field amount : money field kind : residence }
            norm total {
              input supplies : list supply
              rule sum_all : money = sum(s.amount for s in supplies where s.kind == residence.domestic)
              rule n : int = count(s in supplies)
              rule all_domestic : bool = all(s.kind == residence.domestic for s in supplies)
            }
            norm n {
              input where_ : residence
              input way : km
              input day : date
              input supplies : list supply
              rule rate : decimal { by where_ domestic: 19 % eu: 7 % otherwise: 0 % }
              rule zone : int { by way up to 10 km: 0 up to 30 km: 1 otherwise: 2 }
              rule deadline : date = last_day(year(day) + 2, 2)
              rule later : date = add_months(day, 3) + 1
              rule span : int = deadline - day
              rule t : money = total.sum_all(supplies = supplies)
              rule far : bool = way / 1 km > 20
            }"#;
        let (_, a) = run(src);
        assert!(messages(&a, Severity::Error).is_empty(), "{:?}", messages(&a, Severity::Error));
        let ty = |id: &str| a.ty_name(a.member_types[&a.by_id[id]]);
        assert_eq!(ty("u.n.where_"), "residence");
        assert_eq!(ty("u.n.way"), "km");
        assert_eq!(ty("u.total.supplies"), "list supply");
        assert_eq!(ty("u.supply.kind"), "residence");
        let (_, a) = run(r#"module u
            enum residence { value domestic value eu }
            unit km
            unit kg
            record supply { field amount : money }
            norm total { input supplies : list supply input x : int rule s : money = sum(s.amount for s in supplies) }
            norm n {
              input where_ : residence
              input w : km
              input supplies : list supply
              rule a : decimal { by where_ domestic: 1 }
              rule b : int { by w up to 30 km: 0 up to 10 km: 1 otherwise: 2 }
              rule c : int { by w up to 30 km: 0 }
              rule d : bool = w > 3 kg
              rule e : money = total.s(supplies = supplies)
              rule f : money = total.s
              rule g : int = supplies.amount
              rule h : bool = where_ == residence.nowhere
              rule i : km = 5 miles
            }"#);
        let e = messages(&a, Severity::Error);
        let has = |s: &str| e.iter().any(|m| m.contains(s));
        assert!(has("no row for eu"), "{e:?}");
        assert!(has("the bounds of a table go up"), "{e:?}");
        assert!(has("needs `otherwise`"), "{e:?}");
        assert!(has("cannot order km and kg"), "{e:?}");
        assert!(has("missing: x"), "{e:?}");
        assert!(has("depends on its inputs"), "{e:?}");
        assert!(has("only records have fields"), "{e:?}");
        assert!(has("enumeration `residence` has no value `nowhere`"), "{e:?}");
        assert!(has("unknown currency or unit `miles`"), "{e:?}");
    }

    #[test]
    fn reports_unknown_names_and_duplicates() {
        let (_, a) = run(
            "module m\n concept c { variant v { lean \"x\" } }\n concept c { variant v { lean \"x\" } }\n remark r { about nope text de \"x\" }",
        );
        let e = messages(&a, Severity::Error);
        assert!(e.iter().any(|m| m.contains("declared twice")));
        assert!(e.iter().any(|m| m.contains("cannot find `nope`")));
    }
}
