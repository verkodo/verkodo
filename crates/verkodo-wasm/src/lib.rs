// SPDX-License-Identifier: AGPL-3.0-or-later
//! verkodo in the browser, compiled to WebAssembly: the same parser, analysis and hover cards as the command line and
//! the language server, for the web editor. Lean is not part of it – it runs on a server.
//!
//! The editor counts positions in UTF-16 code units, as JavaScript does; verkodo counts bytes. Everything that goes
//! in or out of [`Editor`] is in UTF-16 units.
//!
//! How JavaScript talks to it (no wasm-bindgen): it asks for a buffer (`vk_buffer(len)`, which returns its address in
//! the module's memory), writes a request there as UTF-8 JSON and calls an operation (`vk_load`, `vk_hover`,
//! `vk_definition`, `vk_preview`, `vk_lean`).
//!
//! Lean runs on a server (`POST /api/v1/prove`); its answer goes to `vk_lean` and holds as long as the files are
//! exactly the ones Lean checked – the first change drops it, as in the language server. The operation leaves its answer in the buffer, again as UTF-8 JSON, and returns
//! its length; `vk_address()` says where it starts.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use verkodo_core::proof::{LeanName, Report, ReportData};
use verkodo_core::query::{Found, at};
use verkodo_core::{Analysis, Sources, SymKind, analyze};
use verkodo_syntax::Severity;

/// A source file the editor holds.
#[derive(Debug, Deserialize)]
pub struct FileIn {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct DiagnosticOut {
    pub file: usize,
    pub from: usize,
    pub to: usize,
    pub severity: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct SymbolOut {
    pub file: usize,
    pub id: String,
    pub kind: &'static str,
    pub label: String,
    pub from: usize,
    pub to: usize,
}

/// A diagnostic from the server, in bytes.
#[derive(Debug, Deserialize)]
pub struct DiagnosticIn {
    pub file: usize,
    pub start: usize,
    pub end: usize,
    pub severity: String,
    pub message: String,
}

/// What Lean said about the files, from `POST /api/v1/prove`, with the files it checked.
#[derive(Debug, Deserialize)]
pub struct LeanIn {
    pub files: Vec<FileIn>,
    pub checked: usize,
    pub open: usize,
    pub rejected: usize,
    pub diagnostics: Vec<DiagnosticIn>,
    pub report: ReportData,
}

/// Lean's verdict, while it holds.
#[derive(Debug)]
struct Verdict {
    /// The texts Lean checked; the verdict holds while the editor's files are exactly these.
    texts: Vec<String>,
    report: Report,
    diagnostics: Vec<DiagnosticIn>,
    counts: (usize, usize, usize),
}

#[derive(Debug, Serialize)]
pub struct Loaded {
    pub diagnostics: Vec<DiagnosticOut>,
    /// Lean's counts (checked, open, rejected), while its verdict holds.
    pub lean: Option<(usize, usize, usize)>,
    pub outline: Vec<SymbolOut>,
    /// Paths, for the preview: (id, label).
    pub paths: Vec<(String, String)>,
}

/// The files of the editor, parsed and analysed.
#[derive(Debug)]
pub struct Editor {
    ws: Sources,
    a: Analysis,
    lang: String,
    lean: Option<Verdict>,
}

/// Byte offset → UTF-16 offset in `text`.
fn utf16(text: &str, byte: usize) -> usize {
    text.get(..byte.min(text.len())).map(|s| s.encode_utf16().count()).unwrap_or(0)
}

/// UTF-16 offset → byte offset in `text` (the start of the character it falls into).
fn byte(text: &str, unit: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        if units + c.len_utf16() > unit {
            return i;
        }
        units += c.len_utf16();
    }
    text.len()
}

impl Editor {
    /// Parses and analyses the files; `lang` is the language of labels and cards (`de`, `en`).
    pub fn load(files: Vec<FileIn>, lang: &str) -> (Editor, Loaded) {
        Self::load_keeping(files, lang, None)
    }

