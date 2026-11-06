// SPDX-License-Identifier: AGPL-3.0-or-later
//! verkodo → Lean 4.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use verkodo_core::date::{LEAN_DAY, day_number};
use verkodo_core::proof::Key;
use verkodo_core::{Analysis, Hop, Loc, Sources, SymKind, Target, lean};
use verkodo_syntax::ast::*;
use verkodo_syntax::{Diagnostic, Span};

/// One generated Lean file.
#[derive(Debug)]
pub struct Generated {
    /// The verkodo source file it comes from.
    pub file: usize,
    pub module: String,
    pub code: String,
    /// Which lines (1-based, end exclusive) belong to which part.
    pub decls: Vec<(Key, Range<usize>)>,
    /// Transports that need no Lean code because the path chose the theorem's own variants.
    pub same: Vec<(usize, usize)>,
    /// Pairs of variants a transport went through: (theorem, path) → [(from, to)].
    pub via: HashMap<(usize, usize), Vec<(usize, usize)>>,
    /// The bridges a transport uses: (theorem, path) → bridges.
    pub bridges: HashMap<(usize, usize), Vec<Key>>,
    /// Where the Lean snippets of the source landed in `code` – to carry Lean's positions back.
    pub origins: Vec<Origin>,
}

/// A character copied from a snippet: (byte offset in the generated code, byte offset in the snippet, length).
pub type Char = (usize, usize, usize);

/// One Lean snippet of the source as it appears in the generated code.
#[derive(Debug)]
pub struct Origin {
    /// The span of the Lean string literal in the source file.
    pub span: Span,
    /// Every character copied from the snippet: (byte offset in `code`, byte offset in the snippet, length),
    /// ordered by the first. Names that were replaced (concepts by variants) are not in it.
    pub chars: Vec<Char>,
}

impl Origin {
    /// The byte range in the snippet from the first to the last character of `range` that was copied from it – for
    /// a stretch over several lines, whose indentation verkodo wrote in between.
    pub fn source_bounds(&self, range: Range<usize>) -> Option<Range<usize>> {
        let first = self.chars.iter().find(|c| c.0 >= range.start && c.0 < range.end)?;
        let last = self.chars.iter().rev().find(|c| c.0 >= range.start && c.0 + c.2 <= range.end)?;
        (first.0 <= last.0).then_some(first.1..last.1 + last.2)
    }

    /// The byte range in the snippet of `range` in the generated code, if all of it was copied from the snippet.
    pub fn source_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        let mut i = self.chars.binary_search_by_key(&range.start, |c| c.0).ok()?;
        let (mut gen_end, start) = (self.chars[i].0, self.chars[i].1);
        let mut src_end = start;
        while gen_end < range.end {
            let &(g, s, len) = self.chars.get(i)?;
            if g != gen_end || s != src_end {
                return None;
            }
            gen_end += len;
            src_end += len;
            i += 1;
        }
        (gen_end == range.end).then_some(start..src_end)
    }
}

// Markers in the text of a declaration until `Gen::decl` takes them out: a snippet starts with START, its number and
// START_END, and ends with END; INDENT and the four spaces after it are not from the snippet.
const START: char = '\u{E000}';
const START_END: char = '\u{E001}';
const END: char = '\u{E002}';
const INDENT: char = '\u{E003}';

/// A snippet after substitution: pieces of its text, each copied from the snippet (with its offset there) or not.
#[derive(Debug)]
struct Pending {
    span: Span,
    pieces: Vec<(Range<usize>, Option<usize>)>,
}

/// Exports every module; diagnostics are about what cannot be exported (e.g. a missing bridge).
pub fn export(ws: &Sources, a: &Analysis) -> (Vec<Generated>, Vec<Diagnostic>) {
    let defaults = default_choice(ws, a);
    let mut out = Vec::new();
    let mut diags = Vec::new();
    for (file, ast) in ws.asts.iter().enumerate() {
        let Some(module) = &ast.module else { continue };
        let mut g = Gen {
            ws,
            a,
            file,
            module: module.joined(),
            code: String::new(),
            decls: Vec::new(),
            same: Vec::new(),
            via: HashMap::new(),
            bridges: HashMap::new(),
            day: false,
            defaults: &defaults,
            diags: &mut diags,
            pending: RefCell::new(Vec::new()),
            origins: Vec::new(),
        };
        g.module_file(ast);
        out.push(Generated {
            file,
            module: g.module,
            code: g.code,
            decls: g.decls,
            same: g.same,
            via: g.via,
            bridges: g.bridges,
            origins: g.origins,
        });
    }
    (out, diags)
}

/// Every concept defaults to its first variant.
fn default_choice(ws: &Sources, a: &Analysis) -> HashMap<usize, usize> {
    let mut m = HashMap::new();
    for (i, s) in a.symbols.iter().enumerate() {
        if let Loc::Variant { file, item, variant: 0 } = s.loc
            && let Some(c) = a.parent(i)
            && matches!(ws.asts[file].items[item], Item::Concept(_))
        {
            m.insert(c, i);
        }
    }
    m
}

