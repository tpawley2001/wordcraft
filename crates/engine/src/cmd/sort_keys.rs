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

/// A date in a cell as Word's Sort reads it, as a day count (days since 1970-01-01, the time of
/// day as a fraction). See [`date_value`]; "today" and "this year" come from the local clock.
fn cell_date(t: &str) -> Option<f64> {
    date_value(t, &|| {
        let (y, _, _, _, days) = crate::cmd::local_now();
        (y, days)
    })
}

/// A date as Word's Sort by Date reads it: `M/d/yyyy` or `M/d/yy` (years 00-29 are 2000-2029,
/// 30-99 are 1930-1999; a first number over 12 is the day), ISO `yyyy-MM-dd`, `Mon d, yyyy`,
/// `Month d, yyyy`, `d Month yyyy`, `Month yyyy` (that month's first day), `Month d` (this year),
/// each optionally followed by a time, or a time alone (`10:30`, today). Month names are English,
/// full or three letters, any case. A bare year, an impossible date or words are not dates.
/// `now` gives (this year, today as days since 1970-01-01) and is only asked when needed.
fn date_value(t: &str, now: &dyn Fn() -> (i64, i64)) -> Option<f64> {
    let t = t.trim();
    if let Some(secs) = time_of_day(t) {
        return Some(now().1 as f64 + secs);
    }
    if let Some((y, m, d)) = calendar_date(t, now) {
        return Some(crate::cmd::controls::days_from_civil(y, m, d) as f64);
    }
    // A date followed by a time: the time is the last word, or the last two (`10:30 AM`).
    let words: Vec<&str> = t.split_whitespace().collect();
    for n in [1usize, 2] {
        let Some(split) = words.len().checked_sub(n).filter(|&k| k > 0) else { continue };
        let (date, time) = (words.get(..split)?.join(" "), words.get(split..)?.join(" "));
        if let (Some(secs), Some((y, m, d))) = (time_of_day(&time), calendar_date(&date, now)) {
            return Some(crate::cmd::controls::days_from_civil(y, m, d) as f64 + secs);
        }
    }
    None
}

/// `t` when it is only digits, `len` of them.
fn digits(t: &str, len: std::ops::RangeInclusive<usize>) -> Option<u32> {
    (len.contains(&t.len()) && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.parse().ok()).flatten()
}

/// A time of day `H:mm` or `H:mm:ss`, optionally with `AM`/`PM` (attached or after a space), as a
/// fraction of a day.
fn time_of_day(t: &str) -> Option<f64> {
    let lower = t.trim().to_ascii_lowercase();
    let (clock, pm) = match lower.strip_suffix("am").or_else(|| lower.strip_suffix('a')) {
        Some(c) => (c.trim_end(), Some(false)),
        None => match lower.strip_suffix("pm").or_else(|| lower.strip_suffix('p')) {
            Some(c) => (c.trim_end(), Some(true)),
            None => (lower.as_str(), None),
        },
    };
    let mut parts = clock.split(':');
    let (h, m) = (digits(parts.next()?, 1..=2)?, digits(parts.next()?, 2..=2)?);
    let s = match parts.next() {
        Some(s) => digits(s, 2..=2)?,
        None => 0,
    };
    if parts.next().is_some() || m > 59 || s > 59 {
        return None;
    }
    let h = match pm {
        None if h <= 23 => h,
        Some(pm) if (1..=12).contains(&h) => h % 12 + if pm { 12 } else { 0 },
        _ => return None,
    };
    Some(f64::from(h * 3600 + m * 60 + s) / 86_400.0)
}

/// An English month name, full or its first three letters, any case, a trailing point allowed.
fn month(t: &str) -> Option<u32> {
    const MONTHS: [&str; 12] =
        ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
    let t = t.strip_suffix('.').unwrap_or(t).to_ascii_lowercase();
    let i = MONTHS.iter().position(|m| *m == t || (t.len() == 3 && m.starts_with(t.as_str())))?;
    u32::try_from(i + 1).ok()
}

