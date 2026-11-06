// SPDX-License-Identifier: AGPL-3.0-or-later
//! The formatter: writes a file in one of the two ways of writing verkodo – with delimiters (`{ … }`, `"…"`,
//! ```` ``` ````) or by indentation – and with tabs turned into spaces.
//!
//! It does not print the syntax tree again: every entry keeps its words, its expressions as they were laid out, and
//! the comments and blank lines between entries. What changes is the frame: braces or indentation, levels of two
//! spaces, and how a text is written (`"…"` or ```` ``` ```` or without quotes). Afterwards it reads its result again;
//! if the blocks, entries and tokens are not the same as before, it refuses instead of changing the meaning.

use crate::lexer::{Tok, Token};
use crate::parser::{Block, Entry, parse_layout, prose};
use crate::source::Severity;

/// The way of writing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// `{ … }` around blocks, `"…"` around text and one line of Lean, ```` ``` ```` around more.
    Delimiters,
    /// Blocks and texts by indentation, without delimiters.
    Indentation,
}

const STEP: usize = 2;

/// How to format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub style: Style,
    /// The widest a line of text may be, in characters: longer texts are broken into lines – a one-line text becomes
    /// a block. None: texts stay as they are. Lean code and expressions are never broken.
    pub width: Option<usize>,
}

/// `text` written in `style`, or why not: a file with errors is left alone.
pub fn format(text: &str, style: Style) -> Result<String, String> {
    format_with(text, Options { style, width: None })
}

/// `text` written as `options` say, or why not: a file with errors is left alone.
pub fn format_with(text: &str, options: Options) -> Result<String, String> {
    let style = options.style;
    let mut diags = Vec::new();
    let (_, toks, layout) = parse_layout(text, 0, &mut diags);
    if diags.iter().any(|d| d.severity == Severity::Error) {
        return Err("the file has errors – the formatter leaves it as it is".into());
    }
    let f = Fmt { text, toks: &toks, style, width: options.width };
    let mut out = Vec::new();
    f.entries(&layout, 0, &mut out, text.get(..toks[0].span.start).unwrap_or(""), true);
    // What is left after the last entry: comments at the end of the file.
    if let Some(last) = layout.entries.last() {
        let end = f.toks[last.end.saturating_sub(1).max(last.start)].span.end;
        f.gap(&text[end.min(text.len())..], 0, &mut out, true);
    }
    while out.last().is_some_and(|l: &String| l.trim().is_empty()) {
        out.pop();
    }
    let result = out.join("\n") + "\n";
    // The same blocks, entries and tokens as before – or nothing.
    let mut again = Vec::new();
    let (_, toks2, layout2) = parse_layout(&result, 0, &mut again);
    if again.iter().any(|d| d.severity == Severity::Error) || shape(&toks, &layout) != shape(&toks2, &layout2) {
        return Err("the formatter would change the meaning here – please report it; the file is left as it is".into());
    }
    Ok(result)
}

/// The meaning of a layout without its form: blocks, entries and their tokens, texts whatever their delimiters – a
/// block of prose as the running text it means.
fn shape(toks: &[Token], b: &Block) -> String {
    let mut s = String::from("[");
    for e in &b.entries {
        s.push('(');
        let mut i = e.start;
        let mut blocks = e.blocks.iter().peekable();
        while i < e.end {
            if let Some(child) = blocks.peek()
                && i == block_start(toks, child)
            {
                s += &shape(toks, child);
                i = child.end;
                blocks.next();
                continue;
            }
            let after_lean = i > 0 && matches!(&toks[i - 1].tok, Tok::Ident(w) if w == "lean");
            match &toks[i].tok {
                Tok::Block(v) if !after_lean => s += &format!("{:?} ", prose(v)),
                Tok::Str(v) | Tok::Block(v) => s += &format!("{v:?} "),
                Tok::LBrace | Tok::RBrace => {}
                t => s += &format!("{t:?} "),
            }
            i += 1;
        }
        s.push(')');
    }
    s + "]"
}

