// SPDX-License-Identifier: AGPL-3.0-or-later
//! Evaluates the rules of a norm for given inputs, exactly – numbers and amounts are fractions, as in Lean – and
//! records how: for each rule its default, every exception tried and which one decided. What a debugger shows.

use std::collections::HashMap;
use std::fmt;

use std::collections::BTreeMap;

use verkodo_syntax::ast::{Aggregate, BinOp, Expr, ExprKind, Item, MemberKind, Name, Path, RowKey, Table, UnOp};

use crate::analysis::{Analysis, DATE_PARTS, Loc, ROUNDING, SymKind, Target};
use crate::date::{self, day_number, from_day_number};
use crate::sources::Sources;
use crate::types::Ty;

/// An exact fraction `num / den`, `den > 0`, in lowest terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rat {
    num: i128,
    den: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

impl Rat {
    pub fn new(num: i128, den: i128) -> Option<Rat> {
        if den == 0 {
            return None;
        }
        let g = gcd(num, den).max(1);
        let s = if den < 0 { -1 } else { 1 };
        Some(Rat { num: s * num / g, den: s * den / g })
    }

    pub fn int(n: i128) -> Rat {
        Rat { num: n, den: 1 }
    }

    /// `12_345.67` (underscores allowed) exactly.
    pub fn parse(s: &str) -> Option<Rat> {
        let s = s.replace('_', "");
        let (sign, s) = s.strip_prefix('-').map(|r| (-1, r)).unwrap_or((1, s.as_str()));
        let (int, frac) = s.split_once('.').unwrap_or((s, ""));
        if int.is_empty() && frac.is_empty() || !int.chars().chain(frac.chars()).all(|c| c.is_ascii_digit()) {
            return None;
        }
        let den = 10i128.checked_pow(frac.len() as u32)?;
        let num = format!("{int}{frac}").trim_start_matches('0').parse::<i128>().unwrap_or(0);
        Rat::new(sign * num, den)
    }

    fn add(self, o: Rat) -> Option<Rat> {
        Rat::new(self.num.checked_mul(o.den)?.checked_add(o.num.checked_mul(self.den)?)?, self.den.checked_mul(o.den)?)
    }
    fn sub(self, o: Rat) -> Option<Rat> {
        self.add(Rat { num: -o.num, den: o.den })
    }
    fn mul(self, o: Rat) -> Option<Rat> {
        Rat::new(self.num.checked_mul(o.num)?, self.den.checked_mul(o.den)?)
    }
    fn div(self, o: Rat) -> Option<Rat> {
        Rat::new(self.num.checked_mul(o.den)?, self.den.checked_mul(o.num)?)
    }
    fn floor(self) -> i128 {
        self.num.div_euclid(self.den)
    }
    fn ceil(self) -> i128 {
        -(-self.num).div_euclid(self.den)
    }
    /// The whole number, if it is one and fits.
    fn whole(self) -> Option<i64> {
        if self.den == 1 { i64::try_from(self.num).ok() } else { None }
    }
}

impl Ord for Rat {
    fn cmp(&self, o: &Rat) -> std::cmp::Ordering {
        (self.num * o.den).cmp(&(o.num * self.den))
    }
}