/// A checked (year, month, day).
fn ymd(y: i64, m: u32, d: u32) -> Option<(i64, u32, u32)> {
    ((1..=9999).contains(&y) && (1..=12).contains(&m) && d >= 1 && d <= crate::cmd::controls::days_in_month(y, m)).then_some((y, m, d))
}

/// The calendar date forms of [`date_value`], without a time.
fn calendar_date(t: &str, now: &dyn Fn() -> (i64, i64)) -> Option<(i64, u32, u32)> {
    if let Some(x) = crate::cmd::controls::parse_date(t) {
        return Some(x);
    }
    let mut parts = t.split('/');
    if let (Some(a), Some(b), Some(y), None) = (parts.next(), parts.next(), parts.next(), parts.next()) {
        let (a, b) = (digits(a, 1..=2)?, digits(b, 1..=2)?);
        let y = i64::from(match y.len() {
            2 => digits(y, 2..=2).map(|y| if y < 30 { 2000 + y } else { 1900 + y })?,
            _ => digits(y, 4..=4)?,
        });
        // A first number over 12 can only be the day.
        let (m, d) = if a > 12 { (b, a) } else { (a, b) };
        return ymd(y, m, d);
    }
    let words: Vec<&str> = t.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).collect();
    let day = |w: &str| digits(w, 1..=2);
    let year = |w: &str| digits(w, 4..=4).map(i64::from);
    match words.as_slice() {
        [m, d, y] => match month(m) {
            Some(m) => ymd(year(y)?, m, day(d)?),
            None => ymd(year(y)?, month(d)?, day(m)?),
        },
        [m, x] => {
            let m = month(m)?;
            match year(x) {
                Some(y) => ymd(y, m, 1),
                None => ymd(now().0, m, day(x)?),
            }
        }
        _ => None,
    }
}

/// Whether a Number key reads text in `lang` arithmetically with [`sort_number`] (English or no
/// language, as measured in Word); other languages read one number in their own notation.
fn reads_arithmetic(lang: Option<&str>) -> bool {
    lang.is_none_or(|l| l.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("en")))
}

/// One piece of text read by [`sort_number`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Currency,
    Plus,
    Minus,
    Times,
    Divide,
    Percent,
    Open,
    Close,
}

/// The deepest nesting of parentheses and signs [`sort_number`] follows; deeper text is no number.
const MAX_NUMBER_DEPTH: usize = 64;

/// A paragraph or cell as Word's Sort by Number reads it: arithmetic, as Word's Calculate does.
/// Numbers next to each other (spaces or commas between) add up; `,` groups thousands only before
/// exactly three digits; `-`, `+`, `*` and `/` between numbers calculate, left to right with `*`
/// and `/` first; `%` divides by 100; a number in parentheses is negative; a currency sign may
/// lead a number; a second point starts a new number (`1.5.3` is 1.5 + .3). Words of letters only
/// are skipped. No number at all, a word mixing letters and digits (`2e3`), a leading `+`, a
/// trailing operator, unbalanced parentheses or a sign after the currency sign make it no number.
fn sort_number(text: &str) -> Option<f64> {
    let toks = number_tokens(text)?;
    let mut p = NumberParser { toks: &toks, pos: 0, depth: 0 };
    let x = p.sum(true)?;
    // `+ 0.0` turns -0 into 0, so `(0)` ties with `0`.
    (p.pos == toks.len()).then_some(x + 0.0).filter(|x| x.is_finite())
}

