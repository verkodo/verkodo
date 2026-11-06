// SPDX-License-Identifier: AGPL-3.0-or-later
//! A light reading of Lean 4 snippets: identifiers and binders. This is a stand-in until Lean itself reports what
//! every name refers to (milestone M2, through Lean's info trees).

use std::ops::Range;

/// Identifiers in a Lean snippet with their byte ranges; dotted names stay whole (`Filter.Tendsto`).
pub fn idents(code: &str) -> Vec<(Range<usize>, &str)> {
    let mut out = Vec::new();
    let mut it = code.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if c.is_alphabetic() || c == '_' {
            let mut end = i + c.len_utf8();
            while let Some(&(j, d)) = it.peek() {
                if d.is_alphanumeric() || d == '_' || d == '\'' || d == '.' {
                    end = j + d.len_utf8();
                    it.next();
                } else {
                    break;
                }
            }
            let word = code[i..end].trim_end_matches('.');
            out.push((i..i + word.len(), word));
        } else if c.is_ascii_digit() {
            while it.peek().is_some_and(|&(_, d)| d.is_alphanumeric() || d == '.') {
                it.next();
            }
        }
    }
    out
}

/// Names of the explicit binders `( … )`, in order – the arguments one passes when applying.
pub fn explicit_names(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (i, c) in code.char_indices() {
        match c {
            '(' | '{' | '[' | '⦃' => {
                if depth == 0 && c == '(' {
                    start = Some(i + 1);
                }
                depth += 1;
            }
            ')' | '}' | ']' | '⦄' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(s) = start.take()
                    && let Some((names, _)) = code[s..i].split_once(':')
                {
                    out.extend(names.split_whitespace().map(str::to_string));
                }
            }
            _ => {}
        }
    }
    out
}

/// Binders such as `(a : ℕ → ℝ) (L M : ℝ) {α : Type}` as (name, type) pairs.
pub fn binders(code: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (i, c) in code.char_indices() {
        match c {
            '(' | '{' | '[' | '⦃' => {
                if depth == 0 {
                    start = Some(i + c.len_utf8());
                }
                depth += 1;
            }
            ')' | '}' | ']' | '⦄' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(s) = start.take()
                    && let Some((names, ty)) = code[s..i].split_once(':')
                {
                    for n in names.split_whitespace() {
                        out.push((n.to_string(), ty.trim().to_string()));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_identifiers() {
        let words: Vec<_> = idents("∀ ε > 0, Filter.Tendsto a. x' 2.5 h₁").into_iter().map(|(_, w)| w).collect();
        assert_eq!(words, ["ε", "Filter.Tendsto", "a", "x'", "h₁"]);
    }

    #[test]
    fn reads_explicit_names() {
        assert_eq!(explicit_names("{α : Type} (a : ℕ → α) [inst : Foo] (L M : α)"), ["a", "L", "M"]);
    }

    #[test]
    fn reads_binders() {
        assert_eq!(
            binders("(a : ℕ → ℝ) (L M : ℝ) [inst : Foo (Bar x)]"),
            [
                ("a".into(), "ℕ → ℝ".into()),
                ("L".into(), "ℝ".into()),
                ("M".into(), "ℝ".into()),
                ("inst".into(), "Foo (Bar x)".into())
            ]
        );
    }
}