    /// Like [`Editor::load`], keeping Lean's verdict if it is about exactly these files.
    fn load_keeping(files: Vec<FileIn>, lang: &str, lean: Option<Verdict>) -> (Editor, Loaded) {
        let lean =
            lean.filter(|v| v.texts.len() == files.len() && v.texts.iter().zip(&files).all(|(t, f)| *t == f.text));
        let ws = Sources::from_sources(files.into_iter().map(|f| (f.path, f.text)).collect());
        let a = analyze(&ws);
        let ed = Editor { ws, a, lang: lang.to_string(), lean };
        let loaded = ed.loaded();
        (ed, loaded)
    }

    /// Takes Lean's verdict, if it is about the files the editor holds now; returns whether it did.
    pub fn take_lean(&mut self, lean: LeanIn) -> bool {
        let same = lean.files.len() == self.ws.files.len()
            && lean.files.iter().zip(&self.ws.files).all(|(f, s)| f.text == s.text);
        if same {
            self.lean = Some(Verdict {
                texts: lean.files.into_iter().map(|f| f.text).collect(),
                report: lean.report.into(),
                diagnostics: lean.diagnostics,
                counts: (lean.checked, lean.open, lean.rejected),
            });
        }
        same
    }

    fn report(&self) -> Option<&Report> {
        self.lean.as_ref().map(|v| &v.report)
    }

    fn text(&self, file: usize) -> &str {
        &self.ws.files[file].text
    }

