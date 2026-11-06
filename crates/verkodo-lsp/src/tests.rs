// SPDX-License-Identifier: AGPL-3.0-or-later
//! The server against a client in memory: diagnostics, hover, definition, outline.

use super::*;
use lsp_server::RequestId;

const SOURCE: &str = "module m\n\
concept c {\n  label en \"C\"\n  params lean \"(x : ℝ)\"\n  variant p { label en \"positive\" lean \"0 < x\" }\n}\n\
theorem t {\n  label en \"T\"\n  params lean \"(x : ℝ)\"\n  statement lean \"c x → Real.sqrt x = x\"\n}\n\
path w { step c step t step missing }\n";

struct Client {
    conn: Connection,
    next: i32,
}

impl Client {
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = RequestId::from(self.next);
        self.conn.sender.send(Request::new(id.clone(), method.into(), params).into()).unwrap();
        loop {
            match self.conn.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
                Message::Response(r) if r.id == id => return r.response_result.unwrap(),
                _ => {}
            }
        }
    }

    fn notify(&self, method: &str, params: Value) {
        self.conn.sender.send(Notification::new(method.into(), params).into()).unwrap();
    }

    /// The next diagnostics published for `uri`.
    fn diagnostics(&self, uri: &str) -> Vec<Value> {
        loop {
            if let Message::Notification(n) =
                self.conn.receiver.recv_timeout(std::time::Duration::from_secs(10)).unwrap()
                && n.method == "textDocument/publishDiagnostics"
                && n.params["uri"] == uri
            {
                return n.params["diagnostics"].as_array().unwrap().clone();
            }
        }
    }
}

/// The position of the `nth` occurrence of `word` in SOURCE, plus `shift` characters.
fn pos(word: &str, nth: usize, shift: usize) -> Value {
    let at = SOURCE.match_indices(word).nth(nth).unwrap().0;
    let line = SOURCE[..at].matches('\n').count();
    let col: usize = SOURCE[SOURCE[..at].rfind('\n').map_or(0, |i| i + 1)..at].chars().map(char::len_utf16).sum();
    json!({ "line": line, "character": col + shift })
}

#[test]
fn diagnostics_hover_definition_and_outline() {
    let (server, conn) = Connection::memory();
    let thread = std::thread::spawn(move || run(&server).unwrap());
    let mut client = Client { conn, next: 0 };
    // No workspace folder: only the open file counts. No Lean.
    let init = client
        .request("initialize", json!({ "capabilities": {}, "initializationOptions": { "lang": "en", "lean": false } }));
    assert_eq!(init["capabilities"]["hoverProvider"], true);
    client.notify("initialized", json!({}));

    let dir = std::env::temp_dir().join(format!("verkodo lsp {}", std::process::id()));
    let uri = path_to_uri(&dir.join("m.vk"));
    assert!(uri.contains("verkodo%20lsp"), "{uri}");
    client.notify(
        "textDocument/didOpen",
        json!({ "textDocument": { "uri": uri, "languageId": "verkodo", "version": 1, "text": SOURCE } }),
    );
    let diags = client.diagnostics(&uri);
    let missing =
        diags.iter().find(|d| d["message"].as_str().unwrap().contains("missing")).expect("an error for `missing`");
    assert_eq!(missing["severity"], 1);
    assert_eq!(missing["range"]["start"], pos("missing", 0, 0));

    let at = |word: &str, nth: usize, shift: usize| json!({ "textDocument": { "uri": uri }, "position": pos(word, nth, shift) });
    // Hover on a concept used in a formula: the card as Markdown.
    let hover = client.request("textDocument/hover", at("c x →", 0, 0));
    let text = hover["contents"]["value"].as_str().unwrap();
    assert!(text.starts_with("**Concept: C**"), "{text}");
    assert!(text.contains("`0 < x`"), "{text}");
    assert_eq!(hover["range"]["start"], pos("c x →", 0, 0));
    // Hover on a parameter, behind a non-ASCII character (UTF-16 columns).
    let hover = client.request("textDocument/hover", at("x → Real", 0, 0));
    assert_eq!(hover["contents"]["value"], "```lean\nx : ℝ\n```");
    // A Lean name before Lean has checked.
    let hover = client.request("textDocument/hover", at("Real.sqrt", 0, 2));
    assert!(hover["contents"]["value"].as_str().unwrap().contains("once Lean has checked"), "{hover}");
    // Definition of a step: the declaration of `c`.
    let def = client.request("textDocument/definition", at("step c", 0, 5));
    assert_eq!(def["uri"], uri.as_str());
    assert_eq!(def["range"]["start"], pos("c {", 0, 0));
    // Nothing at a keyword.
    assert_eq!(client.request("textDocument/hover", at("module", 0, 1)), Value::Null);
    // Outline.
    let outline = client.request("textDocument/documentSymbol", json!({ "textDocument": { "uri": uri } }));
    let names: Vec<&str> = outline.as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["c – C", "p – positive", "t – T", "w"]);

    // A change that fixes the error clears the diagnostic.
    let fixed = SOURCE.replace(" step missing", "");
    client.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": fixed }] }),
    );
    let diags = client.diagnostics(&uri);
    assert!(diags.iter().all(|d| !d["message"].as_str().unwrap().contains("missing")), "{diags:?}");

    // Formatting: nothing to do without tabs; with a tab in the indentation, spaces instead.
    let format = json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 2, "insertSpaces": true } });
    assert_eq!(client.request("textDocument/formatting", format.clone()), json!([]));
    let tabbed = fixed.replacen("\n  ", "\n\t", 1);
    client.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 3 }, "contentChanges": [{ "text": tabbed }] }),
    );
    let edits = client.request("textDocument/formatting", format);
    assert_eq!(edits[0]["newText"], fixed.as_str());

    client.request("shutdown", Value::Null);
    client.notify("exit", Value::Null);
    thread.join().unwrap();
}