/// The first token of a block: its `{`, or its first entry.
fn block_start(toks: &[Token], b: &Block) -> usize {
    let _ = toks;
    b.braces.map(|(open, _)| open).or_else(|| b.entries.first().map(|e| e.start)).unwrap_or(b.end)
}

struct Fmt<'a> {
    text: &'a str,
    toks: &'a [Token],
    style: Style,
    width: Option<usize>,
}

impl Fmt<'_> {
    fn src(&self, i: usize) -> &str {
        let s = self.toks[i].span;
        &self.text[s.start..s.end]
    }

    /// The entries of a block at `indent`; `before` is the text between what opened the block and its first entry
    /// (blank lines there only count at the top of the file).
    fn entries(&self, b: &Block, indent: usize, out: &mut Vec<String>, before: &str, top: bool) {
        let mut before = before.to_string();
        for (n, e) in b.entries.iter().enumerate() {
            self.gap(&before, indent, out, n > 0 || top);
            self.entry(e, indent, out);
            let last = self.toks[e.end.saturating_sub(1).max(e.start)].span.end;
            let next = match b.entries.get(n + 1) {
                Some(next) => self.toks[next.start].span.start,
                None => match b.braces {
                    Some((_, close)) => self.toks[close].span.start,
                    None => last,
                },
            };
            before = self.text[last..next.max(last)].to_string();
        }
        // Comments after the last entry, inside the braces.
        if b.braces.is_some() {
            self.gap(&before, indent, out, false);
        }
    }

    /// Comments and blank lines between two entries; a comment on the line of the entry before stays at its end
    /// (with delimiters) or goes to a line of its own (by indentation, where it could join a text without quotes).
    fn gap(&self, s: &str, indent: usize, out: &mut Vec<String>, blank_allowed: bool) {
        let parts: Vec<&str> = s.split('\n').collect();
        let comment = |p: &str| p.find("//").map(|i| p[i..].trim_end().to_string());
        if let Some(c) = parts.first().and_then(|p| comment(p)) {
            match (self.style, out.last_mut()) {
                (Style::Delimiters, Some(line)) => {
                    line.push(' ');
                    line.push_str(&c);
                }
                _ => out.push(format!("{}{c}", " ".repeat(indent))),
            }
        }
        let mut blank = false;
        for (k, p) in parts.iter().enumerate().skip(1) {
            let last = k == parts.len() - 1;
            match comment(p) {
                Some(c) => {
                    if blank && blank_allowed {
                        out.push(String::new());
                    }
                    blank = false;
                    out.push(format!("{}{c}", " ".repeat(indent)));
                }
                None if !last && p.trim().is_empty() => blank = true,
                None => {}
            }
        }
        if blank && blank_allowed && out.last().is_some_and(|l| !l.is_empty()) {
            out.push(String::new());
        }
    }

    fn entry(&self, e: &Entry, indent: usize, out: &mut Vec<String>) {
        let header_end = e.blocks.first().map(|b| block_start(self.toks, b)).unwrap_or(e.end);
        self.header(e, header_end, indent, out);
        for b in &e.blocks {
            self.block(b, indent, out);
        }
    }

    /// The words of an entry before its blocks, on the lines they were on; texts written in the style.
    fn header(&self, e: &Entry, end: usize, indent: usize, out: &mut Vec<String>) {
        let base = self.toks[e.start].line_indent;
        let mut line = " ".repeat(indent);
        for i in e.start..end {
            if i > e.start {
                // What was between the words: kept on one line; a line break keeps the depth relative to the entry.
                let between = &self.text[self.toks[i - 1].span.end..self.toks[i].span.start];
                if between.contains('\n') {
                    out.push(std::mem::take(&mut line));
                    let rel = self.toks[i].line_indent.saturating_sub(base).max(STEP);
                    line = " ".repeat(indent + rel);
                } else if !between.is_empty() {
                    line.push(' ');
                }
            }
            let lean = i > e.start && matches!(&self.toks[i - 1].tok, Tok::Ident(w) if w == "lean");
            match &self.toks[i].tok {
                Tok::Str(v) | Tok::Block(v) => {
                    let bare = i + 1 == end && self.text_may_be_bare(e, i);
                    // Prose from a block is running text; Lean code and `"…"` are word for word.
                    let block = matches!(self.toks[i].tok, Tok::Block(_));
                    if lean || (block && self.width.is_none()) {
                        // Lean code as it is; a block of prose without a width keeps the lines it was given.
                        self.value(v, bare, indent, &mut line, out, false);
                    } else {
                        let v = if block { prose(v) } else { v.clone() };
                        self.prose_value(&v, bare, indent, &mut line, out);
                    }
                }
                _ => line.push_str(self.src(i)),
            }
        }
        out.push(line);
    }

    /// Whether the text at token `i` – the last of the entry's words – may be written without quotes: it follows
    /// `lean`, or `source` that begins the entry, or a language after `label`, `text` or `criteria` that begins it.
    fn text_may_be_bare(&self, e: &Entry, i: usize) -> bool {
        let word = |k: usize, words: &[&str]| matches!(&self.toks[k].tok, Tok::Ident(w) if words.contains(&w.as_str()));
        i > e.start
            && (word(i - 1, &["lean"])
                || (i - 1 == e.start && word(i - 1, &["source"]))
                || (i >= e.start + 2
                    && i - 2 == e.start
                    && word(i - 2, &["label", "text", "criteria"])
                    && matches!(self.toks[i - 1].tok, Tok::Ident(_))))
    }

    /// A text: by indentation without delimiters where it can be; else `"…"` for one line, ```` ``` ```` for more.
    fn value(&self, v: &str, bare: bool, indent: usize, line: &mut String, out: &mut Vec<String>, literal: bool) {
        let lines: Vec<&str> = v.split('\n').collect();
        // `literal`: line breaks that a block of prose would read as spaces – only `"…"` keeps them.
        let one_line = lines.len() == 1 || literal;
        // Leading or trailing blank lines, or a first line that a lexer would take for something else: quotes.
        let plain = !v.is_empty()
            && !lines.first().is_some_and(|l| l.trim().is_empty())
            && !lines.last().is_some_and(|l| l.trim().is_empty())
            && !v.starts_with('"')
            && !v.starts_with("```")
            && !v.starts_with("//")
            && !v.contains("```");
        let inner = " ".repeat(indent + STEP);
        // A text that stood on the lines below follows its keyword directly: one space before its delimiter.
        let space = |line: &mut String| {
            if !line.ends_with(' ') && !line.trim().is_empty() {
                line.push(' ');
            }
        };
        if self.style == Style::Indentation && bare && plain && one_line && !v.contains('\n') && v == v.trim() {
            space(line);
            line.push_str(v);
        } else if self.style == Style::Indentation && bare && plain && !one_line {
            // The lines below, deeper: the line itself ends here.
            while line.ends_with(' ') {
                line.pop();
            }
            for l in lines {
                out.push(std::mem::take(line));
                *line = if l.trim().is_empty() { String::new() } else { format!("{inner}{l}") };
            }
        } else if one_line || !plain {
            space(line);
            line.push('"');
            for c in v.chars() {
                match c {
                    '"' => line.push_str("\\\""),
                    '\\' => line.push_str("\\\\"),
                    '\n' => line.push_str("\\n"),
                    c => line.push(c),
                }
            }
            line.push('"');
        } else {
            space(line);
            line.push_str("```");
            for l in lines {
                out.push(std::mem::take(line));
                *line = if l.trim().is_empty() { String::new() } else { format!("{inner}{l}") };
            }
            out.push(std::mem::take(line));
            *line = format!("{}```", " ".repeat(indent));
        }
    }

    /// A text of prose: as it is where it fits (or no width is set) and has one line; else broken into lines of at
    /// most the width – which a block of prose means alike. A text with a line break inside a paragraph stays `"…"`.
    fn prose_value(&self, v: &str, bare: bool, indent: usize, line: &mut String, out: &mut Vec<String>) {
        let one_line = !v.contains('\n');
        let fits = |line: &String| match self.width {
            None => true,
            Some(w) => {
                let space = usize::from(!line.ends_with(' '));
                let quotes = if bare && self.style == Style::Indentation { 0 } else { 2 };
                line.chars().count() + space + v.chars().count() + quotes <= w
            }
        };
        let paragraphs_only = v.split("\n\n").all(|p| !p.contains('\n') && p == p.trim() && !p.is_empty());
        if !paragraphs_only {
            return self.value(v, bare, indent, line, out, true);
        }
        if one_line && (self.width.is_none() || fits(line)) {
            return self.value(v, bare, indent, line, out, false);
        }
        let width = self.width.unwrap_or(usize::MAX);
        // Greedy: as many words as fit on each line, paragraphs apart.
        let avail = width.saturating_sub(indent + STEP).max(20);
        let mut lines = Vec::new();
        for (n, p) in v.split("\n\n").enumerate() {
            if n > 0 {
                lines.push(String::new());
            }
            let mut cur = String::new();
            for word in p.split(' ') {
                if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > avail {
                    lines.push(std::mem::take(&mut cur));
                }
                if !cur.is_empty() {
                    cur.push(' ');
                }
                cur.push_str(word);
            }
            lines.push(cur);
        }
        let inner = " ".repeat(indent + STEP);
        let space = |line: &mut String| {
            if !line.ends_with(' ') && !line.trim().is_empty() {
                line.push(' ');
            }
        };
        if self.style == Style::Indentation && bare {
            while line.ends_with(' ') {
                line.pop();
            }
        } else {
            space(line);
            line.push_str("```");
        }
        for l in &lines {
            out.push(std::mem::take(line));
            *line = if l.is_empty() { String::new() } else { format!("{inner}{l}") };
        }
        if !(self.style == Style::Indentation && bare) {
            out.push(std::mem::take(line));
            *line = format!("{}```", " ".repeat(indent));
        }
    }

    /// A block an entry opens, at one level deeper than the entry.
    fn block(&self, b: &Block, indent: usize, out: &mut Vec<String>) {
        let line = |i: usize| self.text[..self.toks[i].span.start].matches('\n').count();
        // With delimiters, a block written on one line stays as it was.
        if self.style == Style::Delimiters
            && let Some((open, close)) = b.braces
            && line(open) == line(close)
        {
            let last = out.last_mut().expect("the entry's line");
            last.push(' ');
            last.push_str(&self.text[self.toks[open].span.start..self.toks[close].span.end]);
            return;
        }
        // An empty block cannot be written by indentation: it keeps its braces.
        if b.entries.is_empty() {
            out.last_mut().expect("the entry's line").push_str(" {}");
            return;
        }
        let first = self.toks[b.entries[0].start].span.start;
        let before = match b.braces {
            Some((open, _)) => &self.text[self.toks[open].span.end..first],
            None => "",
        };
        if self.style == Style::Delimiters {
            out.last_mut().expect("the entry's line").push_str(" {");
        }
        self.entries(b, indent + STEP, out, before, false);
        if self.style == Style::Delimiters {
            out.push(format!("{}}}", " ".repeat(indent)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELIMITED: &str = r#"// A header comment.

module m

concept c {
  label de "konvergiert gegen"   // at the end of a line
  params lean "(a : ℕ → ℝ) (L : ℝ)"
  variant p { lean "0 < L" }
  variant q {
    lean "L > 0"
  }

  // Between entries.
  bridge p <-> q {
    proof lean ```
      by
        exact Iff.rfl
    ```
  }
}

norm n {
  input x : money { label de "Umsatz" }
  rule r : money =
    if x <= 5 EUR then 0 EUR
    else x
  rule ok : bool {
    default x <= 5 EUR
    unless x > 3 EUR then false { source "§ 2 \"a\"" }
  }
}

remark k {
  about c
  text de "Zwei\nZeilen."
}
"#;

    #[test]
    fn writes_both_ways_and_back() {
        let indented = format(DELIMITED, Style::Indentation).unwrap();
        assert!(
            indented.contains("concept c\n  label de konvergiert gegen\n  // at the end of a line\n"),
            "{indented}"
        );
        assert!(indented.contains("  variant p\n    lean 0 < L\n"), "{indented}");
        assert!(indented.contains("    proof lean\n      by\n        exact Iff.rfl\n"), "{indented}");
        assert!(indented.contains("  rule r : money =\n    if x <= 5 EUR then 0 EUR\n    else x\n"), "{indented}");
        assert!(indented.contains("      source § 2 \"a\"\n"), "{indented}");
        // A line break inside a paragraph stays word for word: `"…"`, not a block of prose.
        assert!(indented.contains("  text de \"Zwei\\nZeilen.\"\n"), "{indented}");
        assert!(indented.starts_with("// A header comment.\n\nmodule m\n\nconcept c\n"), "{indented}");
        assert!(!indented.contains('{') || indented.contains("{}"), "{indented}");
        // Back: the same meaning, with delimiters again; and each way is a fixed point.
        let back = format(&indented, Style::Delimiters).unwrap();
        assert!(
            back.contains("  bridge p <-> q {\n    proof lean ```\n      by\n        exact Iff.rfl\n    ```\n  }"),
            "{back}"
        );
        assert_eq!(format(&indented, Style::Indentation).unwrap(), indented);
        assert_eq!(format(&back, Style::Delimiters).unwrap(), back);
        // One-line blocks stay one line with delimiters.
        let same = format(DELIMITED, Style::Delimiters).unwrap();
        assert!(same.contains("  variant p { lean \"0 < L\" }\n"), "{same}");
    }

    #[test]
    fn breaks_long_texts_at_the_width() {
        let src = "module m\nremark r {\n  label de \"Ein ziemlich langer Titel, der nicht mehr in eine Zeile passt\"\n  \
                   text de \"Kurz.\"\n  text en \"Erster Absatz mit einigen Wörtern.\\n\\nZweiter.\"\n}\n";
        let opts = |style, width| Options { style, width: Some(width) };
        let ind = format_with(src, opts(Style::Indentation, 40)).unwrap();
        assert!(
            ind.contains("  label de\n    Ein ziemlich langer Titel, der nicht\n    mehr in eine Zeile passt\n"),
            "{ind}"
        );
        assert!(ind.contains("  text de Kurz.\n"), "{ind}");
        assert!(ind.contains("  text en\n    Erster Absatz mit einigen Wörtern.\n\n    Zweiter.\n"), "{ind}");
        let del = format_with(src, opts(Style::Delimiters, 40)).unwrap();
        assert!(del.contains("  label de ```\n    Ein ziemlich langer Titel, der nicht\n"), "{del}");
        // No line longer than the width.
        assert!(ind.lines().chain(del.lines()).all(|l| l.chars().count() <= 40), "{ind}{del}");
        // Each a fixed point; and without a width, a block keeps the lines it was given.
        assert_eq!(format_with(&ind, opts(Style::Indentation, 40)).unwrap(), ind);
        assert_eq!(format_with(&del, opts(Style::Delimiters, 40)).unwrap(), del);
        assert_eq!(format(&ind, Style::Indentation).unwrap(), ind);
        // Exactly at the width: one line, whichever way it was written before.
        let exact = "module m\nremark r {\n  label de \"12345678901234567890123456789\"\n}\n";
        let one = format_with(exact, opts(Style::Indentation, 40)).unwrap();
        assert!(one.contains("  label de 12345678901234567890123456789\n"), "{one}");
        assert_eq!(format_with(&one, opts(Style::Indentation, 40)).unwrap(), one);
        // Wider again: one line.
        let wide = format_with(&ind, opts(Style::Indentation, 120)).unwrap();
        assert!(wide.contains("  label de Ein ziemlich langer Titel, der nicht mehr in eine Zeile passt\n"), "{wide}");
    }

    #[test]
    fn leaves_files_with_errors_alone() {
        assert!(format("module m\nconcept c {", Style::Indentation).is_err());
    }

    #[test]
    fn turns_tabs_into_spaces() {
        let tabbed = "module m\nconcept c\n\tvariant p\n\t\tlean True\n";
        assert_eq!(format(tabbed, Style::Indentation).unwrap(), "module m\nconcept c\n  variant p\n    lean True\n");
    }
}