    fn loaded(&self) -> Loaded {
        let out = |file: usize, start: usize, end: usize, error: bool, message: &str| {
            let t = self.text(file);
            DiagnosticOut {
                file,
                from: utf16(t, start),
                to: utf16(t, end.max(start)),
                severity: if error { "error" } else { "warning" },
                message: message.to_string(),
            }
        };
        // With Lean's verdict, the server's diagnostics: verkodo's and Lean's.
        let diagnostics = match &self.lean {
            Some(v) => v
                .diagnostics
                .iter()
                .filter(|d| d.file < self.ws.files.len())
                .map(|d| out(d.file, d.start, d.end, d.severity == "error", &d.message))
                .collect(),
            None => self
                .ws
                .diags
                .iter()
                .chain(&self.a.diags)
                .filter(|d| d.file < self.ws.files.len())
                .map(|d| out(d.file, d.span.start, d.span.end, d.severity == Severity::Error, &d.message))
                .collect(),
        };
        let label = |i: usize| self.a.label(&self.ws, i, &self.lang).unwrap_or_default();
        let outline = self
            .a
            .symbols
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let file = s.loc.file();
                let t = self.text(file);
                SymbolOut {
                    file,
                    id: s.id.clone(),
                    kind: s.kind.keyword(),
                    label: label(i),
                    from: utf16(t, s.span.start),
                    to: utf16(t, s.span.end),
                }
            })
            .collect();
        let paths = (0..self.a.symbols.len())
            .filter(|&i| self.a.symbols[i].kind == SymKind::Path)
            .map(|i| (self.a.symbols[i].id.clone(), label(i)))
            .collect();
        Loaded { diagnostics, lean: self.lean.as_ref().map(|v| v.counts), outline, paths }
    }

    fn found(&self, file: usize, pos: usize) -> Option<Found<'_>> {
        let t = self.ws.files.get(file)?;
        at(&self.ws, &self.a, self.report(), file, byte(&t.text, pos))
    }

    /// What is at a place, as an HTML card and the range it covers.
    pub fn hover(&self, file: usize, pos: usize) -> Value {
        let Some(found) = self.found(file, pos) else { return Value::Null };
        let de = self.lang == "de";
        let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let html = match &found {
            Found::Symbol { sym, .. } => verkodo_render::card_html(&self.ws, &self.a, self.report(), *sym, &self.lang),
            Found::Local { name, ty, .. } => format!(
                r#"<div class="kind">{}</div><div class="title"><code>{} : {}</code></div>"#,
                "Parameter",
                esc(name),
                esc(ty)
            ),
            Found::Lean { name, .. } => lean_card(name, de),
            Found::External { name, .. } => {
                format!(
                    r#"<div class="kind">Lean</div><div class="title"><code>{}</code></div><p class="hint">{}</p>"#,
                    esc(name),
                    if de {
                        "Ein Name aus Lean oder Mathlib – erklärt, sobald Lean geprüft hat (im Browser noch nicht)."
                    } else {
                        "A name from Lean or Mathlib – explained once Lean has checked (not in the browser yet)."
                    }
                )
            }
            Found::Builtin { name, .. } => format!(
                r#"<div class="kind">{}</div><div class="title"><code>{}</code></div><p>{}</p>"#,
                if de { "Eingebaute Funktion" } else { "Built-in function" },
                esc(name),
                builtin(name, de)
            ),
        };
        let span = found.span();
        let t = self.text(file);
        json!({ "from": utf16(t, span.start), "to": utf16(t, span.end), "html": html })
    }

    /// Where the declaration at a place is declared.
    pub fn definition(&self, file: usize, pos: usize) -> Value {
        let Some(Found::Symbol { sym, .. }) = self.found(file, pos) else { return Value::Null };
        let s = &self.a.symbols[sym];
        let f = s.loc.file();
        let t = self.text(f);
        json!({ "file": f, "from": utf16(t, s.span.start), "to": utf16(t, s.span.end) })
    }

    /// The goals of the tactic at a place, while Lean's verdict holds: `{"from", "to", "before", "after"}` or null.
    pub fn goals(&self, file: usize, pos: usize) -> Value {
        let Some(report) = self.report() else { return Value::Null };
        let Some(t) = self.ws.files.get(file) else { return Value::Null };
        match verkodo_core::query::goals_at(&self.ws, report, file, byte(&t.text, pos)) {
            Some((span, g)) => json!({
                "from": utf16(&t.text, span.start),
                "to": utf16(&t.text, span.end),
                "before": g.before,
                "after": g.after,
            }),
            None => Value::Null,
        }
    }

    /// The norms, with their inputs (name, label, type) – what the debugger asks for.
    pub fn norms(&self) -> Value {
        let norms: Vec<Value> = (0..self.a.symbols.len())
            .filter(|&n| self.a.symbols[n].kind == SymKind::Norm)
            .map(|n| {
                let inputs: Vec<Value> = verkodo_core::eval::evaluate(&self.ws, &self.a, n, &Default::default())
                    .into_iter()
                    .filter(|s| matches!(self.a.symbols[s.sym].kind, SymKind::Input | SymKind::Judgement))
                    .map(|s| {
                        let ty = self.a.member_types.get(&s.sym).map(|t| t.name()).unwrap_or("?");
                        json!({
                            "name": self.a.symbols[s.sym].id.rsplit('.').next().unwrap_or_default(),
                            "label": self.a.label(&self.ws, s.sym, &self.lang).unwrap_or_default(),
                            "type": ty,
                            "judgement": self.a.symbols[s.sym].kind == SymKind::Judgement,
                        })
                    })
                    .collect();
                json!({ "id": self.a.symbols[n].id, "label": self.a.label(&self.ws, n, &self.lang), "inputs": inputs })
            })
            .collect();
        Value::Array(norms)
    }

    /// Every member of a norm evaluated for the inputs (name → text as typed), with how each rule decided.
    pub fn evaluate(&self, norm: &str, inputs: &std::collections::HashMap<String, String>) -> Value {
        let Some(&n) = self.a.by_id.get(norm) else { return Value::Null };
        let steps: Vec<Value> = verkodo_core::eval::evaluate(&self.ws, &self.a, n, inputs)
            .into_iter()
            .map(|s| {
                let sym = &self.a.symbols[s.sym];
                json!({
                    "id": sym.id,
                    "kind": sym.kind.keyword(),
                    "label": self.a.label(&self.ws, s.sym, &self.lang),
                    "value": s.value.to_string(),
                    "known": !matches!(s.value, verkodo_core::eval::Value::Unknown(_)),
                    "default": s.default.map(|d| d.to_string()),
                    "exceptions": s.exceptions.iter().map(|(c, d)| json!([c.to_string(), d])).collect::<Vec<_>>(),
                    "file": sym.loc.file(),
                    "from": utf16(self.text(sym.loc.file()), sym.span.start),
                    "to": utf16(self.text(sym.loc.file()), sym.span.end),
                })
            })
            .collect();
        Value::Array(steps)
    }

    /// The page of a path (by id), or the index without one, as HTML.
    pub fn preview(&self, path: Option<&str>) -> String {
        match path.and_then(|p| self.a.by_id.get(p)) {
            Some(&p) if self.a.symbols[p].kind == SymKind::Path => {
                verkodo_render::path_html(&self.ws, &self.a, self.report(), p, &self.lang)
            }
            _ => verkodo_render::index_html(&self.ws, &self.a, self.report(), &self.lang),
        }
    }
}