#[test]
fn positions_and_uris() {
    let text = "aε\n𝔸b";
    assert_eq!(position(text, text.find('b').unwrap()), json!({ "line": 1, "character": 2 }));
    assert_eq!(offset(text, 1, 2), text.find('b').unwrap());
    assert_eq!(offset(text, 0, 99), text.find('\n').unwrap());
    let path = Path::new("/tmp/a b/ü.vk");
    assert_eq!(uri_to_path(&path_to_uri(path)).as_deref(), Some(path));
}

#[test]
fn formats_in_the_way_of_writing_asked_for() {
    let (server, conn) = Connection::memory();
    let thread = std::thread::spawn(move || run(&server).unwrap());
    let mut client = Client { conn, next: 0 };
    let options = json!({ "lang": "en", "lean": false, "style": "indentation" });
    client.request("initialize", json!({ "capabilities": {}, "initializationOptions": options }));
    client.notify("initialized", json!({}));
    let uri = path_to_uri(&std::env::temp_dir().join(format!("verkodo-fmt-{}/m.vk", std::process::id())));
    let open = |text: &str, version: i32| json!({ "textDocument": { "uri": uri, "languageId": "verkodo", "version": version, "text": text } });
    client.notify("textDocument/didOpen", open("module m\nconcept c {\n  variant p { lean \"0 < 1\" }\n}\n", 1));
    let format = json!({ "textDocument": { "uri": uri }, "options": { "tabSize": 2, "insertSpaces": true } });
    let edits = client.request("textDocument/formatting", format.clone());
    assert_eq!(edits[0]["newText"], "module m\nconcept c\n  variant p\n    lean 0 < 1\n");
    // A file with errors is left alone, and the editor learns why.
    client.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": "module m\nconcept c {" }] }),
    );
    let id = client.next;
    client.next += 1;
    client.conn.sender.send(Request::new(id.into(), "textDocument/formatting".into(), format).into()).unwrap();
    let answer = loop {
        if let Message::Response(r) = client.conn.receiver.recv().unwrap()
            && r.id == id.into()
        {
            break r;
        }
    };
    assert!(answer.response_result.unwrap_err().message.contains("has errors"));
    client.request("shutdown", Value::Null);
    client.notify("exit", Value::Null);
    thread.join().unwrap();
}