fn number_tokens(text: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = text.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let digit = |i: usize| at(i).is_some_and(|c| c.is_ascii_digit());
    let mut toks = Vec::new();
    let mut i = 0usize;
    while let Some(c) = at(i) {
        if c.is_ascii_digit() || (c == '.' && digit(i + 1)) {
            let start = i;
            let mut lit = String::new();
            let mut point = false;
            while let Some(c) = at(i) {
                if c.is_ascii_digit() {
                    lit.push(c);
                } else if c == '.' && !point {
                    point = true;
                    lit.push(c);
                } else if c == ','
                    && !point
                    && i > start
                    && digit(i + 1)
                    && digit(i + 2)
                    && digit(i + 3)
                    && !digit(i + 4)
                    && !at(i + 4).is_some_and(char::is_alphanumeric)
                {
                    // A thousands separator: skip it.
                } else {
                    break;
                }
                i += 1;
            }
            // A number running into letters (`2e3`, `7th`) is a word mixing letters and digits.
            if at(i).is_some_and(char::is_alphanumeric) {
                return None;
            }
            if lit.starts_with('.') {
                lit.insert(0, '0');
            }
            toks.push(Tok::Num(lit.trim_end_matches('.').parse().ok()?));
            continue;
        }
        if c.is_alphanumeric() {
            // A word: letters only are skipped; letters with digits make the text no number.
            while let Some(c) = at(i).filter(|c| c.is_alphanumeric()) {
                if c.is_ascii_digit() {
                    return None;
                }
                i += 1;
            }
            continue;
        }
        let tok = match c {
            '$' | '€' | '£' | '¥' | '৳' => Some(Tok::Currency),
            '+' => Some(Tok::Plus),
            '-' | '\u{2212}' => Some(Tok::Minus),
            '*' => Some(Tok::Times),
            '/' => Some(Tok::Divide),
            '%' => Some(Tok::Percent),
            '(' => Some(Tok::Open),
            ')' => Some(Tok::Close),
            // Spaces, commas, stray points and other marks only separate numbers.
            _ => None,
        };
        toks.extend(tok);
        i += 1;
    }
    Some(toks)
}

struct NumberParser<'a> {
    toks: &'a [Tok],
    pos: usize,
    depth: usize,
}

impl NumberParser<'_> {
    fn peek(&self) -> Option<Tok> {
        self.toks.get(self.pos).copied()
    }

    fn eat(&mut self, t: Tok) -> bool {
        let hit = self.peek() == Some(t);
        if hit {
            self.pos += 1;
        }
        hit
    }

    /// Terms joined by `+`, `-` or, when `adjacent` allows it, by nothing (they add up).
    fn sum(&mut self, adjacent: bool) -> Option<f64> {
        let mut x = self.product()?;
        loop {
            if self.eat(Tok::Plus) {
                x += self.product()?;
            } else if self.eat(Tok::Minus) {
                x -= self.product()?;
            } else if adjacent && matches!(self.peek(), Some(Tok::Num(_) | Tok::Currency | Tok::Open)) {
                x += self.product()?;
            } else {
                return Some(x);
            }
        }
    }

    fn product(&mut self) -> Option<f64> {
        let mut x = self.unary()?;
        loop {
            if self.eat(Tok::Times) {
                x *= self.unary()?;
            } else if self.eat(Tok::Divide) {
                x /= self.unary()?;
            } else {
                return Some(x);
            }
        }
    }

    fn unary(&mut self) -> Option<f64> {
        self.depth += 1;
        if self.depth > MAX_NUMBER_DEPTH {
            return None;
        }
        let x = if self.eat(Tok::Minus) { self.unary().map(|x| -x) } else { self.percent() };
        self.depth -= 1;
        x
    }

    fn percent(&mut self) -> Option<f64> {
        let mut x = self.primary()?;
        while self.eat(Tok::Percent) {
            x /= 100.0;
        }
        Some(x)
    }

    fn primary(&mut self) -> Option<f64> {
        match self.peek()? {
            Tok::Num(x) => {
                self.pos += 1;
                Some(x)
            }
            Tok::Currency => {
                self.pos += 1;
                match self.peek()? {
                    Tok::Num(x) => {
                        self.pos += 1;
                        Some(x)
                    }
                    _ => None,
                }
            }
            Tok::Open => {
                self.pos += 1;
                let start = self.pos;
                let x = self.sum(false)?;
                let plain = self.pos - start == 1 || (self.pos - start == 2 && self.toks.get(start) == Some(&Tok::Currency));
                if !self.eat(Tok::Close) {
                    return None;
                }
                // A number in parentheses is negative; around a calculation they group.
                Some(if plain { -x } else { x })
            }
            _ => None,
        }
    }
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
        SortType::Number if reads_arithmetic(number_lang) => sort_number(text).map_or(CellKey::Missing, CellKey::Value),
        SortType::Number => number().map_or(CellKey::Missing, CellKey::Value),
        SortType::Date => cell_date(text).map_or(CellKey::Missing, CellKey::Value),
    }
}