/// A Lean or Mathlib name as Lean explained it: its type, doc string and module.
fn lean_card(n: &LeanName, de: bool) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut out = format!(
        r#"<div class="kind">Lean</div><div class="title"><code>{}</code></div><div class="row"><code>{}</code></div>"#,
        esc(&n.name),
        esc(&n.ty)
    );
    if let Some(doc) = &n.doc {
        out += &format!(r#"<p class="doc">{}</p>"#, esc(doc));
    }
    if let Some(m) = &n.module {
        let word = if de { "deklariert in" } else { "declared in" };
        out += &format!(r#"<div class="row"><span class="key">{word}</span><code>{}</code></div>"#, esc(m));
    }
    out
}

/// What a built-in function of the legal profile does.
fn builtin(name: &str, de: bool) -> &'static str {
    match (name, de) {
        ("min", true) => "Der kleinste der Werte.",
        ("min", false) => "The smallest of the values.",
        ("max", true) => "Der größte der Werte.",
        ("max", false) => "The largest of the values.",
        ("round_down", true) => "Rundet auf ein Vielfaches der Schrittweite ab („abrunden“).",
        ("round_down", false) => "Rounds down to a multiple of the step.",
        ("round_up", true) => "Rundet auf ein Vielfaches der Schrittweite auf („aufrunden“).",
        ("round_up", false) => "Rounds up to a multiple of the step.",
        ("round", true) => {
            "Rundet auf das nächste Vielfache der Schrittweite, Hälften vom Nullpunkt weg („kaufmännisch“)."
        }
        ("round", false) => "Rounds to the nearest multiple of the step, halves away from zero.",
        _ => "",
    }
}

// ---- the interface to JavaScript ----

thread_local! {
    static BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static EDITOR: RefCell<Option<Editor>> = const { RefCell::new(None) };
}

/// Reads the request from the buffer, runs `op`, leaves the answer in the buffer and returns its length.
fn answer(op: impl FnOnce(Value) -> Value) -> u32 {
    let request = BUFFER.with(|b| serde_json::from_slice::<Value>(&b.borrow()).unwrap_or(Value::Null));
    let out = serde_json::to_vec(&op(request)).unwrap_or_else(|_| b"null".to_vec());
    let len = out.len() as u32;
    BUFFER.with(|b| *b.borrow_mut() = out);
    len
}

fn num(v: &Value, key: &str) -> usize {
    v.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}

fn with_editor(op: impl FnOnce(&Editor) -> Value) -> Value {
    EDITOR.with(|e| e.borrow().as_ref().map(op).unwrap_or(Value::Null))
}

/// A buffer of `len` bytes for the next request; returns its address.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_buffer(len: u32) -> u32 {
    BUFFER.with(|b| {
        let mut b = b.borrow_mut();
        b.clear();
        b.resize(len as usize, 0);
        b.as_ptr() as usize as u32
    })
}

/// Where the buffer starts – read the answer there.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_address() -> u32 {
    BUFFER.with(|b| b.borrow().as_ptr() as usize as u32)
}

