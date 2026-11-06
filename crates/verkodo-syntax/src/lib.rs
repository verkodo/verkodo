// SPDX-License-Identifier: AGPL-3.0-or-later
//! Syntax of the verkodo language: source files, diagnostics, lexer, syntax tree and parser.

pub mod ast;
pub mod format;
pub mod lexer;
pub mod parser;
pub mod source;

pub use parser::parse;
pub use source::{Diagnostic, Severity, SourceFile, Span};
