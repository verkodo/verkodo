// SPDX-License-Identifier: AGPL-3.0-or-later
//! Turns source text into tokens. Keywords are plain identifiers; the parser gives them meaning.
//!
//! verkodo can be written two ways, and both may be mixed: with delimiters – `{ … }` for blocks, `"…"` for text and
//! one line of Lean, ```` ``` ```` around longer Lean code or text – where line breaks do not matter; or by
//! indentation, without them. For that every token knows the indentation of its line and whether it is the first on
//! it, and text without quotes is read here: after `lean`, after `source` at the start of a line and after
//! `label`, `text` or `criteria` and a language at the start of a line, the rest of the line is the text – or, if
//! the line ends there, the lines below that are indented deeper. A tab counts up to the next column that is a
//! multiple of [`TAB`].

use crate::source::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Str(String),
    /// Lean code between fences (```` ``` ````), without its common indentation: no escapes, many lines.
    Block(String),
    Number(String),
    Date(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    Colon,
    Comma,
    Dot,
    Iff,
    Arrow,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Ident(s) => format!("`{s}`"),
            Tok::Str(_) => "a string".into(),
            Tok::Block(_) => "a block of Lean code".into(),
            Tok::Number(n) => format!("number `{n}`"),
            Tok::Date(d) => format!("date `{d}`"),
            Tok::Eof => "end of file".into(),
            other => format!("`{}`", punct(other)),
        }
    }
}

fn punct(t: &Tok) -> &'static str {
    match t {
        Tok::LBrace => "{",
        Tok::RBrace => "}",
        Tok::LParen => "(",
        Tok::RParen => ")",
        Tok::Colon => ":",
        Tok::Comma => ",",
        Tok::Dot => ".",
        Tok::Iff => "<->",
        Tok::Arrow => "->",
        Tok::Eq => "=",
        Tok::EqEq => "==",
        Tok::Ne => "!=",
        Tok::Lt => "<",
        Tok::Le => "<=",
        Tok::Gt => ">",
        Tok::Ge => ">=",
        Tok::Plus => "+",
        Tok::Minus => "-",
        Tok::Star => "*",
        Tok::Slash => "/",
        _ => "?",
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
    /// The indentation of the line the token starts on, in columns.
    pub line_indent: usize,
    /// Whether it is the first token on its line.
    pub first: bool,
}

/// Tab stops: a tab counts up to the next column that is a multiple of this – one level of the usual indentation of
/// two spaces.
pub const TAB: usize = 2;

/// The indentation of a line in columns, and its length in bytes.
fn indentation(line: &str) -> (usize, usize) {
    let mut col = 0;
    for (i, c) in line.char_indices() {
        match c {
            ' ' => col += 1,
            '\t' => col = (col / TAB + 1) * TAB,
            _ => return (col, i),
        }
    }
    (col, line.len())
}