/// `{"files": [{"path", "text"}], "lang"}` → diagnostics, outline and paths.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_load() -> u32 {
    answer(|req| {
        let files: Vec<FileIn> =
            req.get("files").cloned().and_then(|f| serde_json::from_value(f).ok()).unwrap_or_default();
        let lang = req.get("lang").and_then(Value::as_str).unwrap_or("de");
        let kept = EDITOR.with(|e| e.borrow_mut().take().and_then(|ed| ed.lean));
        let (ed, loaded) = Editor::load_keeping(files, lang, kept);
        EDITOR.with(|e| *e.borrow_mut() = Some(ed));
        serde_json::to_value(loaded).unwrap_or(Value::Null)
    })
}

/// What `POST /api/v1/prove` answered, with `"files"` – the files sent – → whether the verdict holds for the files the
/// editor holds now (`taken`), and the diagnostics, outline and counts that follow (`loaded`).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_lean() -> u32 {
    answer(|req| {
        let Ok(lean) = serde_json::from_value::<LeanIn>(req) else { return Value::Null };
        EDITOR.with(|e| {
            let mut e = e.borrow_mut();
            let Some(ed) = e.as_mut() else { return Value::Null };
            let taken = ed.take_lean(lean);
            json!({ "taken": taken, "loaded": ed.loaded() })
        })
    })
}

/// `{"text", "style": "delimiters" | "indentation" | null, "width": n | null}` → `{"text"}` or `{"error"}`; without a style, only tabs
/// become spaces.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_format() -> u32 {
    use verkodo_syntax::format::{Options, Style, format_with};
    answer(|req| {
        let text = req.get("text").and_then(Value::as_str).unwrap_or_default();
        let width = req.get("width").and_then(Value::as_u64).map(|w| w as usize);
        let result = match req.get("style").and_then(Value::as_str) {
            Some("delimiters") => format_with(text, Options { style: Style::Delimiters, width }),
            Some("indentation") => format_with(text, Options { style: Style::Indentation, width }),
            _ => Ok(verkodo_syntax::lexer::spaces_for_tabs(text)),
        };
        match result {
            Ok(t) => json!({ "text": t }),
            Err(e) => json!({ "error": e }),
        }
    })
}

/// `{"file", "pos"}` → the goals of the tactic there, or null.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_goals() -> u32 {
    answer(|req| with_editor(|ed| ed.goals(num(&req, "file"), num(&req, "pos"))))
}

/// `{}` → the norms with their inputs.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_norms() -> u32 {
    answer(|_| with_editor(Editor::norms))
}

/// `{"norm", "inputs": {name: text}}` → every member with its value and how it was decided.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_eval() -> u32 {
    answer(|req| {
        let norm = req.get("norm").and_then(Value::as_str).unwrap_or_default().to_string();
        let inputs = req.get("inputs").cloned().and_then(|i| serde_json::from_value(i).ok()).unwrap_or_default();
        with_editor(|ed| ed.evaluate(&norm, &inputs))
    })
}

/// `{"file", "pos"}` → `{"from", "to", "html"}` or null.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_hover() -> u32 {
    answer(|req| with_editor(|ed| ed.hover(num(&req, "file"), num(&req, "pos"))))
}

/// `{"file", "pos"}` → `{"file", "from", "to"}` or null.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_definition() -> u32 {
    answer(|req| with_editor(|ed| ed.definition(num(&req, "file"), num(&req, "pos"))))
}

/// `{"path": id or null}` → the HTML of the page.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn vk_preview() -> u32 {
    answer(|req| with_editor(|ed| Value::String(ed.preview(req.get("path").and_then(Value::as_str)))))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"module m
