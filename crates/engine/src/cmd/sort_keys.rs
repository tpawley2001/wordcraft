//! Shared paragraph and table sort values and comparisons.

use super::table_formula::parse_number;
use crate::CmdError;
use serde_json::Value;
use wordcraft_doc::collate::{SortKey, sort_key};

/// How one sort key reads its cells: `Auto` (numbers first, numerically, then text) is the
/// one-column form's rule; `Text`, `Number` and `Date` are the Sort dialog's Type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SortType {
    Auto,
    Text,
    Number,
    Date,
}

/// One cell's value for a key.
#[derive(Clone, Debug)]
pub(super) enum CellKey {
    /// A number, or a date as its day count (`Auto`, `Number`, `Date`).
    Value(f64),
    /// Text (`Auto` without a number, `Text`).
    Text(SortKey),
    /// No number or date where the key wants one.
    Missing,
}

/// A date in a cell: `M/d/yyyy` (one or two digit month and day, four-digit year) or ISO
/// `yyyy-MM-dd`, as a day count. Impossible dates (`2/30/2026`) are not dates.
fn cell_date(t: &str) -> Option<i64> {
    let t = t.trim();
    let (y, m, d) = match crate::cmd::controls::parse_date(t) {
        Some(x) => x,
        None => {
            let mut parts = t.split('/');
            let (m, d, y) = (parts.next()?, parts.next()?, parts.next()?);
            let digits = |s: &str, n: std::ops::RangeInclusive<usize>| n.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit());
            if parts.next().is_some() || !digits(m, 1..=2) || !digits(d, 1..=2) || !digits(y, 4..=4) {
                return None;
            }
            let (y, m, d): (i64, u32, u32) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
            if !(1..=9999).contains(&y) || !(1..=12).contains(&m) || d == 0 || d > crate::cmd::controls::days_in_month(y, m) {
                return None;
            }
            (y, m, d)
        }
    };
    Some(y * 372 + i64::from(m) * 31 + i64::from(d))
}

/// The Sort dialog's default Type for a column's body cells: `number` when every non-empty one
/// holds a number, `date` when every one holds a date, else `text`.
pub fn sort_type<'a>(cells: impl IntoIterator<Item = &'a str>, lang: Option<&str>) -> &'static str {
    let cells: Vec<&str> = cells.into_iter().map(str::trim).filter(|c| !c.is_empty()).collect();
    if cells.is_empty() {
        "text"
    } else if cells.iter().all(|c| parse_number(c, lang).is_some_and(f64::is_finite)) {
        "number"
    } else if cells.iter().all(|c| cell_date(c).is_some()) {
        "date"
    } else {
        "text"
    }
}

impl SortType {
    pub(super) fn read(v: &Value) -> Result<Self, CmdError> {
        match v.get("type") {
            None => Ok(Self::Auto),
            Some(Value::String(t)) => match t.as_str() {
                "text" => Ok(Self::Text),
                "number" => Ok(Self::Number),
                "date" => Ok(Self::Date),
                _ => Err(CmdError::Params(format!("unknown type `{t}` (text, number or date)"))),
            },
            _ => Err(CmdError::Params("`type` must be text, number or date".into())),
        }
    }
}

pub(super) fn cell_key(text: &str, ty: SortType, lang: Option<&str>, number_lang: Option<&str>) -> CellKey {
    let text = text.trim();
    let number = || parse_number(text, number_lang).filter(|x| x.is_finite());
    match ty {
        SortType::Auto => number().map_or_else(|| CellKey::Text(sort_key(text, lang)), CellKey::Value),
        SortType::Text => CellKey::Text(sort_key(text, lang)),
        SortType::Number => number().map_or(CellKey::Missing, CellKey::Value),
        SortType::Date => cell_date(text).map_or(CellKey::Missing, |d| CellKey::Value(d as f64)),
    }
}

pub(super) fn compare(a: &[CellKey], b: &[CellKey], order: &[bool]) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Equal, Greater, Less};
    for ((x, y), &desc) in a.iter().zip(b).zip(order) {
        let o = match (x, y) {
            (CellKey::Value(x), CellKey::Value(y)) => x.total_cmp(y),
            (CellKey::Text(x), CellKey::Text(y)) => x.cmp(y),
            (CellKey::Value(_), _) => Less,
            (_, CellKey::Value(_)) => Greater,
            _ => Equal,
        };
        let o = if desc { o.reverse() } else { o };
        if o != Equal {
            return o;
        }
    }
    Equal
}