impl PartialOrd for Rat {
    fn partial_cmp(&self, o: &Rat) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

/// The value of a literal – a number, percentage, amount or date (its day number) – if the expression is one.
pub fn literal(e: &Expr) -> Option<Rat> {
    match &e.kind {
        ExprKind::Number(n) | ExprKind::Money(n, _) => Rat::parse(n),
        ExprKind::Percent(n) => Rat::parse(n)?.div(Rat::int(100)),
        ExprKind::Date(d) => day_number(d).map(|n| Rat::int(n.into())),
        ExprKind::Paren(x) => literal(x),
        ExprKind::Unary(UnOp::Neg, x) => Rat::int(0).sub(literal(x)?),
        _ => None,
    }
}

impl fmt::Display for Rat {
    /// Exactly where the fraction is a decimal; otherwise rounded to six places, marked with `≈`.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut d = self.den;
        while d % 2 == 0 {
            d /= 2;
        }
        while d % 5 == 0 {
            d /= 5;
        }
        let (places, exact) = if d == 1 {
            let mut p = 0;
            while 10i128.pow(p) % self.den != 0 {
                p += 1;
            }
            (p, true)
        } else {
            (6, false)
        };
        let scale = 10i128.pow(places);
        let scaled = (self.num * scale + if exact { 0 } else { self.den / 2 * self.num.signum() }) / self.den;
        let sign = if scaled < 0 { "-" } else { "" };
        let (int, frac) = (scaled.abs() / scale, scaled.abs() % scale);
        let approx = if exact { "" } else { "≈ " };
        if places == 0 {
            write!(f, "{approx}{sign}{int}")
        } else {
            write!(f, "{approx}{sign}{int}.{:0width$}", frac, width = places as usize)
        }
    }
}

/// A value of the legal profile.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Num(Rat),
    /// A day number.
    Date(i64),
    /// A value of an enumeration: its symbol.
    Enum(usize),
    /// A record: its fields by name.
    Record(BTreeMap<String, Value>),
    List(Vec<Value>),
    /// Not known: an input left open, or what cannot be computed (a division by zero, a number too large) – with
    /// why.
    Unknown(String),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(r) => write!(f, "{r}"),
            Value::Date(d) => write!(f, "{}", from_day_number(*d)),
            Value::Enum(s) => write!(f, "#{s}"),
            Value::Record(fields) => {
                let parts: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            Value::List(xs) => {
                let parts: Vec<String> = xs.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Unknown(why) => write!(f, "? ({why})"),
        }
    }
}

impl Value {
    /// As a person reads it: values of enumerations by name.
    pub fn show(&self, a: &Analysis) -> String {
        match self {
            Value::Enum(s) => a.symbols[*s].id.rsplit('.').next().unwrap_or_default().to_string(),
            Value::Record(fields) => {
                let parts: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {}", v.show(a))).collect();
                format!("{{{}}}", parts.join(", "))
            }
            Value::List(xs) => format!("[{}]", xs.iter().map(|v| v.show(a)).collect::<Vec<_>>().join(", ")),
            other => other.to_string(),
        }
    }
}

/// How a rule came to its value.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// The member (input, constant, rule).
    pub sym: usize,
    pub value: Value,
    /// For a rule: its default and each exception tried, in order – (condition's value, whether it decided).
    pub default: Option<Value>,
    pub exceptions: Vec<(Value, bool)>,
}

/// Every member of a norm with its value, inputs first, then constants and rules in the order they are written.
pub fn evaluate(ws: &Sources, a: &Analysis, norm: usize, inputs: &HashMap<String, String>) -> Vec<Step> {
    let mut ev = Eval::new(ws, a, Some(inputs), HashMap::new());
    let members: Vec<usize> = (0..a.symbols.len()).filter(|&m| a.parent(m) == Some(norm)).collect();
    let mut steps: Vec<Step> = members
        .iter()
        .map(|&m| {
            let value = ev.member(m);
            let (default, exceptions) = ev.trace.get(&m).cloned().unwrap_or_default();
            Step { sym: m, value, default, exceptions }
        })
        .collect();
    let order = |s: &Step| match a.symbols[s.sym].kind {
        SymKind::Input | SymKind::Judgement => 0,
        SymKind::Constant => 1,
        _ => 2,
    };
    steps.sort_by_key(|s| (order(s), a.symbols[s.sym].span.start));
    steps
}

