// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runs Lean on the generated files and reads its messages back.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use verkodo_core::proof::{DeclLocation, Goals, Key, LeanName, Report, Status};
use verkodo_core::{Analysis, Loc, Sources};
use verkodo_syntax::ast::Item;
use verkodo_syntax::{Diagnostic, Span};

use crate::export::{Generated, export};

#[derive(Debug)]
pub struct ProveOutcome {
    pub report: Report,
    pub diags: Vec<Diagnostic>,
    pub files: Vec<PathBuf>,
}

/// The Lean program that checks a generated file and reports the names it resolved (`names.lean`).
const NAMES_TOOL: &str = include_str!("names.lean");
const NAMES_TOOL_FILE: &str = "generated/verkodo_names.lean";

/// Writes the Lean files below `<lean_dir>/generated/` and checks each with Lean (`lake env lean --run` with
/// `names.lean`, which elaborates the file like `lean` and also reports every name it resolved).
pub fn prove(ws: &Sources, a: &Analysis, lean_dir: &Path) -> io::Result<ProveOutcome> {
    let (generated, mut diags) = export(ws, a);
    let gen_dir = lean_dir.join("generated");
    std::fs::create_dir_all(&gen_dir)?;
    std::fs::write(lean_dir.join(NAMES_TOOL_FILE), NAMES_TOOL)?;
    let mut report = Report::default();
    let mut files = Vec::new();
    for g in &generated {
        let rel = format!("generated/{}.lean", g.module);
        let names_rel = format!("generated/{}.names.jsonl", g.module);
        let path = lean_dir.join(&rel);
        std::fs::write(&path, &g.code)?;
        let _ = std::fs::remove_file(lean_dir.join(&names_rel));
        let out = Command::new("lake")
            .args(["env", "lean", "--run", NAMES_TOOL_FILE, &rel, &names_rel])
            .current_dir(lean_dir)
            .output()
            .map_err(|e| io::Error::new(e.kind(), format!("cannot run `lake` in {}: {e}", lean_dir.display())))?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let mut messages = parse_messages(&text, &rel);
        // `names.lean` fails when Lean reported an error: never call a file checked that Lean did not accept.
        if !out.status.success() && !messages.iter().any(|m| m.error) {
            let first: Vec<&str> = text.lines().take(3).collect();
            let text = format!("Lean failed ({}) with no error verkodo could read: {}", out.status, first.join(" / "));
            messages.push(Message { line: 0, error: true, text });
        }
        apply(ws, a, g, &messages, &path, &mut report, &mut diags);
        if let Ok(jsonl) = std::fs::read_to_string(lean_dir.join(&names_rel)) {
            names_back(a, g, &jsonl, &mut report);
        }
        files.push(path);
    }
    for names in report.names.values_mut() {
        names.sort_by_key(|n| (n.range.start, n.range.end));
        names.dedup();
    }
    Ok(ProveOutcome { report, diags, files })
}

#[derive(Debug, PartialEq)]
struct Message {
    line: usize,
    error: bool,
    text: String,
}

/// Lean prints `file:line:col: error: text`, continued on following lines; newer versions tag some messages with a
/// name, as in `error(lean.unknownIdentifier): text`.
fn parse_messages(out: &str, rel: &str) -> Vec<Message> {
    let prefix = format!("{rel}:");
    let mut msgs: Vec<Message> = Vec::new();
    for l in out.lines() {
        if let Some(rest) = l.strip_prefix(&prefix) {
            let mut parts = rest.splitn(4, ':');
            let line = parts.next().and_then(|x| x.trim().parse().ok());
            let _col = parts.next();
            let sev = parts.next().map(str::trim);
            let text = parts.next().map(str::trim).unwrap_or_default();
            if let (Some(line), Some(sev)) = (line, sev) {
                let severity = sev.split('(').next().unwrap_or_default();
                msgs.push(Message { line, error: severity == "error", text: text.to_string() });
                continue;
            }
        }
        if let Some(m) = msgs.last_mut() {
            m.text.push('\n');
            m.text.push_str(l);
        } else if !l.trim().is_empty() {
            msgs.push(Message { line: 0, error: true, text: l.to_string() });
        }
    }
    msgs
}

