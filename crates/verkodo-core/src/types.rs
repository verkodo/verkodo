// SPDX-License-Identifier: AGPL-3.0-or-later
//! Types of the legal profile.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Bool,
    Money,
    Int,
    Decimal,
    Date,
    /// A value of an enumeration: the symbol of its `enum`.
    Enum(usize),
    /// An amount of a unit (`30 km`): the symbol of its `unit`.
    Quantity(usize),
    /// A record: the symbol of its `record`.
    Record(usize),
    /// A list of records: the symbol of the `record`.
    List(usize),
    /// A number literal: becomes `int` or `decimal` from its context.
    Num,
    /// After an error; agrees with everything so that one mistake is reported once.
    Unknown,
}

/// The currencies amounts of money may be written in.
pub const CURRENCIES: &[&str] = &["EUR"];

impl Ty {
    /// A built-in type by its name.
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

    /// The name of a built-in type, or what kind of type a declared one is (`Analysis::ty_name` names it).
    pub fn name(self) -> &'static str {
        match self {
            Ty::Bool => "bool",
            Ty::Money => "money",
            Ty::Int => "int",
            Ty::Decimal => "decimal",
            Ty::Date => "date",
            Ty::Enum(_) => "enumeration",
            Ty::Quantity(_) => "amount",
            Ty::Record(_) => "record",
            Ty::List(_) => "list",
            Ty::Num => "number",
            Ty::Unknown => "unknown",
        }
    }

    /// Numbers and amounts: what adds up, orders, rounds.
    pub fn is_numeric(self) -> bool {
        matches!(self, Ty::Money | Ty::Quantity(_) | Ty::Int | Ty::Decimal | Ty::Num | Ty::Unknown)
    }

    /// Amounts: of money or of a unit.
    pub fn is_amount(self) -> bool {
        matches!(self, Ty::Money | Ty::Quantity(_))
    }

    /// Plain numbers without a unit.
    pub fn is_scalar(self) -> bool {
        matches!(self, Ty::Int | Ty::Decimal | Ty::Num | Ty::Unknown)
    }

    /// Whole numbers: `int`, or a literal that may become one.
    pub fn is_integral(self) -> bool {
        matches!(self, Ty::Int | Ty::Num | Ty::Unknown)
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