/// Reads an input of a built-in type as typed in: `true`/`false` (also `ja`/`nein`), a number (`40000.99`, also
/// `40.000,99`), a date.
pub fn parse_input(ty: &str, text: &str) -> Value {
    let t = text.trim();
    if t.is_empty() {
        return Value::Unknown("not given".into());
    }
    match ty {
        "bool" => match t {
            "true" | "ja" | "wahr" | "yes" | "1" => Value::Bool(true),
            "false" | "nein" | "falsch" | "no" | "0" => Value::Bool(false),
            _ => Value::Unknown(format!("`{t}` is not true or false")),
        },
        "date" => day_number(t).map(Value::Date).unwrap_or_else(|| Value::Unknown(format!("`{t}` is not a date"))),
        _ => {
            let plain = t.trim_end_matches(['€', ' ', '%']);
            let plain = plain.split_once(' ').map(|(n, _)| n).unwrap_or(plain).trim_end_matches("EUR").trim();
            // A German number: dots group thousands, the comma is the decimal point.
            let normal = if plain.contains(',') { plain.replace('.', "").replace(',', ".") } else { plain.to_string() };
            let percent = t.ends_with('%');
            match Rat::parse(&normal) {
                Some(r) if percent => r.div(Rat::int(100)).map(Value::Num).unwrap_or_else(|| unknown("too large")),
                Some(r) => Value::Num(r),
                None => Value::Unknown(format!("`{t}` is not a number")),
            }
        }
    }
}

/// Reads an input of any type: built-in types as `parse_input`, a value of an enumeration by its name, records and
/// lists as JSON (`[{"amount": "100", "kind": "domestic"}]`).
pub fn parse_value(a: &Analysis, ty: Ty, text: &str) -> Value {
    let t = text.trim();
    if t.is_empty() {
        return Value::Unknown("not given".into());
    }
    match ty {
        Ty::Enum(_) | Ty::Record(_) | Ty::List(_) => {
            let json = serde_json::from_str(t).unwrap_or_else(|_| serde_json::Value::String(t.to_string()));
            from_json(a, ty, &json)
        }
        Ty::Quantity(_) | Ty::Money | Ty::Decimal | Ty::Int | Ty::Num => parse_input("decimal", t),
        other => parse_input(other.name(), t),
    }
}

fn from_json(a: &Analysis, ty: Ty, j: &serde_json::Value) -> Value {
    use serde_json::Value as J;
    match (ty, j) {
        (Ty::Enum(e), J::String(s)) => {
            let id = format!("{}.{}", a.symbols[e].id, s.trim());
            match a.by_id.get(&id) {
                Some(&v) if a.symbols[v].kind == SymKind::Value => Value::Enum(v),
                _ => Value::Unknown(format!("`{s}` is not a value of `{}`", a.ty_name(ty))),
            }
        }
        (Ty::Record(r), J::Object(o)) => {
            let fields =
                (0..a.symbols.len()).filter(|&f| a.symbols[f].kind == SymKind::Field && a.parent(f) == Some(r));
            Value::Record(
                fields
                    .map(|f| {
                        let name = a.symbols[f].id.rsplit('.').next().unwrap_or_default().to_string();
                        let ft = a.member_types.get(&f).copied().unwrap_or(Ty::Unknown);
                        let v = o.get(&name).map(|x| from_json(a, ft, x)).unwrap_or_else(|| unknown("not given"));
                        (name, v)
                    })
                    .collect(),
            )
        }
        (Ty::List(r), J::Array(xs)) => Value::List(xs.iter().map(|x| from_json(a, Ty::Record(r), x)).collect()),
        (_, J::String(s)) => parse_value(a, ty, s),
        (_, J::Bool(b)) => parse_input("bool", &b.to_string()),
        (_, J::Number(n)) => parse_value(a, ty, &n.to_string()),
        _ => Value::Unknown(format!("not {}", a.ty_name(ty))),
    }
}

type Trace = (Option<Value>, Vec<(Value, bool)>);

struct Eval<'a> {
    ws: &'a Sources,
    a: &'a Analysis,
    /// Inputs as typed in, by name.
    text: Option<&'a HashMap<String, String>>,
    /// Inputs given as values, by symbol – when a norm is used by another.
    given: HashMap<usize, Value>,
    memo: HashMap<usize, Value>,
    trace: HashMap<usize, Trace>,
    /// The elements an aggregate goes through.
    locals: Vec<(String, Value)>,
    depth: usize,
}

