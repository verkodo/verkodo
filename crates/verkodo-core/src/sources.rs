// SPDX-License-Identifier: AGPL-3.0-or-later
//! A set of parsed source files. Reading them from disk is I/O and happens in `verkodo-sdk`.

use verkodo_syntax::ast::File;
use verkodo_syntax::{Diagnostic, Severity, SourceFile, parse};

/// The file extension of verkodo sources, without the dot.
pub const EXTENSION: &str = "vk";

/// Parsed source files with their syntax trees and the diagnostics of lexer and parser.
#[derive(Debug)]
pub struct Sources {
    pub files: Vec<SourceFile>,
    pub asts: Vec<File>,
    /// Lexer and parser diagnostics.
    pub diags: Vec<Diagnostic>,
}

impl Sources {
    /// Parses sources given as (path, text).
    pub fn from_sources(sources: Vec<(String, String)>) -> Self {
        let mut ws = Sources { files: Vec::new(), asts: Vec::new(), diags: Vec::new() };
        for (i, (path, text)) in sources.into_iter().enumerate() {
            ws.asts.push(parse(&text, i, &mut ws.diags));
            ws.files.push(SourceFile::new(path, text));
        }
        ws
    }

    /// `path:line:col: error: message`
    pub fn format(&self, d: &Diagnostic) -> String {
        let f = &self.files[d.file];
        let (line, col) = f.line_col(d.span.start);
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        format!("{}:{line}:{col}: {sev}: {}", f.path, d.message)
    }
}