fn apply(
    ws: &Sources,
    a: &Analysis,
    g: &Generated,
    msgs: &[Message],
    path: &Path,
    report: &mut Report,
    diags: &mut Vec<Diagnostic>,
) {
    for m in msgs.iter().filter(|m| m.error && !g.decls.iter().any(|(_, r)| r.contains(&m.line))) {
        let first = m.text.lines().next().unwrap_or_default();
        let msg = format!("Lean: {first} (in {}:{})", path.display(), m.line);
        diags.push(Diagnostic::error(g.file, Span::default(), msg));
    }
    for (key, range) in &g.decls {
        let inside: Vec<&Message> = msgs.iter().filter(|m| range.contains(&m.line)).collect();
        let errors: Vec<String> = inside.iter().filter(|m| m.error).map(|m| m.text.clone()).collect();
        let status = if !errors.is_empty() {
            let first = errors[0].lines().next().unwrap_or_default().to_string();
            let (file, span, what) = locate(ws, a, *key);
            let msg = format!("Lean rejects {what}: {first} (see {}:{})", path.display(), range.start);
            diags.push(Diagnostic::error(file, span, msg));
            Status::Failed(errors)
        } else if inside.iter().any(|m| m.text.contains("sorry")) {
            Status::Open
        } else {
            Status::Checked
        };
        report.status.insert(*key, status);
    }
    // A transport is only as good as the theorem it carries over and the bridges it goes through.
    for (key, _) in &g.decls {
        let Key::Transport { theorem, path } = *key else { continue };
        if report.status.get(key) != Some(&Status::Checked) {
            continue;
        }
        let used = std::iter::once(Key::Theorem(theorem))
            .chain(g.bridges.get(&(theorem, path)).into_iter().flatten().copied());
        // Rejected before open before checked.
        let worst = used
            .filter_map(|k| report.status.get(&k).cloned())
            .max_by_key(|s| match s {
                Status::Checked => 0,
                Status::Open => 1,
                Status::Failed(_) => 2,
            })
            .filter(|s| *s != Status::Checked);
        if let Some(base) = worst {
            report.status.insert(*key, base);
        }
    }
    for &(theorem, path) in &g.same {
        if let Some(base) = report.status.get(&Key::Theorem(theorem)).cloned() {
            report.status.insert(Key::Transport { theorem, path }, base);
        }
    }
    for (k, v) in &g.via {
        report.transports.insert(*k, v.clone());
    }
}

/// A line of what `names.lean` writes about a tactic.
#[derive(Debug, serde::Deserialize)]
struct Tactic {
    start: usize,
    end: usize,
    before: Vec<String>,
    after: Vec<String>,
}

/// One line of what `names.lean` writes about a name.
#[derive(Debug, serde::Deserialize)]
struct Resolved {
    start: usize,
    end: usize,
    name: String,
    #[serde(rename = "type")]
    ty: String,
    doc: Option<String>,
    module: Option<String>,
    decl: Option<ResolvedDecl>,
}

#[derive(Debug, serde::Deserialize)]
struct ResolvedDecl {
    file: String,
    line: u32,
    character: u32,
}

/// Carries the names Lean resolved in a generated file back onto the Lean snippets of the source. Declarations of
/// verkodo itself are left out – they are symbols already.
fn names_back(a: &Analysis, g: &Generated, jsonl: &str, report: &mut Report) {
    for line in jsonl.lines().filter(|l| !l.trim().is_empty()) {
        if line.contains("\"goals\":true") {
            if let Ok(t) = serde_json::from_str::<Tactic>(line) {
                goals_back(g, t, report);
            }
            continue;
        }
        let Ok(r) = serde_json::from_str::<Resolved>(line) else {
            continue;
        };
        if a.by_id.contains_key(&r.name) {
            continue;
        }
        for o in &g.origins {
            if let Some(range) = o.source_range(r.start..r.end) {
                report.names.entry((g.file, o.span)).or_default().push(LeanName {
                    range,
                    name: r.name.clone(),
                    ty: r.ty.clone(),
                    doc: r.doc.as_deref().map(str::trim).filter(|d| !d.is_empty()).map(str::to_string),
                    module: r.module.clone(),
                    decl: r.decl.as_ref().map(|d| DeclLocation {
                        file: d.file.clone(),
                        line: d.line,
                        character: d.character,
                    }),
                });
            }
        }
    }
}

