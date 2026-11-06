// SPDX-License-Identifier: AGPL-3.0-or-later
//! A path as a Typst document, for PDF. There is no hover on paper: every term is a link – to its step if the path
//! introduces it, otherwise to an entry in the appendix, which explains every term used and not introduced, as deep
//! as it goes, and every Lean or Mathlib name with what Lean said about it.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use verkodo_core::proof::{LeanName, Report};
use verkodo_core::{Analysis, Loc, Sources, Target};
use verkodo_syntax::ast::Item;

use crate::R;
use crate::format::Format;

/// Typst markup: plain text goes in as string literals, every embedded expression ends with `;`.
#[derive(Debug, Default)]
struct Typst {
    /// Declarations that something links to.
    linked: RefCell<BTreeSet<usize>>,
    /// Lean names that something links to, by full name.
    lean: RefCell<BTreeMap<String, LeanName>>,
}

/// A Typst string literal.
fn lit(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A Typst label for an id; a character a label cannot hold becomes its code point between underscores.
pub(crate) fn label(prefix: &str, id: &str) -> String {
    let mut out = format!("<{prefix}-");
    for c in id.chars() {
        if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
            out.push(c);
        } else {
            out.push_str(&format!("_{:x}_", c as u32));
        }
    }
    out.push('>');
    out
}

impl Format for Typst {
    fn text(&self, s: &str) -> String {
        s.split('\n')
            .filter(|l| !l.is_empty())
            .map(|l| format!("#{};", lit(l)))
            .collect::<Vec<_>>()
            .join("#linebreak();")
    }

    fn sym(&self, sym: usize, id: &str, text: &str) -> String {
        self.linked.borrow_mut().insert(sym);
        format!("#link({})[{}];", label("s", id), self.text(text))
    }

    fn local(&self, word: &str, _name: &str, _ty: &str, _note: &str) -> String {
        self.text(word)
    }

    fn external(&self, word: &str, _name: &str, _note: &str) -> String {
        self.text(word)
    }

    fn lean_name(&self, word: &str, n: &LeanName, _declared_in: &str) -> String {
        self.lean.borrow_mut().entry(n.name.clone()).or_insert_with(|| n.clone());
        format!("#link({})[{}];", label("lean", &n.name), self.text(word))
    }

    fn code(&self, inner: &str) -> String {
        format!("#code[{inner}];")
    }

    fn mono(&self, s: &str) -> String {
        format!("#code[{}];", self.text(s))
    }

    fn row(&self, key: &str, value: &str) -> String {
        format!("#row[{key}][{value}];\n")
    }

    fn key(&self, s: &str) -> String {
        format!("#text(fill: reed)[{}];", self.text(s))
    }

    fn status(&self, class: &str, s: &str) -> String {
        format!("#text(fill: {class}, weight: \"medium\")[{}];", self.text(s))
    }

    fn note(&self, inner: &str) -> String {
        format!("#text(fill: reed)[{}{inner}{}];", self.text("("), self.text(")"))
    }

    fn error(&self, s: &str) -> String {
        format!("#lean-error({});", lit(s))
    }

    fn details(&self, summary: &str, body: &str) -> String {
        format!("{summary}#block(above: .4em)[{body}];")
    }

    fn para(&self, s: &str) -> String {
        format!("#block[{}];\n", self.text(s))
    }

    fn source(&self, word: &str, s: &str) -> String {
        format!("#src[{}: {}];\n", self.text(word), self.text(s))
    }

    fn ordered(&self, items: &[String]) -> String {
        let items: Vec<String> = items.iter().map(|x| format!("[{x}]")).collect();
        format!("#enum({});", items.join(", "))
    }

    fn nbsp(&self) -> &'static str {
        "~"
    }

    fn linebreak(&self) -> &'static str {
        "#linebreak();"
    }
}

/// Style and helpers of the document. Colours of the verkodo brand; fonts: IBM Plex if `brand_fonts` (they must be
/// installed or given to `typst --font-path`), else the fonts Typst brings along.
fn preamble(title: &str, lang: &str, brand_fonts: bool) -> String {
    // Formulas need symbols (⟺, ℝ, ⟨⟩) that not every font has: the fonts Typst brings fill the gaps.
    let (sans, mono) = if brand_fonts {
        (
            r#"("IBM Plex Sans", "Libertinus Serif", "New Computer Modern Math", "DejaVu Sans Mono")"#,
            r#"("IBM Plex Mono", "DejaVu Sans Mono", "New Computer Modern Math")"#,
        )
    } else {
        (
            r#"("Libertinus Serif", "New Computer Modern Math", "DejaVu Sans Mono")"#,
            r#"("DejaVu Sans Mono", "New Computer Modern Math")"#,
        )
    };
    format!(
        r##"#let ink = rgb("#14202B")
#let accent = rgb("#127D74")
#let steel = rgb("#4A5D70")
#let reed = rgb("#7D8590")
#let rule-color = rgb("#C9D1D9")
#let ok = rgb("#2F7D4A")
#let open = rgb("#B3402F")
#let sans = {sans}
#let mono = {mono}

#set document(title: {title})
#set page(paper: "a4", margin: (x: 22mm, top: 22mm, bottom: 24mm), numbering: "1",
  footer: context align(center, text(font: mono, size: 7pt, fill: reed, counter(page).display())))
#set text(font: sans, size: 10pt, lang: {lang}, fill: ink)
#set par(justify: false)
#show link: set text(fill: steel)
#show heading: set text(font: mono, weight: "bold", fill: ink)
#show heading.where(level: 1): it => block(below: 1em, text(size: 17pt, it.body))
#show heading.where(level: 2): it => block(above: 1.6em, below: .8em, sticky: true, text(size: 11.5pt, it.body))
#show heading.where(level: 3): it => block(above: 1.2em, below: .6em, sticky: true, text(size: 10pt, it.body))

#let code(body) = text(font: mono, size: 8.5pt, body)
#let row(key, body) = block(above: .5em, below: .5em, grid(columns: (7.5em, 1fr), column-gutter: .8em,
  text(size: 8.5pt, fill: reed, key), body))