/// A paragraph's (or field's) value for a key: as [`cell_key`], except that a Number key in a
/// language not read arithmetically also reads a number that starts the text ("10 apples").
pub(super) fn para_key(text: &str, ty: SortType, lang: Option<&str>) -> CellKey {
    match cell_key(text, ty, lang, lang) {
        CellKey::Missing if ty == SortType::Number && !reads_arithmetic(lang) => {
            text.split_whitespace().next().map_or(CellKey::Missing, |first| cell_key(first, ty, lang, lang))
        }
        k => k,
    }
}

pub(super) fn compare(a: &[CellKey], b: &[CellKey], order: &[bool]) -> std::cmp::Ordering {
    use std::cmp::Ordering::{Equal, Greater, Less};
    for ((x, y), &desc) in a.iter().zip(b).zip(order) {
        let o = match (x, y) {
            (CellKey::Value(x), CellKey::Value(y)) => x.total_cmp(y),
            (CellKey::Text(x), CellKey::Text(y)) => x.cmp(y),
            // Without a number or date sorts below every one, as in Word: first ascending.
            (CellKey::Missing, CellKey::Value(_)) => Less,
            (CellKey::Value(_), CellKey::Missing) => Greater,
            // `Auto`: numbers before text.
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

#[cfg(test)]
mod tests {
    use super::{date_value, sort_number};
    use crate::cmd::controls::days_from_civil;

    /// 14 November 2026.
    fn now() -> (i64, i64) {
        (2026, days_from_civil(2026, 11, 14))
    }

    fn day(y: i64, m: u32, d: u32) -> Option<f64> {
        Some(days_from_civil(y, m, d) as f64)
    }

    #[test]
    fn month_and_day_without_a_year_is_this_year() {
        assert_eq!(date_value("March 3", &now), day(2026, 3, 3));
        assert_eq!(date_value("dec 25", &now), day(2026, 12, 25));
        assert_eq!(date_value("Feb 30", &now), None);
    }

    #[test]
    fn a_time_alone_is_today_and_a_time_after_a_date_orders_the_day() {
        assert_eq!(date_value("10:30", &now), Some(days_from_civil(2026, 11, 14) as f64 + 10.5 / 24.0));
        assert_eq!(date_value("2:15 PM", &now), Some(days_from_civil(2026, 11, 14) as f64 + 14.25 / 24.0));
        assert_eq!(date_value("1/2/2020 10:30", &now), Some(days_from_civil(2020, 1, 2) as f64 + 10.5 / 24.0));
        assert!(date_value("1/2/2020 9:00", &now) < date_value("1/2/2020 10:30", &now));
        for t in ["25:00", "10:60", "13:00 PM", "10:", ":30", "10:3"] {
            assert_eq!(date_value(t, &now), None, "{t}");
        }
    }

    #[test]
    fn hostile_number_text_is_no_number_not_a_crash() {
        let deep = "(".repeat(10_000) + "1" + &")".repeat(10_000);
        assert_eq!(sort_number(&deep), None);
        assert_eq!(sort_number(&"-".repeat(10_000)), None);
        assert_eq!(sort_number(&"9".repeat(400)), None);
        assert_eq!(sort_number("1/0"), None);
        assert_eq!(sort_number("(0)").map(f64::to_bits), Some(0.0f64.to_bits()));
        assert_eq!(sort_number("3+4"), Some(7.0));
        assert_eq!(sort_number("(2, 3)"), None);
    }
}
