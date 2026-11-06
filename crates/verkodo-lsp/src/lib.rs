// SPDX-License-Identifier: AGPL-3.0-or-later
//! The language server of verkodo (Language Server Protocol over stdin and stdout): diagnostics while typing, hover
//! with what a hover card on the web shows, go-to-definition – also into the Lean and Mathlib sources – and an
//! outline of each file. On saving, Lean checks the workspace in the background (if there is a Lean project); its
//! verdict and what it says about Lean and Mathlib names come in when it is done.
//!
//! Everything goes through `verkodo-sdk`. The few types of the protocol it needs are written out here as JSON.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lsp_server::{Connection, Message, Notification, Request, Response};
use serde_json::{Value, json};
use verkodo_sdk::knowledge::{self, Analysis, Diagnostic, Found, Report, Severity, Sources, SymKind};

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Runs the server on stdin and stdout until the client ends it.
pub fn stdio() -> Result<(), Error> {
    let (connection, io) = Connection::stdio();
    run(&connection)?;
    drop(connection);
    io.join()?;
    Ok(())
}

/// Runs the server on a connection until the client ends it.
pub fn run(connection: &Connection) -> Result<(), Error> {
    let (id, params) = connection.initialize_start()?;
    let server = Server::new(&params, connection.sender.clone());
    connection.initialize_finish(
        id,
        json!({
            "capabilities": {
                "positionEncoding": "utf-16",
                "textDocumentSync": { "openClose": true, "change": 1, "save": { "includeText": false } },
                "hoverProvider": true,
                "definitionProvider": true,
                "documentSymbolProvider": true,
                "documentFormattingProvider": true
            },
            "serverInfo": { "name": "verkodo", "version": env!("CARGO_PKG_VERSION") }
        }),
    )?;
    server.check();
    for message in &connection.receiver {
        match message {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                let response = server.request(&req);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Notification(n) => server.notification(&n),
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// What the server knows; shared with the thread that runs Lean.
#[derive(Debug)]
struct State {
    /// Texts of the files open in the editor, which win over the disk.
    open: HashMap<PathBuf, String>,
    sources: Sources,
    analysis: Analysis,
    /// Lean's report and diagnostics – only while they belong to the texts above.
    report: Option<Report>,
    lean_diags: Vec<Diagnostic>,
    /// Counts the changes, so that a Lean run on older texts is thrown away.
    generation: u64,
    proving: bool,
    /// Files that got diagnostics last time, to clear them when there are none.
    published: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
struct Server {
    /// Where `.vk` files are searched (the workspace folders).
    roots: Vec<PathBuf>,
    lang: String,
    /// The way of writing that formatting writes; none: formatting only turns tabs into spaces.
    style: Option<knowledge::Style>,
    /// The widest a line of text may be when formatting; none: texts are not broken.
    width: Option<usize>,
    /// The Lean project that checks on save, if any.
    lean: Option<PathBuf>,
    state: Arc<Mutex<State>>,
    sender: crossbeam_channel::Sender<Message>,
}

impl Server {
    /// Options (`initializationOptions`): `lang` (`"de"` or `"en"`, default `"de"`), `lean` (the Lean project; default
    /// `lean/` in the workspace if it has a lakefile; `false` for none), `style` (`"delimiters"` or `"indentation"`:
    /// how formatting writes files; without it, formatting only turns tabs into spaces), `width` (the widest a line of
    /// text may be; longer texts are broken into lines).
    fn new(params: &Value, sender: crossbeam_channel::Sender<Message>) -> Self {
        let mut roots: Vec<PathBuf> = params["workspaceFolders"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f["uri"].as_str().and_then(uri_to_path))
            .collect();
        if roots.is_empty()
            && let Some(root) = params["rootUri"].as_str().and_then(uri_to_path)
        {
            roots.push(root);
        }
        let options = &params["initializationOptions"];
        let lang = options["lang"].as_str().unwrap_or("de").to_string();
        let width = options["width"].as_u64().map(|w| w as usize);
        let style = match options["style"].as_str() {
            Some("delimiters") => Some(knowledge::Style::Delimiters),
            Some("indentation") => Some(knowledge::Style::Indentation),
            _ => None,
        };
        let lean = match &options["lean"] {
            Value::Bool(false) => None,
            Value::String(dir) => Some(PathBuf::from(dir)),
            _ => roots.iter().map(|r| r.join("lean")).find(|d| d.join("lakefile.toml").is_file()),
        };
        let empty = Sources::from_sources(Vec::new());
        let analysis = knowledge::analyze(&empty);
        let state = State {
            open: HashMap::new(),
            sources: empty,
            analysis,
            report: None,
            lean_diags: Vec::new(),
            generation: 0,
            proving: false,
            published: HashSet::new(),
        };
        Server { roots, lang, style, width, lean, state: Arc::new(Mutex::new(state)), sender }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Reads and checks the workspace again (with the open texts) and publishes the diagnostics.
    fn check(&self) {
        let mut st = self.lock();
        let sources = match knowledge::load_with(&self.roots, &st.open) {
            Ok(s) => s,
            Err(e) => {
                self.log(&format!("verkodo: cannot read the workspace: {e}"));
                return;
            }
        };
        st.analysis = knowledge::analyze(&sources);
        st.sources = sources;
        st.generation += 1;
        st.report = None;
        st.lean_diags.clear();
        self.publish(&mut st);
    }

    /// Lets Lean check the workspace in the background, unless it is already doing so.
    fn prove(&self) {
        let Some(lean) = self.lean.clone() else { return };
        let (generation, open) = {
            let mut st = self.lock();
            if st.proving || st.analysis.diags.iter().chain(&st.sources.diags).any(|d| d.severity == Severity::Error) {
                return;
            }
            st.proving = true;
            (st.generation, st.open.clone())
        };
        let server = self.clone();
        std::thread::spawn(move || {
            let outcome = knowledge::load_with(&server.roots, &open).and_then(|sources| {
                let analysis = knowledge::analyze(&sources);
                knowledge::prove(&sources, &analysis, &lean)
            });
            let mut st = server.lock();
            st.proving = false;
            match outcome {
                Ok(o) if st.generation == generation => {
                    st.report = Some(o.report);
                    st.lean_diags = o.diags;
                    server.publish(&mut st);
                }
                Ok(_) => {}
                Err(e) => server.log(&format!("verkodo: Lean did not run: {e}")),
            }
        });
    }

    fn publish(&self, st: &mut State) {
        let mut by_file: HashMap<usize, Vec<Value>> = HashMap::new();
        for d in st.sources.diags.iter().chain(&st.analysis.diags).chain(&st.lean_diags) {
            let Some(file) = st.sources.files.get(d.file) else { continue };
            by_file.entry(d.file).or_default().push(json!({
                "range": range(&file.text, d.span.start, d.span.end),
                "severity": if d.severity == Severity::Error { 1 } else { 2 },
                "source": "verkodo",
                "message": d.message,
            }));
        }
        let mut now = HashSet::new();
        for (i, file) in st.sources.files.iter().enumerate() {
            let path = PathBuf::from(&file.path);
            let diags = by_file.remove(&i).unwrap_or_default();
            if diags.is_empty() && !st.published.contains(&path) {
                continue;
            }
            if !diags.is_empty() {
                now.insert(path.clone());
            }
            self.notify("textDocument/publishDiagnostics", json!({ "uri": path_to_uri(&path), "diagnostics": diags }));
        }
        for gone in st.published.difference(&now).filter(|p| !st.sources.files.iter().any(|f| Path::new(&f.path) == *p))
        {
            self.notify("textDocument/publishDiagnostics", json!({ "uri": path_to_uri(gone), "diagnostics": [] }));
        }
        st.published = now;
    }

    fn notify(&self, method: &str, params: Value) {
        let _ = self.sender.send(Message::Notification(Notification::new(method.to_string(), params)));
    }

    fn log(&self, message: &str) {
        self.notify("window/logMessage", json!({ "type": 2, "message": message }));
    }

    fn notification(&self, n: &Notification) {
        let p = &n.params;
        let path = p["textDocument"]["uri"].as_str().and_then(uri_to_path);
        match (n.method.as_str(), path) {
            ("textDocument/didOpen", Some(path)) => {
                let text = p["textDocument"]["text"].as_str().unwrap_or_default().to_string();
                self.lock().open.insert(path, text);
                self.check();
            }
            ("textDocument/didChange", Some(path)) => {
                // Full sync: the last change holds the whole text.
                if let Some(text) =
                    p["contentChanges"].as_array().and_then(|c| c.last()).and_then(|c| c["text"].as_str())
                {
                    self.lock().open.insert(path, text.to_string());
                    self.check();
                }
            }
            ("textDocument/didClose", Some(path)) => {
                self.lock().open.remove(&path);
                self.check();
            }
            ("textDocument/didSave", Some(_)) => {
                self.check();
                self.prove();
            }
            _ => {}
        }
    }

    fn request(&self, req: &Request) -> Response {
        let result = match req.method.as_str() {
            "textDocument/hover" => self.hover(&req.params),
            "textDocument/definition" => self.definition(&req.params),
            "textDocument/documentSymbol" => self.symbols(&req.params),
            "textDocument/formatting" => match self.format(&req.params) {
                Ok(edits) => edits,
                // RequestFailed: the editor shows why.
                Err(why) => return Response::new_err(req.id.clone(), -32803, why),
            },
            _ => return Response::new_err(req.id.clone(), -32601, format!("unknown method {}", req.method)),
        };
        Response::new_ok(req.id.clone(), result)
    }

    /// The file and byte offset a `TextDocumentPositionParams` points at, with what is there.
    fn found<T>(&self, params: &Value, then: impl FnOnce(&State, usize, Found) -> T) -> Option<T> {
        let path = params["textDocument"]["uri"].as_str().and_then(uri_to_path)?;
        let st = self.lock();
        let file = st.sources.files.iter().position(|f| Path::new(&f.path) == path)?;
        let text = &st.sources.files[file].text;
        let line = params["position"]["line"].as_u64()? as usize;
        let character = params["position"]["character"].as_u64()? as usize;
        let offset = offset(text, line, character);
        let found = knowledge::at(&st.sources, &st.analysis, st.report.as_ref(), file, offset)?;
        Some(then(&st, file, found))
    }

    /// Formatting: the way of writing of the option `style`, or else only tabs in the indentation become spaces. One
    /// edit for the whole text, or none if nothing changes; a file with errors is left as it is.
    fn format(&self, params: &Value) -> Result<Value, String> {
        let Some(path) = params["textDocument"]["uri"].as_str().and_then(uri_to_path) else { return Ok(json!([])) };
        let text = {
            let st = self.lock();
            st.open.get(&path).cloned()
        }
        .or_else(|| std::fs::read_to_string(&path).ok());
        let Some(text) = text else { return Ok(json!([])) };
        let formatted = match self.style {
            Some(style) => knowledge::format_with(&text, knowledge::FormatOptions { style, width: self.width })?,
            None => knowledge::spaces_for_tabs(&text),
        };
        if formatted == text {
            return Ok(json!([]));
        }
        Ok(
            json!([{ "range": { "start": position(&text, 0), "end": position(&text, text.len()) }, "newText": formatted }]),
        )
    }

    /// What is at a place, and – in a Lean proof Lean has checked – the goals of the tactic there.
    fn hover(&self, params: &Value) -> Value {
        let found = self.found(params, |st, file, found| {
            let text = &st.sources.files[file].text;
            let span = found.span();
            let value = match found {
                Found::Symbol { sym, .. } => {
                    knowledge::symbol_hover(&st.sources, &st.analysis, st.report.as_ref(), sym, &self.lang)
                }
                Found::Local { name, ty, .. } => format!("```lean\n{name} : {ty}\n```"),
                Found::Lean { name, .. } => knowledge::lean_hover(name, &self.lang),
                Found::External { name, .. } => {
                    let note = if self.lang == "de" {
                        "Lean/Mathlib-Name – Typ und Erklärung, sobald Lean geprüft hat (beim Speichern)"
                    } else {
                        "Lean/Mathlib name – type and explanation once Lean has checked (on saving)"
                    };
                    format!("`{name}`\n\n{note}")
                }
                Found::Builtin { name, .. } => format!("`{name}`"),
            };
            (value, range(text, span.start, span.end))
        });
        let goals = self.goals(params);
        let (value, at) = match (found, goals) {
            (Some((v, r)), Some((g, _))) => (format!("{v}\n\n---\n\n{g}"), r),
            (Some((v, r)), None) => (v, r),
            (None, Some((g, r))) => (g, r),
            (None, None) => return Value::Null,
        };
        json!({ "contents": { "kind": "markdown", "value": value }, "range": at })
    }

    /// The goals of the tactic at a place, as Markdown, with the range of the tactic.
    fn goals(&self, params: &Value) -> Option<(String, Value)> {
        let path = params["textDocument"]["uri"].as_str().and_then(uri_to_path)?;
        let st = self.lock();
        let report = st.report.as_ref()?;
        let file = st.sources.files.iter().position(|f| Path::new(&f.path) == path)?;
        let text = &st.sources.files[file].text;
        let line = params["position"]["line"].as_u64()? as usize;
        let character = params["position"]["character"].as_u64()? as usize;
        let (span, g) = knowledge::goals_at(&st.sources, report, file, offset(text, line, character))?;
        Some((knowledge::goals_hover(g, &self.lang), range(text, span.start, span.end)))
    }

    fn definition(&self, params: &Value) -> Value {
        self.found(params, |st, _, found| match found {
            Found::Symbol { sym, .. } => {
                let s = &st.analysis.symbols[sym];
                let file = &st.sources.files[s.loc.file()];
                json!({
                    "uri": path_to_uri(Path::new(&file.path)),
                    "range": range(&file.text, s.span.start, s.span.end),
                })
            }
            Found::Lean { name, .. } => match &name.decl {
                Some(d) => {
                    let at = json!({ "line": d.line.saturating_sub(1), "character": d.character });
                    json!({ "uri": path_to_uri(Path::new(&d.file)), "range": { "start": at, "end": at } })
                }
                None => Value::Null,
            },
            _ => Value::Null,
        })
        .unwrap_or(Value::Null)
    }

    /// The declarations of a file, for the outline.
    fn symbols(&self, params: &Value) -> Value {
        let Some(path) = params["textDocument"]["uri"].as_str().and_then(uri_to_path) else {
            return Value::Null;
        };
        let st = self.lock();
        let Some(file) = st.sources.files.iter().position(|f| Path::new(&f.path) == path) else {
            return json!([]);
        };
        let text = &st.sources.files[file].text;
        let uri = path_to_uri(&path);
        let list: Vec<Value> = st
            .analysis
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| s.loc.file() == file)
            .map(|(i, s)| {
                let label = knowledge::label(&st.sources, &st.analysis, i, &self.lang);
                let short = s.id.rsplit('.').next().unwrap_or_default();
                let container = st.analysis.parent(i).map(|p| st.analysis.symbols[p].id.clone());
                json!({
                    "name": if label == short { short.to_string() } else { format!("{short} – {label}") },
                    "kind": symbol_kind(s.kind),
                    "location": { "uri": uri, "range": range(text, s.span.start, s.span.end) },
                    "containerName": container,
                })
            })
            .collect();
        json!(list)
    }
}

/// The kind of a declaration in the protocol's terms (`SymbolKind`).
fn symbol_kind(kind: SymKind) -> u32 {
    match kind {
        SymKind::Concept => 5,                     // Class
        SymKind::Variant => 22,                    // EnumMember
        SymKind::Theorem | SymKind::Example => 12, // Function
        SymKind::Remark => 15,                     // String
        SymKind::Norm => 3,                        // Namespace
        SymKind::Input | SymKind::Judgement => 13, // Variable
        SymKind::Constant => 14,                   // Constant
        SymKind::Rule => 12,                       // Function
        SymKind::Path => 18,                       // Array
    }
}

/// A byte offset as a position of the protocol: line from 0, column in UTF-16 code units.
fn position(text: &str, offset: usize) -> Value {
    let offset = offset.min(text.len());
    let before = &text[..text.floor_char_boundary(offset)];
    let line = before.matches('\n').count();
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let character: usize = before[line_start..].chars().map(char::len_utf16).sum();
    json!({ "line": line, "character": character })
}

fn range(text: &str, start: usize, end: usize) -> Value {
    json!({ "start": position(text, start), "end": position(text, end.max(start)) })
}

/// A position of the protocol as a byte offset; past the end of a line means its end.
fn offset(text: &str, line: usize, character: usize) -> usize {
    let line_start = if line == 0 {
        0
    } else {
        match text.match_indices('\n').nth(line - 1) {
            Some((i, _)) => i + 1,
            None => return text.len(),
        }
    };
    let mut units = 0;
    for (i, c) in text[line_start..].char_indices() {
        if c == '\n' || units >= character {
            return line_start + i;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// The path of a `file:` URI.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(b) = rest.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

/// The `file:` URI of a path.
fn path_to_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests;
