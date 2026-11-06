// SPDX-License-Identifier: AGPL-3.0-or-later
//! Recursive-descent parser. It recovers from errors by skipping a token, so one mistake yields one diagnostic and
//! the rest of the file is still read.

use crate::ast::*;
use crate::lexer::{Tok, Token, lex};
use crate::source::{Diagnostic, Span};

pub fn parse(text: &str, file: usize, diags: &mut Vec<Diagnostic>) -> File {
    parse_layout(text, file, diags).0
}

/// Parses and also returns the tokens and the layout – where blocks and their entries are – for the formatter.
pub fn parse_layout(text: &str, file: usize, diags: &mut Vec<Diagnostic>) -> (File, Vec<Token>, Block) {
    let toks = lex(text, file, diags);
    let mut p = Parser { toks, pos: 0, file, diags, events: Vec::new() };
    let f = p.file();
    let mut at = 0;
    let layout = build(&p.events, &mut at);
    (f, p.toks, layout)
}

/// Running text from a block of prose: a line break is a space – lines may be broken anywhere, by hand or by the
/// formatter –, a blank line separates paragraphs.
pub fn prose(block: &str) -> String {
    block
        .split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// A block as written: its entries, and its braces if it has them (token indices of `{` and `}`); the file is a
/// block without braces.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Block {
    pub braces: Option<(usize, usize)>,
    pub entries: Vec<Entry>,
    /// The token after the block.
    pub end: usize,
}

/// An entry of a block – a declaration, a `label`, a `step` … –: its tokens `start..end`, with the blocks it opens.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub start: usize,
    pub end: usize,
    pub blocks: Vec<Block>,
}

/// What the parser saw of the layout, in order.
#[derive(Clone, Debug)]
enum Event {
    Open(Option<usize>),
    Entry(usize),
    Close { brace: Option<usize>, end: usize },
}

/// The block whose `Open` is at `events[*at]`.
fn build(events: &[Event], at: &mut usize) -> Block {
    let mut b = Block::default();
    let open = match events.get(*at) {
        Some(Event::Open(o)) => *o,
        _ => None,
    };
    *at += 1;
    while let Some(e) = events.get(*at) {
        match e {
            Event::Open(_) => {
                let child = build(events, at);
                if let Some(entry) = b.entries.last_mut() {
                    entry.blocks.push(child);
                }
            }
            Event::Entry(start) => {
                if let Some(prev) = b.entries.last_mut() {
                    prev.end = *start;
                }
                b.entries.push(Entry { start: *start, end: *start, blocks: Vec::new() });
                *at += 1;
            }
            Event::Close { brace, end } => {
                let last_end = brace.unwrap_or(*end);
                if let Some(prev) = b.entries.last_mut() {
                    prev.end = last_end;
                }
                b.braces = open.zip(*brace);
                b.end = *end;
                *at += 1;
                break;
            }
        }
    }
    // An entry ends at the latest where the blocks it opened end.
    for e in &mut b.entries {
        if let Some(last) = e.blocks.last() {
            e.end = e.end.max(last.end);
        }
    }
    b
}

/// Words that end an expression or continue the declaration around it: a number followed by one of these is not an
/// amount of a unit (`if x > 5 then …`).
const KEYWORDS: &[&str] = &[
    "and",
    "or",
    "not",
    "if",
    "then",
    "else",
    "unless",
    "when",
    "default",
    "by",
    "up",
    "to",
    "otherwise",
    "for",
    "in",
    "where",
    "label",
    "source",
    "text",
    "criteria",
    "input",
    "judgement",
    "constant",
    "rule",
    "obligation",
    "permission",
    "prohibition",
    "value",
    "field",
    "valid",
    "from",
    "replaces",
    "step",
    "true",
    "false",
];

struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    file: usize,
    diags: &'a mut Vec<Diagnostic>,
    events: Vec<Event>,
}