fn unknown(why: &str) -> Value {
    Value::Unknown(why.into())
}

impl<'a> Eval<'a> {
    fn new(
        ws: &'a Sources,
        a: &'a Analysis,
        text: Option<&'a HashMap<String, String>>,
        given: HashMap<usize, Value>,
    ) -> Self {
        Eval { ws, a, text, given, memo: HashMap::new(), trace: HashMap::new(), locals: Vec::new(), depth: 0 }
    }

    fn member(&mut self, m: usize) -> Value {
        if let Some(v) = self.memo.get(&m) {
            return v.clone();
        }
        if self.depth > 64 {
            return unknown("rules refer to each other in a circle");
        }
        let Loc::Member { file, item, member } = self.a.symbols[m].loc else {
            return unknown("not a member of a norm");
        };
        let Item::Norm(n) = &self.ws.asts[file].items[item] else { return unknown("not a norm") };
        let mem = &n.members[member];
        self.depth += 1;
        let v = match &mem.kind {
            MemberKind::Input | MemberKind::Judgement { .. } => match self.given.get(&m) {
                Some(v) => v.clone(),
                None => {
                    let ty = self.a.member_types.get(&m).copied().unwrap_or(Ty::Unknown);
                    let text = self.text.and_then(|t| t.get(&mem.name.text)).map(String::as_str).unwrap_or("");
                    parse_value(self.a, ty, text)
                }
            },
            MemberKind::Constant(e) => self.expr(file, e),
            MemberKind::Rule { default, unless } => {
                let d = default.as_ref().map(|e| self.expr(file, e));
                let mut tried = Vec::new();
                let mut value = None;
                for u in unless {
                    let c = self.expr(file, &u.cond);
                    let decided = value.is_none() && c == Value::Bool(true);
                    if decided {
                        value = Some(self.expr(file, &u.value));
                    } else if value.is_none() && matches!(c, Value::Unknown(_)) {
                        // Whether this exception applies is not known – nor, then, the rule.
                        value = Some(unknown("an exception depends on what is not known"));
                    }
                    tried.push((c, decided));
                }
                self.trace.insert(m, (d.clone(), tried));
                value.or(d).unwrap_or_else(|| unknown("no default"))
            }
        };
        self.depth -= 1;
        self.memo.insert(m, v.clone());
        v
    }

    fn expr(&mut self, file: usize, e: &Expr) -> Value {
        use Value::*;
        match &e.kind {
            ExprKind::Number(_) | ExprKind::Money(..) | ExprKind::Percent(_) => {
                literal(e).map(Num).unwrap_or_else(|| unknown("number"))
            }
            ExprKind::Bool(b) => Bool(*b),
            ExprKind::Date(d) => day_number(d).map(Date).unwrap_or_else(|| unknown("no such date")),
            ExprKind::Paren(x) => self.expr(file, x),
            ExprKind::Error => unknown("an error in the source"),
            ExprKind::Ref(p) => self.reference(file, p),
            ExprKind::Unary(UnOp::Not, x) => match self.expr(file, x) {
                Bool(b) => Bool(!b),
                other => other,
            },
            ExprKind::Unary(UnOp::Neg, x) => match self.expr(file, x) {
                Num(r) => Rat::int(0).sub(r).map(Num).unwrap_or_else(|| unknown("too large")),
                other => other,
            },
            ExprKind::If(c, x, y) => match self.expr(file, c) {
                Bool(true) => self.expr(file, x),
                Bool(false) => self.expr(file, y),
                other => other,
            },
            ExprKind::Call(p, args) => {
                let vals: Vec<Value> = args.iter().map(|x| self.expr(file, x)).collect();
                if let Some(u) = vals.iter().find(|v| matches!(v, Unknown(_))) {
                    return u.clone();
                }
                call(&p.joined(), &vals)
            }
            ExprKind::Aggregate(agg) => self.aggregate(file, agg),
            ExprKind::Apply(p, args) => self.apply(file, p, args),
            ExprKind::Table(t) => self.table(file, t),
            ExprKind::Binary(op, x, y) => {
                let (l, r) = (self.expr(file, x), self.expr(file, y));
                binary(*op, l, r)
            }
        }
    }

