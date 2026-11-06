// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pages of verkodo knowledge, one content in several formats – each a feature: `html` (an index of every
//! declaration and one page per path; every resolved name is a link with a hover card whose links lead deeper, nothing
//! loaded from elsewhere), `typst` (a path as a document for PDF: links and an appendix instead of hover), `markdown`
//! (hover in editors).

#![cfg_attr(not(any(feature = "html", feature = "typst", feature = "markdown")), allow(dead_code))]

use std::fmt::Write as _;
#[cfg(feature = "html")]
use std::io;
#[cfg(feature = "html")]
use std::path::{Path, PathBuf};

use verkodo_core::proof::{Key, LeanName, Report, Status};
use verkodo_core::{Analysis, Loc, Sources, SymKind, Target};
use verkodo_syntax::ast::*;

#[cfg(feature = "html")]
mod assets;
mod format;
#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "typst")]
mod typst;

use format::Format;
#[cfg(feature = "html")]
use format::{Html, esc};
#[cfg(feature = "markdown")]
pub use markdown::{goals_hover, lean_hover, symbol_hover};
#[cfg(feature = "typst")]
pub use typst::path_document;

#[cfg(feature = "html")]
pub fn write_site(
    ws: &Sources,
    a: &Analysis,
    report: Option<&Report>,
    out: &Path,
    lang: &str,
) -> io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(out)?;
    let r = R { ws, a, report, lang, f: &Html };
    let mut written = Vec::new();
    let index = out.join("index.html");
    std::fs::write(&index, r.index())?;
    written.push(index);
    for (i, s) in a.symbols.iter().enumerate() {
        if s.kind == SymKind::Path {
            let p = out.join(format!("{}.html", s.id));
            std::fs::write(&p, r.path_page(i))?;
            written.push(p);
        }
    }
    Ok(written)
}

#[cfg(feature = "html")]
/// The index page as HTML, for a preview that writes no files.
pub fn index_html(ws: &Sources, a: &Analysis, report: Option<&Report>, lang: &str) -> String {
    R { ws, a, report, lang, f: &Html }.index()
}

#[cfg(feature = "html")]
/// The page of a path as HTML (`path` is its symbol).
pub fn path_html(ws: &Sources, a: &Analysis, report: Option<&Report>, path: usize, lang: &str) -> String {
    R { ws, a, report, lang, f: &Html }.path_page(path)
}

#[cfg(feature = "html")]
/// The hover card of a declaration as HTML – what a page shows on hover. Its classes are styled by [`CSS`].
pub fn card_html(ws: &Sources, a: &Analysis, report: Option<&Report>, sym: usize, lang: &str) -> String {
    R { ws, a, report, lang, f: &Html }.card(sym)
}

#[cfg(feature = "html")]
/// The style of the generated pages and cards.
pub const CSS: &str = assets::CSS;

struct R<'a> {
    ws: &'a Sources,
    a: &'a Analysis,
    report: Option<&'a Report>,
    lang: &'a str,
    f: &'a dyn Format,
}