impl Parser<'_> {
    // ---- token helpers ----

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn span(&self) -> Span {
        self.toks[self.pos].span
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn at(&self, t: &Tok) -> bool {
        self.peek() == t
    }

    fn at_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Tok::Ident(s) if s == kw)
    }

    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(self.file, span, msg));
    }

    fn expected(&mut self, what: &str) {
        let found = self.peek().describe();
        let span = self.span();
        self.error(span, format!("expected {what}, found {found}"));
    }

    /// Reports the current token as unexpected and skips it.
    fn unexpected(&mut self, context: &str) {
        let found = self.peek().describe();
        let span = self.span();
        self.error(span, format!("unexpected {found} in {context}"));
        self.bump();
    }

    fn expect(&mut self, t: &Tok, what: &str) -> bool {
        if self.at(t) {
            self.bump();
            true
        } else {
            self.expected(what);
            false
        }
    }

    fn expect_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.bump();
            true
        } else {
            self.expected(&format!("`{kw}`"));
            false
        }
    }

    fn name(&mut self) -> Option<Name> {
        if let Tok::Ident(s) = self.peek() {
            let text = s.clone();
            let span = self.bump().span;
            Some(Name { text, span })
        } else {
            self.expected("a name");
            None
        }
    }

    /// A text: `"…"`, a block between fences, or text without quotes (see the lexer).
    fn string(&mut self) -> Option<(String, Span)> {
        if let Tok::Str(s) | Tok::Block(s) = self.peek() {
            let value = s.clone();
            Some((value, self.bump().span))
        } else {
            self.expected("a string");
            None
        }
    }

    fn date(&mut self) -> Option<(String, Span)> {
        if let Tok::Date(d) = self.peek() {
            let value = d.clone();
            Some((value, self.bump().span))
        } else {
            self.expected("a date (YYYY-MM-DD)");
            None
        }
    }

    fn path(&mut self) -> Option<Path> {
        let first = self.name()?;
        let mut span = first.span;
        let mut segments = vec![first];
        while self.at(&Tok::Dot) {
            self.bump();
            let seg = self.name()?;
            span = span.to(seg.span);
            segments.push(seg);
        }
        Some(Path { segments, span })
    }

    /// `lean "<code>"`
    /// `lean "…"` or `lean` and a block between fences.
    fn lean(&mut self) -> Option<Lean> {
        if !self.expect_kw("lean") {
            return None;
        }
        self.string().map(|(code, span)| Lean { code, span })
    }

    fn text(&mut self) -> Option<Text> {
        let start = self.bump().span;
        let lang = self.name()?;
        let (value, end) = self.prose()?;
        Some(Text { lang, value, span: start.to(end) })
    }

    /// A text of prose – a label, a remark's text, criteria, a source: `"…"` word for word; a block (between fences
    /// or by indentation) as running text, see [`prose`].
    fn prose(&mut self) -> Option<(String, Span)> {
        let block = matches!(self.peek(), Tok::Block(_));
        self.string().map(|(v, span)| (if block { prose(&v) } else { v }, span))
    }

    /// Parses `label` and `source` lines; returns whether one was found.
    fn meta(&mut self, meta: &mut Meta) -> bool {
        if self.at_kw("label") {
            if let Some(t) = self.text() {
                meta.labels.push(t);
            }
            true
        } else if self.at_kw("source") {
            self.bump();
            if let Some(s) = self.prose() {
                meta.sources.push(s);
            }
            true
        } else {
            false
        }
    }

    /// The indentation of the line of the token just read – for a keyword, the level its block is deeper than.
    fn level(&self) -> usize {
        self.toks[self.pos.saturating_sub(1)].line_indent
    }

    /// Whether a block starts here: `{`, or a line indented deeper than `level`.
    fn at_block(&self, level: usize) -> bool {
        let t = &self.toks[self.pos];
        t.tok == Tok::LBrace || (t.first && t.line_indent > level && t.tok != Tok::Eof)
    }

    /// Runs `body` for each entry of a block – `{ … }`, or the lines indented deeper than `level` (the line of the
    /// keyword that opens it) – and returns the span where it ends.
    fn block(&mut self, level: usize, mut body: impl FnMut(&mut Self)) -> Span {
        if self.at(&Tok::LBrace) {
            self.events.push(Event::Open(Some(self.pos)));
            self.bump();
            while !self.at(&Tok::RBrace) && !self.at(&Tok::Eof) {
                let before = self.pos;
                self.events.push(Event::Entry(self.pos));
                body(self);
                if self.pos == before {
                    self.bump();
                }
            }
            let end = self.span();
            let brace = self.at(&Tok::RBrace).then_some(self.pos);
            self.expect(&Tok::RBrace, "`}`");
            self.events.push(Event::Close { brace, end: self.pos });
            return end;
        }
        if !self.at_block(level) {
            self.expected("`{` or lines indented below");
            return self.span();
        }
        self.events.push(Event::Open(None));
        // Until a line at this level or further left, or a brace that closes a block around.
        let inside = |p: &Self| {
            let t = &p.toks[p.pos];
            t.tok != Tok::Eof && t.tok != Tok::RBrace && !(t.first && t.line_indent <= level)
        };
        while inside(self) {
            let before = self.pos;
            self.events.push(Event::Entry(self.pos));
            body(self);
            if self.pos == before {
                self.bump();
            }
        }
        self.events.push(Event::Close { brace: None, end: self.pos });
        self.toks[self.pos.saturating_sub(1)].span
    }

    /// An optional block of `label` and `source` – in braces or indented below `level`.
    fn opt_meta_block(&mut self, level: usize, meta: &mut Meta) -> Option<Span> {
        if !self.at_block(level) {
            return None;
        }
        Some(self.block(level, |p| {
            if !p.meta(meta) {
                p.unexpected("this block (only `label` and `source` are allowed)");
            }
        }))
    }

    // ---- items ----

    fn file(&mut self) -> File {
        let mut file = File::default();
        self.events.push(Event::Open(None));
        self.events.push(Event::Entry(self.pos));
        if self.at_kw("module") {
            self.bump();
            file.module = self.path();
            if self.at_kw("original") {
                self.bump();
                file.originals.extend(self.name());
                while self.at(&Tok::Comma) {
                    self.bump();
                    file.originals.extend(self.name());
                }
            }
        } else if self.at_kw("translation") {
            self.bump();
            file.translation = self.path();
            while !self.at(&Tok::Eof) {
                self.events.push(Event::Entry(self.pos));
                file.translated.extend(self.translated());
            }
            self.events.push(Event::Close { brace: None, end: self.pos });
            return file;
        } else {
            self.expected("`module <name>` or `translation <module>` at the start of the file");
        }
        while !self.at(&Tok::Eof) {
            self.events.push(Event::Entry(self.pos));
            let item = match self.peek() {
                Tok::Ident(k) if k == "concept" => self.concept().map(Item::Concept),
                Tok::Ident(k) if k == "theorem" || k == "example" => self.theorem().map(Item::Theorem),
                Tok::Ident(k) if k == "remark" => self.remark().map(Item::Remark),
                Tok::Ident(k) if k == "norm" => self.norm().map(Item::Norm),
                Tok::Ident(k) if k == "path" => self.path_decl().map(Item::Path),
                Tok::Ident(k) if k == "enum" => self.enum_decl().map(Item::Enum),
                Tok::Ident(k) if k == "unit" => self.unit_decl().map(Item::Unit),
                Tok::Ident(k) if k == "record" => self.record_decl().map(Item::Record),
                _ => {
                    self.unexpected(
                        "a file (expected concept, theorem, example, remark, norm, path, enum, unit or record)",
                    );
                    continue;
                }
            };
            file.items.extend(item);
        }
        self.events.push(Event::Close { brace: None, end: self.pos });
        file
    }

    /// `<declaration> { label fr "…" text fr "…" criteria fr "…" }` in a translation file.
    fn translated(&mut self) -> Option<Translated> {
        let level = self.toks[self.pos].line_indent;
        let target = match self.path() {
            Some(p) => p,
            None => {
                self.bump();
                return None;
            }
        };
        let mut t =
            Translated { span: target.span, target, labels: Vec::new(), texts: Vec::new(), criteria: Vec::new() };
        let end = self.block(level, |p| {
            if p.at_kw("label") {
                t.labels.extend(p.text());
            } else if p.at_kw("text") {
                t.texts.extend(p.text());
            } else if p.at_kw("criteria") {
                t.criteria.extend(p.text());
            } else {
                p.unexpected("a translation (expected label, text or criteria)");
            }
        });
        t.span = t.span.to(end);
        Some(t)
    }

    fn concept(&mut self) -> Option<Concept> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut c = Concept {
            name,
            meta: Meta::default(),
            params: None,
            ty: None,
            variants: Vec::new(),
            bridges: Vec::new(),
            span: start,
        };
        let end = self.block(level, |p| {
            if p.meta(&mut c.meta) {
            } else if p.at_kw("params") {
                p.bump();
                c.params = p.lean();
            } else if p.at_kw("type") {
                p.bump();
                c.ty = p.lean();
            } else if p.at_kw("variant") {
                c.variants.extend(p.variant());
            } else if p.at_kw("bridge") {
                c.bridges.extend(p.bridge());
            } else {
                p.unexpected("a concept (expected label, source, params, type, variant or bridge)");
            }
        });
        c.span = start.to(end);
        Some(c)
    }

    fn variant(&mut self) -> Option<Variant> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut v = Variant { name, meta: Meta::default(), def: None, span: start };
        let end = self.block(level, |p| {
            if p.meta(&mut v.meta) {
            } else if p.at_kw("lean") {
                v.def = p.lean();
            } else {
                p.unexpected("a variant (expected label, source or lean)");
            }
        });
        v.span = start.to(end);
        Some(v)
    }

    fn bridge(&mut self) -> Option<Bridge> {
        let start = self.bump().span;
        let level = self.level();
        let left = self.name()?;
        let eq = self.at(&Tok::Eq);
        if !eq && !self.expect(&Tok::Iff, "`<->` or `=`") {
            return None;
        }
        if eq {
            self.bump();
        }
        let right = self.name()?;
        let mut b = Bridge { left, right, eq, proof: None, span: start };
        let mut end = b.right.span;
        if self.at_block(level) {
            end = self.block(level, |p| {
                if p.at_kw("proof") {
                    p.bump();
                    b.proof = p.lean();
                } else {
                    p.unexpected("a bridge (expected proof)");
                }
            });
        }
        b.span = start.to(end);
        Some(b)
    }

    fn theorem(&mut self) -> Option<Theorem> {
        let kw = self.bump();
        let level = kw.line_indent;
        let kind = if kw.tok == Tok::Ident("example".into()) { TheoremKind::Example } else { TheoremKind::Theorem };
        let name = self.name()?;
        let mut t = Theorem {
            kind,
            name,
            meta: Meta::default(),
            params: None,
            using: Vec::new(),
            statement: None,
            proof: None,
            span: kw.span,
        };
        let end = self.block(level, |p| {
            if p.meta(&mut t.meta) {
            } else if p.at_kw("params") {
                p.bump();
                t.params = p.lean();
            } else if p.at_kw("using") {
                p.bump();
                t.using.extend(p.path());
                while p.at(&Tok::Comma) {
                    p.bump();
                    t.using.extend(p.path());
                }
            } else if p.at_kw("statement") {
                p.bump();
                t.statement = p.lean();
            } else if p.at_kw("proof") {
                p.bump();
                t.proof = p.lean();
            } else {
                p.unexpected("a theorem (expected label, source, params, using, statement or proof)");
            }
        });
        t.span = kw.span.to(end);
        if t.statement.is_none() {
            self.error(t.name.span, format!("`{}` has no statement", t.name.text));
        }
        Some(t)
    }

    fn remark(&mut self) -> Option<Remark> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut r = Remark { name, meta: Meta::default(), about: Vec::new(), texts: Vec::new(), span: start };
        let end = self.block(level, |p| {
            if p.meta(&mut r.meta) {
            } else if p.at_kw("about") {
                p.bump();
                r.about.extend(p.path());
                while p.at(&Tok::Comma) {
                    p.bump();
                    r.about.extend(p.path());
                }
            } else if p.at_kw("text") {
                r.texts.extend(p.text());
            } else {
                p.unexpected("a remark (expected label, source, about or text)");
            }
        });
        r.span = start.to(end);
        Some(r)
    }

    fn norm(&mut self) -> Option<Norm> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut n = Norm {
            name,
            meta: Meta::default(),
            valid_from: None,
            valid_to: None,
            replaces: None,
            members: Vec::new(),
            span: start,
        };
        let end = self.block(level, |p| {
            if p.meta(&mut n.meta) {
            } else if p.at_kw("valid") {
                p.bump();
                if p.expect_kw("from") {
                    n.valid_from = p.date();
                }
                if p.at_kw("to") {
                    p.bump();
                    n.valid_to = p.date();
                }
            } else if p.at_kw("replaces") {
                p.bump();
                n.replaces = p.path();
            } else if ["input", "judgement", "constant", "rule", "obligation", "permission", "prohibition"]
                .iter()
                .any(|k| p.at_kw(k))
            {
                n.members.extend(p.member());
            } else {
                p.unexpected("a norm (expected label, source, valid, replaces, input, judgement, constant, rule, obligation, permission or prohibition)");
            }
        });
        n.span = start.to(end);
        Some(n)
    }

    fn member(&mut self) -> Option<Member> {
        let kw = self.bump();
        let level = kw.line_indent;
        let name = self.name()?;
        let modality = match &kw.tok {
            Tok::Ident(k) if k == "obligation" => Some(Modality::Obligation),
            Tok::Ident(k) if k == "permission" => Some(Modality::Permission),
            Tok::Ident(k) if k == "prohibition" => Some(Modality::Prohibition),
            _ => None,
        };
        // An obligation, permission or prohibition is a truth value: whether it applies.
        let ty = if modality.is_some() {
            TypeRef::builtin("bool", name.span)
        } else {
            if !self.expect(&Tok::Colon, "`:` and a type") {
                return None;
            }
            self.type_ref()?
        };
        let mut meta = Meta::default();
        let kind = match &kw.tok {
            Tok::Ident(k) if k == "input" => MemberKind::Input,
            Tok::Ident(k) if k == "judgement" => {
                let mut criteria = Vec::new();
                if self.at_block(level) {
                    self.block(level, |p| {
                        if p.meta(&mut meta) {
                        } else if p.at_kw("criteria") {
                            criteria.extend(p.text());
                        } else {
                            p.unexpected("a judgement (expected label, source or criteria)");
                        }
                    });
                }
                MemberKind::Judgement { criteria }
            }
            Tok::Ident(k) if k == "constant" => {
                self.expect(&Tok::Eq, "`=`");
                MemberKind::Constant(self.expr())
            }
            _ => {
                if self.at(&Tok::Eq) {
                    self.bump();
                    MemberKind::Rule { default: Some(self.expr()), unless: Vec::new() }
                } else {
                    let mut default = None;
                    let mut unless = Vec::new();
                    // A table: `by <subject>` and its rows.
                    let mut table: Option<(Span, Table)> = None;
                    self.block(level, |p| {
                        if p.meta(&mut meta) {
                        } else if p.at_kw("default") || (modality.is_some() && p.at_kw("when")) {
                            p.bump();
                            default = Some(p.expr());
                        } else if p.at_kw("by") {
                            let start = p.bump().span;
                            let subject = p.expr();
                            if table.is_some() {
                                p.error(start, "a rule has one table: one `by`");
                            }
                            table = Some((start, Table { subject, rows: Vec::new(), otherwise: None }));
                        } else if p.at_kw("up") || p.at_kw("otherwise") || p.at_row() {
                            p.row(&mut table);
                        } else if p.at_kw("unless") {
                            let start = p.bump().span;
                            let unless_level = p.level();
                            let cond = p.expr();
                            p.expect_kw("then");
                            let value = p.expr();
                            let mut u = Unless { cond, value, meta: Meta::default(), span: start };
                            let end = p.opt_meta_block(unless_level, &mut u.meta).unwrap_or(u.value.span);
                            u.span = start.to(end);
                            unless.push(u);
                        } else {
                            let what = if modality.is_some() { "when" } else { "default, by" };
                            p.unexpected(&format!("a rule (expected label, source, {what} or unless)"));
                        }
                    });
                    if let Some((start, tbl)) = table {
                        if default.is_some() {
                            self.error(start, "a rule has a default or a table (`by`), not both");
                        }
                        let end = tbl.rows.last().map(|r| r.span).or(tbl.otherwise.as_ref().map(|o| o.span));
                        let span = start.to(end.unwrap_or(tbl.subject.span));
                        default = Some(Expr { kind: ExprKind::Table(Box::new(tbl)), span });
                    }
                    MemberKind::Rule { default, unless }
                }
            }
        };
        let end = self.opt_meta_block(level, &mut meta).unwrap_or(self.toks[self.pos.saturating_sub(1)].span);
        Some(Member { name, ty, meta, kind, modality, span: kw.span.to(end) })
    }

    fn path_decl(&mut self) -> Option<PathDecl> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut d = PathDecl { name, meta: Meta::default(), steps: Vec::new(), span: start };
        let end = self.block(level, |p| {
            if p.meta(&mut d.meta) {
            } else if p.at_kw("step") {
                p.bump();
                d.steps.extend(p.path());
            } else {
                p.unexpected("a path (expected label, source or step)");
            }
        });
        d.span = start.to(end);
        Some(d)
    }

    /// `[list] <type>`.
    fn type_ref(&mut self) -> Option<TypeRef> {
        let start = self.span();
        let list = self.at_kw("list");
        if list {
            self.bump();
        }
        let name = self.path()?;
        let span = start.to(name.span);
        Some(TypeRef { list, name, span })
    }

    /// `enum <name> { label … value <name> [{ label … }] … }`.
    fn enum_decl(&mut self) -> Option<EnumDecl> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut e = EnumDecl { name, meta: Meta::default(), values: Vec::new(), span: start };
        let end = self.block(level, |p| {
            if p.meta(&mut e.meta) {
            } else if p.at_kw("value") {
                let kw = p.bump();
                if let Some(name) = p.name() {
                    let mut meta = Meta::default();
                    let end = p.opt_meta_block(kw.line_indent, &mut meta).unwrap_or(name.span);
                    e.values.push(EnumValue { name, meta, span: kw.span.to(end) });
                }
            } else {
                p.unexpected("an enumeration (expected label, source or value)");
            }
        });
        e.span = start.to(end);
        Some(e)
    }

    /// `unit <name> [{ label … }]`.
    fn unit_decl(&mut self) -> Option<UnitDecl> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut meta = Meta::default();
        let end = self.opt_meta_block(level, &mut meta).unwrap_or(name.span);
        Some(UnitDecl { name, meta, span: start.to(end) })
    }

    /// `record <name> { label … field <name> : <type> [{ label … }] … }`.
    fn record_decl(&mut self) -> Option<RecordDecl> {
        let start = self.bump().span;
        let level = self.level();
        let name = self.name()?;
        let mut r = RecordDecl { name, meta: Meta::default(), fields: Vec::new(), span: start };
        let end = self.block(level, |p| {
            if p.meta(&mut r.meta) {
            } else if p.at_kw("field") {
                let kw = p.bump();
                let Some(name) = p.name() else { return };
                if !p.expect(&Tok::Colon, "`:` and a type") {
                    return;
                }
                let Some(ty) = p.type_ref() else { return };
                let mut meta = Meta::default();
                let end = p.opt_meta_block(kw.line_indent, &mut meta).unwrap_or(ty.span);
                r.fields.push(RecordField { name, ty, meta, span: kw.span.to(end) });
            } else {
                p.unexpected("a record (expected label, source or field)");
            }
        });
        r.span = start.to(end);
        Some(r)
    }

    /// Whether a row keyed by a value starts here: `<name>:`.
    fn at_row(&self) -> bool {
        matches!(self.peek(), Tok::Ident(_))
            && !KEYWORDS.contains(&match self.peek() {
                Tok::Ident(s) => s.as_str(),
                _ => "",
            })
            && {
                // A path, then `:`.
                let mut i = self.pos;
                while matches!(self.toks[i].tok, Tok::Ident(_))
                    && self.toks.get(i + 1).is_some_and(|t| t.tok == Tok::Dot)
                {
                    i += 2;
                }
                self.toks.get(i + 1).is_some_and(|t| t.tok == Tok::Colon)
            }
    }

    /// One row of a table: `up to <bound>: <value>`, `<value>: <value>` or `otherwise: <value>`.
    fn row(&mut self, table: &mut Option<(Span, Table)>) {
        let start = self.span();
        let key = if self.at_kw("otherwise") {
            self.bump();
            None
        } else if self.at_kw("up") {
            self.bump();
            self.expect_kw("to");
            Some(RowKey::UpTo(self.expr()))
        } else {
            match self.path() {
                Some(p) => Some(RowKey::Value(p)),
                None => return,
            }
        };
        self.expect(&Tok::Colon, "`:` and the value");
        let value = self.expr();
        let span = start.to(value.span);
        let Some((_, t)) = table else {
            self.error(start, "a row of a table belongs after `by <subject>`");
            return;
        };
        match key {
            None if t.otherwise.is_some() => self.error(start, "a table has one `otherwise`"),
            None => t.otherwise = Some(value),
            Some(_) if t.otherwise.is_some() => self.error(start, "`otherwise` is the last row of a table"),
            Some(key) => t.rows.push(Row { key, value, span }),
        }
    }

    // ---- expressions ----

    fn expr(&mut self) -> Expr {
        self.or_expr()
    }

    fn binary(l: Expr, op: BinOp, r: Expr) -> Expr {
        let span = l.span.to(r.span);
        Expr { kind: ExprKind::Binary(op, Box::new(l), Box::new(r)), span }
    }

    fn or_expr(&mut self) -> Expr {
        let mut l = self.and_expr();
        while self.at_kw("or") {
            self.bump();
            let r = self.and_expr();
            l = Self::binary(l, BinOp::Or, r);
        }
        l
    }

    fn and_expr(&mut self) -> Expr {
        let mut l = self.not_expr();
        while self.at_kw("and") {
            self.bump();
            let r = self.not_expr();
            l = Self::binary(l, BinOp::And, r);
        }
        l
    }

    fn not_expr(&mut self) -> Expr {
        if self.at_kw("not") {
            let start = self.bump().span;
            let e = self.not_expr();
            let span = start.to(e.span);
            return Expr { kind: ExprKind::Unary(UnOp::Not, Box::new(e)), span };
        }
        self.cmp_expr()
    }

    fn cmp_expr(&mut self) -> Expr {
        let l = self.add_expr();
        let op = match self.peek() {
            Tok::Lt => BinOp::Lt,
            Tok::Le => BinOp::Le,
            Tok::Gt => BinOp::Gt,
            Tok::Ge => BinOp::Ge,
            Tok::EqEq => BinOp::Eq,
            Tok::Ne => BinOp::Ne,
            _ => return l,
        };
        self.bump();
        let r = self.add_expr();
        Self::binary(l, op, r)
    }

    fn add_expr(&mut self) -> Expr {
        let mut l = self.mul_expr();
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => return l,
            };
            self.bump();
            let r = self.mul_expr();
            l = Self::binary(l, op, r);
        }
    }

    fn mul_expr(&mut self) -> Expr {
        let mut l = self.unary_expr();
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                _ => return l,
            };
            self.bump();
            let r = self.unary_expr();
            l = Self::binary(l, op, r);
        }
    }

    fn unary_expr(&mut self) -> Expr {
        if self.at(&Tok::Minus) {
            let start = self.bump().span;
            let e = self.unary_expr();
            let span = start.to(e.span);
            return Expr { kind: ExprKind::Unary(UnOp::Neg, Box::new(e)), span };
        }
        self.primary()
    }

    fn primary(&mut self) -> Expr {
        let span = self.span();
        match self.peek().clone() {
            Tok::Number(n) => {
                self.bump();
                if self.at(&Tok::Percent) {
                    let end = self.bump().span;
                    return Expr { kind: ExprKind::Percent(n), span: span.to(end) };
                }
                // An amount: a currency or a unit right after the number, on the same line.
                let next = &self.toks[self.pos];
                if let Tok::Ident(c) = &next.tok
                    && !next.first
                    && !KEYWORDS.contains(&c.as_str())
                {
                    let cur = Name { text: c.clone(), span: next.span };
                    self.bump();
                    let span = span.to(cur.span);
                    return Expr { kind: ExprKind::Money(n, cur), span };
                }
                Expr { kind: ExprKind::Number(n), span }
            }
            Tok::Date(d) => {
                self.bump();
                Expr { kind: ExprKind::Date(d), span }
            }
            Tok::LParen => {
                self.bump();
                let e = self.expr();
                let end = self.span();
                self.expect(&Tok::RParen, "`)`");
                Expr { kind: ExprKind::Paren(Box::new(e)), span: span.to(end) }
            }
            Tok::Ident(k) if k == "true" || k == "false" => {
                self.bump();
                Expr { kind: ExprKind::Bool(k == "true"), span }
            }
            Tok::Ident(k) if k == "if" => {
                self.bump();
                let c = self.expr();
                self.expect_kw("then");
                let a = self.expr();
                self.expect_kw("else");
                let b = self.expr();
                let span = span.to(b.span);
                Expr { kind: ExprKind::If(Box::new(c), Box::new(a), Box::new(b)), span }
            }
            Tok::Ident(_) => {
                let Some(path) = self.path() else {
                    return Expr { kind: ExprKind::Error, span };
                };
                if self.at(&Tok::LParen) {
                    return self.call(path, span);
                }
                let span = path.span;
                Expr { kind: ExprKind::Ref(path), span }
            }
            _ => {
                self.expected("an expression");
                self.bump();
                Expr { kind: ExprKind::Error, span }
            }
        }
    }

    /// After `<path>(`: arguments (`min(a, b)`), inputs by name (`norm.rule(x = …, y = …)`) or an aggregate
    /// (`sum(s.amount for s in supplies where …)`, `count(s in supplies)`).
    fn call(&mut self, path: Path, span: Span) -> Expr {
        self.bump();
        let named =
            |p: &Self| matches!(p.peek(), Tok::Ident(_)) && p.toks.get(p.pos + 1).is_some_and(|t| t.tok == Tok::Eq);
        let kind = if named(self) {
            let mut args = Vec::new();
            while named(self) {
                let Some(name) = self.name() else { break };
                self.bump();
                args.push((name, self.expr()));
                if !self.at(&Tok::Comma) {
                    break;
                }
                self.bump();
            }
            ExprKind::Apply(path, args)
        } else if self.at(&Tok::RParen) {
            ExprKind::Call(path, Vec::new())
        } else {
            let first = self.expr();
            if self.at_kw("for") || self.at_kw("in") {
                let (body, var) = if self.at_kw("for") {
                    self.bump();
                    (Some(first), self.name())
                } else {
                    let var = match &first.kind {
                        ExprKind::Ref(p) if p.segments.len() == 1 => Some(p.segments[0].clone()),
                        _ => {
                            self.error(first.span, "expected the name of each element, as in `count(s in supplies)`");
                            None
                        }
                    };
                    (None, var)
                };
                self.expect_kw("in");
                let list = self.expr();
                let filter = if self.at_kw("where") {
                    self.bump();
                    Some(self.expr())
                } else {
                    None
                };
                let func = path.segments[0].clone();
                if path.segments.len() > 1 {
                    self.error(path.span, "an aggregate is `sum`, `count`, `all` or `any`");
                }
                match var {
                    Some(var) => ExprKind::Aggregate(Box::new(Aggregate { func, body, var, list, filter })),
                    None => ExprKind::Error,
                }
            } else {
                let mut args = vec![first];
                while self.at(&Tok::Comma) {
                    self.bump();
                    args.push(self.expr());
                }
                ExprKind::Call(path, args)
            }
        };
        let end = self.span();
        self.expect(&Tok::RParen, "`)`");
        Expr { kind, span: span.to(end) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> File {
        let mut d = Vec::new();
        let f = parse(src, 0, &mut d);
        assert!(d.is_empty(), "{d:?}");
        f
    }

    #[test]
    fn parses_concept_with_variants_and_bridge() {
        let f = parse_ok(
            r#"module m
            concept c { label de "x" params lean "(a : ℕ)"
              variant v { lean "a = a" } variant w { lean "True" }
              bridge v <-> w { proof lean "by simp" } }"#,
        );
        let Item::Concept(c) = &f.items[0] else { panic!() };
        assert_eq!(c.variants.len(), 2);
        assert_eq!(c.bridges[0].right.text, "w");
        assert!(!c.bridges[0].eq && c.ty.is_none());
        let f = parse_ok(
            r#"module m
            concept d { params lean "(x y : ℝ)" type lean "ℝ"
              variant v { lean "|x - y|" } variant w { lean "Real.sqrt ((x - y) ^ 2)" } bridge v = w }"#,
        );
        let Item::Concept(d) = &f.items[0] else { panic!() };
        assert_eq!(d.ty.as_ref().map(|t| t.code.as_str()), Some("ℝ"));
        assert!(d.bridges[0].eq);
    }

    #[test]
    fn parses_rule_with_exception_and_precedence() {
        let f = parse_ok(
            r#"module m
            norm n {
              input a : money
              rule r : bool { default a <= 2 * 3 EUR and not false unless true then false { source "s" } }
            }"#,
        );
        let Item::Norm(n) = &f.items[0] else { panic!() };
        let MemberKind::Rule { default: Some(d), unless } = &n.members[1].kind else { panic!() };
        assert!(matches!(d.kind, ExprKind::Binary(BinOp::And, _, _)));
        assert_eq!(unless[0].meta.sources.len(), 1);
    }

    #[test]
    fn parses_obligations_permissions_and_prohibitions() {
        let f = parse_ok(
            r#"module m
            norm n {
              input w : bool
              permission may = not w
              prohibition must_not { label en "x" when w unless false then false }
              obligation must = w
            }"#,
        );
        let Item::Norm(n) = &f.items[0] else { panic!() };
        let kinds: Vec<_> = n.members.iter().map(|m| m.modality).collect();
        assert_eq!(kinds, [None, Some(Modality::Permission), Some(Modality::Prohibition), Some(Modality::Obligation)]);
        assert_eq!(n.members[2].ty.text(), "bool");
        assert!(matches!(&n.members[2].kind, MemberKind::Rule { default: Some(_), unless } if unless.len() == 1));
    }

    /// What a file says, without where: items with their names, labels, Lean code, texts and members.
    fn content(f: &File) -> String {
        let mut out = String::new();
        let lean = |l: &Option<Lean>| l.as_ref().map(|l| l.code.clone()).unwrap_or_default();
        let labels = |m: &Meta| {
            m.labels.iter().map(|t| format!("{}={}", t.lang.text, t.value)).collect::<Vec<_>>().join(",")
                + &m.sources.iter().map(|s| format!(" src={}", s.0)).collect::<String>()
        };
        for it in &f.items {
            match it {
                Item::Concept(c) => {
                    out += &format!("concept {} [{}] {}\n", c.name.text, labels(&c.meta), lean(&c.params));
                    for v in &c.variants {
                        out += &format!("  variant {} [{}] {}\n", v.name.text, labels(&v.meta), lean(&v.def));
                    }
                    for b in &c.bridges {
                        out += &format!("  bridge {} {} {}\n", b.left.text, b.right.text, lean(&b.proof));
                    }
                }
                Item::Theorem(t) => {
                    out += &format!(
                        "theorem {} [{}] {} | {} | {}\n",
                        t.name.text,
                        labels(&t.meta),
                        lean(&t.params),
                        lean(&t.statement),
                        lean(&t.proof)
                    );
                }
                Item::Remark(r) => {
                    let texts: Vec<String> = r.texts.iter().map(|t| format!("{}={}", t.lang.text, t.value)).collect();
                    out += &format!("remark {} {}\n", r.name.text, texts.join(" / "));
                }
                Item::Norm(n) => {
                    out += &format!("norm {} [{}]\n", n.name.text, labels(&n.meta));
                    for m in &n.members {
                        let kind = match &m.kind {
                            MemberKind::Rule { default, unless } => {
                                format!("rule {} unless {}", default.is_some(), unless.len())
                            }
                            other => format!("{other:?}").chars().take(5).collect(),
                        };
                        out += &format!("  {} : {} {} [{}]\n", m.name.text, m.ty.text(), kind, labels(&m.meta));
                    }
                }
                Item::Path(p) => {
                    let steps: Vec<String> = p.steps.iter().map(|s| s.joined()).collect();
                    out += &format!("path {} {}\n", p.name.text, steps.join(" "));
                }
                Item::Enum(e) => {
                    let values: Vec<String> =
                        e.values.iter().map(|v| format!("{} [{}]", v.name.text, labels(&v.meta))).collect();
                    out += &format!("enum {} [{}] {}\n", e.name.text, labels(&e.meta), values.join(" "));
                }
                Item::Unit(u) => out += &format!("unit {} [{}]\n", u.name.text, labels(&u.meta)),
                Item::Record(r) => {
                    let fields: Vec<String> = r
                        .fields
                        .iter()
                        .map(|f| format!("{} : {} [{}]", f.name.text, f.ty.text(), labels(&f.meta)))
                        .collect();
                    out += &format!("record {} [{}] {}\n", r.name.text, labels(&r.meta), fields.join(" "));
                }
            }
        }
        out
    }

    #[test]
    fn reads_both_ways_of_writing_alike() {
        let braces = r#"module m
concept converges {
  label de "konvergiert gegen"
  params lean "(a : ℕ → ℝ) (L : ℝ)"
  variant eps_n { lean "∀ ε > 0, ∃ N : ℕ, ∀ n ≥ N, |a n - L| < ε" }
  variant filter {
    lean "Filter.Tendsto a Filter.atTop (nhds L)"
  }
  bridge eps_n <-> filter {
    proof lean ```
      by
        rw [Metric.tendsto_atTop]
        simp only [Real.dist_eq]
    ```
  }
}
theorem t {
  label de "Satz"
  params lean "(a : ℕ → ℝ) (L : ℝ)"
  statement lean "converges a L → True"
  proof lean "fun _ => trivial"
}
remark r {
  about t
  text de "Erster Absatz.\n\nZweiter Absatz, über zwei Zeilen."
}
norm n {
  source "§ 1"
  input x : money { label de "Umsatz" }
  rule ok : bool {
    default x <= 5 EUR
    unless x > 3 EUR then false { source "§ 2" }
  }
}
path p { step converges.eps_n step t }
"#;
        let indented = r#"module m
concept converges
  label de konvergiert gegen
  params lean (a : ℕ → ℝ) (L : ℝ)
  variant eps_n
    lean ∀ ε > 0, ∃ N : ℕ, ∀ n ≥ N, |a n - L| < ε
  variant filter { lean "Filter.Tendsto a Filter.atTop (nhds L)" }
  bridge eps_n <-> filter
    proof lean
      by
      	rw [Metric.tendsto_atTop]
        simp only [Real.dist_eq]
theorem t
  label de Satz
  params lean (a : ℕ → ℝ) (L : ℝ)
  statement lean converges a L → True
  proof lean fun _ => trivial
remark r
  about t
  text de
    Erster Absatz.

    Zweiter Absatz,
    über zwei Zeilen.
norm n
  source § 1
  input x : money
    label de Umsatz
  rule ok : bool
    default x <= 5 EUR
    unless x > 3 EUR then false
      source § 2
path p
  step converges.eps_n
  step t
"#;
        let a = content(&parse_ok(braces));
        assert_eq!(content(&parse_ok(indented)), a);
        assert!(a.contains("bridge eps_n filter by\n  rw [Metric.tendsto_atTop]\n  simp only [Real.dist_eq]"), "{a}");
        assert!(a.contains("rule true unless 1 [ src=§ 2]") || a.contains("ok : bool rule true unless 1"), "{a}");
    }

    #[test]
    fn reads_data_types_in_both_ways_of_writing() {
        let braces = r#"module m
enum residence {
  label de "Ansässigkeit"
  value domestic { label de "Inland" }
  value elsewhere
}
unit km { label de "Kilometer" }
record supply {
  field amount : money { label de "Entgelt" }
  field kind : residence
}
norm n {
  input where_ : residence
  input supplies : list supply
  input way : km
  rule total : money = sum(s.amount for s in supplies where s.kind == residence.domestic)
  rule many : int = count(s in supplies)
  rule rate : decimal {
    label de "Satz"
    by where_
    domestic: 19 %
    otherwise: 7 %
  }
  rule zone : int {
    by total
    up to 12_348 EUR: 0
    up to 17_799 EUR: 1
    otherwise: 2
  }
  rule far : bool = way > 20 km
  rule other : money = n.total(supplies = supplies, way = 0 km, where_ = residence.elsewhere)
}
"#;
        let indented = r#"module m
enum residence
  label de Ansässigkeit
  value domestic
    label de Inland
  value elsewhere
unit km
  label de Kilometer
record supply
  field amount : money
    label de Entgelt
  field kind : residence
norm n
  input where_ : residence
  input supplies : list supply
  input way : km
  rule total : money = sum(s.amount for s in supplies where s.kind == residence.domestic)
  rule many : int = count(s in supplies)
  rule rate : decimal
    label de Satz
    by where_
    domestic: 19 %
    otherwise: 7 %
  rule zone : int
    by total
    up to 12_348 EUR: 0
    up to 17_799 EUR: 1
    otherwise: 2
  rule far : bool = way > 20 km
  rule other : money = n.total(supplies = supplies, way = 0 km, where_ = residence.elsewhere)
"#;
        let f = parse_ok(braces);
        let a = content(&f);
        assert_eq!(content(&parse_ok(indented)), a);
        assert!(a.contains("enum residence [de=Ansässigkeit] domestic [de=Inland] elsewhere []"), "{a}");
        assert!(a.contains("record supply [] amount : money [de=Entgelt] kind : residence []"), "{a}");
        assert!(a.contains("supplies : list supply"), "{a}");
        let Item::Norm(n) = &f.items[3] else { panic!() };
        let rule = |i: usize| match &n.members[i].kind {
            MemberKind::Rule { default: Some(d), .. } => d.kind.clone(),
            _ => panic!(),
        };
        assert!(matches!(rule(3), ExprKind::Aggregate(a) if a.func.text == "sum" && a.filter.is_some()));
        assert!(matches!(rule(4), ExprKind::Aggregate(a) if a.func.text == "count" && a.body.is_none()));
        let ExprKind::Table(t) = rule(5) else { panic!() };
        assert!(matches!(&t.rows[0].key, RowKey::Value(p) if p.joined() == "domestic"));
        assert!(matches!(&t.rows[0].value.kind, ExprKind::Percent(n) if n == "19"));
        let ExprKind::Table(t) = rule(6) else { panic!() };
        assert_eq!(t.rows.len(), 2);
        assert!(t.otherwise.is_some());
        let ExprKind::Binary(_, _, r) = rule(7) else { panic!() };
        assert!(matches!(&r.kind, ExprKind::Money(n, u) if n == "20" && u.text == "km"));
        assert!(matches!(rule(8), ExprKind::Apply(p, args) if p.joined() == "n.total" && args.len() == 3));
        // A number followed by a keyword is no amount.
        let f = parse_ok("module m\nnorm n { input x : int rule r : int = if x > 5 then 1 else 2 }");
        let Item::Norm(n) = &f.items[0] else { panic!() };
        assert!(matches!(
            &n.members[1].kind,
            MemberKind::Rule { default: Some(Expr { kind: ExprKind::If(..), .. }), .. }
        ));
    }

    #[test]
    fn recovers_after_errors() {
        let mut d = Vec::new();
        let f = parse("module m\n concept c { bogus 1 }\n path p { step c }", 0, &mut d);
        assert_eq!(f.items.len(), 2);
        assert!(!d.is_empty());
    }
}