struct Gen<'a> {
    ws: &'a Sources,
    a: &'a Analysis,
    file: usize,
    module: String,
    code: String,
    decls: Vec<(Key, Range<usize>)>,
    same: Vec<(usize, usize)>,
    via: HashMap<(usize, usize), Vec<(usize, usize)>>,
    bridges: HashMap<(usize, usize), Vec<Key>>,
    /// Whether `verkodo.day` is in the file already.
    day: bool,
    defaults: &'a HashMap<usize, usize>,
    diags: &'a mut Vec<Diagnostic>,
    /// Snippets handed out by `subst`, by number, until `decl` places them.
    pending: RefCell<Vec<Pending>>,
    origins: Vec<Origin>,
}

/// The Lean type of a type of the legal profile. Numbers and amounts are rationals – exact, as the law computes;
/// OPEN: `int` as ℤ (needs casts where it meets other numbers).
/// The rounding functions of the legal profile, for a step `u > 0`.
const LEAN_ROUNDING: &str = "/-- verkodo: `x` rounded down to a multiple of `u` (\"abrunden\"). -/
def verkodo.round_down (x u : ℚ) : ℚ := ⌊x / u⌋ * u

/-- verkodo: `x` rounded up to a multiple of `u` (\"aufrunden\"). -/
def verkodo.round_up (x u : ℚ) : ℚ := ⌈x / u⌉ * u

/-- verkodo: `x` rounded to the nearest multiple of `u`, halves away from zero (\"kaufmännisch runden\"). -/
def verkodo.round (x u : ℚ) : ℚ := if 0 ≤ x then ⌊x / u + 1 / 2⌋ * u else -(⌊-x / u + 1 / 2⌋ * u)";

fn lean_ty(ty: &str) -> &'static str {
    match ty {
        "bool" => "Bool",
        // Day numbers, as `verkodo.day` counts them.
        "date" => "ℤ",
        _ => "ℚ",
    }
}

fn indent(s: &str) -> String {
    s.lines().collect::<Vec<_>>().join(&format!("\n{INDENT}    "))
}

impl Gen<'_> {
    fn line(&self) -> usize {
        self.code.matches('\n').count() + 1
    }

    fn decl(&mut self, key: Key, text: &str) {
        let start = self.line();
        let text = self.place(text);
        self.code.push_str(text.trim_end());
        self.code.push('\n');
        let end = self.line();
        self.decls.push((key, start..end));
        self.code.push('\n');
    }

    /// Takes the markers out of the text of a declaration that goes to the end of `code`, and records where its
    /// snippets land.
    fn place(&mut self, text: &str) -> String {
        let base = self.code.len();
        let pending = self.pending.borrow();
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        // The snippet we are in, its position in the substituted text, and the characters found so far.
        let mut current: Option<(usize, usize, Vec<Char>)> = None;
        while let Some(c) = chars.next() {
            match c {
                START => {
                    let mut n = String::new();
                    for d in chars.by_ref() {
                        if d == START_END {
                            break;
                        }
                        n.push(d);
                    }
                    current = n.parse().ok().map(|k| (k, 0, Vec::new()));
                }
                END => {
                    if let Some((k, _, found)) = current.take()
                        && let Some(p) = pending.get(k)
                    {
                        self.origins.push(Origin { span: p.span, chars: found });
                    }
                }
                INDENT => {
                    for _ in 0..4 {
                        if chars.peek() == Some(&' ') {
                            chars.next();
                            out.push(' ');
                        }
                    }
                }
                _ => {
                    if let Some((k, pos, found)) = current.as_mut() {
                        let piece = pending.get(*k).and_then(|p| p.pieces.iter().find(|(r, _)| r.contains(pos)));
                        if let Some((r, Some(src))) = piece {
                            found.push((base + out.len(), src + (*pos - r.start), c.len_utf8()));
                        }
                        *pos += c.len_utf8();
                    }
                    out.push(c);
                }
            }
        }
        out
    }

    /// The Lean name of a symbol, relative to this module where possible.
    fn name(&self, sym: usize) -> String {
        let id = &self.a.symbols[sym].id;
        match id.strip_prefix(&format!("{}.", self.module)) {
            Some(rel) => rel.to_string(),
            None => format!("_root_.{id}"),
        }
    }

    /// Lean code with concepts replaced by the chosen variants, between markers that `place` takes out again.
    fn subst(&self, l: &Lean, choice: &HashMap<usize, usize>) -> String {
        let mut out = String::new();
        let mut pieces = Vec::new();
        let mut copy = |out: &mut String, from: usize, to: usize| {
            pieces.push((out.len()..out.len() + (to - from), Some(from)));
            out.push_str(&l.code[from..to]);
        };
        let mut pos = 0;
        for r in self.a.lean_refs.get(&(self.file, l.span)).into_iter().flatten() {
            let Target::Symbol(s) = r.target else {
                continue;
            };
            copy(&mut out, pos, r.range.start);
            let target = match self.a.symbols[s].kind {
                SymKind::Concept => choice.get(&s).copied().unwrap_or(s),
                _ => s,
            };
            out.push_str(&self.name(target));
            pos = r.range.end;
        }
        copy(&mut out, pos, l.code.len());
        let mut pending = self.pending.borrow_mut();
        pending.push(Pending { span: l.span, pieces });
        format!("{START}{}{START_END}{out}{END}", pending.len() - 1)
    }

    /// The concepts a Lean snippet mentions.
    fn concepts_in(&self, l: &Lean) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .a
            .lean_refs
            .get(&(self.file, l.span))
            .into_iter()
            .flatten()
            .filter_map(|r| match r.target {
                Target::Symbol(s) if self.a.symbols[s].kind == SymKind::Concept => Some(s),
                _ => None,
            })
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    fn module_file(&mut self, ast: &File) {
        self.code.push_str(&format!(
            "-- Generated by verkodo from module `{}` – do not edit.\nimport Mathlib\n\nnamespace {}\n\n",
            self.module, self.module
        ));
        let me = self.module.clone();
        for it in &ast.items {
            if let Item::Concept(c) = it {
                self.concept(c);
            }
        }
        for it in &ast.items {
            if let Item::Norm(n) = it {
                self.norm(n);
            }
        }
        for t in self.theorems_in_order(ast) {
            self.theorem(t);
        }
        for it in &ast.items {
            if let Item::Path(p) = it {
                self.path(ast, p);
            }
        }
        self.code.push_str(&format!("end {me}\n"));
    }

    fn sym(&self, id: &str) -> Option<usize> {
        self.a.by_id.get(id).copied()
    }

    fn concept(&mut self, c: &Concept) {
        let cid = format!("{}.{}", self.module, c.name.text);
        let params = c.params.as_ref().map(|p| p.code.clone()).unwrap_or_default();
        let defaults = self.defaults.clone();
        for v in &c.variants {
            let (Some(vs), Some(def)) = (self.sym(&format!("{cid}.{}", v.name.text)), &v.def) else {
                continue;
            };
            // An object may need choice or the reals, which Lean cannot compute: `noncomputable` costs nothing in
            // proofs.
            let (head, ty) = match &c.ty {
                Some(t) => ("noncomputable abbrev", self.subst(t, &defaults)),
                None => ("abbrev", "Prop".to_string()),
            };
            let text = format!(
                "/-- verkodo: `{cid}.{}` -/\n{head} {}.{} {params} : {ty} :=\n  {}",
                v.name.text,
                c.name.text,
                v.name.text,
                indent(&self.subst(def, &defaults))
            );
            self.decl(Key::Variant(vs), &text);
        }
        let Some(concept) = self.sym(&cid) else {
            return;
        };
        let all = c.params.as_ref().map(|p| lean::binders(&p.code)).unwrap_or_default();
        let explicit = c.params.as_ref().map(|p| lean::explicit_names(&p.code)).unwrap_or_default();
        for (index, b) in c.bridges.iter().enumerate() {
            let body = |name: &str| c.variants.iter().find(|v| v.name.text == name).and_then(|v| v.def.as_ref());
            let (Some(l), Some(r)) = (body(&b.left.text), body(&b.right.text)) else {
                continue;
            };
            let (cn, ln, rn) = (&c.name.text, &b.left.text, &b.right.text);
            let proof = b.proof.as_ref().map(|p| self.subst(p, &defaults)).unwrap_or_else(|| "by sorry".into());
            // Statements: pointwise `l_iff_r`, made an equality by `propext`; objects: pointwise `l_eq_r_at`.
            let (point, rel, lift) = if c.ty.is_some() {
                (format!("{cn}.{ln}_eq_{rn}_at"), "=", "")
            } else {
                (format!("{cn}.{ln}_iff_{rn}"), "↔", "propext ")
            };
            let funext = if all.is_empty() {
                format!("exact {lift}{point}")
            } else {
                let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
                format!("funext {}\n  exact {lift}({point} {})", names.join(" "), explicit.join(" "))
            };
            let text = format!(
                "theorem {point} {params} :\n    ({}) {rel} ({}) :=\n  {}\n\ntheorem {cn}.{ln}_eq_{rn} : @{cn}.{ln} = @{cn}.{rn} := by\n  {funext}",
                self.subst(l, &defaults),
                self.subst(r, &defaults),
                indent(&proof)
            );
            self.decl(Key::Bridge { concept, index }, &text);
        }
    }

    /// Theorems so that each comes after the theorems it uses.
    fn theorems_in_order<'t>(&self, ast: &'t File) -> Vec<&'t Theorem> {
        let thms: Vec<(usize, &Theorem)> = ast
            .items
            .iter()
            .filter_map(|it| match it {
                Item::Theorem(t) => self.sym(&format!("{}.{}", self.module, t.name.text)).map(|s| (s, t)),
                _ => None,
            })
            .collect();
        let mut done = HashSet::new();
        let mut out = Vec::new();
        fn visit<'t>(
            s: usize,
            thms: &[(usize, &'t Theorem)],
            a: &Analysis,
            done: &mut HashSet<usize>,
            out: &mut Vec<&'t Theorem>,
        ) {
            if !done.insert(s) {
                return;
            }
            for &d in &a.deps[s] {
                if thms.iter().any(|(x, _)| *x == d) {
                    visit(d, thms, a, done, out);
                }
            }
            if let Some((_, t)) = thms.iter().find(|(x, _)| *x == s) {
                out.push(t);
            }
        }
        for &(s, _) in &thms {
            visit(s, &thms, self.a, &mut done, &mut out);
        }
        out
    }

    /// The variants a theorem is written against.
    fn home(&self, t: &Theorem) -> HashMap<usize, usize> {
        let mut choice = self.defaults.clone();
        for u in &t.using {
            if let Some(Target::Symbol(v)) = self.a.refs.get(&(self.file, u.span))
                && let Some(c) = self.a.parent(*v)
            {
                choice.insert(c, *v);
            }
        }
        choice
    }

    fn theorem(&mut self, t: &Theorem) {
        let Some(s) = self.sym(&format!("{}.{}", self.module, t.name.text)) else {
            return;
        };
        let Some(st) = &t.statement else { return };
        let home = self.home(t);
        let params = t.params.as_ref().map(|p| p.code.clone()).unwrap_or_default();
        let proof = t.proof.as_ref().map(|p| self.subst(p, &home)).unwrap_or_else(|| "by sorry".into());
        let text = format!(
            "/-- verkodo: `{}.{}` -/\ntheorem {} {params} :\n    {} :=\n  {}",
            self.module,
            t.name.text,
            t.name.text,
            indent(&self.subst(st, &home)),
            indent(&proof)
        );
        self.decl(Key::Theorem(s), &text);
    }

    fn path(&mut self, ast: &File, p: &PathDecl) {
        let Some(path) = self.sym(&format!("{}.{}", self.module, p.name.text)) else {
            return;
        };
        let mut choice = self.defaults.clone();
        let mut chosen = HashSet::new();
        for step in &p.steps {
            if let Some(Target::Symbol(v)) = self.a.refs.get(&(self.file, step.span))
                && self.a.symbols[*v].kind == SymKind::Variant
                && let Some(c) = self.a.parent(*v)
                && chosen.insert(c)
            {
                choice.insert(c, *v);
            }
        }
        let mut body = String::new();
        let mut keys = Vec::new();
        for step in &p.steps {
            let Some(Target::Symbol(s)) = self.a.refs.get(&(self.file, step.span)).cloned() else {
                continue;
            };
            let Some(t) = ast.items.iter().find_map(|it| match it {
                Item::Theorem(t) if self.sym(&format!("{}.{}", self.module, t.name.text)) == Some(s) => Some(t),
                _ => None,
            }) else {
                continue;
            };
            let Some(st) = &t.statement else { continue };
            let home = self.home(t);
            let mut rewrites = Vec::new();
            let mut pairs = Vec::new();
            let mut bridges = Vec::new();
            let mut missing = false;
            for c in self.concepts_in(st) {
                let (Some(&from), Some(&to)) = (choice.get(&c), home.get(&c)) else {
                    continue;
                };
                if from == to {
                    continue;
                }
                match self.rewrite(c, from, to) {
                    Some((rw, hops)) => {
                        rewrites.extend(rw);
                        // Shown from the theorem's own variant to the path's, one bridge after the other.
                        pairs.extend(hops.iter().rev().map(|h| (h.to, h.from)));
                        bridges.extend(hops.iter().map(|h| Key::Bridge { concept: c, index: h.bridge }));
                    }
                    None => {
                        missing = true;
                        let msg = format!(
                            "cannot carry `{}` over to this path: no bridge between `{}` and `{}`",
                            t.name.text, self.a.symbols[to].id, self.a.symbols[from].id
                        );
                        self.diags.push(Diagnostic::warning(self.file, step.span, msg));
                    }
                }
            }
            if missing {
                continue;
            }
            if rewrites.is_empty() {
                self.same.push((s, path));
                continue;
            }
            self.via.insert((s, path), pairs);
            self.bridges.insert((s, path), bridges);
            let params = t.params.as_ref().map(|p| p.code.clone()).unwrap_or_default();
            let args = t.params.as_ref().map(|p| lean::explicit_names(&p.code)).unwrap_or_default();
            let text = format!(
                "theorem {} {params} :\n    {} := by\n  rw [{}]\n  exact _root_.{}.{} {}",
                t.name.text,
                indent(&self.subst(st, &choice)),
                rewrites.join(", "),
                self.module,
                t.name.text,
                args.join(" ")
            );
            keys.push((Key::Transport { theorem: s, path }, text));
        }
        if keys.is_empty() {
            return;
        }
        body.push_str(&format!(
            "-- path `{}`: theorems carried over along bridges\nnamespace {}\n\n",
            p.name.text, p.name.text
        ));
        self.code.push_str(&body);
        for (k, text) in keys {
            self.decl(k, &text);
        }
        self.code.push_str(&format!("end {}\n\n", p.name.text));
    }

    // ---- the legal profile ----

    /// A norm as a namespace: its inputs as a structure `Inputs`, constants as definitions, rules as functions of
    /// the inputs (`(i : Inputs)`). Numbers and amounts are rationals (ℚ), truth values `Bool`; an exception
    /// (`unless`) overrides the default, and the first exception that applies wins.
    fn norm(&mut self, n: &Norm) {
        let nid = format!("{}.{}", self.module, n.name.text);
        let Some(norm) = self.sym(&nid) else { return };
        let members: Vec<(usize, &Member)> =
            n.members.iter().filter_map(|m| self.sym(&format!("{nid}.{}", m.name.text)).map(|s| (s, m))).collect();
        if let Err(why) = self.check_norm(&members) {
            let msg = format!("norm `{}` is not exported to Lean: {why}", n.name.text);
            self.diags.push(Diagnostic::warning(self.file, n.name.span, msg));
            return;
        }
        if !self.day {
            self.code.push_str(LEAN_DAY);
            self.code.push_str("\n\n");
            self.code.push_str(LEAN_ROUNDING);
            self.code.push_str("\n\n");
            self.day = true;
        }
        self.code.push_str(&format!("-- norm `{}`\nnamespace {}\n\n", n.name.text, n.name.text));
        let fields: String = members
            .iter()
            .filter(|(_, m)| matches!(m.kind, MemberKind::Input | MemberKind::Judgement { .. }))
            .map(|(s, m)| {
                let what =
                    if matches!(m.kind, MemberKind::Judgement { .. }) { " – a judgement, not a fact" } else { "" };
                format!(
                    "  /-- verkodo: `{}`{what} -/\n  {} : {}\n",
                    self.a.symbols[*s].id,
                    m.name.text,
                    lean_ty(&m.ty.text)
                )
            })
            .collect();
        // When this version applies, as day numbers; `in_force` says whether it applies on a day.
        let bound = |name: &str, d: &Option<(String, Span)>| {
            d.as_ref().and_then(|(text, _)| {
                day_number(text).map(|n| format!("/-- verkodo: {text} -/\ndef {name} : ℤ := {n}\n\n"))
            })
        };
        let (from, to) = (bound("valid_from", &n.valid_from), bound("valid_to", &n.valid_to));
        let test = match (&from, &to) {
            (Some(_), Some(_)) => "decide (valid_from ≤ d) && decide (d ≤ valid_to)",
            (Some(_), None) => "decide (valid_from ≤ d)",
            (None, Some(_)) => "decide (d ≤ valid_to)",
            (None, None) => "true",
        };
        let validity = format!(
            "{}{}/-- verkodo: whether `{nid}` applies on day `d` (`verkodo.day y m d`) -/\ndef in_force (d : ℤ) : Bool :=\n  {test}",
            from.unwrap_or_default(),
            to.unwrap_or_default()
        );
        self.decl(
            Key::Norm(norm),
            &format!("/-- verkodo: the inputs of `{nid}` -/\nstructure Inputs where\n{fields}\n{validity}"),
        );
        for (s, m) in self.member_order(&members) {
            let (name, ty, id) = (&m.name.text, lean_ty(&m.ty.text), &self.a.symbols[s].id);
            let text = match &m.kind {
                MemberKind::Input | MemberKind::Judgement { .. } => continue,
                MemberKind::Constant(e) => format!("/-- verkodo: `{id}` -/\ndef {name} : {ty} :=\n  {}", self.lexpr(e)),
                MemberKind::Rule { default, unless } => {
                    let mut body = default.as_ref().map(|d| self.lexpr(d)).unwrap_or_default();
                    for u in unless.iter().rev() {
                        body = format!("bif {} then {}\n  else {body}", self.lexpr(&u.cond), self.lexpr(&u.value));
                    }
                    let what = match m.modality {
                        Some(Modality::Obligation) => " – an obligation: whether it applies",
                        Some(Modality::Permission) => " – a permission: whether it applies",
                        Some(Modality::Prohibition) => " – a prohibition: whether it applies",
                        None => "",
                    };
                    format!("/-- verkodo: `{id}`{what} -/\ndef {name} (i : Inputs) : {ty} :=\n  {body}")
                }
            };
            self.decl(Key::Member(s), &text);
        }
        self.code.push_str(&format!("end {}\n\n", n.name.text));
    }

    /// Why a norm cannot be exported yet, if it cannot.
    fn check_norm(&self, members: &[(usize, &Member)]) -> Result<(), String> {
        let own: HashSet<usize> = members.iter().map(|(s, _)| *s).collect();
        let mut currency: Option<&str> = None;
        for (_, m) in members {
            let mut exprs: Vec<&Expr> = Vec::new();
            match &m.kind {
                MemberKind::Input | MemberKind::Judgement { .. } => {}
                MemberKind::Constant(e) => exprs.push(e),
                MemberKind::Rule { default, unless } => {
                    exprs.extend(default);
                    for u in unless {
                        exprs.push(&u.cond);
                        exprs.push(&u.value);
                    }
                }
            }
            let mut stack = exprs;
            while let Some(e) = stack.pop() {
                match &e.kind {
                    ExprKind::Ref(p) => match self.a.refs.get(&(self.file, p.span)) {
                        Some(Target::Symbol(t)) if own.contains(t) => {
                            let const_uses_input = matches!(m.kind, MemberKind::Constant(_))
                                && self.a.symbols[*t].kind != SymKind::Constant;
                            if const_uses_input {
                                return Err(format!("constant `{}` depends on inputs or rules", m.name.text));
                            }
                        }
                        _ => return Err(format!("`{}` refers to `{}` outside the norm", m.name.text, p.joined())),
                    },
                    ExprKind::Error => return Err(format!("`{}` has an error", m.name.text)),
                    ExprKind::Money(_, cur) => {
                        if currency.is_some_and(|c| c != cur.text) {
                            return Err("amounts in more than one currency".into());
                        }
                        currency = Some(&cur.text);
                    }
                    ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::Date(_) => {}
                    ExprKind::Unary(_, x) | ExprKind::Paren(x) => stack.push(x),
                    ExprKind::Binary(_, x, y) => stack.extend([x.as_ref(), y.as_ref()]),
                    ExprKind::If(c, x, y) => stack.extend([c.as_ref(), x.as_ref(), y.as_ref()]),
                    ExprKind::Call(_, args) => stack.extend(args),
                }
            }
        }
        Ok(())
    }

    /// Constants and rules so that each comes after what it uses.
    fn member_order<'m>(&self, members: &[(usize, &'m Member)]) -> Vec<(usize, &'m Member)> {
        let mut done = HashSet::new();
        let mut out = Vec::new();
        fn visit<'m>(
            s: usize,
            members: &[(usize, &'m Member)],
            a: &Analysis,
            done: &mut HashSet<usize>,
            out: &mut Vec<(usize, &'m Member)>,
        ) {
            if !done.insert(s) {
                return;
            }
            for &d in &a.deps[s] {
                if members.iter().any(|(x, _)| *x == d) {
                    visit(d, members, a, done, out);
                }
            }
            if let Some(&(_, m)) = members.iter().find(|(x, _)| *x == s) {
                out.push((s, m));
            }
        }
        for &(s, _) in members {
            visit(s, members, self.a, &mut done, &mut out);
        }
        out
    }

    /// An expression of the legal profile in Lean; `check_norm` has made sure it can be written.
    fn lexpr(&self, e: &Expr) -> String {
        let num = |n: &str| format!("({} : ℚ)", n.replace('_', ""));
        match &e.kind {
            ExprKind::Ref(p) => match self.a.refs.get(&(self.file, p.span)) {
                Some(Target::Symbol(s)) => {
                    let name = self.a.symbols[*s].id.rsplit('.').next().unwrap_or_default();
                    match self.a.symbols[*s].kind {
                        SymKind::Input | SymKind::Judgement => format!("i.{name}"),
                        SymKind::Rule => format!("({name} i)"),
                        _ => name.to_string(),
                    }
                }
                _ => "sorry".into(),
            },
            ExprKind::Number(n) | ExprKind::Money(n, _) => num(n),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Unary(UnOp::Not, x) => format!("(!{})", self.lexpr(x)),
            ExprKind::Unary(UnOp::Neg, x) => format!("(-{})", self.lexpr(x)),
            ExprKind::Paren(x) => self.lexpr(x),
            ExprKind::If(c, x, y) => format!("(bif {} then {} else {})", self.lexpr(c), self.lexpr(x), self.lexpr(y)),
            ExprKind::Call(p, args) if verkodo_core::analysis::ROUNDING.contains(&p.joined().as_str()) => {
                let args: Vec<String> = args.iter().map(|x| self.lexpr(x)).collect();
                format!("(verkodo.{} {})", p.joined(), args.join(" "))
            }
            ExprKind::Call(p, args) => {
                let mut args = args.iter().map(|x| self.lexpr(x)).rev();
                let last = args.next().unwrap_or_default();
                args.fold(last, |acc, x| format!("({} {x} {acc})", p.joined()))
            }
            ExprKind::Binary(op, x, y) => {
                let (x, y) = (self.lexpr(x), self.lexpr(y));
                match op {
                    BinOp::And => format!("({x} && {y})"),
                    BinOp::Or => format!("({x} || {y})"),
                    BinOp::Lt => format!("decide ({x} < {y})"),
                    BinOp::Le => format!("decide ({x} ≤ {y})"),
                    BinOp::Gt => format!("decide ({x} > {y})"),
                    BinOp::Ge => format!("decide ({x} ≥ {y})"),
                    BinOp::Eq => format!("({x} == {y})"),
                    BinOp::Ne => format!("({x} != {y})"),
                    BinOp::Add => format!("({x} + {y})"),
                    BinOp::Sub => format!("({x} - {y})"),
                    BinOp::Mul => format!("({x} * {y})"),
                    BinOp::Div => format!("({x} / {y})"),
                }
            }
            ExprKind::Date(d) => match day_number(d) {
                Some(n) => format!("({n} : ℤ)"),
                None => "sorry".into(),
            },
            ExprKind::Error => "sorry".into(),
        }
    }

    /// The rewrites that turn variant `from` into `to`, along the shortest chain of bridges of the concept, and
    /// the bridges they use.
    fn rewrite(&self, concept: usize, from: usize, to: usize) -> Option<(Vec<String>, Vec<Hop>)> {
        let hops = self.a.bridge_route(self.ws, from, to)?;
        let short = |v: usize| self.a.symbols[v].id.rsplit('.').next().unwrap_or_default().to_string();
        let cn = self.name(concept);
        let rewrites = hops
            .iter()
            .map(|h| {
                let (f, t) = (short(h.from), short(h.to));
                if h.forward { format!("{cn}.{f}_eq_{t}") } else { format!("← {cn}.{t}_eq_{f}") }
            })
            .collect();
        Some((rewrites, hops))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verkodo_core::analyze;

    #[test]
    fn exports_variants_bridges_and_transports() {
        let src = r#"module m
            concept c { params lean "(x : ℝ)"
              variant p { lean "0 < x" } variant q { lean "x > 0" }
              bridge p <-> q { proof lean "Iff.rfl" } }
            theorem t { params lean "(x : ℝ)" using c.q statement lean "c x → c x" proof lean "id" }
            path w { step c.p step t }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let (g, diags) = export(&ws, &a);
        assert!(diags.is_empty());
        let code = &g[0].code;
        assert!(code.contains("abbrev c.p (x : ℝ) : Prop :=\n  0 < x"), "{code}");
        assert!(code.contains("theorem c.p_iff_q (x : ℝ) :\n    (0 < x) ↔ (x > 0)"), "{code}");
        assert!(code.contains("theorem t (x : ℝ) :\n    c.q x → c.q x"), "{code}");
        assert!(code.contains("namespace w"), "{code}");
        assert!(code.contains("rw [c.p_eq_q]\n  exact _root_.m.t x"), "{code}");
        let keys: Vec<Key> = g[0].decls.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys.len(), 5);
        assert!(!code.contains(START) && !code.contains(INDENT), "markers left in the code");
    }

    #[test]
    fn carries_theorems_over_chains_of_bridges() {
        let src = r#"module m
            concept c { params lean "(x : ℝ)"
              variant p { lean "0 < x" } variant q { lean "x > 0" } variant r { lean "0 < x ∧ True" }
              bridge q <-> p { proof lean "Iff.rfl" } bridge q <-> r { proof lean "by simp" } }
            theorem t { params lean "(x : ℝ)" using c.p statement lean "c x → c x" proof lean "id" }
            path w { step c.r step t }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        let (g, diags) = export(&ws, &a);
        assert!(diags.is_empty(), "{diags:?}");
        let code = &g[0].code;
        // From the path's variant r back to the theorem's p: r → q against the bridge, q → p along it.
        assert!(code.contains("rw [← c.q_eq_r, c.q_eq_p]\n  exact _root_.m.t x"), "{code}");
        let (t, w) = (a.by_id["m.t"], a.by_id["m.w"]);
        let v = |n: &str| a.by_id[&format!("m.c.{n}")];
        assert_eq!(g[0].via[&(t, w)], [(v("p"), v("q")), (v("q"), v("r"))]);
        let c = a.by_id["m.c"];
        assert_eq!(g[0].bridges[&(t, w)], [Key::Bridge { concept: c, index: 1 }, Key::Bridge { concept: c, index: 0 }]);
    }

    #[test]
    fn exports_concepts_that_define_objects() {
        let src = r#"module m
            concept d { params lean "(x y : ℝ)" type lean "ℝ"
              variant a { lean "|x - y|" } variant b { lean "Real.sqrt ((x - y) ^ 2)" }
              bridge a = b { proof lean "(Real.sqrt_sq_eq_abs (x - y)).symm" } }
            theorem s { params lean "(x y : ℝ)" statement lean "d x y = d y x" proof lean "abs_sub_comm x y" }
            path w { step d.b step s }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let (g, diags) = export(&ws, &a);
        assert!(diags.is_empty(), "{diags:?}");
        let code = &g[0].code;
        assert!(code.contains("noncomputable abbrev d.a (x y : ℝ) : ℝ :=\n  |x - y|"), "{code}");
        assert!(code.contains("theorem d.a_eq_b_at (x y : ℝ) :\n    (|x - y|) = (Real.sqrt ((x - y) ^ 2))"), "{code}");
        assert!(code.contains("funext x y\n  exact (d.a_eq_b_at x y)"), "{code}");
        assert!(code.contains("theorem s (x y : ℝ) :\n    d.a x y = d.a y x"), "{code}");
        assert!(code.contains("rw [← d.a_eq_b]\n  exact _root_.m.s x y"), "{code}");
    }

    #[test]
    fn exports_norms_as_functions() {
        let src = r#"module m
            norm n {
              input x : money
              input w : bool
              judgement fair : bool { criteria en "reasonable" }
              prohibition no = w
              rule cents : money = round(x, 0.01 EUR)
              constant cap : money = 1_000 EUR
              rule below : bool = x <= cap
              rule ok : bool { default below and not w unless x > 2 * cap then true unless w then false }
            }
            norm d { valid from 2025-01-01 input day : date rule late : bool = day >= 2025-07-01 }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let (g, diags) = export(&ws, &a);
        let code = &g[0].code;
        assert!(code.contains("structure Inputs where\n  /-- verkodo: `m.n.x` -/\n  x : ℚ"), "{code}");
        assert!(code.contains("def cap : ℚ :=\n  (1000 : ℚ)"), "{code}");
        assert!(code.contains("/-- verkodo: `m.n.fair` – a judgement, not a fact -/\n  fair : Bool"), "{code}");
        assert!(code.contains("def cents (i : Inputs) : ℚ :=\n  (verkodo.round i.x (0.01 : ℚ))"), "{code}");
        assert_eq!(code.matches("def verkodo.round ").count(), 1, "{code}");
        assert!(
            code.contains("`m.n.no` – a prohibition: whether it applies -/\ndef no (i : Inputs) : Bool :=\n  i.w"),
            "{code}"
        );
        assert!(code.contains("def below (i : Inputs) : Bool :=\n  decide (i.x ≤ cap)"), "{code}");
        // The first exception that applies wins; rules come after the rules they use.
        assert!(
            code.contains(
                "bif decide (i.x > ((2 : ℚ) * cap)) then true\n  else bif i.w then false\n  else ((below i) && (!i.w))"
            ),
            "{code}"
        );
        assert!(code.find("def below").unwrap() < code.find("def ok").unwrap());
        // Dates are day numbers; every norm says when it applies.
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(code.matches("def verkodo.day").count(), 1, "{code}");
        assert!(code.contains("  day : ℤ"), "{code}");
        assert!(code.contains("decide (i.day ≥ (20270 : ℤ))"), "{code}");
        assert!(code.contains("def valid_from : ℤ := 20089"), "{code}");
        assert!(code.contains("def in_force (d : ℤ) : Bool :=\n  decide (valid_from ≤ d)\n"), "{code}");
        let keys: Vec<Key> = g[0].decls.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys.iter().filter(|k| matches!(k, Key::Member(_))).count(), 6);
    }

    #[test]
    fn origins_lead_back_to_the_snippets() {
        let src = r#"module m
            concept c { params lean "(x : ℝ)" variant p { lean "0 < x ∧\nNat.succ 0 = 1" } }
            theorem t { params lean "(x : ℝ)" statement lean "c x → Real.sqrt x = x" proof lean "sorry" }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        let (g, _) = export(&ws, &a);
        let g = &g[0];
        let back = |snippet: &str, word: &str| {
            let at = g.code.find(word).unwrap();
            g.origins.iter().find_map(|o| o.source_range(at..at + word.len())).map(|r| snippet[r].to_string())
        };
        // In the second line of an indented snippet, and after a name that was replaced (`c` → `c.p`).
        assert_eq!(back("0 < x ∧\nNat.succ 0 = 1", "Nat.succ").as_deref(), Some("Nat.succ"));
        assert_eq!(back("c x → Real.sqrt x = x", "Real.sqrt").as_deref(), Some("Real.sqrt"));
        // A replaced name does not lead back.
        let at = g.code.find("c.p x →").unwrap();
        assert!(g.origins.iter().all(|o| o.source_range(at..at + 3).is_none()));
    }
}
