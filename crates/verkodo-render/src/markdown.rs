// SPDX-License-Identifier: AGPL-3.0-or-later
//! Markdown for hover in an editor: the same content as a hover card on the web, without links – the editor goes to
//! a definition itself.

use verkodo_core::proof::{Goals, LeanName, Report};
use verkodo_core::{Analysis, Sources};

use crate::R;
use crate::format::Format;

#[derive(Debug)]
struct Markdown;

/// Markdown text, with the characters that would format escaped. Not inside code (`code` takes it as it is).
fn md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Code in backticks, with enough of them for the backticks inside.
fn tick(s: &str) -> String {
    let mut fence = String::from("`");
    while s.contains(&fence) {
        fence.push('`');
    }
    let pad = if s.starts_with('`') || s.ends_with('`') { " " } else { "" };
    format!("{fence}{pad}{s}{pad}{fence}")
}

// Inside `code`, pieces come as they are: `text` marks them so that `code` can tell them from markup.
const RAW: char = '\u{E010}';

impl Format for Markdown {
    fn text(&self, s: &str) -> String {
        format!("{RAW}{s}{RAW}")
    }

    fn sym(&self, _sym: usize, _id: &str, text: &str) -> String {
        self.text(text)
    }

    fn local(&self, word: &str, _name: &str, _ty: &str, _note: &str) -> String {
        self.text(word)
    }

    fn external(&self, word: &str, _name: &str, _note: &str) -> String {
        self.text(word)
    }

    fn lean_name(&self, word: &str, _n: &LeanName, _declared_in: &str) -> String {
        self.text(word)
    }

    fn code(&self, inner: &str) -> String {
        tick(&inner.replace(RAW, ""))
    }

    fn mono(&self, s: &str) -> String {
        tick(s)
    }

    fn row(&self, key: &str, value: &str) -> String {
        format!("*{key}* {value}  \n")
    }

    fn key(&self, s: &str) -> String {
        format!("*{}*", self.text(s))
    }

    fn status(&self, _class: &str, s: &str) -> String {
        format!("**{}**", self.text(s))
    }

    fn note(&self, inner: &str) -> String {
        format!("({inner})")
    }

    fn error(&self, s: &str) -> String {
        format!("\n```text\n{s}\n```\n")
    }

    fn details(&self, summary: &str, body: &str) -> String {
        format!("{summary}  \n{body}")
    }

    fn para(&self, s: &str) -> String {
        format!("\n{}\n\n", self.text(s))
    }

    fn source(&self, word: &str, s: &str) -> String {
        format!("*{}: {}*  \n", self.text(word), self.text(s))
    }

    fn ordered(&self, items: &[String]) -> String {
        let lines: Vec<String> = items.iter().enumerate().map(|(i, x)| format!("{}. {x}", i + 1)).collect();
        format!("\n{}\n", lines.join("\n"))
    }

    fn nbsp(&self) -> &'static str {
        "\u{a0}"
    }

    fn linebreak(&self) -> &'static str {
        "  \n"
    }
}

/// Turns the marked plain text outside code into escaped Markdown.
fn finish(s: &str) -> String {
    let mut out = String::new();
    for (i, part) in s.split(RAW).enumerate() {
        if i % 2 == 1 { out.push_str(&md(part)) } else { out.push_str(part) }
    }
    out
}

/// The hover of a declaration: kind and title, its id, and what a hover card on the web shows.
pub fn symbol_hover(ws: &Sources, a: &Analysis, report: Option<&Report>, sym: usize, lang: &str) -> String {
    let r = R { ws, a, report, lang, f: &Markdown };
    let head = format!(
        "**{}: {}**  \n{}\n\n",
        Markdown.text(r.kind_word(sym)),
        Markdown.text(&r.title(sym)),
        tick(&a.symbols[sym].id)
    );
    finish(&(head + &r.body(sym)))
}

/// The hover of a Lean or Mathlib name: its type, its doc string (Markdown already) and the module that declares it.
pub fn lean_hover(n: &LeanName, lang: &str) -> String {
    let mut out = format!("```lean\n{} : {}\n```\n", n.name, n.ty);
    if let Some(doc) = &n.doc {
        out.push_str(&format!("\n{doc}\n"));
    }
    if let Some(m) = &n.module {
        let word = if lang == "de" { "deklariert in" } else { "declared in" };
        out.push_str(&format!("\n*{word}* {}\n", tick(m)));
    }
    out
}

/// The goals before and after a tactic, as Lean prints them – what an infoview shows.
pub fn goals_hover(g: &Goals, lang: &str) -> String {
    let de = lang == "de";
    let block = |goals: &[String]| -> String {
        if goals.is_empty() {
            format!("*{}*\n", if de { "keine – hier ist alles gezeigt" } else { "none – all is shown here" })
        } else {
            goals.iter().map(|g| format!("```lean\n{g}\n```\n")).collect()
        }
    };
    format!(
        "**{}**\n{}\n**{}**\n{}",
        if de { "Ziele vor der Taktik" } else { "Goals before the tactic" },
        block(&g.before),
        if de { "danach" } else { "after" },
        block(&g.after)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_shows_the_card_as_markdown() {
        let src = r#"module m
            concept c { label en "C_1 [x]" params lean "(x : ℝ)" variant p { label en "pos" lean "0 < x" } }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = verkodo_core::analyze(&ws);
        let md = symbol_hover(&ws, &a, None, a.by_id["m.c"], "en");
        assert!(md.starts_with("**Concept: C\\_1 \\[x\\]**  \n`m.c`\n\n"), "{md}");
        assert!(md.contains("*Parameters* `(x : ℝ)`"), "{md}");
        assert!(md.contains("*pos* `0 < x`"), "{md}");
        assert_eq!(tick("a`b"), "``a`b``");
    }
}
