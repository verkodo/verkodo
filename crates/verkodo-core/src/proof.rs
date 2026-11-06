// SPDX-License-Identifier: AGPL-3.0-or-later
//! What Lean said about each formal part.

use std::collections::HashMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};
use verkodo_syntax::Span;

/// A part of the knowledge that Lean checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    /// The definition of a variant.
    Variant(usize),
    /// Bridge number `index` of a concept.
    Bridge { concept: usize, index: usize },
    /// A theorem or example as written (against its own variants).
    Theorem(usize),
    /// A theorem carried over along bridges to the variants a path chose.
    Transport { theorem: usize, path: usize },
    /// The inputs of a norm (a Lean structure).
    Norm(usize),
    /// A constant or rule of a norm (a Lean definition).
    Member(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    /// Lean accepted it without `sorry`.
    Checked,
    /// Lean accepted it, but a proof is missing (`sorry`).
    Open,
    /// Lean rejected it.
    Failed(Vec<String>),
}

/// A name in a Lean snippet as Lean resolved it: a constant of Lean, Mathlib or another library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeanName {
    /// Byte range in the snippet's code.
    pub range: Range<usize>,
    /// The full name, e.g. `Filter.Tendsto`.
    pub name: String,
    /// The type, as Lean prints it.
    pub ty: String,
    /// The doc string, if the constant has one.
    pub doc: Option<String>,
    /// The module that declares it, e.g. `Mathlib.Order.Filter.Defs`.
    pub module: Option<String>,
    /// Where it is declared, if Lean found the source.
    pub decl: Option<DeclLocation>,
}

/// The place of a declaration in a Lean source file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclLocation {
    /// The source file, as Lean found it.
    pub file: String,
    /// The line of the declared name, from 1.
    pub line: u32,
    /// Its column in UTF-16 code units, from 0 – as editors count.
    pub character: u32,
}

/// A tactic in a Lean snippet and what was left to show before and after it, as Lean prints goals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goals {
    /// Byte range in the snippet's code.
    pub range: Range<usize>,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub status: HashMap<Key, Status>,
    /// Bridges a transport went through, for display: (theorem, path) → variant pairs (from, to).
    pub transports: HashMap<(usize, usize), Vec<(usize, usize)>>,
    /// Names Lean resolved in each Lean snippet, keyed like `Analysis::lean_refs` (file, span of the literal);
    /// sorted by range, without the declarations of verkodo itself.
    pub names: HashMap<(usize, Span), Vec<LeanName>>,
    /// The tactics in each Lean snippet with their goals, keyed like `names`; sorted by range.
    pub goals: HashMap<(usize, Span), Vec<Goals>>,
}

impl Report {
    pub fn get(&self, key: Key) -> Option<&Status> {
        self.status.get(&key)
    }

    /// How many parts Lean checked, left open and rejected.
    pub fn counts(&self) -> (usize, usize, usize) {
        let count = |f: fn(&Status) -> bool| self.status.values().filter(|s| f(s)).count();
        (count(|s| *s == Status::Checked), count(|s| *s == Status::Open), count(|s| matches!(s, Status::Failed(_))))
    }
}

/// A [`Report`] as plain lists, to send it as JSON – from a server that ran Lean to an editor in the browser. Its
/// symbol numbers and spans belong to the same files in the same order, analysed by the same version of verkodo.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportData {
    pub status: Vec<(Key, Status)>,
    pub transports: Vec<TransportEntry>,
    pub names: Vec<NamesEntry>,
    #[serde(default)]
    pub goals: Vec<GoalsEntry>,
}

/// (file, start, end of the Lean literal) → its tactics with their goals.
pub type GoalsEntry = ((usize, usize, usize), Vec<Goals>);

/// (theorem, path) → the pairs of variants it went through.
pub type TransportEntry = ((usize, usize), Vec<(usize, usize)>);

/// (file, start, end of the Lean literal) → the names Lean resolved in it.
pub type NamesEntry = ((usize, usize, usize), Vec<LeanName>);

impl From<&Report> for ReportData {
    fn from(r: &Report) -> Self {
        let mut d = ReportData {
            status: r.status.iter().map(|(k, s)| (*k, s.clone())).collect(),
            transports: r.transports.iter().map(|(k, v)| (*k, v.clone())).collect(),
            names: r.names.iter().map(|((f, s), n)| ((*f, s.start, s.end), n.clone())).collect(),
            goals: r.goals.iter().map(|((f, s), g)| ((*f, s.start, s.end), g.clone())).collect(),
        };
        // A stable order, so that the same report gives the same JSON.
        d.status.sort_by_key(|(k, _)| format!("{k:?}"));
        d.transports.sort_by_key(|(k, _)| *k);
        d.names.sort_by_key(|(k, _)| *k);
        d.goals.sort_by_key(|(k, _)| *k);
        d
    }
}

impl From<ReportData> for Report {
    fn from(d: ReportData) -> Self {
        Report {
            status: d.status.into_iter().collect(),
            transports: d.transports.into_iter().collect(),
            names: d.names.into_iter().map(|((f, start, end), n)| ((f, Span::new(start, end)), n)).collect(),
            goals: d.goals.into_iter().map(|((f, start, end), g)| ((f, Span::new(start, end)), g)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_goes_through_json_and_back() {
        let mut r = Report::default();
        r.status.insert(Key::Theorem(3), Status::Checked);
        r.status.insert(Key::Bridge { concept: 1, index: 0 }, Status::Failed(vec!["no".into()]));
        r.status.insert(Key::Transport { theorem: 3, path: 7 }, Status::Open);
        r.transports.insert((3, 7), vec![(1, 2)]);
        let name = LeanName {
            range: 0..4,
            name: "nhds".into(),
            ty: "X → Filter X".into(),
            doc: None,
            module: None,
            decl: None,
        };
        r.names.insert((0, Span::new(10, 20)), vec![name]);
        let json = serde_json::to_string(&ReportData::from(&r)).unwrap();
        let back: Report = serde_json::from_str::<ReportData>(&json).unwrap().into();
        assert_eq!(back.status, r.status);
        assert_eq!(back.transports, r.transports);
        assert_eq!(back.names, r.names);
        assert_eq!(r.counts(), (1, 1, 1));
    }
}
