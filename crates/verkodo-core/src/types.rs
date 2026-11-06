// SPDX-License-Identifier: AGPL-3.0-or-later
//! Types of the legal profile.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    Money,
    Int,
    Decimal,
    Date,
    /// A number literal: becomes `int` or `decimal` from its context.
    Num,
    /// After an error; agrees with everything so that one mistake is reported once.
    Unknown,
}

impl Ty {
    pub fn parse(s: &str) -> Option<Ty> {
        Some(match s {
            "bool" => Ty::Bool,
            "money" => Ty::Money,
            "int" => Ty::Int,
            "decimal" => Ty::Decimal,
            "date" => Ty::Date,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Ty::Bool => "bool",
            Ty::Money => "money",
            Ty::Int => "int",
            Ty::Decimal => "decimal",
            Ty::Date => "date",
            Ty::Num => "number",
            Ty::Unknown => "unknown",
        }
    }

    pub fn is_numeric(self) -> bool {
        matches!(self, Ty::Money | Ty::Int | Ty::Decimal | Ty::Num | Ty::Unknown)
    }

    /// Plain numbers without a unit.
    pub fn is_scalar(self) -> bool {
        matches!(self, Ty::Int | Ty::Decimal | Ty::Num | Ty::Unknown)
    }

    /// The common type of two operands, if there is one.
    pub fn unify(a: Ty, b: Ty) -> Option<Ty> {
        use Ty::*;
        match (a, b) {
            (Unknown, x) | (x, Unknown) => Some(x),
            (x, y) if x == y => Some(x),
            (Num, x @ (Int | Decimal)) | (x @ (Int | Decimal), Num) => Some(x),
            (Int, Decimal) | (Decimal, Int) => Some(Decimal),
            _ => None,
        }
    }
}
