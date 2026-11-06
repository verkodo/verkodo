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
    let (y, m, d) = civil(n);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Year, month and day of a day number – the inverse of `days_from_civil` (H. Hinnant, "chrono-Compatible Low-Level
/// Date Algorithms").
pub fn civil(n: i64) -> (i64, i64, i64) {
    let z = n + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The day number of year `y`, month `m`, day `d` – counting on past the end: month 13 is January of the next year,
/// day 0 the last day of the month before, day 32 of January the 1st of February. As `verkodo.date` in Lean.
pub fn date(y: i64, m: i64, d: i64) -> i64 {
    let (y, m) = (y + (m - 1).div_euclid(12), (m - 1).rem_euclid(12) + 1);
    days_from_civil(y, m, 1) + d - 1
}

/// The last day of month `m` of year `y`.
pub fn last_day(y: i64, m: i64) -> i64 {
    date(y, m + 1, 1) - 1
}

/// `n` months after day `day` (before, for `n < 0`): the same day of the month, or the last day of the month where
/// that month is shorter – as § 188 (2), (3) BGB counts a period of months.
pub fn add_months(day: i64, n: i64) -> i64 {
    let (y, m, d) = civil(day);
    let months = y * 12 + (m - 1) + n;
    let (y, m) = (months.div_euclid(12), months.rem_euclid(12) + 1);
    days_from_civil(y, m, d.min(days_in_month(y, m)))
}

/// The day of the week, 1 Monday … 7 Sunday (ISO 8601); 1970-01-01 was a Thursday.
pub fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7) + 1
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

/// The dates of the legal profile in Lean: `verkodo.day` and the functions on dates, the same formulas over ℤ, where
/// `/` and `%` round down for positive divisors like `div_euclid` and `rem_euclid`.
pub const LEAN_DAY: &str =
    "/-- verkodo: the day number of a date (days since 1970-01-01), as dates of norms are written in Lean. -/
def verkodo.day (y m d : ℤ) : ℤ :=
  let y := if m ≤ 2 then y - 1 else y
  let era := y / 400
  let yoe := y - era * 400
  let doy := (153 * (if m > 2 then m - 3 else m + 9) + 2) / 5 + d - 1
  era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468

/-- verkodo: year, month and day of a day number. -/
def verkodo.civil (n : ℤ) : ℤ × ℤ × ℤ :=
  let z := n + 719468
  let era := z / 146097
  let doe := z - era * 146097
  let yoe := (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365
  let doy := doe - (365 * yoe + yoe / 4 - yoe / 100)
  let mp := (5 * doy + 2) / 153
  let d := doy - (153 * mp + 2) / 5 + 1
  let m := if mp < 10 then mp + 3 else mp - 9
  (yoe + era * 400 + (if m ≤ 2 then 1 else 0), m, d)

/-- verkodo: the year of a day. -/
def verkodo.year (n : ℤ) : ℤ := (verkodo.civil n).1

/-- verkodo: the month of a day, 1–12. -/
def verkodo.month (n : ℤ) : ℤ := (verkodo.civil n).2.1

/-- verkodo: the day of the month. -/
def verkodo.dom (n : ℤ) : ℤ := (verkodo.civil n).2.2

/-- verkodo: the quarter of a day, 1–4. -/
def verkodo.quarter (n : ℤ) : ℤ := (verkodo.month n - 1) / 3 + 1

/-- verkodo: the day of the week, 1 Monday … 7 Sunday. -/
def verkodo.weekday (n : ℤ) : ℤ := (n + 3) % 7 + 1

/-- verkodo: the day number of year `y`, month `m`, day `d`, counting on past the end of a month or year. -/
def verkodo.date (y m d : ℤ) : ℤ :=
  verkodo.day (y + (m - 1) / 12) ((m - 1) % 12 + 1) 1 + d - 1

/-- verkodo: the last day of month `m` of year `y`. -/
def verkodo.last_day (y m : ℤ) : ℤ := verkodo.date y (m + 1) 1 - 1

/-- verkodo: `k` months after day `n`, at most the last day of that month (§ 188 BGB). -/
def verkodo.add_months (n k : ℤ) : ℤ :=
  let months := verkodo.year n * 12 + (verkodo.month n - 1) + k
  let y := months / 12
  let m := months % 12 + 1
  verkodo.day y m (min (verkodo.dom n) (verkodo.dom (verkodo.last_day y m)))

/-- verkodo: `k` years after day `n` – `12 k` months. -/
def verkodo.add_years (n k : ℤ) : ℤ := verkodo.add_months n (12 * k)";

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

    #[test]
    fn computes_with_dates() {
        let d = |s: &str| day_number(s).unwrap();
        assert_eq!(date(2025, 13, 1), d("2026-01-01"));
        assert_eq!(date(2025, 3, 0), d("2025-02-28"));
        assert_eq!(date(2025, 0, 1), d("2024-12-01"));
        assert_eq!(last_day(2027, 2), d("2027-02-28"));
        assert_eq!(last_day(2028, 2), d("2028-02-29"));
        assert_eq!(add_months(d("2025-01-31"), 1), d("2025-02-28"));
        assert_eq!(add_months(d("2024-02-29"), 12), d("2025-02-28"));
        assert_eq!(add_months(d("2025-03-15"), -3), d("2024-12-15"));
        assert_eq!(weekday(d("2026-11-06")), 5);
        assert_eq!(weekday(d("1970-01-01")), 4);
        assert_eq!(civil(d("2024-02-29")), (2024, 2, 29));
    }
}
