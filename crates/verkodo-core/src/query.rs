// SPDX-License-Identifier: AGPL-3.0-or-later
//! What is at a place in a source file – for hover and go-to-definition in an editor.

use verkodo_syntax::Span;
use verkodo_syntax::lexer::lean_offsets;

use crate::analysis::{Analysis, Target};
use crate::proof::{Goals, LeanName, Report};
use crate::sources::Sources;

/// What a place in a source file refers to; `span` is where it is written, in bytes of that file.
#[derive(Clone, Debug, PartialEq)]
pub enum Found<'a> {
    /// A declaration of verkodo – where it is declared or where it is used.
    Symbol { sym: usize, span: Span },
    /// A parameter inside a Lean snippet.
    Local { name: String, ty: String, span: Span },
    /// A Lean or Mathlib name as Lean resolved it.
    Lean { name: &'a LeanName, span: Span },
    /// A Lean or Mathlib name that Lean has not explained yet (no report).
    External { name: String, span: Span },
    /// A built-in function of the legal profile.
    Builtin { name: String, span: Span },
}

impl Found<'_> {
    pub fn span(&self) -> Span {
        match self {
            Found::Symbol { span, .. }
            | Found::Local { span, .. }
            | Found::Lean { span, .. }
            | Found::External { span, .. }
            | Found::Builtin { span, .. } => *span,
        }
    }
}

/// What is at byte `offset` of `file`. Names in Lean snippets come from the analysis (declarations, parameters) and,
/// if there is a report, from Lean itself.
pub fn at<'r>(ws: &Sources, a: &Analysis, report: Option<&'r Report>, file: usize, offset: usize) -> Option<Found<'r>> {
    let inside = |s: Span| s.start <= offset && offset < s.end;
    // Declared names.
    if let Some((sym, s)) = a.symbols.iter().enumerate().find(|(_, s)| s.loc.file() == file && inside(s.span)) {
        return Some(Found::Symbol { sym, span: s.span });
    }
    // Names in expressions, steps, `using`, `about`, bridge sides.
    for ((f, span), target) in &a.refs {
        if *f == file && inside(*span) {
            return Some(match target {
                Target::Symbol(sym) => Found::Symbol { sym: *sym, span: *span },
                Target::Builtin(name) => Found::Builtin { name: name.clone(), span: *span },
                Target::Local { name, ty } => Found::Local { name: name.clone(), ty: ty.clone(), span: *span },
                Target::External(name) => Found::External { name: name.clone(), span: *span },
            });
        }
    }
    // Names in Lean snippets: the place in the literal, through escapes, to the place in the snippet's code.
    let text = &ws.files.get(file)?.text;
    let literal = a
        .lean_refs
        .keys()
        .chain(report.into_iter().flat_map(|r| r.names.keys()))
        .find(|(f, span)| *f == file && inside(*span))
        .map(|(_, span)| *span)?;
    let offsets = lean_offsets(text.get(literal.start..literal.end)?);
    let code = offsets.iter().rposition(|&o| literal.start + o <= offset)?;
    let to_span = |r: &std::ops::Range<usize>| {
        let start = literal.start + offsets.get(r.start).copied()?;
        let end = literal.start + offsets.get(r.end).copied()?;
        Some(Span::new(start, end))
    };
    let contains = |r: &std::ops::Range<usize>| r.start <= code && code < r.end;
    if let Some(r) = a.lean_refs.get(&(file, literal)).into_iter().flatten().find(|r| contains(&r.range)) {
        let lean = report.and_then(|rep| rep.names.get(&(file, literal))?.iter().find(|n| n.range == r.range));
        let span = to_span(&r.range)?;
        return Some(match (&r.target, lean) {
            (Target::Symbol(sym), _) => Found::Symbol { sym: *sym, span },
            (Target::Local { name, ty }, _) => Found::Local { name: name.clone(), ty: ty.clone(), span },
            (_, Some(name)) => Found::Lean { name, span },
            (Target::External(name), None) => Found::External { name: name.clone(), span },
            (Target::Builtin(name), None) => Found::Builtin { name: name.clone(), span },
        });
    }
    let name = report?.names.get(&(file, literal))?.iter().find(|n| contains(&n.range))?;
    Some(Found::Lean { name, span: to_span(&name.range)? })
}