// „Grüße“: characters outside ASCII before the declarations
norm n {
  label en "N"
  input x : money { label en "income" }
  rule r : money = round_down(x, 1 EUR) + nope
}
path p { label en "P" step n.r }"#;

    fn editor() -> (Editor, Loaded) {
        Editor::load(vec![FileIn { path: "m.vk".into(), text: SRC.into() }], "en")
    }

    /// The UTF-16 offset of the `nth` occurrence of `word`.
    fn pos(word: &str, nth: usize) -> usize {
        utf16(SRC, SRC.match_indices(word).nth(nth).unwrap().0)
    }

    #[test]
    fn counts_in_utf16() {
        let s = "a„b𝔸c";
        assert_eq!(utf16(s, s.find('b').unwrap()), 2);
        assert_eq!(utf16(s, s.find('c').unwrap()), 5);
        assert_eq!(byte(s, 5), s.find('c').unwrap());
        assert_eq!(byte(s, 4), s.find('𝔸').unwrap()); // inside the surrogate pair
    }

    #[test]
    fn loads_diagnostics_outline_and_paths() {
        let (_, l) = editor();
        let d = &l.diagnostics[0];
        assert_eq!((d.severity, d.message.as_str()), ("error", "cannot find `nope`"));
        assert_eq!((d.from, d.to), (pos("nope", 0), pos("nope", 0) + 4));
        assert!(l.outline.iter().any(|s| s.id == "m.n.x" && s.kind == "input" && s.label == "income"));
        assert_eq!(l.paths, [("m.p".to_string(), "P".to_string())]);
    }

    #[test]
    fn lean_holds_while_the_files_are_the_same() {
        let text = "module m\ntheorem t { statement lean \"nhds 0 = nhds 0\" proof lean \"rfl\" }";
        let files = || vec![FileIn { path: "m.vk".into(), text: text.into() }];
        let (mut ed, l) = Editor::load(files(), "en");
        assert!(l.lean.is_none());
        let t = ed.a.by_id["m.t"];
        let literal = text.find("\"nhds").unwrap();
        let report = serde_json::json!({
            "status": [[{ "Theorem": t }, "Checked"]],
            "transports": [],
            "names": [[[0, literal, literal + 17], [{
                "range": { "start": 0, "end": 4 }, "name": "nhds", "ty": "X → Filter X",
                "doc": "The <neighbourhood> filter.", "module": "Mathlib.Topology.Defs.Filter", "decl": null
            }]]]
        });
        let lean = |texts: Vec<FileIn>| LeanIn {
            files: texts,
            checked: 1,
            open: 0,
            rejected: 0,
            diagnostics: vec![],
            report: serde_json::from_value(report.clone()).unwrap(),
        };
        // About other files: not taken.
        assert!(!ed.take_lean(lean(vec![FileIn { path: "m.vk".into(), text: "module x".into() }])));
        assert!(ed.take_lean(lean(files())));
        assert_eq!(ed.loaded().lean, Some((1, 0, 0)));
        // Lean names explain themselves, escaped.
        let h = ed.hover(0, utf16(text, text.find("nhds").unwrap()));
        let html = h["html"].as_str().unwrap();
        assert!(html.contains("X → Filter X") && html.contains("&lt;neighbourhood&gt;"), "{html}");
        assert!(html.contains("Mathlib.Topology.Defs.Filter"), "{html}");
        // The same files keep the verdict, a change drops it.
        let (ed, l) = Editor::load_keeping(files(), "en", ed.lean);
        assert_eq!(l.lean, Some((1, 0, 0)));
        let changed = vec![FileIn { path: "m.vk".into(), text: format!("{text}\n") }];
        let (_, l) = Editor::load_keeping(changed, "en", ed.lean);
        assert!(l.lean.is_none());
    }

    #[test]
    fn hovers_and_finds_definitions() {
        let (ed, _) = editor();
        // `x` in the rule: the card of the input, and its declaration.
        let at_x = pos("(x,", 0) + 1;
        let h = ed.hover(0, at_x);
        assert!(h["html"].as_str().unwrap().contains("income"), "{h}");
        assert_eq!((h["from"].as_u64(), h["to"].as_u64()), (Some(at_x as u64), Some(at_x as u64 + 1)));
        let d = ed.definition(0, at_x);
        assert_eq!(d["from"].as_u64(), Some(pos("x : money", 0) as u64));
        // A built-in function explains itself.
        let r = ed.hover(0, pos("round_down", 0));
        assert!(r["html"].as_str().unwrap().contains("Rounds down"), "{r}");
        assert!(ed.hover(0, 0).is_null());
        // The preview of the path, and the index.
        assert!(ed.preview(Some("m.p")).contains("<h1>P</h1>"));
        assert!(ed.preview(None).contains("<h1>verkodo</h1>"));
    }
}
