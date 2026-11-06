// SPDX-License-Identifier: AGPL-3.0-or-later
//! Source files, byte spans and diagnostics.

/// A byte range in a source file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// The smallest span covering both.
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Index of the file in the workspace.
    pub file: usize,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn error(file: usize, span: Span, message: impl Into<String>) -> Self {
        Self { severity: Severity::Error, file, span, message: message.into() }
    }

    pub fn warning(file: usize, span: Span, message: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, file, span, message: message.into() }
    }
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: String,
    pub text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { path: path.into(), text, line_starts }
    }

    /// One-based line and column (in characters) of a byte offset.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        };
        let start = self.line_starts[line];
        let col = self.text.get(start..offset).map_or(0, |s| s.chars().count()) + 1;
        (line + 1, col)
    }
}