/// Carries a tactic and its goals back onto the Lean snippet it was written in; one entry per place.
fn goals_back(g: &Generated, t: Tactic, report: &mut Report) {
    for o in &g.origins {
        if let Some(range) = o.source_bounds(t.start..t.end) {
            let list = report.goals.entry((g.file, o.span)).or_default();
            if !list.iter().any(|x| x.range == range) {
                list.push(Goals { range, before: t.before, after: t.after });
                list.sort_by_key(|x| (x.range.start, std::cmp::Reverse(x.range.end)));
            }
            return;
        }
    }
}

/// Where a part is in the verkodo source, and how to name it in a message.
fn locate(ws: &Sources, a: &Analysis, key: Key) -> (usize, Span, String) {
    let at = |s: usize| (a.symbols[s].loc.file(), a.symbols[s].span);
    match key {
        Key::Variant(v) => {
            let (f, s) = at(v);
            (f, s, format!("the definition `{}`", a.symbols[v].id))
        }
        Key::Theorem(t) => {
            let (f, s) = at(t);
            (f, s, format!("`{}`", a.symbols[t].id))
        }
        Key::Transport { theorem, path } => {
            let (f, s) = at(theorem);
            (f, s, format!("`{}` carried over to path `{}`", a.symbols[theorem].id, a.symbols[path].id))
        }
        Key::Norm(n) => {
            let (f, s) = at(n);
            (f, s, format!("the inputs of `{}`", a.symbols[n].id))
        }
        Key::Member(m) => {
            let (f, s) = at(m);
            (f, s, format!("`{}`", a.symbols[m].id))
        }
        Key::Bridge { concept, index } => {
            let (f, mut s) = at(concept);
            if let Loc::Item { file, item } = a.symbols[concept].loc
                && let Item::Concept(c) = &ws.asts[file].items[item]
                && let Some(b) = c.bridges.get(index)
            {
                s = b.span;
            }
            (f, s, format!("a bridge of `{}`", a.symbols[concept].id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_back_onto_the_snippet() {
        let src = r#"module m
            concept c { params lean "(x : ℝ)" variant p { lean "0 < Real.sqrt x" } }"#;
        let ws = Sources::from_sources(vec![("m.vk".into(), src.into())]);
        let a = verkodo_core::analyze(&ws);
        let (g, _) = crate::export(&ws, &a);
        let g = &g[0];
        let at = g.code.find("Real.sqrt").unwrap();
        let p = g.code.find("abbrev c.p").unwrap() + "abbrev ".len();
        let jsonl = format!(
            "{{\"start\":{at},\"end\":{},\"name\":\"Real.sqrt\",\"type\":\"ℝ → ℝ\",\"doc\":\" The square root. \",\"module\":\"Mathlib.X\"}}\n\
             {{\"start\":{p},\"end\":{},\"name\":\"m.c.p\",\"type\":\"ℝ → Prop\",\"doc\":null,\"module\":null}}\n",
            at + "Real.sqrt".len(),
            p + 3
        );
        let mut report = Report::default();
        names_back(&a, g, &jsonl, &mut report);
        let names: Vec<&LeanName> = report.names.values().flatten().collect();
        assert_eq!(names.len(), 1, "{names:?}");
        assert_eq!(names[0].name, "Real.sqrt");
        assert_eq!(names[0].range, 4..13);
        assert_eq!(names[0].doc.as_deref(), Some("The square root."));
    }

    #[test]
    fn parses_lean_messages() {
        let out = "generated/m.lean:12:4: error: unsolved goals\nx : ℕ\n⊢ False\ngenerated/m.lean:20:8: warning: declaration uses 'sorry'\n";
        let m = parse_messages(out, "generated/m.lean");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].line, 12);
        assert!(m[0].error && m[0].text.contains("⊢ False"));
        assert!(!m[1].error && m[1].text.contains("sorry"));
        // A message tagged with its name.
        let out = "generated/m.lean:7:1: error(lean.unknownIdentifier): Unknown identifier `x`\n";
        let m = parse_messages(out, "generated/m.lean");
        assert!(m[0].error && m[0].line == 7, "{m:?}");
    }
}