/// The tactic at byte `offset` of `file` – the innermost one around it – with its goals, and where it is written.
pub fn goals_at<'r>(ws: &Sources, report: &'r Report, file: usize, offset: usize) -> Option<(Span, &'r Goals)> {
    let text = &ws.files.get(file)?.text;
    let (literal, list) =
        report.goals.iter().find(|((f, span), _)| *f == file && span.start <= offset && offset <= span.end)?;
    let literal = literal.1;
    let offsets = lean_offsets(text.get(literal.start..literal.end)?);
    let code = offsets.iter().rposition(|&o| literal.start + o <= offset)?;
    let g = list
        .iter()
        .filter(|g| g.range.start <= code && code <= g.range.end)
        .min_by_key(|g| g.range.end - g.range.start)?;
    let start = literal.start + offsets.get(g.range.start).copied()?;
    let end = literal.start + offsets.get(g.range.end).copied()?;
    Some((Span::new(start, end), g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze;

    #[test]
    fn finds_declarations_references_and_names_in_formulas() {
        let src = "module m\nconcept c { params lean \"(x : ℝ)\" variant p { lean \"0 < x\" } }\n\
                   theorem t { params lean \"(x : ℝ)\" statement lean \"c x → Real.sqrt x = x\" }\n\
                   path w { step c step t }";
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        let at_word = |word: &str, nth: usize| {
            let offset = src.match_indices(word).nth(nth).unwrap().0;
            at(&ws, &a, None, 0, offset)
        };
        let c = a.by_id["m.c"];
        // The declaration, a reference in a formula, a step of a path.
        assert_eq!(at_word("c {", 0).map(|f| f.span()), Some(a.symbols[c].span));
        assert!(matches!(at_word("c x →", 0), Some(Found::Symbol { sym, .. }) if sym == c));
        let step = src.find("step c").unwrap() + "step ".len();
        assert!(matches!(at(&ws, &a, None, 0, step), Some(Found::Symbol { sym, .. }) if sym == c));
        // A parameter, and a Lean name nobody explained yet.
        let x = src.find("x → Real").unwrap();
        assert!(matches!(at(&ws, &a, None, 0, x), Some(Found::Local { ref name, .. }) if name == "x"));
        match at_word("Real.sqrt", 0) {
            Some(Found::External { name, span }) => {
                assert_eq!(name, "Real.sqrt");
                assert_eq!(&src[span.start..span.end], "Real.sqrt");
            }
            other => panic!("{other:?}"),
        }
        // Nothing at a keyword.
        assert_eq!(at_word("module", 0), None);
    }

    #[test]
    fn finds_names_in_blocks_of_lean_code() {
        let src = "module m\nconcept c { variant v { lean \"True\" } }\n\
                   theorem t {\n  statement lean \"c\"\n  proof lean ```\n    by\n      exact trivial\n  ```\n}";
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let literal = ws.asts[0].items.iter().find_map(|i| match i {
            verkodo_syntax::ast::Item::Theorem(t) => t.proof.as_ref().map(|p| p.span),
            _ => None,
        });
        let proof = ws.asts[0].items.iter().find_map(|i| match i {
            verkodo_syntax::ast::Item::Theorem(t) => t.proof.clone(),
            _ => None,
        });
        assert_eq!(proof.unwrap().code, "by\n  exact trivial");
        // Lean reports `trivial` at its place in the code; it is found at its place in the source.
        let code_at = "by\n  exact ".len();
        let mut report = Report::default();
        let name = LeanName {
            range: code_at..code_at + 7,
            name: "trivial".into(),
            ty: "True".into(),
            doc: None,
            module: None,
            decl: None,
        };
        report.names.insert((0, literal.unwrap()), vec![name]);
        let offset = src.find("trivial").unwrap() + 2;
        match at(&ws, &a, Some(&report), 0, offset) {
            Some(Found::Lean { name, span }) => {
                assert_eq!(name.name, "trivial");
                assert_eq!(&src[span.start..span.end], "trivial");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn finds_the_goals_of_the_innermost_tactic() {
        let src = "module m\ntheorem t {\n  statement lean \"True ∧ True\"\n  proof lean ```\n    by\n      constructor\n      · trivial\n      · trivial\n  ```\n}";
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        assert!(a.diags.is_empty(), "{:?}", a.diags);
        let literal = ws.asts[0].items.iter().find_map(|i| match i {
            verkodo_syntax::ast::Item::Theorem(t) => t.proof.as_ref().map(|p| p.span),
            _ => None,
        });
        let code = "by\n  constructor\n  · trivial\n  · trivial";
        let at = |w: &str, nth: usize| code.match_indices(w).nth(nth).unwrap().0;
        let goals = |r: std::ops::Range<usize>, b: &str| Goals { range: r, before: vec![b.into()], after: vec![] };
        let mut report = Report::default();
        report.goals.insert(
            (0, literal.unwrap()),
            vec![
                goals(0..code.len(), "⊢ True ∧ True"),
                goals(at("constructor", 0)..at("constructor", 0) + 11, "⊢ True ∧ True"),
                goals(at("trivial", 1)..at("trivial", 1) + 7, "⊢ True (second)"),
            ],
        );
        // In the second `trivial` of the source: its goal, and where it is written.
        let offset = src.match_indices("trivial").nth(1).unwrap().0 + 3;
        let (span, g) = goals_at(&ws, &report, 0, offset).unwrap();
        assert_eq!(g.before, ["⊢ True (second)"]);
        assert_eq!(&src[span.start..span.end], "trivial");
        // On `by`: the whole proof.
        let (_, g) = goals_at(&ws, &report, 0, src.find("by\n").unwrap()).unwrap();
        assert_eq!(g.range, 0..code.len());
    }

    #[test]
    fn finds_lean_names_from_the_report() {
        let src = "module m\ntheorem t { statement lean \"nhds \\\"x\\\" = nhds 0\" }";
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = analyze(&ws);
        let literal = ws.asts[0].items.iter().find_map(|i| match i {
            verkodo_syntax::ast::Item::Theorem(t) => t.statement.as_ref().map(|s| s.span),
            _ => None,
        });
        let mut report = Report::default();
        let name = LeanName {
            range: 11..15,
            name: "nhds".into(),
            ty: "X → Filter X".into(),
            doc: None,
            module: None,
            decl: None,
        };
        report.names.insert((0, literal.unwrap()), vec![name]);
        // The second `nhds` comes after the escaped quotes.
        let offset = src.rfind("nhds").unwrap() + 1;
        match at(&ws, &a, Some(&report), 0, offset) {
            Some(Found::Lean { name, span }) => {
                assert_eq!(name.name, "nhds");
                assert_eq!(&src[span.start..span.end], "nhds");
            }
            other => panic!("{other:?}"),
        }
    }
}