    fn reference(&mut self, file: usize, p: &Path) -> Value {
        let base = match self.a.refs.get(&(file, p.span)) {
            Some(Target::Local { name, .. }) => match self.locals.iter().rev().find(|(n, _)| n == name) {
                Some((_, v)) => v.clone(),
                None => unknown("no such element"),
            },
            Some(Target::Symbol(s)) if self.a.symbols[*s].kind == SymKind::Value => Value::Enum(*s),
            Some(Target::Symbol(s)) if self.a.parent(*s).is_some() => self.member(*s),
            _ => unknown(&format!("`{}` cannot be computed here", p.joined())),
        };
        let Some(fields) = self.a.fields.get(&(file, p.span)) else { return base };
        fields.iter().fold(base, |v, &f| match v {
            Value::Record(mut fs) => {
                let name = self.a.symbols[f].id.rsplit('.').next().unwrap_or_default();
                fs.remove(name).unwrap_or_else(|| unknown("no such field"))
            }
            other @ Value::Unknown(_) => other,
            _ => unknown("not a record"),
        })
    }

    fn aggregate(&mut self, file: usize, agg: &Aggregate) -> Value {
        let list = match self.expr(file, &agg.list) {
            Value::List(xs) => xs,
            u @ Value::Unknown(_) => return u,
            _ => return unknown("not a list"),
        };
        let func = agg.func.text.as_str();
        let mut sum = Rat::int(0);
        let mut count = 0i128;
        let mut truth = func == "all";
        let mut open: Option<Value> = None;
        for x in list {
            self.locals.push((agg.var.text.clone(), x));
            let keep = match &agg.filter {
                Some(f) => self.expr(file, f),
                None => Value::Bool(true),
            };
            let body = match (&keep, &agg.body) {
                (Value::Bool(true), Some(b)) => Some(self.expr(file, b)),
                _ => None,
            };
            self.locals.pop();
            match (keep, body) {
                (Value::Bool(false), _) => {}
                (u @ Value::Unknown(_), _) => open = open.or(Some(u)),
                (_, Some(Value::Num(r))) => match sum.add(r) {
                    Some(s) => sum = s,
                    None => return unknown("too large"),
                },
                (_, Some(Value::Bool(b))) => {
                    truth = if func == "all" { truth && b } else { truth || b };
                }
                (_, Some(u @ Value::Unknown(_))) => open = open.or(Some(u)),
                (_, None) => count += 1,
                _ => return unknown("not a number or truth value"),
            }
        }
        // `all` and `any` decide without what is not known where they can.
        match func {
            "all" if !truth => Value::Bool(false),
            "any" if truth => Value::Bool(true),
            _ if open.is_some() => open.unwrap_or_else(|| unknown("not known")),
            "sum" => Value::Num(sum),
            "count" => Value::Num(Rat::int(count)),
            _ => Value::Bool(truth),
        }
    }

    fn apply(&mut self, file: usize, p: &Path, args: &[(Name, Expr)]) -> Value {
        let Some(Target::Symbol(target)) = self.a.refs.get(&(file, p.span)).cloned() else {
            return unknown("no such rule");
        };
        let mut given = HashMap::new();
        for (name, x) in args {
            let v = self.expr(file, x);
            if let Some(Target::Symbol(i)) = self.a.refs.get(&(file, name.span)) {
                given.insert(*i, v);
            }
        }
        let mut sub = Eval::new(self.ws, self.a, None, given);
        sub.depth = self.depth + 1;
        sub.member(target)
    }

