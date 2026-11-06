// SPDX-License-Identifier: AGPL-3.0-or-later
//! Dates of the legal profile: `YYYY-MM-DD` in the proleptic Gregorian calendar, as day numbers (days since
//! 1970-01-01) so that they order and count like integers – in verkodo and in Lean alike.

/// The day number of a date written `YYYY-MM-DD`; `None` if it is not a date (e.g. `2025-02-30`).
pub fn day_number(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// The date of a day number, as `YYYY-MM-DD`.
pub fn from_day_number(n: i64) -> String {
    // The inverse of `days_from_civil` (H. Hinnant, "chrono-Compatible Low-Level Date Algorithms").
    let z = n + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 (H. Hinnant's algorithm). The same formula is generated into Lean as `verkodo.day`, so
/// both count alike.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `verkodo.day` in Lean: the same formula over ℤ, where `/` rounds down for positive divisors like `div_euclid`.
pub const LEAN_DAY: &str =
    "/-- verkodo: the day number of a date (days since 1970-01-01), as dates of norms are written in Lean. -/
def verkodo.day (y m d : ℤ) : ℤ :=
  let y := if m ≤ 2 then y - 1 else y
  let era := y / 400
  let yoe := y - era * 400
  let doy := (153 * (if m > 2 then m - 3 else m + 9) + 2) / 5 + d - 1
  era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_days() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2025-01-01"), Some(20_089));
        assert_eq!(day_number("2024-12-31"), Some(20_088));
        assert_eq!(day_number("2000-03-01").unwrap() - day_number("2000-02-28").unwrap(), 2);
        assert_eq!(day_number("1900-03-01").unwrap() - day_number("1900-02-28").unwrap(), 1);
        assert_eq!(day_number("2025-02-29"), None);
        assert_eq!(day_number("2025-13-01"), None);
        for n in [-800_000, -1, 0, 20_088, 20_089, 900_000] {
            assert_eq!(day_number(&from_day_number(n)), Some(n));
        }
    }
}