#let src(body) = block(above: .4em, text(size: 8.5pt, fill: reed, body))
#let lean-error(s) = block(above: .4em, width: 100%, inset: 6pt, fill: open.transparentize(92%),
  text(font: mono, size: 7.5pt, raw(s)))
#let id(s) = block(sticky: true, above: .3em, below: .5em, text(font: mono, size: 7.5pt, fill: reed, s))
"##,
        title = lit(title),
        lang = lit(lang),
    )
}

/// The Typst document of one path. `brand_fonts`: IBM Plex instead of the fonts Typst brings along.
pub fn path_document(
    ws: &Sources,
    a: &Analysis,
    report: Option<&Report>,
    path: usize,
    lang: &str,
    brand_fonts: bool,
) -> String {
    let f = Typst::default();
    let r = R { ws, a, report, lang, f: &f };
    let title = r.label(path);
    let mut out = format!(
        "// {} `{}` – do not edit.\n{}\n",
        r.w("generated"),
        a.symbols[path].id,
        preamble(&title, lang, brand_fonts)
    );
    out += &format!("= {}\n#text(size: 9pt, fill: reed)[{}];\n\n", f.text(&title), f.text(r.w("hint_pdf")));

    let mut steps = Vec::new();
    if let Loc::Item { file, item } = a.symbols[path].loc
        && let Item::Path(d) = &ws.asts[file].items[item]
    {
        for step in &d.steps {
            if let Some(Target::Symbol(s)) = a.refs.get(&(file, step.span))
                && !steps.contains(s)
            {
                steps.push(*s);
            }
        }
    }
    for (n, &s) in steps.iter().enumerate() {
        out += &format!(
            "== {}~~{}{} {}\n#id({});\n{}{}\n",
            n + 1,
            f.text(r.kind_word(s)),
            f.text(":"),
            f.text(&r.title(s)) + " " + &label("s", &a.symbols[s].id),
            lit(&a.symbols[s].id),
            r.body(s),
            r.in_path(s, path)
        );
    }

    // The appendix: every declaration something links to and the path does not introduce – and what those link to.
    let mut done: BTreeSet<usize> = steps.iter().copied().collect();
    let mut appendix = String::new();
    loop {
        let next = f.linked.borrow().iter().copied().find(|s| !done.contains(s));
        let Some(s) = next else { break };
        done.insert(s);
        appendix += &format!(
            "=== {}{} {} {}\n#id({});\n{}\n",
            f.text(r.kind_word(s)),
            f.text(":"),
            f.text(&r.title(s)),
            label("s", &a.symbols[s].id),
            lit(&a.symbols[s].id),
            r.body(s)
        );
    }
    let lean = f.lean.borrow();
    if !appendix.is_empty() || !lean.is_empty() {
        out += &format!("#pagebreak(weak: true)\n== {}\n\n{appendix}", f.text(r.w("appendix")));
    }
    if !lean.is_empty() {
        out += &format!("== {}\n\n", f.text(r.w("lean_names")));
        for n in lean.values() {
            out += &format!("=== {} {}\n", f.mono(&n.name), label("lean", &n.name));
            out += &f.row(&f.text(r.w("type")), &f.mono(&n.ty));
            if let Some(doc) = &n.doc {
                out += &f.para(doc);
            }
            if let Some(m) = &n.module {
                out += &f.source(r.w("declared_in"), m);
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_quoted_and_labels_are_safe() {
        let f = Typst::default();
        assert_eq!(f.text("a \"b\" #c\nd"), r##"#"a \"b\" #c";#linebreak();#"d";"##);
        assert_eq!(label("s", "m.x"), "<s-m.x>");
        assert_eq!(label("lean", "Foo.«bar baz»"), "<lean-Foo._ab_bar_20_baz_bb_>");
    }

    #[test]
    fn a_path_links_its_steps_and_explains_the_rest_in_the_appendix() {
        let src = r#"module m
            concept c { label en "C" params lean "(x : ℝ)" variant p { lean "0 < x" } }
            concept d { label en "D" params lean "(x : ℝ)" variant q { lean "c x ∧ x < 1" } }
            theorem t { label en "T" params lean "(x : ℝ)" statement lean "d x → c x" }
            path w { label en "Way" step d step t }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = verkodo_core::analyze(&ws);
        assert!(a.diags.iter().all(|d| d.severity == verkodo_syntax::Severity::Warning), "{:?}", a.diags);
        let path = a.by_id["m.w"];
        let doc = path_document(&ws, &a, None, path, "en", false);
        assert!(doc.contains("#set document(title: \"Way\")"), "{doc}");
        // Steps carry labels; `c` is not a step, so it is explained in the appendix – with a label for every link.
        assert!(doc.contains("<s-m.d>") && doc.contains("<s-m.t>"), "{doc}");
        assert!(doc.contains("Appendix: further terms"), "{doc}");
        for target in
            doc.match_indices("#link(<").map(|(i, _)| &doc[i + 6..i + 6 + doc[i + 6..].find('>').unwrap() + 1])
        {
            let defined = doc.matches(target).count() > doc.matches(&format!("#link({target})")).count();
            assert!(defined, "no label for {target}");
        }
    }
}