impl R<'_> {
    fn w(&self, key: &str) -> &'static str {
        let (de, en) = match key {
            "concept" => ("Begriff", "Concept"),
            "variant" => ("Definition", "Definition"),
            "theorem" => ("Satz", "Theorem"),
            "example" => ("Beispiel", "Example"),
            "remark" => ("Bemerkung", "Remark"),
            "norm" => ("Norm", "Provision"),
            "input" => ("Eingabe", "Input"),
            "judgement" => ("Wertung", "Judgement"),
            "obligation" => ("Pflicht", "Obligation"),
            "permission" => ("Erlaubnis", "Permission"),
            "prohibition" => ("Verbot", "Prohibition"),
            "when" => ("gilt, wenn", "applies when"),
            "criteria" => ("Maßstab", "Criteria"),
            "judgement_note" => (
                "unbestimmter Rechtsbegriff – keine Tatsache, sondern eine Wertung, die jemand begründen muss",
                "undetermined legal term – not a fact but a judgement someone has to justify",
            ),
            "depends_on_judgement" => ("hängt ab von der Wertung", "depends on the judgement"),
            "constant" => ("Wert", "Value"),
            "rule" => ("Regel", "Rule"),
            "path" => ("Weg", "Path"),
            "params" => ("Parameter", "Parameters"),
            "param" => ("Parameter", "parameter"),
            "statement" => ("Aussage", "Statement"),
            "proof" => ("Beweis", "Proof"),
            "proof_given" => ("in Lean angegeben, nicht geprüft (--prove)", "given in Lean, not checked (--prove)"),
            "proof_open" => ("OFFEN", "OPEN"),
            "checked" => ("✓ von Lean geprüft", "✓ checked by Lean"),
            "rejected" => ("von Lean abgelehnt", "rejected by Lean"),
            "unchecked" => ("nicht an Lean übergeben", "not passed to Lean"),
            "lean" => ("Lean", "Lean"),
            "in_path" => ("In diesem Weg", "In this path"),
            "carried" => ("übertragen über", "carried over via"),
            "as_written" => ("wie geschrieben", "as written"),
            "source" => ("Quelle", "Source"),
            "valid_from" => ("gilt ab", "valid from"),
            "valid_to" => ("bis", "until"),
            "replaces" => ("ersetzt die Fassung", "replaces the version"),
            "replaced_by" => ("ersetzt durch", "replaced by"),
            "type" => ("Typ", "Type"),
            "default" => ("Grundsatz", "As a rule"),
            "unless" => ("außer wenn", "unless"),
            "then" => ("dann", "then"),
            "and" => ("und", "and"),
            "or" => ("oder", "or"),
            "not" => ("nicht", "not"),
            "if" => ("wenn", "if"),
            "else" => ("sonst", "otherwise"),
            "true" => ("wahr", "true"),
            "false" => ("falsch", "false"),
            "bridge" => ("Äquivalenz", "Equivalence"),
            "bridge_eq" => ("Gleichheit", "Equality"),
            "variants" => ("Varianten", "Variants"),
            "other_variants" => ("Andere Definitionen", "Other definitions"),
            "bridged" => ("äquivalent (Brücke)", "equivalent (bridge)"),
            "bridged_eq" => ("gleich (Brücke)", "equal (bridge)"),
            "unbridged" => ("ohne Brücke", "no bridge"),
            "about" => ("zu", "about"),
            "members" => ("Bestandteile", "Parts"),
            "steps" => ("Schritte", "Steps"),
            "external" => (
                "Lean/Mathlib-Name – Typ und Erklärung von Lean mit --prove",
                "Lean/Mathlib name – type and explanation from Lean with --prove",
            ),
            "declared_in" => ("deklariert in", "declared in"),
            "all" => ("Alle Deklarationen", "All declarations"),
            "paths" => ("Wege", "Paths"),
            "back" => ("Übersicht", "Overview"),
            "hint" => (
                "Über jeden unterstrichenen Begriff fahren – die Erklärung enthält wieder Begriffe, beliebig tief.",
                "Hover over any underlined term – its explanation contains terms again, as deep as you like.",
            ),
            "hint_pdf" => (
                "Farbige Begriffe führen zu ihrer Erklärung – in diesem Weg oder im Anhang.",
                "Coloured terms lead to their explanation – in this path or in the appendix.",
            ),
            "appendix" => ("Anhang: weitere Begriffe", "Appendix: further terms"),
            "lean_names" => ("Namen aus Lean und Mathlib", "Names from Lean and Mathlib"),
            "generated" => ("Erzeugt von verkodo aus", "Generated by verkodo from"),
            "enum" => ("Aufzählung", "Enumeration"),
            "value" => ("Ausprägung", "Value"),
            "values" => ("Ausprägungen", "Values"),
            "unit" => ("Einheit", "Unit"),
            "record" => ("Datensatz", "Record"),
            "field" => ("Feld", "Field"),
            "fields" => ("Felder", "Fields"),
            "by" => ("nach", "by"),
            "up_to" => ("bis", "up to"),
            "for_each" => ("für jedes", "for each"),
            "in" => ("in", "in"),
            "where" => ("mit", "where"),
            "sum" => ("Summe", "sum"),
            "count" => ("Anzahl", "count"),
            "agg_all" => ("alle", "all"),
            "agg_any" => ("mindestens eines", "any"),
            _ => ("?", "?"),
        };
        if self.lang == "de" { de } else { en }
    }

    fn label(&self, sym: usize) -> String {
        self.a
            .label(self.ws, sym, self.lang)
            .unwrap_or_else(|| self.a.symbols[sym].id.rsplit('.').next().unwrap_or_default().to_string())
    }

    /// A word of the page as markup.
    fn t(&self, key: &str) -> String {
        self.f.text(self.w(key))
    }

    fn sym_ref(&self, sym: usize, text: &str) -> String {
        self.f.sym(sym, &self.a.symbols[sym].id, text)
    }

    fn sym_ref_at(&self, file: usize, span: verkodo_syntax::Span, fallback: &str, use_label: bool) -> String {
        match self.a.refs.get(&(file, span)) {
            Some(Target::Symbol(s)) => {
                let text = if use_label { self.label(*s) } else { fallback.to_string() };
                self.sym_ref(*s, &text)
            }
            _ => self.f.text(fallback),
        }
    }

    // ---- Lean and expressions ----

    /// A Lean snippet with every name linked: declarations of verkodo and parameters from the analysis, every other
    /// name with what Lean said about it (with a report), else with a note.
    fn lean(&self, file: usize, l: &Lean) -> String {
        let names: &[LeanName] =
            self.report.and_then(|r| r.names.get(&(file, l.span))).map(Vec::as_slice).unwrap_or_default();
        let from_lean = |range: &std::ops::Range<usize>| names.iter().find(|n| n.range == *range);
        let mut marks: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for r in self.a.lean_refs.get(&(file, l.span)).into_iter().flatten() {
            let word = &l.code[r.range.clone()];
            let html = match &r.target {
                Target::Symbol(s) => self.sym_ref(*s, word),
                Target::Local { name, ty } => self.f.local(word, name, ty, self.w("param")),
                Target::External(n) => match from_lean(&r.range) {
                    Some(x) => self.f.lean_name(word, x, self.w("declared_in")),
                    None => self.f.external(word, n, self.w("external")),
                },
                Target::Builtin(_) => self.f.text(word),
            };
            marks.push((r.range.clone(), html));
        }
        for n in names {
            if !marks.iter().any(|(r, _)| r.start < n.range.end && n.range.start < r.end) {
                marks.push((n.range.clone(), self.f.lean_name(&l.code[n.range.clone()], n, self.w("declared_in"))));
            }
        }
        marks.sort_by_key(|(r, _)| r.start);
        let mut out = String::new();
        let mut pos = 0;
        for (r, html) in marks {
            if r.start < pos {
                continue;
            }
            out.push_str(&self.f.text(&l.code[pos..r.start]));
            out.push_str(&html);
            pos = r.end;
        }
        out.push_str(&self.f.text(&l.code[pos..]));
        self.f.code(&out)
    }

    fn number(&self, n: &str) -> String {
        let n = n.replace('_', "");
        let (int, frac) = n.split_once('.').unwrap_or((&n, ""));
        let (group, dec) = if self.lang == "de" { ('.', ',') } else { (',', '.') };
        let mut grouped = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(group);
            }
            grouped.push(c);
        }
        if frac.is_empty() { grouped } else { format!("{grouped}{dec}{frac}") }
    }

    fn expr(&self, file: usize, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Ref(p) => self.sym_ref_at(file, p.span, &p.joined(), true),
            ExprKind::Money(n, unit) if unit.text != "EUR" => {
                format!(
                    "{}{}{}",
                    self.f.text(&self.number(n)),
                    self.f.nbsp(),
                    self.sym_ref_at(file, unit.span, &unit.text, true)
                )
            }
            ExprKind::Money(n, cur) => {
                let sym = if cur.text == "EUR" { "€" } else { &cur.text };
                if self.lang == "de" {
                    format!("{}{}{}", self.f.text(&self.number(n)), self.f.nbsp(), self.f.text(sym))
                } else {
                    self.f.text(&format!("{sym}{}", self.number(n)))
                }
            }
            ExprKind::Number(n) => self.f.text(&self.number(n)),
            ExprKind::Percent(n) => {
                let sep = if self.lang == "de" { self.f.nbsp() } else { "" };
                format!("{}{sep}{}", self.f.text(&self.number(n)), self.f.text("%"))
            }
            ExprKind::Bool(b) => self.t(if *b { "true" } else { "false" }),
            ExprKind::Date(d) => self.f.text(d),
            ExprKind::Unary(UnOp::Not, x) => format!("{} {}", self.t("not"), self.expr(file, x)),
            ExprKind::Unary(UnOp::Neg, x) => format!("{}{}", self.f.text("−"), self.expr(file, x)),
            ExprKind::Paren(x) => format!("{}{}{}", self.f.text("("), self.expr(file, x), self.f.text(")")),
            ExprKind::If(c, x, y) => format!(
                "{} {}{} {} {}{} {} {}",
                self.t("if"),
                self.expr(file, c),
                self.f.text(","),
                self.t("then"),
                self.expr(file, x),
                self.f.text(","),
                self.t("else"),
                self.expr(file, y)
            ),
            ExprKind::Call(p, args) => {
                let args: Vec<String> = args.iter().map(|x| self.expr(file, x)).collect();
                format!(
                    "{}{}{}",
                    self.f.text(&format!("{}(", p.joined())),
                    args.join(&self.f.text(", ")),
                    self.f.text(")")
                )
            }
            ExprKind::Binary(op, l, r) => {
                let op = match op {
                    BinOp::And => self.w("and"),
                    BinOp::Or => self.w("or"),
                    BinOp::Lt => "<",
                    BinOp::Le => "≤",
                    BinOp::Gt => ">",
                    BinOp::Ge => "≥",
                    BinOp::Eq => "=",
                    BinOp::Ne => "≠",
                    BinOp::Add => "+",
                    BinOp::Sub => "−",
                    BinOp::Mul => "·",
                    BinOp::Div => "∕",
                };
                let sep = if matches!(e.kind, ExprKind::Binary(BinOp::And | BinOp::Or, ..)) {
                    self.f.linebreak()
                } else {
                    " "
                };
                format!("{}{sep}{} {}", self.expr(file, l), self.f.text(op), self.expr(file, r))
            }
            ExprKind::Aggregate(agg) => {
                let word = match agg.func.text.as_str() {
                    "all" => "agg_all",
                    "any" => "agg_any",
                    other => other,
                };
                let mut out = format!("{}{}", self.t(word), self.f.text("("));
                if let Some(b) = &agg.body {
                    out += &format!("{} {} ", self.expr(file, b), self.t("for_each"));
                }
                out += &format!("{} {} {}", self.f.text(&agg.var.text), self.t("in"), self.expr(file, &agg.list));
                if let Some(f) = &agg.filter {
                    out += &format!(" {} {}", self.t("where"), self.expr(file, f));
                }
                out + &self.f.text(")")
            }
            ExprKind::Apply(p, args) => {
                let args: Vec<String> = args
                    .iter()
                    .map(|(n, x)| {
                        format!(
                            "{} {} {}",
                            self.sym_ref_at(file, n.span, &n.text, true),
                            self.f.text("="),
                            self.expr(file, x)
                        )
                    })
                    .collect();
                format!(
                    "{}{}{}{}",
                    self.sym_ref_at(file, p.span, &p.joined(), true),
                    self.f.text("("),
                    args.join(&self.f.text(", ")),
                    self.f.text(")")
                )
            }
            ExprKind::Table(tbl) => {
                let mut out = format!("{} {}", self.t("by"), self.expr(file, &tbl.subject));
                for r in &tbl.rows {
                    let key = match &r.key {
                        RowKey::UpTo(b) => format!("{} {}", self.t("up_to"), self.expr(file, b)),
                        RowKey::Value(p) => self.sym_ref_at(file, p.span, &p.joined(), true),
                    };
                    out += &format!("{}{key}{} {}", self.f.linebreak(), self.f.text(":"), self.expr(file, &r.value));
                }
                if let Some(o) = &tbl.otherwise {
                    out +=
                        &format!("{}{}{} {}", self.f.linebreak(), self.t("else"), self.f.text(":"), self.expr(file, o));
                }
                out
            }
            ExprKind::Error => self.f.text("?"),
        }
    }

    fn sources(&self, meta: &Meta) -> String {
        meta.sources.iter().map(|(s, _)| self.f.source(self.w("source"), s)).collect()
    }

    fn row(&self, key: &str, value: &str) -> String {
        self.f.row(&self.t(key), value)
    }

    // ---- Lean's verdict ----

    /// How Lean judged a part; without a report only whether a proof is given.
    fn status(&self, key: Key, given: bool) -> String {
        let span = |class: &str, word: &str| self.f.status(class, self.w(word));
        match self.report {
            None if given => self.t("proof_given"),
            None => span("open", "proof_open"),
            Some(r) => match r.get(key) {
                Some(Status::Checked) => span("ok", "checked"),
                Some(Status::Open) => span("open", "proof_open"),
                Some(Status::Failed(errors)) => format!(
                    "{}{}",
                    self.f.status("open", self.w("rejected")),
                    self.f.error(errors.first().map(String::as_str).unwrap_or_default())
                ),
                None => self.t("unchecked"),
            },
        }
    }

    #[cfg(any(feature = "html", feature = "typst"))]
    /// For a theorem on a path page: whether it holds for the variants this path chose.
    fn in_path(&self, sym: usize, path: usize) -> String {
        let (Some(r), SymKind::Theorem | SymKind::Example) = (self.report, self.a.symbols[sym].kind) else {
            return String::new();
        };
        let key = Key::Transport { theorem: sym, path };
        let how = match r.transports.get(&(sym, path)) {
            Some(pairs) => {
                let via: Vec<String> = pairs
                    .iter()
                    .map(|&(from, to)| {
                        format!(
                            "{}{}{}",
                            self.sym_ref(from, &self.label(from)),
                            self.f.text(if self.a.defines_object(self.ws, from) { " = " } else { " ⟺ " }),
                            self.sym_ref(to, &self.label(to))
                        )
                    })
                    .collect();
                format!("{} {}", self.t("carried"), via.join(&self.f.text(", ")))
            }
            None => self.t("as_written"),
        };
        self.row("in_path", &format!("{how}{}{}", self.f.text(" – "), self.status(key, true)))
    }

    // ---- bodies ----

    /// A version of a norm by its label and when it begins, e.g. "Kleinunternehmer (ab 2020-01-01)".
    fn version_label(&self, norm: usize) -> String {
        let from = match self.a.symbols[norm].loc {
            Loc::Item { file, item } => match &self.ws.asts[file].items[item] {
                Item::Norm(n) => n.valid_from.as_ref().map(|(d, _)| d.clone()),
                _ => None,
            },
            _ => None,
        };
        match from {
            Some(d) => format!("{} ({} {d})", self.label(norm), self.w("valid_from")),
            None => self.label(norm),
        }
    }

    fn body(&self, sym: usize) -> String {
        let s = &self.a.symbols[sym];
        let ws = self.ws;
        let mut out = String::new();
        match s.loc {
            Loc::Item { file, item } => match &ws.asts[file].items[item] {
                Item::Concept(c) => {
                    if let Some(p) = &c.params {
                        out += &self.row("params", &self.lean(file, p));
                    }
                    if let Some(t) = &c.ty {
                        out += &self.row("type", &self.lean(file, t));
                    }
                    for v in &c.variants {
                        let vid = format!("{}.{}", s.id, v.name.text);
                        let Some(&vs) = self.a.by_id.get(&vid) else {
                            continue;
                        };
                        let def = v.def.as_ref().map(|d| self.lean(file, d)).unwrap_or_default();
                        out += &self.f.row(&self.sym_ref(vs, &self.label(vs)), &def);
                    }
                    for (index, b) in c.bridges.iter().enumerate() {
                        let status = self.status(Key::Bridge { concept: sym, index }, b.proof.is_some());
                        let l = self.sym_ref_at(file, b.left.span, &b.left.text, true);
                        let r = self.sym_ref_at(file, b.right.span, &b.right.text, true);
                        let (op, word) = if b.eq { (" = ", "bridge_eq") } else { (" ⟺ ", "bridge") };
                        let line = format!("{l}{}{r} {}", self.f.text(op), self.f.note(&status));
                        out += &self.row(word, &line);
                    }
                    out += &self.sources(&c.meta);
                }
                Item::Theorem(t) => {
                    if let Some(p) = &t.params {
                        out += &self.row("params", &self.lean(file, p));
                    }
                    if let Some(st) = &t.statement {
                        out += &self.row("statement", &self.lean(file, st));
                    }
                    let status = self.status(Key::Theorem(sym), t.proof.is_some());
                    out += &match &t.proof {
                        Some(p) => self.row("proof", &self.f.details(&status, &self.lean(file, p))),
                        None => self.row("proof", &status),
                    };
                    out += &self.sources(&t.meta);
                }
                Item::Remark(r) => {
                    let about: Vec<String> =
                        r.about.iter().map(|p| self.sym_ref_at(file, p.span, &p.joined(), true)).collect();
                    if !about.is_empty() {
                        out += &self.row("about", &about.join(&self.f.text(", ")));
                    }
                    if let Some(t) = self.a.text_in(self.ws, sym, verkodo_core::Field::Text, self.lang) {
                        out += &self.f.para(&t);
                    }
                    out += &self.sources(&r.meta);
                }
                Item::Norm(n) => {
                    out += &self.sources(&n.meta);
                    if let Some((d, _)) = &n.valid_from {
                        let mut v = self.f.text(d);
                        if let Some((t, _)) = &n.valid_to {
                            let _ = write!(v, " {} {}", self.t("valid_to"), self.f.text(t));
                        }
                        out += &self.row("valid_from", &v);
                    }
                    if let Some(&old) = self.a.replaces.get(&sym) {
                        out += &self.row("replaces", &self.sym_ref(old, &self.version_label(old)));
                    }
                    if let Some(&new) = self.a.replaced_by.get(&sym) {
                        out += &self.row("replaced_by", &self.sym_ref(new, &self.version_label(new)));
                    }
                    let members: Vec<String> = n
                        .members
                        .iter()
                        .filter_map(|m| self.a.by_id.get(&format!("{}.{}", s.id, m.name.text)))
                        .map(|&m| self.sym_ref(m, &self.label(m)))
                        .collect();
                    out += &self.row("members", &members.join(&self.f.text(" · ")));
                    if self.report.is_some() {
                        out += &self.row("lean", &self.status(Key::Norm(sym), true));
                    }
                }
                Item::Path(d) => {
                    let steps: Vec<String> =
                        d.steps.iter().map(|p| self.sym_ref_at(file, p.span, &p.joined(), true)).collect();
                    out += &self.f.ordered(&steps);
                }
                Item::Enum(e) => {
                    let values: Vec<String> = e
                        .values
                        .iter()
                        .filter_map(|v| self.a.by_id.get(&format!("{}.{}", s.id, v.name.text)))
                        .map(|&v| self.sym_ref(v, &self.label(v)))
                        .collect();
                    out += &self.row("values", &values.join(&self.f.text(" · ")));
                    out += &self.sources(&e.meta);
                }
                Item::Unit(u) => out += &self.sources(&u.meta),
                Item::Record(r) => {
                    let fields: Vec<String> = r
                        .fields
                        .iter()
                        .filter_map(|f| self.a.by_id.get(&format!("{}.{}", s.id, f.name.text)))
                        .map(|&f| self.sym_ref(f, &self.label(f)))
                        .collect();
                    out += &self.row("fields", &fields.join(&self.f.text(" · ")));
                    out += &self.sources(&r.meta);
                }
            },
            Loc::Variant { file, item, variant } => {
                let Item::Concept(c) = &ws.asts[file].items[item] else {
                    return out;
                };
                let v = &c.variants[variant];
                if let Some(p) = &c.params {
                    out += &self.row("params", &self.lean(file, p));
                }
                if let Some(t) = &c.ty {
                    out += &self.row("type", &self.lean(file, t));
                }
                if let Some(d) = &v.def {
                    out += &self.row("variant", &self.lean(file, d));
                }
                if self.report.is_some() {
                    out += &self.row("lean", &self.status(Key::Variant(sym), true));
                }
                let others: Vec<String> = c
                    .variants
                    .iter()
                    .filter(|o| o.name.text != v.name.text)
                    .filter_map(|o| self.a.by_id.get(&format!("{}.{}", s.id.rsplit_once('.').unwrap().0, o.name.text)))
                    .map(|&o| {
                        let note = match (self.a.bridged(ws, sym, o), c.ty.is_some()) {
                            (true, true) => self.w("bridged_eq"),
                            (true, false) => self.w("bridged"),
                            (false, _) => self.w("unbridged"),
                        };
                        format!("{} {}", self.sym_ref(o, &self.label(o)), self.f.note(&self.f.text(note)))
                    })
                    .collect();
                if !others.is_empty() {
                    out += &self.row("other_variants", &others.join(&self.f.text(", ")));
                }
                out += &self.sources(&v.meta);
            }
            Loc::Member { file, item, member } => {
                let type_row = |ty: &TypeRef| {
                    let name = match self.a.refs.get(&(file, ty.name.span)) {
                        Some(_) => self.sym_ref_at(file, ty.name.span, &ty.name.joined(), true),
                        None => self.f.mono(&ty.name.joined()),
                    };
                    let list = if ty.list { format!("{} ", self.f.mono("list")) } else { String::new() };
                    self.row("type", &format!("{list}{name}"))
                };
                let n = match &ws.asts[file].items[item] {
                    Item::Norm(n) => n,
                    Item::Record(r) => {
                        out += &type_row(&r.fields[member].ty);
                        out += &self.sources(&r.fields[member].meta);
                        return out;
                    }
                    Item::Enum(e) => {
                        if let Some(p) = self.a.parent(sym) {
                            out += &self.row("enum", &self.sym_ref(p, &self.label(p)));
                        }
                        out += &self.sources(&e.values[member].meta);
                        return out;
                    }
                    _ => return out,
                };
                let m = &n.members[member];
                out += &type_row(&m.ty);
                match &m.kind {
                    MemberKind::Input => {}
                    MemberKind::Judgement { .. } => {
                        if let Some(t) = self.a.text_in(self.ws, sym, verkodo_core::Field::Criteria, self.lang) {
                            out += &self.row("criteria", &self.f.text(&t));
                        }
                        out += &self.f.para(self.w("judgement_note"));
                    }
                    MemberKind::Constant(e) => out += &self.row("constant", &self.expr(file, e)),
                    MemberKind::Rule { default, unless } => {
                        if let Some(e) = default {
                            let word = if m.modality.is_some() { "when" } else { "default" };
                            out += &self.row(word, &self.expr(file, e));
                        }
                        for u in unless {
                            let line = format!(
                                "{} {} {}{}",
                                self.expr(file, &u.cond),
                                self.f.key(self.w("then")),
                                self.expr(file, &u.value),
                                self.sources(&u.meta)
                            );
                            out += &self.row("unless", &line);
                        }
                    }
                }
                if matches!(m.kind, MemberKind::Rule { .. } | MemberKind::Constant(_)) {
                    let behind: Vec<String> =
                        self.a.judgements_behind(sym).into_iter().map(|j| self.sym_ref(j, &self.label(j))).collect();
                    if !behind.is_empty() {
                        out += &self.row("depends_on_judgement", &behind.join(&self.f.text(", ")));
                    }
                }
                if self.report.is_some() && matches!(m.kind, MemberKind::Rule { .. } | MemberKind::Constant(_)) {
                    out += &self.row("lean", &self.status(Key::Member(sym), true));
                }
                out += &self.sources(&m.meta);
            }
        }
        out
    }

    fn kind_word(&self, sym: usize) -> &'static str {
        match self.modality(sym) {
            Some(m) => self.w(m.keyword()),
            None => self.w(self.a.symbols[sym].kind.keyword()),
        }
    }

    /// Whether a rule is an obligation, a permission or a prohibition.
    fn modality(&self, sym: usize) -> Option<Modality> {
        let Loc::Member { file, item, member } = self.a.symbols[sym].loc else { return None };
        let Item::Norm(n) = &self.ws.asts[file].items[item] else { return None };
        n.members[member].modality
    }

    fn title(&self, sym: usize) -> String {
        match self.a.symbols[sym].kind {
            SymKind::Variant => {
                let concept = self.a.parent(sym).map(|c| self.label(c)).unwrap_or_default();
                format!("{} ({})", concept, self.label(sym))
            }
            _ => self.label(sym),
        }
    }

    #[cfg(feature = "html")]
    fn card(&self, sym: usize) -> String {
        format!(
            r#"<div class="kind">{}</div><div class="title">{}</div><div class="id">{}</div>{}"#,
            self.kind_word(sym),
            esc(&self.title(sym)),
            esc(&self.a.symbols[sym].id),
            self.body(sym)
        )
    }

    #[cfg(feature = "html")]
    fn templates(&self) -> String {
        (0..self.a.symbols.len())
            .map(|i| format!(r#"<template id="t-{}">{}</template>"#, esc(&self.a.symbols[i].id), self.card(i)))
            .collect()
    }

    #[cfg(feature = "html")]
    fn page(&self, title: &str, main: &str) -> String {
        format!(
            r#"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>{css}</style>
</head>
<body>
<main>
{main}
</main>
{templates}
<script>{js}</script>
</body>
</html>
"#,
            lang = esc(self.lang),
            title = esc(title),
            css = assets::CSS,
            js = assets::JS,
            templates = self.templates(),
        )
    }

    // ---- pages ----

    #[cfg(feature = "html")]
    fn index(&self) -> String {
        let mut main = format!(r#"<h1>verkodo</h1><p class="hint">{}</p>"#, self.w("hint"));
        let paths: Vec<usize> =
            (0..self.a.symbols.len()).filter(|&i| self.a.symbols[i].kind == SymKind::Path).collect();
        if !paths.is_empty() {
            let _ = write!(main, "<h2>{}</h2><ul>", self.w("paths"));
            for p in paths {
                let id = esc(&self.a.symbols[p].id);
                let _ = write!(main, r#"<li><a href="{id}.html">{}</a></li>"#, esc(&self.label(p)));
            }
            main.push_str("</ul>");
        }
        let _ = write!(main, "<h2>{}</h2>", self.w("all"));
        for (file, ast) in self.ws.asts.iter().enumerate() {
            let Some(module) = &ast.module else { continue };
            let _ = write!(main, "<h3><code>{}</code></h3>", esc(&module.joined()));
            for (i, s) in self.a.symbols.iter().enumerate() {
                if s.loc.file() == file {
                    let _ = write!(main, r#"<section class="decl" id="s-{}">{}</section>"#, esc(&s.id), self.card(i));
                }
            }
        }
        self.page("verkodo", &main)
    }

    #[cfg(feature = "html")]
    fn path_page(&self, path: usize) -> String {
        let Loc::Item { file, item } = self.a.symbols[path].loc else {
            return String::new();
        };
        let Item::Path(d) = &self.ws.asts[file].items[item] else {
            return String::new();
        };
        let title = self.label(path);
        let mut main = format!(
            r#"<nav><a href="index.html">← {}</a></nav><h1>{}</h1><p class="hint">{}</p>"#,
            self.w("back"),
            esc(&title),
            self.w("hint")
        );
        for (n, step) in d.steps.iter().enumerate() {
            let Some(Target::Symbol(s)) = self.a.refs.get(&(file, step.span)) else {
                continue;
            };
            let _ = write!(
                main,
                r#"<section class="step"><h2><span class="num">{}</span> {}: {}</h2>{}</section>"#,
                n + 1,
                self.kind_word(*s),
                esc(&self.title(*s)),
                self.body(*s) + &self.in_path(*s, path)
            );
        }
        self.page(&title, &main)
    }
}
