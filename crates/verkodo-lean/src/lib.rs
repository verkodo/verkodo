// SPDX-License-Identifier: AGPL-3.0-or-later
//! The Lean bridge: every module becomes one Lean file (variants as `abbrev`s, bridges as `↔` plus an equality of
//! the definitions, theorems against their own variants, and per path the theorems carried over along bridges to
//! the variants the path chose). `lake env lean` checks the files; its messages come back as a [`Report`] and as
//! diagnostics on the verkodo source.

mod export;
mod run;

pub use export::{Generated, export};
pub use run::{ProveOutcome, prove};