/// The text with the tabs in every line's indentation turned into spaces, to the same columns – what formatting
/// does. Tabs after the indentation stay.
pub fn spaces_for_tabs(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let (cols, bytes) = indentation(line);
            if line[..bytes].contains('\t') {
                format!("{}{}", " ".repeat(cols), &line[bytes..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The start of the line that contains byte `i`.
fn line_start(text: &str, i: usize) -> usize {
    text[..i].rfind('\n').map(|n| n + 1).unwrap_or(0)
}

/// Where text without quotes starts after the tokens so far: after `lean`; after `source` first on its line; after a
/// language following `label`, `text` or `criteria` first on its line.
fn raw_follows(out: &[Token]) -> bool {
    let n = out.len();
    let kw = |t: &Token, words: &[&str]| matches!(&t.tok, Tok::Ident(w) if words.contains(&w.as_str()));
    match out.last() {
        Some(t) if kw(t, &["lean"]) => true,
        Some(t) if kw(t, &["source"]) && t.first => true,
        Some(t) if matches!(t.tok, Tok::Ident(_)) && n >= 2 => {
            let before = &out[n - 2];
            kw(before, &["label", "text", "criteria"]) && before.first && !t.first
        }
        _ => false,
    }
}

/// Text without quotes at `i` (spaces skipped already): the rest of the line, or – if the line ends here – the lines
/// below indented deeper than this one. Returns the token and the length it covers.
fn raw(text: &str, i: usize) -> Option<(Tok, Span)> {
    let rest = &text[i..];
    let line_end = rest.find('\n').map(|n| i + n).unwrap_or(text.len());
    let on_line = text[i..line_end].trim_end();
    if !on_line.is_empty() {
        return Some((Tok::Str(on_line.to_string()), Span::new(i, i + on_line.len())));
    }
    // The lines below, while they are blank or deeper than this line.
    let (base, _) = indentation(&text[line_start(text, i)..]);
    let mut end = line_end;
    let mut last = line_end;
    while end < text.len() {
        let next_start = end + 1;
        let next_end = text[next_start..].find('\n').map(|n| next_start + n).unwrap_or(text.len());
        let line = &text[next_start..next_end];
        if !line.trim().is_empty() {
            if indentation(line).0 <= base {
                break;
            }
            last = next_end;
        }
        end = next_end;
    }
    if last == line_end {
        return None;
    }
    let literal = &text[line_end..last];
    let (code, _) = dedent_block(literal);
    Some((Tok::Block(code), Span::new(line_end, last)))
}

/// The lines of an indented block written without delimiters (the literal starts at the line break before them),
/// without the indentation they share: the code and the offset in the literal of each of its bytes, and one more.
fn dedent_block(literal: &str) -> (String, Vec<usize>) {
    let mut lines = Vec::new();
    let mut at = 1;
    for line in literal[1..].split('\n') {
        lines.push((at, line));
        at += line.len() + 1;
    }
    dedent(&lines, literal.len())
}

/// Lines (offset in the literal, text) without the indentation the non-blank ones share; tabs in what is left of
/// the indentation become spaces. Returns the code and the offset in the literal of each of its bytes, then `end`.
fn dedent(lines: &[(usize, &str)], end: usize) -> (String, Vec<usize>) {
    let indent = lines.iter().filter(|(_, l)| !l.trim().is_empty()).map(|(_, l)| indentation(l).0).min().unwrap_or(0);
    let mut code = String::new();
    let mut offsets = Vec::new();
    for (n, (start, line)) in lines.iter().enumerate() {
        if n > 0 {
            code.push('\n');
            offsets.push(start - 1);
        }
        let (cols, bytes) = indentation(line);
        // What is left of the indentation, as spaces: with spaces only, each leads back to its space; where tabs
        // were, to the first character after them.
        let keep = cols.saturating_sub(indent);
        if !line.trim().is_empty() {
            code.push_str(&" ".repeat(keep));
            if line[..bytes].contains('\t') {
                offsets.extend(std::iter::repeat_n(start + bytes, keep));
            } else {
                offsets.extend((bytes - keep..bytes).map(|k| start + k));
            }
        }
        code.push_str(&line[bytes..]);
        offsets.extend((bytes..line.len()).map(|k| start + k));
    }
    offsets.push(end.min(lines.last().map(|(s, l)| s + l.len()).unwrap_or(end)));
    (code, offsets)
}

const PUNCT: &[(&str, Tok)] = &[
    ("<->", Tok::Iff),
    ("->", Tok::Arrow),
    ("<=", Tok::Le),
    (">=", Tok::Ge),
    ("==", Tok::EqEq),
    ("!=", Tok::Ne),
    ("{", Tok::LBrace),
    ("}", Tok::RBrace),
    ("(", Tok::LParen),
    (")", Tok::RParen),
    (":", Tok::Colon),
    (",", Tok::Comma),
    (".", Tok::Dot),
    ("=", Tok::Eq),
    ("<", Tok::Lt),
    (">", Tok::Gt),
    ("+", Tok::Plus),
    ("-", Tok::Minus),
    ("*", Tok::Star),
    ("/", Tok::Slash),
];

pub fn lex(text: &str, file: usize, diags: &mut Vec<Diagnostic>) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0;
    // The line of the last token: (its start, whether a token is on it).
    let mut line = (usize::MAX, false);
    let push = |out: &mut Vec<Token>, line: &mut (usize, bool), tok: Tok, span: Span| {
        let start = line_start(text, span.start);
        let first = line.0 != start || !line.1;
        *line = (start, true);
        let (line_indent, _) = indentation(&text[start..]);
        out.push(Token { tok, span, line_indent, first });
    };
    while let Some(c) = text[i..].chars().next() {
        let rest = &text[i..];
        if c == ' ' || c == '\t' {
            i += 1;
        } else if raw_follows(&out)
            && !rest.starts_with('"')
            && !rest.starts_with(FENCE)
            && !rest.starts_with("//")
            && let Some((tok, span)) = raw(text, i)
        {
            push(&mut out, &mut line, tok, span);
            i = span.end;
        } else if c.is_whitespace() {
            i += c.len_utf8();
        } else if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with(FENCE) {
            let (value, len, closed) = block(rest);
            if !closed {
                diags.push(Diagnostic::error(file, Span::new(i, i + len), "unterminated block (```)"));
            }
            push(&mut out, &mut line, Tok::Block(value), Span::new(i, i + len));
            i += len;
        } else if c == '"' {
            let (value, len, closed) = string(rest);
            if !closed {
                diags.push(Diagnostic::error(file, Span::new(i, i + len), "unterminated string"));
            }
            push(&mut out, &mut line, Tok::Str(value), Span::new(i, i + len));
            i += len;
        } else if is_date(rest) {
            push(&mut out, &mut line, Tok::Date(rest[..10].to_string()), Span::new(i, i + 10));
            i += 10;
        } else if c.is_ascii_digit() {
            let len = number_len(rest);
            push(&mut out, &mut line, Tok::Number(rest[..len].to_string()), Span::new(i, i + len));
            i += len;
        } else if c.is_alphabetic() || c == '_' {
            let len = rest.find(|ch: char| !(ch.is_alphanumeric() || ch == '_')).unwrap_or(rest.len());
            push(&mut out, &mut line, Tok::Ident(rest[..len].to_string()), Span::new(i, i + len));
            i += len;
        } else if let Some((p, tok)) = PUNCT.iter().find(|(p, _)| rest.starts_with(p)) {
            push(&mut out, &mut line, tok.clone(), Span::new(i, i + p.len()));
            i += p.len();
        } else {
            let span = Span::new(i, i + c.len_utf8());
            diags.push(Diagnostic::error(file, span, format!("unexpected character `{c}`")));
            i += c.len_utf8();
        }
    }
    push(&mut out, &mut line, Tok::Eof, Span::new(text.len(), text.len()));
    out
}

/// The fence around a block of Lean code.
const FENCE: &str = "```";

/// Reads a block starting at its fence; returns the code, the length in bytes and whether it was closed.
fn block(rest: &str) -> (String, usize, bool) {
    let (code, _, len, closed) = block_parts(rest);
    (code, len, closed)
}

/// A block of Lean code starting at its fence: the code, the byte offset in the literal of every byte of the code
/// and one more for its end, the length of the literal and whether it was closed.
///
/// The code starts on the line after the opening fence if nothing else is on that line, and ends before the closing
/// fence (on a line of its own, that line's indentation is not code). The indentation all lines of code share is
/// removed – Lean counts columns, and the block may be indented like the verkodo around it.
fn block_parts(rest: &str) -> (String, Vec<usize>, usize, bool) {
    let body_start = FENCE.len();
    let (end, closed) = match rest[body_start..].find(FENCE) {
        Some(e) => (body_start + e, true),
        None => (rest.len(), false),
    };
    let len = if closed { end + FENCE.len() } else { end };
    let body = &rest[body_start..end];
    // The lines of the body with their offsets in the literal.
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut at = body_start;
    for line in body.split('\n') {
        lines.push((at, line));
        at += line.len() + 1;
    }
    // Nothing after the opening fence: the code starts on the next line; only spaces before the closing fence: that
    // line is not code.
    if lines.len() > 1 && lines[0].1.trim().is_empty() {
        lines.remove(0);
    }
    if lines.len() > 1 && lines.last().is_some_and(|(_, l)| l.trim().is_empty()) {
        lines.pop();
    }
    let (code, offsets) = dedent(&lines, end);
    (code, offsets, len, closed)
}

/// For a Lean snippet as written – a string literal or a block between fences –: the byte offset in the literal of
/// every byte of its code, and one more for the end. So a place in the code (a Lean name in a formula) is found in
/// the source again.
pub fn lean_offsets(literal: &str) -> Vec<usize> {
    if literal.starts_with(FENCE) {
        block_parts(literal).1
    } else if literal.starts_with('"') {
        string_offsets(literal)
    } else if literal.starts_with('\n') {
        dedent_block(literal).1
    } else {
        // One line without quotes: the code is the literal.
        (0..=literal.len()).collect()
    }
}

/// Reads a string literal starting at `"`; returns the value, the length in bytes and whether it was closed.
fn string(rest: &str) -> (String, usize, bool) {
    let mut value = String::new();
    let mut chars = rest.char_indices().skip(1);
    while let Some((j, c)) = chars.next() {
        match c {
            '"' => return (value, j + 1, true),
            '\\' => match chars.next() {
                Some((_, 'n')) => value.push('\n'),
                Some((_, other)) => value.push(other),
                None => break,
            },
            _ => value.push(c),
        }
    }
    (value, rest.len(), false)
}

/// For a string literal starting at `"`: the byte offset in the literal of every byte of its value, and one more for
/// the end – so that a place in the value (e.g. a Lean name in a formula) can be found in the source again.
pub fn string_offsets(literal: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut chars = literal.char_indices().skip(1).peekable();
    while let Some((j, c)) = chars.next() {
        match c {
            '"' => {
                out.push(j);
                return out;
            }
            '\\' => {
                if let Some((_, e)) = chars.next() {
                    let value = if e == 'n' { '\n' } else { e };
                    out.extend(std::iter::repeat_n(j, value.len_utf8()));
                }
            }
            _ => out.extend((0..c.len_utf8()).map(|k| j + k)),
        }
    }
    out.push(literal.len());
    out
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    b.len() >= 10
        && digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && !b.get(10).is_some_and(u8::is_ascii_digit)
}

/// Digits with `_` as separator and at most one decimal point followed by a digit.
fn number_len(s: &str) -> usize {
    let b = s.as_bytes();
    let mut len = 0;
    let mut seen_dot = false;
    while len < b.len() {
        let c = b[len];
        if c.is_ascii_digit() || c == b'_' {
            len += 1;
        } else if c == b'.' && !seen_dot && b.get(len + 1).is_some_and(u8::is_ascii_digit) {
            seen_dot = true;
            len += 1;
        } else {
            break;
        }
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_offsets_lead_back_through_escapes() {
        let lit = r#""a\"ε\nb" rest"#;
        let (value, ..) = string(lit);
        assert_eq!(value, "a\"ε\nb");
        let off = string_offsets(lit);
        assert_eq!(off.len(), value.len() + 1);
        let b = value.find('b').unwrap();
        assert_eq!(&lit[off[b]..off[b] + 1], "b");
        assert_eq!(&lit[off[value.find('ε').unwrap()]..][..2], "ε");
        assert_eq!(off[value.len()], lit.find("\" rest").unwrap());
    }

    fn toks(s: &str) -> Vec<Tok> {
        let mut d = Vec::new();
        let t = lex(s, 0, &mut d);
        assert!(d.is_empty(), "{d:?}");
        t.into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn lexes_mixed_input() {
        assert_eq!(
            toks("rule x : money = 25_000 EUR // c\n valid from 2025-01-01 a<->b \"ε\\\"\""),
            vec![
                Tok::Ident("rule".into()),
                Tok::Ident("x".into()),
                Tok::Colon,
                Tok::Ident("money".into()),
                Tok::Eq,
                Tok::Number("25_000".into()),
                Tok::Ident("EUR".into()),
                Tok::Ident("valid".into()),
                Tok::Ident("from".into()),
                Tok::Date("2025-01-01".into()),
                Tok::Ident("a".into()),
                Tok::Iff,
                Tok::Ident("b".into()),
                Tok::Str("ε\"".into()),
                Tok::Eof,
            ]
        );
    }

    #[test]
    fn reads_blocks_of_lean_code() {
        let src = "proof lean ```\n    intro ε hε\n      exact ⟨0, rfl⟩\n  ``` x";
        let t = toks(src);
        assert_eq!(t[2], Tok::Block("intro ε hε\n  exact ⟨0, rfl⟩".into()));
        assert_eq!(t[3], Tok::Ident("x".into()));
        // Every byte of the code leads back to the same byte of the source.
        let lit = &src[src.find("```").unwrap()..src.rfind("```").unwrap() + 3];
        let Tok::Block(code) = &t[2] else { panic!() };
        let off = lean_offsets(lit);
        assert_eq!(off.len(), code.len() + 1);
        for (i, b) in code.bytes().enumerate() {
            assert_eq!(lit.as_bytes()[off[i]], b, "byte {i}");
        }
        // One line, between fences on the same line.
        assert_eq!(toks("```rfl```")[0], Tok::Block("rfl".into()));
        let mut d = Vec::new();
        lex("```open", 0, &mut d);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn formatting_turns_tabs_into_spaces() {
        assert_eq!(spaces_for_tabs("a\n\tb\n \tc\td\n"), "a\n  b\n  c\td\n");
    }

    #[test]
    fn reports_unterminated_string() {
        let mut d = Vec::new();
        lex("\"open", 0, &mut d);
        assert_eq!(d.len(), 1);
    }
}