    fn table(&mut self, file: usize, t: &Table) -> Value {
        let subject = self.expr(file, &t.subject);
        if let Value::Unknown(_) = subject {
            return subject;
        }
        for row in &t.rows {
            let hit = match (&row.key, &subject) {
                (RowKey::Value(key), Value::Enum(v)) => {
                    matches!(self.a.refs.get(&(file, key.span)), Some(Target::Symbol(k)) if k == v)
                }
                (RowKey::UpTo(bound), _) => match binary(BinOp::Le, subject.clone(), self.expr(file, bound)) {
                    Value::Bool(b) => b,
                    other => return other,
                },
                _ => false,
            };
            if hit {
                return self.expr(file, &row.value);
            }
        }
        match &t.otherwise {
            Some(o) => self.expr(file, o),
            None => unknown("no row applies"),
        }
    }
}

/// A built-in function on known values.
fn call(name: &str, vals: &[Value]) -> Value {
    use Value::*;
    let rats: Vec<Rat> = vals.iter().filter_map(|v| if let Num(r) = v { Some(*r) } else { None }).collect();
    let ints: Option<Vec<i64>> = vals
        .iter()
        .map(|v| match v {
            Num(r) => r.whole(),
            Date(d) => Some(*d),
            _ => None,
        })
        .collect();
    let int = |n: i64| Num(Rat::int(n.into()));
    if ROUNDING.contains(&name) {
        let [x, step] = rats[..] else { return unknown("rounding needs a value and a step") };
        let Some(q) = x.div(step) else { return unknown("a step of zero") };
        let n = match name {
            "round_down" => q.floor(),
            "round_up" => q.ceil(),
            // Halves away from zero.
            _ => {
                let half = Rat::new(1, 2).expect("½");
                if x.num >= 0 { q.add(half).map(Rat::floor) } else { q.sub(half).map(Rat::ceil) }.unwrap_or(0)
            }
        };
        return Rat::int(n).mul(step).map(Num).unwrap_or_else(|| unknown("too large"));
    }
    let pick = |better: std::cmp::Ordering| {
        let order = |a: &Value, b: &Value| match (a, b) {
            (Num(x), Num(y)) => x.cmp(y),
            (Date(x), Date(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        };
        vals.iter().cloned().reduce(|a, b| if order(&b, &a) == better { b } else { a })
    };
    match (name, ints.as_deref()) {
        ("min", _) => pick(std::cmp::Ordering::Less).unwrap_or_else(|| unknown("min of nothing")),
        ("max", _) => pick(std::cmp::Ordering::Greater).unwrap_or_else(|| unknown("max of nothing")),
        ("abs", _) => match rats[..] {
            [x] => Num(if x.num < 0 { Rat::int(0).sub(x).unwrap_or(x) } else { x }),
            _ => unknown("abs of nothing"),
        },
        ("div" | "mod", Some([_, 0])) => unknown("division by zero"),
        ("div", Some([a, b])) => int(a.div_euclid(*b)),
        ("mod", Some([a, b])) => int(a.rem_euclid(*b)),
        (part, Some([d])) if DATE_PARTS.contains(&part) => {
            let (y, m, day) = date::civil(*d);
            int(match part {
                "year" => y,
                "quarter" => (m - 1) / 3 + 1,
                "month" => m,
                "day" => day,
                _ => date::weekday(*d),
            })
        }
        ("date", Some([y, m, d])) => Date(date::date(*y, *m, *d)),
        ("last_day", Some([y, m])) => Date(date::last_day(*y, *m)),
        ("add_months", Some([d, n])) => Date(date::add_months(*d, *n)),
        ("add_years", Some([d, n])) => Date(date::add_months(*d, 12 * n)),
        (_, None) => unknown("not whole numbers"),
        _ => unknown(&format!("unknown function `{name}`")),
    }
}

/// An operator on two values.
fn binary(op: BinOp, l: Value, r: Value) -> Value {
    use Value::*;
    // `and`/`or` decide without the unknown side where they can.
    match (op, &l, &r) {
        (BinOp::And, Bool(false), _) | (BinOp::And, _, Bool(false)) => return Bool(false),
        (BinOp::Or, Bool(true), _) | (BinOp::Or, _, Bool(true)) => return Bool(true),
        _ => {}
    }
    if let Unknown(_) = l {
        return l;
    }
    if let Unknown(_) = r {
        return r;
    }
    // Days after and before a date, and the days between two.
    let days = |r: Rat| r.whole();
    match (op, &l, &r) {
        (BinOp::Add, Date(d), Num(n)) | (BinOp::Add, Num(n), Date(d)) => {
            return days(*n).map(|n| Date(d + n)).unwrap_or_else(|| unknown("not whole days"));
        }
        (BinOp::Sub, Date(d), Num(n)) => {
            return days(*n).map(|n| Date(d - n)).unwrap_or_else(|| unknown("not whole days"));
        }
        (BinOp::Sub, Date(a), Date(b)) => return Num(Rat::int((a - b).into())),
        _ => {}
    }
    let num = |v: &Value| if let Num(r) = v { Some(*r) } else { None };
    let order = |l: &Value, r: &Value| match (l, r) {
        (Num(a), Num(b)) => Some(a.cmp(b)),
        (Date(a), Date(b)) => Some(a.cmp(b)),
        (Bool(a), Bool(b)) => Some(a.cmp(b)),
        (Enum(a), Enum(b)) => Some(a.cmp(b)),
        _ => None,
    };
    use std::cmp::Ordering::*;
    let arith = |f: fn(Rat, Rat) -> Option<Rat>| match (num(&l), num(&r)) {
        (Some(a), Some(b)) => f(a, b).map(Num).unwrap_or_else(|| unknown("division by zero or too large")),
        _ => unknown("not numbers"),
    };
    match op {
        BinOp::And | BinOp::Or => match (&l, &r) {
            (Bool(a), Bool(b)) => Bool(if op == BinOp::And { *a && *b } else { *a || *b }),
            _ => unknown("not truth values"),
        },
        BinOp::Add => arith(Rat::add),
        BinOp::Sub => arith(Rat::sub),
        BinOp::Mul => arith(Rat::mul),
        BinOp::Div => arith(Rat::div),
        cmp => match order(&l, &r) {
            Some(o) => Bool(match cmp {
                BinOp::Lt => o == Less,
                BinOp::Le => o != Greater,
                BinOp::Gt => o == Greater,
                BinOp::Ge => o != Less,
                BinOp::Eq => o == Equal,
                _ => o != Equal,
            }),
            None => unknown("cannot compare"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    fn run(src: &str, norm: &str, inputs: &[(&str, &str)]) -> HashMap<String, Step> {
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let inputs: HashMap<String, String> = inputs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        evaluate(&ws, &a, a.by_id[norm], &inputs)
            .into_iter()
            .map(|s| (a.symbols[s.sym].id.rsplit('.').next().unwrap().to_string(), s))
            .collect()
    }

    #[test]
    fn evaluates_exactly_and_shows_which_exception_decided() {
        let src = r#"module m
            norm n {
              input x : money
              input w : bool
              constant cap : money = 1_000.50 EUR
              rule third : money = round_down(x / 3, 0.01 EUR)
              rule ok : bool { default x <= cap unless w then false unless x > 2 * cap then true }
            }"#;
        let s = run(src, "m.n", &[("x", "1.000,50"), ("w", "nein")]);
        assert_eq!(s["x"].value.to_string(), "1000.5");
        assert_eq!(s["third"].value.to_string(), "333.5");
        assert_eq!(s["ok"].value, Value::Bool(true));
        assert_eq!(s["ok"].exceptions, [(Value::Bool(false), false), (Value::Bool(false), false)]);
        // The first exception that applies decides.
        let s = run(src, "m.n", &[("x", "5000"), ("w", "true")]);
        assert_eq!(s["ok"].value, Value::Bool(false));
        assert_eq!(s["ok"].exceptions, [(Value::Bool(true), true), (Value::Bool(true), false)]);
        // Unknown inputs make unknown results – unless `and` can decide anyway.
        let s = run(src, "m.n", &[("x", "5000")]);
        assert!(matches!(s["ok"].value, Value::Unknown(_)));
        assert_eq!(Rat::new(1, 3).unwrap().to_string(), "≈ 0.333333");
        assert_eq!(Rat::parse("-0.25").unwrap().to_string(), "-0.25");
    }

    #[test]
    fn evaluates_enumerations_records_lists_tables_dates_and_norms_using_norms() {
        let src = r#"module m
            enum residence { value domestic value eu value third }
            unit km
            record supply { field amount : money field kind : residence }
            norm total {
              input supplies : list supply
              rule domestic_sum : money = sum(s.amount for s in supplies where s.kind == residence.domestic)
              rule n : int = count(s in supplies)
              rule none_third : bool = all(s.kind != residence.third for s in supplies)
            }
            norm n {
              input where_ : residence
              input way : km
              input day : date
              input supplies : list supply
              rule rate : decimal { by where_ domestic: 19 % eu: 7 % otherwise: 0 % }
              rule zone : int { by way up to 10 km: 0 up to 30 km: 1 otherwise: 2 }
              rule deadline : date = last_day(year(day) + 2, 2)
              rule quarter_after : date = date(year(day), 3 * quarter(day) + 1, 1)
              rule month_end : date = add_months(day, 1)
              rule days_left : int = deadline - day
              rule weekday_of : int = weekday(day)
              rule t : money = total.domestic_sum(supplies = supplies)
              rule cents : int = mod(div(17, 5), 2) + abs(-3)
            }"#;
        let list = r#"[{"amount": "100", "kind": "domestic"}, {"amount": "50,5", "kind": "eu"}, {"amount": 20, "kind": "domestic"}]"#;
        let s = run(src, "m.n", &[("where_", "eu"), ("way", "10"), ("day", "2025-01-31"), ("supplies", list)]);
        assert_eq!(s["rate"].value.to_string(), "0.07");
        assert_eq!(s["zone"].value.to_string(), "0");
        assert_eq!(s["deadline"].value.to_string(), "2027-02-28");
        assert_eq!(s["quarter_after"].value.to_string(), "2025-04-01");
        assert_eq!(s["month_end"].value.to_string(), "2025-02-28");
        assert_eq!(s["days_left"].value.to_string(), "758");
        assert_eq!(s["weekday_of"].value.to_string(), "5");
        assert_eq!(s["t"].value.to_string(), "120");
        assert_eq!(s["cents"].value.to_string(), "4");
        let s = run(src, "m.n", &[("where_", "third"), ("way", "30.5"), ("day", "2025-12-15")]);
        assert_eq!(s["rate"].value.to_string(), "0");
        assert_eq!(s["zone"].value.to_string(), "2");
        assert_eq!(s["quarter_after"].value.to_string(), "2026-01-01");
        assert!(matches!(s["t"].value, Value::Unknown(_)));
        let s = run(src, "m.total", &[("supplies", list)]);
        assert_eq!(s["n"].value.to_string(), "3");
        assert_eq!(s["none_third"].value, Value::Bool(true));
    }

    #[test]
    fn the_income_tax_example_as_lean_proved_it() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/estg/p32a.vk")).unwrap();
        let s = run(&src, "estg.p32a.income_tax", &[("taxable_income", "40000")]);
        assert_eq!(s["tax_exact"].value.to_string(), "7209.632598131");
        assert_eq!(s["tax"].value.to_string(), "7209");
        let s = run(&src, "estg.p32a.income_tax", &[("taxable_income", "12348.99")]);
        assert_eq!(s["tax"].value.to_string(), "0");
    }
}
