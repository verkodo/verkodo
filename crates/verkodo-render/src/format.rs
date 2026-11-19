// SPDX-License-Identifier: AGPL-3.0-or-later
//! How the pieces of a page are written down. The renderer decides what to show; a [`Format`] decides how – HTML with
//! hover cards, or Typst for PDF, where links lead to the steps of the path and to an appendix instead.

use verkodo_core::proof::LeanName;

/// Writes the pieces of a page. Every method returns markup; `text` is the only way plain text gets in.
pub(crate) trait Format {
    /// Plain text, escaped.
    fn text(&self, s: &str) -> String;
    /// A link to a declaration of verkodo.
    fn sym(&self, sym: usize, id: &str, text: &str) -> String;
    /// A parameter inside a Lean snippet.
    fn local(&self, word: &str, name: &str, ty: &str, note: &str) -> String;
    /// A Lean name that Lean has not explained (no report).
    fn external(&self, word: &str, name: &str, note: &str) -> String;
    /// A Lean name with what Lean said about it.
    fn lean_name(&self, word: &str, n: &LeanName, declared_in: &str) -> String;
    /// Lean code; `inner` is markup already.
    fn code(&self, inner: &str) -> String;
    /// A short piece of code, e.g. a type.
    fn mono(&self, s: &str) -> String;
    /// A row of a body: a key (markup) and its value (markup).
    fn row(&self, key: &str, value: &str) -> String;
    /// A key word inside a value, e.g. "then".
    fn key(&self, s: &str) -> String;
    /// A verdict word: `ok` or `open`.
    fn status(&self, class: &str, s: &str) -> String;
    /// A note in parentheses after something; `inner` is markup.
    fn note(&self, inner: &str) -> String;
    /// An error message of Lean, as it came.
    fn error(&self, s: &str) -> String;
    /// Something shown on request (the proof of a theorem); both parts are markup.
    fn details(&self, summary: &str, body: &str) -> String;
    /// A paragraph of plain text.
    fn para(&self, s: &str) -> String;
    /// The source of a declaration: the word "Source" and the source, both plain text.
    /// A source, linked to its address when it cites a document that has one.
    fn source(&self, word: &str, s: &str, url: Option<&str>) -> String;
    /// A link to an address outside.
    fn link(&self, text: &str, url: &str) -> String;
    /// A numbered list of markup items.
    fn ordered(&self, items: &[String]) -> String;
    /// A space that does not break.
    fn nbsp(&self) -> &'static str;
    /// A line break.
    fn linebreak(&self) -> &'static str;
}

#[cfg(feature = "html")]
pub(crate) fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// HTML with hover cards (`data-tip`, `data-ref`); the script in `assets.rs` shows them. `hrefs` says for every
/// symbol where its link leads; without one, a link only opens its card. `card`: the markup of a card in a site's
/// data – links carry only their id (the script adds where they lead, from the search), a proof only its verdict.
#[derive(Debug, Default)]
#[cfg(feature = "html")]
pub(crate) struct Html {
    pub(crate) hrefs: Vec<String>,
    pub(crate) card: bool,
}

#[cfg(feature = "html")]
impl Html {
    fn tip(&self, text: &str, tip: &str, class: &str) -> String {
        format!(r#"<span class="ref {class}" data-tip="{}">{}</span>"#, esc(tip), esc(text))
    }
}

#[cfg(feature = "html")]
impl Format for Html {
    fn text(&self, s: &str) -> String {
        esc(s)
    }

    fn sym(&self, sym: usize, id: &str, text: &str) -> String {
        let id = esc(id);
        if self.card {
            return format!(r#"<a class="ref" data-ref="{id}">{}</a>"#, esc(text));
        }
        match self.hrefs.get(sym) {
            Some(href) => format!(r#"<a class="ref" href="{}" data-ref="{id}">{}</a>"#, esc(href), esc(text)),
            None => format!(r#"<a class="ref" role="button" tabindex="0" data-ref="{id}">{}</a>"#, esc(text)),
        }
    }

    fn local(&self, word: &str, name: &str, ty: &str, note: &str) -> String {
        self.tip(word, &format!("{name} : {ty} ({note})"), "local")
    }

    fn external(&self, word: &str, name: &str, note: &str) -> String {
        self.tip(word, &format!("{name} – {note}"), "external")
    }

    fn lean_name(&self, word: &str, n: &LeanName, declared_in: &str) -> String {
        let mut tip = format!("{} : {}", n.name, n.ty);
        if let Some(doc) = &n.doc {
            tip.push_str("\n\n");
            tip.push_str(doc);
        }
        if let Some(m) = &n.module {
            tip.push_str(&format!("\n\n{declared_in} {m}"));
        }
        self.tip(word, &tip, "external")
    }

    fn code(&self, inner: &str) -> String {
        format!(r#"<code class="lean">{inner}</code>"#)
    }

    fn mono(&self, s: &str) -> String {
        format!("<code>{}</code>", esc(s))
    }

    fn row(&self, key: &str, value: &str) -> String {
        format!(r#"<div class="row"><span class="key">{key}</span> {value}</div>"#)
    }

    fn key(&self, s: &str) -> String {
        format!(r#"<span class="key">{}</span>"#, esc(s))
    }

    fn status(&self, class: &str, s: &str) -> String {
        format!(r#"<span class="{class}">{}</span>"#, esc(s))
    }

    fn note(&self, inner: &str) -> String {
        format!(r#"<span class="status">({inner})</span>"#)
    }

    fn error(&self, s: &str) -> String {
        format!(r#"<pre class="lean-error">{}</pre>"#, esc(s))
    }

    fn details(&self, summary: &str, body: &str) -> String {
        if self.card {
            return summary.to_string();
        }
        format!("<details><summary>{summary}</summary>{body}</details>")
    }

    fn para(&self, s: &str) -> String {
        format!(r#"<p class="text">{}</p>"#, esc(s))
    }

    fn source(&self, word: &str, s: &str, url: Option<&str>) -> String {
        let s = match url {
            Some(u) => self.link(s, u),
            None => esc(s),
        };
        format!(r#"<div class="src">{}: {s}</div>"#, esc(word))
    }

    fn link(&self, text: &str, url: &str) -> String {
        format!(r#"<a class="ext" href="{}" rel="noopener">{}</a>"#, esc(url), esc(text))
    }

    fn ordered(&self, items: &[String]) -> String {
        format!("<ol>{}</ol>", items.iter().map(|x| format!("<li>{x}</li>")).collect::<String>())
    }

    fn nbsp(&self) -> &'static str {
        "&nbsp;"
    }

    fn linebreak(&self) -> &'static str {
        "<br>"
    }
}
