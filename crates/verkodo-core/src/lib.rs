// SPDX-License-Identifier: AGPL-3.0-or-later
//! The domain of verkodo, without I/O: the knowledge graph – every declaration gets a stable id, references are
//! resolved (also inside Lean formulas), legal rules are type-checked and paths are checked for order.

pub mod analysis;
pub mod date;
pub mod eval;
pub mod lean;
pub mod proof;
pub mod query;
pub mod sources;
pub mod types;

pub use analysis::{Analysis, Coverage, Field, Hop, LeanRef, Loc, SymKind, Symbol, Target, analyze};
pub use sources::{EXTENSION, Sources};
pub use types::Ty;

use verkodo_syntax::ast::Meta;

/// The label in `lang`, else in English, else the first one.
pub fn label<'a>(meta: &'a Meta, lang: &str) -> Option<&'a str> {
    let find = |l: &str| meta.labels.iter().find(|t| t.lang.text == l);
    find(lang).or_else(|| find("en")).or_else(|| meta.labels.first()).map(|t| t.value.as_str())
}
