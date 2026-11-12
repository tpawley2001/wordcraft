//! Table formulas (Table Layout › Data › Formula): `=SUM(ABOVE)`, cell references and ranges
//! (`=SUM(B2:B4)`, `=A1*2`), arithmetic, and the functions Word offers in tables. A formula field
//! is re-evaluated by Update Field (F9), so its result follows the cells it reads.
//!
//! Cell references count rows from 1 at the top and columns from A at the left, by grid column.
//! ABOVE, LEFT, RIGHT and BELOW sum the first run of numbers in that direction, as Word does: the
//! cells next to the formula's cell are skipped while they hold no number (empty or text), then
//! the numbers are summed up to the next cell that holds none. A function needs its arguments:
//! `=SUM()` is a syntax error, as it is in Word.

use std::collections::HashMap;

use wordcraft_doc::table::{MAX_COLS, MAX_ROWS};
use wordcraft_doc::{Cell, Table};

use crate::CmdError;

/// The longest formula accepted, in bytes.
pub const MAX_FORMULA: usize = 512;
/// The longest numeric picture (`\#` switch) used, in characters.
const MAX_PICTURE: usize = 64;
/// The deepest nesting of parentheses, function calls, unary signs and `^` chains.
const MAX_DEPTH: usize = 32;
/// The most cells one evaluation reads; past it the formula is too complex to work out, so a
/// hostile document full of huge ranges can't make Update Field hang.
pub const MAX_CELL_VISITS: usize = 100_000;

/// Evaluate `formula` (with or without its leading `=`) for the cell in row `r`, cell index `c`.
pub fn evaluate(t: &Table, r: usize, c: usize, formula: &str) -> Result<f64, CmdError> {
    evaluate_with(t, None, None, r, c, formula)
}

pub fn evaluate_with(
    t: &Table,
    doc: Option<&wordcraft_doc::Document>,
    default_lang: Option<&str>,
    r: usize,
    c: usize,
    formula: &str,
) -> Result<f64, CmdError> {
    evaluate_in(Some(t), doc, default_lang, r, c, formula)
}

/// Evaluate `formula` for the cell in row `r`, cell index `c` of `t`, or with no table (`t` is
/// `None`) for a formula field in an ordinary paragraph, where cell references, ranges and
/// ABOVE/LEFT/RIGHT/BELOW are errors and every bare name is a bookmark.
fn evaluate_in(
    t: Option<&Table>,
    doc: Option<&wordcraft_doc::Document>,
    default_lang: Option<&str>,
    r: usize,
    c: usize,
    formula: &str,
) -> Result<f64, CmdError> {
    evaluate_look(t, doc, default_lang, r, c, formula).map(|(x, _)| x)
}

/// As [`evaluate_in`], with the look of the operands the result came from (see [`Look`]).
fn evaluate_look(
    t: Option<&Table>,
    doc: Option<&wordcraft_doc::Document>,
    default_lang: Option<&str>,
    r: usize,
    c: usize,
    formula: &str,
) -> Result<(f64, Option<Look>), CmdError> {
    if formula.len() > MAX_FORMULA {
        return Err(syntax(format!("the formula is longer than {MAX_FORMULA} characters")));
    }
    let body = formula.trim();
    let body = body.strip_prefix('=').unwrap_or(body);
    let mut parser = Parser { toks: tokenize(body)?, pos: 0, depth: 0, in_table: t.is_some() };
    let expr = parser.compare()?;
    if parser.pos != parser.toks.len() {
        return Err(syntax("unexpected text after the formula".into()));
    }
    let g = t.map_or(0, |t| t.grid_col(r, c));
    let here = Here::new(t, doc, default_lang, r, c, g);
    match eval(&expr, &here)? {
        Val::Num(x, look) => Ok((finite(x)?, look)),
        Val::List(..) => Err(syntax("a range or ABOVE/LEFT/RIGHT/BELOW needs a function, such as SUM".into())),
    }
}

/// The text a formula result shows: a whole number as is, otherwise rounded to two decimals with
/// trailing zeros dropped (`= 10 / 4` is `2.5`, `= 1 / 3` is `0.33`), as Word shows it.
pub fn format_result(v: f64) -> String {
    format_plain(v, None)
}

/// A result with no `\#` picture and no operand look, in `lang`: two decimals at most, trailing
/// zeros and a bare decimal point dropped, for English or no language. Other languages keep their
/// two decimals (`3,50`), as before (not measured).
fn format_plain(v: f64, lang: Option<&str>) -> String {
    let text = wordcraft_doc::locale_number::format_number(v, lang);
    if !scans_text(lang) || !text.contains('.') {
        return text;
    }
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" || trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
}

/// The text a formula field shows: its result under its `\#` numeric picture when it has one (see
/// [`format_picture`]), otherwise as [`format_result`] shows it, and then under its `\*` switches:
/// a number format such as `\* CardText`, then a capitalization switch such as `\* Upper`.
pub fn evaluate_field(t: &Table, r: usize, c: usize, instr: &str) -> Result<String, CmdError> {
    evaluate_field_with(t, None, None, r, c, instr)
}

pub fn evaluate_field_with(
    t: &Table,
    doc: Option<&wordcraft_doc::Document>,
    default_lang: Option<&str>,
    r: usize,
    c: usize,
    instr: &str,
) -> Result<String, CmdError> {
    let (formula, switch) = split_switch(instr);
    let (value, look) = evaluate_look(Some(t), doc, default_lang, r, c, formula)?;
    let text = match (switch.and_then(picture_of), look.filter(|l| *l != Look::default())) {
        (Some(picture), _) => format_picture(value, &picture),
        (None, Some(look)) => format_look(value, look),
        (None, None) => {
            let out_lang = doc
                .and_then(|d| {
                    t.rows.get(r).and_then(|row| row.cells.get(c)).and_then(|cl| {
                        cl.blocks.iter().filter_map(|b| b.as_para()).find(|p| !p.plain_text().trim().is_empty()).and_then(|p| super::para_lang(d, p))
                    })
                })
                .or_else(|| {
                    doc.and_then(|d| {
                        t.rows.iter().flat_map(|row| row.cells.iter()).take(100).find_map(|cl| {
                            cl.blocks
                                .iter()
                                .filter_map(|b| b.as_para())
                                .find(|p| !p.plain_text().trim().is_empty())
                                .and_then(|p| super::para_lang(d, p))
                        })
                    })
                })
                .or_else(|| default_lang.map(str::to_string));
            format_plain(value, out_lang.as_deref())
        }
    };
    Ok(match switch {
        Some(switch) => format_switches(switch, value, text),
        None => text,
    })
}

/// The text a formula field outside a table shows: as [`evaluate_field_with`], with no cells to
/// read. A bookmark name is an operand that reads the number the bookmark marks.
pub fn evaluate_text_field(doc: &wordcraft_doc::Document, default_lang: Option<&str>, instr: &str) -> Result<String, CmdError> {
    let (formula, switch) = split_switch(instr);
    let (value, look) = evaluate_look(None, Some(doc), default_lang, 0, 0, formula)?;
    let text = match (switch.and_then(picture_of), look.filter(|l| *l != Look::default())) {
        (Some(picture), _) => format_picture(value, &picture),
        (None, Some(look)) => format_look(value, look),
        (None, None) => format_plain(value, default_lang),
    };
    Ok(match switch {
        Some(switch) => format_switches(switch, value, text),
        None => text,
    })
}

/// The text of a formula result under the field's `\*` switches, where `text` is `value` formatted
/// with the `\#` picture, or in the cell's language when there is no picture. A number format such
/// as `\* CardText`, `\* OrdText` or `\* roman` writes a whole number the format covers (up to
/// 999,999 for CardText and OrdText, 1 to 50,000 for roman, 1 to 780 for alphabetic), in lowercase
/// for CardText and OrdText. `\* Hex` writes a whole number from 0 to 4,294,967,295 in uppercase
/// hexadecimal digits. `\* DollarText` writes a result from 0 to 999,999.99 in lowercase cardinal
/// text with its first two decimals (rounded) over 100 (see
/// [`wordcraft_layout::fields::dollar_text`]). Any other result keeps its digits. The capitalization
/// switches (`\* Upper`, `\* Lower`, `\* Caps`, `\* FirstCap`) then apply to the text in order. Other
/// `\*` switches leave the text as it is. Quoted text, such as a `\#` picture, is not a switch.
fn format_switches(switches: &str, value: f64, text: String) -> String {
    use wordcraft_doc::section::NumFormat;
    let unquoted = switches.split('"').step_by(2).collect::<Vec<_>>().join(" ");
    let fmt = wordcraft_layout::fields::format_switch(&unquoted).filter(|f| *f != NumFormat::Decimal);
    match fmt {
        Some(NumFormat::DollarText) => {
            let text = wordcraft_layout::fields::dollar_text(value).unwrap_or(text);
            wordcraft_layout::fields::apply_case_switch(switches, text)
        }
        Some(fmt) if number_format_covers(fmt, value) => wordcraft_layout::fields::field_number(switches, fmt, value as u32),
        _ => wordcraft_layout::fields::apply_case_switch(switches, text),
    }
}

/// Whether the `\*` number format `fmt` writes `value`: a whole number up to 999,999 for CardText
/// and OrdText, 1 to 50,000 for roman, 1 to 780 for alphabetic, and 0 to `u32::MAX` otherwise.
pub(crate) fn number_format_covers(fmt: wordcraft_doc::section::NumFormat, value: f64) -> bool {
    use wordcraft_doc::section::NumFormat;
    let range = match fmt {
        NumFormat::CardinalText | NumFormat::OrdinalText => 0.0..=999_999.0,
        NumFormat::LowerRoman | NumFormat::UpperRoman => 1.0..=50_000.0,
        NumFormat::LowerLetter | NumFormat::UpperLetter => 1.0..=780.0,
        _ => 0.0..=f64::from(u32::MAX),
    };
    value.fract() == 0.0 && range.contains(&value)
}

/// Splits a field's switches (`\#` and `\*`) off its formula: `=SUM(ABOVE) \# "0.00"` gives the
/// formula and the switch text (from the first switch on).
pub fn split_switch(instr: &str) -> (&str, Option<&str>) {
    match [instr.find("\\#"), instr.find("\\*")].into_iter().flatten().min() {
        Some(at) => (instr[..at].trim_end(), Some(&instr[at..])),
        None => (instr, None),
    }
}

/// The picture of the `\#` switch in a field's switch text: a quoted one (`"#,##0.00"`) or a
/// single word.
pub(crate) fn picture_of(switch: &str) -> Option<String> {
    let rest = switch[switch.find("\\#")? + 2..].trim_start();
    let picture = match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next().unwrap_or_default(),
        None => rest.split_whitespace().next().unwrap_or_default(),
    };
    let picture: String = picture.chars().take(MAX_PICTURE).collect();
    (!picture.is_empty()).then_some(picture)
}

/// The text of `value` under a numeric picture, as Word's `\#` pictures read: `0` is a digit that
/// always shows, `#` one that shows when it has a value, `,` groups thousands, `.` marks the
/// decimals, `%` multiplies by 100, and anything else shows as it is. Sections split by `;` give the
/// positive, negative and zero forms; with one section a negative value takes a `-` in front.
pub fn format_picture(value: f64, picture: &str) -> String {
    let sections: Vec<&str> = picture.split(';').collect();
    let (section, x, minus) = if value < 0.0 && sections.len() >= 2 {
        (sections[1], -value, false)
    } else if value == 0.0 && sections.len() >= 3 {
        (sections[2], 0.0, false)
    } else {
        (sections[0], value.abs(), value < 0.0)
    };
    let Some(start) = section.find(['#', '0']) else { return section.to_string() };
    let rest = &section[start..];
    let len = rest.find(|c: char| !matches!(c, '#' | '0' | ',' | '.')).unwrap_or(rest.len());
    let (core, suffix) = rest.split_at(len);
    let prefix = &section[..start];
    let x = if prefix.contains('%') || suffix.contains('%') { x * 100.0 } else { x };
    let (int_pic, frac_pic) = match core.split_once('.') {
        Some((int_pic, frac_pic)) => (int_pic, Some(frac_pic)),
        None => (core, None),
    };
    let min_int = int_pic.matches('0').count().min(MAX_PICTURE);
    let max_frac = frac_pic.map_or(0, |f| f.matches(['0', '#']).count()).min(15);
    let min_frac = frac_pic.map_or(0, |f| f.matches('0').count()).min(max_frac);
    let scale = 10f64.powi(max_frac as i32);
    let rounded = (x * scale).round() / scale;
    if !rounded.is_finite() {
        return format_result(value);
    }
    let digits = format!("{:.*}", max_frac, rounded);
    let (whole, decimals) = match digits.split_once('.') {
        Some((whole, decimals)) => (whole.to_string(), decimals.to_string()),
        None => (digits.clone(), String::new()),
    };
    let whole = if whole == "0" && min_int == 0 { String::new() } else { format!("{whole:0>min_int$}") };
    let whole = if int_pic.contains(',') { group_thousands(&whole) } else { whole };
    let mut decimals = decimals;
    while decimals.len() > min_frac && decimals.ends_with('0') {
        decimals.pop();
    }
    let mut body = whole;
    if !decimals.is_empty() {
        body.push('.');
        body.push_str(&decimals);
    }
    let sign = if minus && rounded != 0.0 { "-" } else { "" };
    format!("{sign}{prefix}{body}{suffix}")
}

/// `1234567` → `1,234,567`.
fn group_thousands(digits: &str) -> String {
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A cell's number as a formula or a sort key reads it: surrounding spaces, currency signs,
/// thousands separators and `%` are ignored, and accounting negatives in parentheses count as
/// negative. Anything else (letters included) makes the cell text, not a number.
pub fn parse_number(text: &str, lang: Option<&str>) -> Option<f64> {
    let t = text.trim();
    let (t, negative) = match t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(inner) => (inner.trim(), true),
        None => (t, false),
    };
    if let Some(x) = wordcraft_doc::locale_number::parse_number(t, lang) {
        let x = if negative { -x.abs() } else { x };
        return x.is_finite().then_some(x);
    }
    let cleaned: String = t.chars().filter(|c| !matches!(c, ',' | '$' | '€' | '£' | '¥' | '৳' | '%') && !c.is_whitespace()).collect();
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')) {
        return None;
    }
    let x = cleaned.parse::<f64>().ok()?;
    let x = if negative { -x } else { x };
    x.is_finite().then_some(x)
}

/// The number in a cell, if its text is one.
pub(crate) fn cell_number_in(c: &Cell, lang: Option<&str>) -> Option<f64> {
    parse_number(&super::table::cell_text(c), lang)
}

/// How an operand's numbers were written, carried into a formula's result when the field has no
/// `\#` picture (see [`format_look`]). The default look is a plain number.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Look {
    /// A `$` before a number.
    currency: bool,
    /// A `,` grouping thousands.
    thousands: bool,
    /// A number in parentheses (a negative).
    parens: bool,
    /// The most decimals written in one number (a `%` adds two).
    decimals: usize,
}

/// Whether operand text in `lang` is read number by number with [`scan_operand`]: English, or no
/// language. Other languages read the operand as one number, as before (not measured).
fn scans_text(lang: Option<&str>) -> bool {
    lang.is_none_or(|l| l.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("en")))
}

/// The value of an operand's text (a cell or a bookmark) as a formula reads it, and the look of its
/// numbers. Every number in the text counts and the rest is skipped: a `-` before a number (spaces
/// between are fine) subtracts it, a number in parentheses is negative, `%` after it divides it by
/// 100, `,` groups thousands only before exactly three digits, a second `.` starts a new number, and
/// a space ends one. A number with an exponent (`1e3`) counts as 0. Text without numbers is 0.
fn scan_operand(text: &str) -> (f64, Look) {
    let chars: Vec<char> = text.chars().collect();
    let digit_at = |i: usize| chars.get(i).is_some_and(char::is_ascii_digit);
    let mut look = Look::default();
    let mut sum = 0.0;
    let mut i = 0usize;
    let (mut minus, mut open, mut dollar) = (false, false, false);
    while let Some(&ch) = chars.get(i) {
        if !(ch.is_ascii_digit() || (ch == '.' && digit_at(i + 1))) {
            match ch {
                '-' | '\u{2212}' => minus = true,
                '(' => open = true,
                '$' => dollar = true,
                c if c.is_whitespace() => {}
                _ => (minus, open, dollar) = (false, false, false),
            }
            i += 1;
            continue;
        }
        let mut digits = String::from("0");
        while let Some(&d) = chars.get(i) {
            if d.is_ascii_digit() {
                digits.push(d);
            } else if d == ',' && digits.len() > 1 && (1..=3).all(|k| digit_at(i + k)) && !digit_at(i + 4) {
                look.thousands = true;
            } else {
                break;
            }
            i += 1;
        }
        let mut decimals = 0usize;
        if chars.get(i) == Some(&'.') && digit_at(i + 1) {
            digits.push('.');
            i += 1;
            while let Some(&d) = chars.get(i).filter(|d| d.is_ascii_digit()) {
                digits.push(d);
                decimals = decimals.saturating_add(1);
                i += 1;
            }
        }
        let mut x = digits.parse::<f64>().unwrap_or(0.0);
        let signed = matches!(chars.get(i + 1), Some('+' | '-')) && digit_at(i + 2);
        if matches!(chars.get(i), Some('e' | 'E')) && (digit_at(i + 1) || signed) {
            x = 0.0;
            i += if signed { 2 } else { 1 };
            while digit_at(i) {
                i += 1;
            }
        }
        if chars.get(i) == Some(&'%') {
            x /= 100.0;
            decimals = decimals.saturating_add(2);
            i += 1;
        }
        if open {
            let mut j = i;
            while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                j += 1;
            }
            if chars.get(j) == Some(&')') {
                x = -x;
                look.parens = true;
                i = j + 1;
            }
        }
        if minus {
            x = -x;
        }
        look.currency |= dollar;
        look.decimals = look.decimals.max(decimals);
        sum += x;
        (minus, open, dollar) = (false, false, false);
    }
    (sum, look)
}

/// The text of a formula result with the look of the operands it came from: a `$` operand gives
/// `$` and at least two decimals (more when the operand wrote more), a thousands separator groups
/// the result's whole part, parentheses show a negative result as `(4)` (and so does a currency
/// result: `$5 - 10` is `($5.00)`), and otherwise the result keeps at least as many decimals as the
/// operand wrote, trailing zeros dropped. A plain look shows as [`format_result`] does.
fn format_look(value: f64, look: Look) -> String {
    if look == Look::default() {
        return format_result(value);
    }
    let places = look.decimals.clamp(2, 15);
    let digits = format!("{:.*}", places, value.abs());
    let (whole, frac) = digits.split_once('.').unwrap_or((digits.as_str(), ""));
    let frac = if look.currency { frac } else { frac.trim_end_matches('0') };
    let mut body = if look.thousands { group_thousands(whole) } else { whole.to_string() };
    if !frac.is_empty() {
        body.push('.');
        body.push_str(frac);
    }
    if look.currency {
        body.insert(0, '$');
    }
    let zero = digits.chars().all(|c| matches!(c, '0' | '.'));
    match (value < 0.0 && !zero, look.parens || look.currency) {
        (true, true) => format!("({body})"),
        (true, false) => format!("-{body}"),
        (false, _) => body,
    }
}

/// The look of a result worked out from two operands (or a function's arguments): the union of
/// their looks, as Word shows it. A `$` on either makes it a currency, a thousands separator on
/// either groups it, parentheses on either show a negative in parentheses, and it keeps the most
/// decimals either wrote (`$5 + 3` is `$8.00`, `1,234.50 + 3` is `1,237.5`). A literal number has
/// no look, so the other operand's look is kept.
fn merge_looks(a: Option<Look>, b: Option<Look>) -> Option<Look> {
    match (a, b) {
        (Some(x), Some(y)) => Some(Look {
            currency: x.currency || y.currency,
            thousands: x.thousands || y.thousands,
            parens: x.parens || y.parens,
            decimals: x.decimals.max(y.decimals),
        }),
        (x, None) => x,
        (None, y) => y,
    }
}

fn syntax(msg: String) -> CmdError {
    CmdError::Failed(format!("syntax error: {msg}"))
}

fn finite(x: f64) -> Result<f64, CmdError> {
    if x.is_finite() { Ok(x) } else { Err(CmdError::Failed("the result is not a number".into())) }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    /// A name: a function, a keyword, a cell reference (letters then digits) or a bookmark,
    /// upper-cased.
    Name(String),
    Op(char),
    /// A comparison: `=`, `<>`, `<`, `<=`, `>` or `>=`.
    Cmp(&'static str),
}

fn parse_num_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<Tok, CmdError> {
    let mut lit = String::new();
    while let Some(&d) = chars.peek() {
        if d.is_ascii_digit() || d == '.' {
            lit.push(d);
            chars.next();
        } else {
            break;
        }
    }
    let x = lit.parse::<f64>().map_err(|_| syntax(format!("`{lit}` isn't a number")))?;
    Ok(Tok::Num(x))
}

fn parse_name_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Tok {
    let mut name = String::new();
    while let Some(&d) = chars.peek() {
        // Bookmark names (`Net_Total`) may hold `_` after their first letter.
        if d.is_ascii_alphanumeric() || d == '_' {
            name.push(d.to_ascii_uppercase());
            chars.next();
        } else {
            break;
        }
    }
    Tok::Name(name)
}

fn parse_cmp_token(ch: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Tok {
    chars.next();
    let next = chars.peek().copied();
    let (op, longer) = match (ch, next) {
        ('<', Some('=')) => ("<=", true),
        ('<', Some('>')) => ("<>", true),
        ('>', Some('=')) => (">=", true),
        ('<', _) => ("<", false),
        ('>', _) => (">", false),
        _ => ("=", false),
    };
    if longer {
        chars.next();
    }
    Tok::Cmp(op)
}

fn tokenize(s: &str) -> Result<Vec<Tok>, CmdError> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else if ch.is_ascii_digit() || ch == '.' {
            out.push(parse_num_token(&mut chars)?);
        } else if ch.is_ascii_alphabetic() {
            out.push(parse_name_token(&mut chars));
        } else if matches!(ch, '+' | '-' | '*' | '/' | '^' | '(' | ')' | ',' | ':') {
            out.push(Tok::Op(ch));
            chars.next();
        } else if matches!(ch, '<'..='>') {
            out.push(parse_cmp_token(ch, &mut chars));
        } else {
            return Err(syntax(format!("`{ch}` isn't part of a formula")));
        }
    }
    Ok(out)
}

/// The keywords that name the cells next to the formula's cell.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Above,
    Left,
    Right,
    Below,
}

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    /// A cell: (row, grid column).
    Cell(usize, usize, String),
    /// A rectangle of cells: (row0, grid col0, row1, grid col1), inclusive.
    Range(usize, usize, usize, usize),
    Side(Side),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    /// A comparison: 1 when it holds, 0 when it doesn't.
    Cmp(&'static str, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    /// A bookmark: the number the bookmark of this name (upper-cased, matched ignoring case) marks.
    Bookmark(String),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
    /// Whether the formula sits in a table cell: outside one, every bare name is a bookmark.
    in_table: bool,
}

impl Parser {
    fn peek_op(&self, c: char) -> bool {
        matches!(self.toks.get(self.pos), Some(Tok::Op(x)) if *x == c)
    }

    fn eat_op(&mut self, c: char) -> bool {
        let hit = self.peek_op(c);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn enter(&mut self) -> Result<(), CmdError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { Err(syntax("the formula is nested too deeply".into())) } else { Ok(()) }
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// The loosest level: one comparison of two sums (comparisons don't chain).
    fn compare(&mut self) -> Result<Expr, CmdError> {
        let lhs = self.expr()?;
        let Some(Tok::Cmp(op)) = self.toks.get(self.pos).cloned() else { return Ok(lhs) };
        self.pos += 1;
        let rhs = self.expr()?;
        Ok(Expr::Cmp(op, Box::new(lhs), Box::new(rhs)))
    }

    fn expr(&mut self) -> Result<Expr, CmdError> {
        let mut lhs = self.term()?;
        loop {
            let op = if self.eat_op('+') {
                '+'
            } else if self.eat_op('-') {
                '-'
            } else {
                return Ok(lhs);
            };
            let rhs = self.term()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn term(&mut self) -> Result<Expr, CmdError> {
        let mut lhs = self.power()?;
        loop {
            let op = if self.eat_op('*') {
                '*'
            } else if self.eat_op('/') {
                '/'
            } else {
                return Ok(lhs);
            };
            let rhs = self.power()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn power(&mut self) -> Result<Expr, CmdError> {
        let base = self.unary()?;
        if self.eat_op('^') {
            // `^` is right-associative, so each one in a chain nests one level deeper.
            self.enter()?;
            let exp = self.power()?;
            self.leave();
            return Ok(Expr::Bin('^', Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<Expr, CmdError> {
        if self.eat_op('-') {
            self.enter()?;
            let inner = self.unary()?;
            self.leave();
            return Ok(Expr::Neg(Box::new(inner)));
        }
        if self.eat_op('+') {
            self.enter()?;
            let inner = self.unary()?;
            self.leave();
            return Ok(inner);
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, CmdError> {
        let Some(tok) = self.toks.get(self.pos).cloned() else {
            return Err(syntax("the formula ends too soon".into()));
        };
        self.pos += 1;
        match tok {
            Tok::Num(x) => Ok(Expr::Num(x)),
            Tok::Op('(') => {
                self.enter()?;
                let inner = self.compare()?;
                if !self.eat_op(')') {
                    return Err(syntax("a `(` is missing its `)`".into()));
                }
                self.leave();
                Ok(inner)
            }
            Tok::Name(name) => {
                if self.eat_op('(') {
                    self.enter()?;
                    let mut args = Vec::new();
                    if !self.eat_op(')') {
                        loop {
                            args.push(self.compare()?);
                            if self.eat_op(')') {
                                break;
                            }
                            if !self.eat_op(',') {
                                return Err(syntax(format!("`{name}` needs `,` or `)` between its arguments")));
                            }
                        }
                    }
                    self.leave();
                    return Ok(Expr::Call(name, args));
                }
                let side = match name.as_str() {
                    "ABOVE" => Some(Side::Above),
                    "LEFT" => Some(Side::Left),
                    "RIGHT" => Some(Side::Right),
                    "BELOW" => Some(Side::Below),
                    _ => None,
                };
                if let Some(side) = side {
                    return Ok(Expr::Side(side));
                }
                // TRUE and FALSE need no parentheses.
                if matches!(name.as_str(), "TRUE" | "FALSE") {
                    return Ok(Expr::Call(name, Vec::new()));
                }
                // Any other name is a bookmark, unless it is shaped like a cell reference in a table.
                let Some((row, col)) = cell_ref(&name).filter(|_| self.in_table) else {
                    return Ok(Expr::Bookmark(name));
                };
                if self.eat_op(':') {
                    let Some(Tok::Name(other)) = self.toks.get(self.pos).cloned() else {
                        return Err(syntax("a range ends with a cell reference, such as B4".into()));
                    };
                    self.pos += 1;
                    let (row2, col2) = cell_ref(&other).ok_or_else(|| syntax(format!("`{other}` isn't a cell reference")))?;
                    return Ok(Expr::Range(row.min(row2), col.min(col2), row.max(row2), col.max(col2)));
                }
                Ok(Expr::Cell(row, col, name))
            }
            Tok::Op(c) => Err(syntax(format!("`{c}` is in the wrong place"))),
            Tok::Cmp(op) => Err(syntax(format!("`{op}` is in the wrong place"))),
        }
    }
}

/// `B4` → (row 3, column 1): the row counts from 1 and the column from A.
fn cell_ref(name: &str) -> Option<(usize, usize)> {
    let letters: String = name.chars().take_while(char::is_ascii_alphabetic).collect();
    let digits = name.get(letters.len()..)?;
    if letters.is_empty() || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = letters.bytes().try_fold(0usize, |acc, b| acc.checked_mul(26)?.checked_add((b.to_ascii_uppercase() - b'A') as usize + 1))? - 1;
    let row = digits.parse::<usize>().ok()?.checked_sub(1)?;
    (col < MAX_COLS && row < MAX_ROWS).then_some((row, col))
}

/// Where the formula's cell sits.
struct Here<'a> {
    /// The formula's table, or `None` for a formula field in an ordinary paragraph.
    t: Option<&'a Table>,
    doc: Option<&'a wordcraft_doc::Document>,
    default_lang: Option<&'a str>,
    r: usize,
    c: usize,
    g: usize,
    /// Cells read so far in this evaluation (see [`MAX_CELL_VISITS`]).
    visits: std::cell::Cell<usize>,
    /// The document's bookmarks by ASCII-lowercased name (the first in document order), looked up
    /// once per evaluation.
    bookmarks: std::cell::OnceCell<HashMap<String, (String, wordcraft_doc::Pos)>>,
    /// The text of each bookmark read so far, by its lowercased name.
    bookmark_texts: std::cell::RefCell<HashMap<String, String>>,
}

impl<'a> Here<'a> {
    fn new(t: Option<&'a Table>, doc: Option<&'a wordcraft_doc::Document>, default_lang: Option<&'a str>, r: usize, c: usize, g: usize) -> Self {
        Here {
            t,
            doc,
            default_lang,
            r,
            c,
            g,
            visits: std::cell::Cell::new(0),
            bookmarks: std::cell::OnceCell::new(),
            bookmark_texts: std::cell::RefCell::new(HashMap::new()),
        }
    }

    /// Count `n` more cells read, failing once the evaluation has read too many.
    fn visit(&self, n: usize) -> Result<(), CmdError> {
        let total = self.visits.get().saturating_add(n);
        self.visits.set(total);
        if total > MAX_CELL_VISITS { Err(CmdError::Failed("Formula too complex".into())) } else { Ok(()) }
    }

    /// The formula's table, or an error when the formula is not in one.
    fn table(&self) -> Result<&Table, CmdError> {
        self.t.ok_or_else(|| CmdError::Failed("cell references, ranges and ABOVE/LEFT/RIGHT/BELOW need a table".into()))
    }

    /// The number the bookmark `name` marks (its name matched ignoring ASCII case, first in
    /// document order), counted against the budget; text that is not a number counts as 0.
    /// In English (see [`scans_text`]) every number in its text counts (see [`scan_operand`]) and
    /// the result keeps their look; otherwise text that is not one number counts as 0.
    fn bookmark_number(&self, name: &str) -> Result<(f64, Option<Look>), CmdError> {
        self.visit(1)?;
        let undefined = || CmdError::Failed(format!("Undefined Bookmark, {name}"));
        let doc = self.doc.ok_or_else(undefined)?;
        let key = name.to_ascii_lowercase();
        let cached = self.bookmark_texts.try_borrow().ok().and_then(|m| m.get(&key).cloned());
        let text = match cached {
            Some(text) => text,
            None => {
                let marks = self.bookmarks.get_or_init(|| {
                    let mut map = HashMap::new();
                    for (n, pos) in doc.bookmarks() {
                        map.entry(n.to_ascii_lowercase()).or_insert((n, pos));
                    }
                    map
                });
                let (real, start) = marks.get(&key).ok_or_else(undefined)?;
                let text = super::references::bookmark_text_at(doc, real, start).unwrap_or_default();
                if let Ok(mut m) = self.bookmark_texts.try_borrow_mut() {
                    m.insert(key, text.clone());
                }
                text
            }
        };
        if !scans_text(self.default_lang) {
            return Ok((parse_number(&text, self.default_lang).unwrap_or(0.0), None));
        }
        let (x, look) = scan_operand(&text);
        Ok((x, Some(look)))
    }

    /// The value of the cell reference `name` (grid column `g` of row `r`) and its look, counted
    /// against the budget. In English every number in the cell's text counts and a cell without
    /// one is 0 (see [`scan_operand`]); otherwise the cell must hold one number, as before. A
    /// reference outside the table is an error.
    fn cell_operand(&self, r: usize, g: usize, name: &str) -> Result<(f64, Option<Look>), CmdError> {
        let missing = || CmdError::Failed(format!("{name} doesn't hold a number"));
        self.visit(1)?;
        let t = self.table()?;
        let cell = t.cell_at_grid(r, g).and_then(|ci| t.rows.get(r).and_then(|row| row.cells.get(ci))).ok_or_else(missing)?;
        let lang = self.cell_lang(cell);
        if !scans_text(lang.as_deref()) {
            return Ok((self.cell_number(cell).ok_or_else(missing)?, None));
        }
        let (x, look) = scan_operand(&super::table::cell_text(cell));
        Ok((x, Some(look)))
    }

    fn cell_lang(&self, c: &Cell) -> Option<String> {
        let para_lang = self.doc.and_then(|doc| {
            c.blocks.iter().filter_map(|b| b.as_para()).find(|p| !p.plain_text().trim().is_empty()).and_then(|p| super::para_lang(doc, p))
        });
        para_lang.or_else(|| self.default_lang.map(str::to_string))
    }

    fn cell_number(&self, c: &Cell) -> Option<f64> {
        let lang = self.cell_lang(c);
        let text = super::table::cell_text(c);
        parse_number(&text, lang.as_deref())
    }

    /// A range's cell (grid column `g` of row `r`) as a formula reads it, counted against the
    /// budget: in English (see [`scans_text`]) like a single cell reference, every number in its
    /// text with their look (see [`scan_operand`]), and a cell without a digit is left out; in
    /// other languages the cell must hold one number, with no look.
    fn range_cell(&self, r: usize, g: usize) -> Result<Option<(f64, Option<Look>)>, CmdError> {
        self.visit(1)?;
        let t = self.table()?;
        let Some(cell) = t.cell_at_grid(r, g).and_then(|ci| t.rows.get(r).and_then(|row| row.cells.get(ci))) else {
            return Ok(None);
        };
        let lang = self.cell_lang(cell);
        if !scans_text(lang.as_deref()) {
            return Ok(self.cell_number(cell).map(|x| (x, None)));
        }
        let text = super::table::cell_text(cell);
        if !text.chars().any(|c| c.is_ascii_digit()) {
            return Ok(None);
        }
        let (x, look) = scan_operand(&text);
        Ok(Some((x, Some(look))))
    }

    /// A cell in an ABOVE/LEFT/RIGHT/BELOW run: the number it holds, if its text is one, and the
    /// look it gives the result. In English (see [`scans_text`]) a `$` or a thousands separator
    /// gives its look, and a number in parentheses is marked as one (see [`side_numbers`]): it
    /// is negative and gives the parentheses look.
    fn side_cell(&self, c: &Cell) -> Option<SideCell> {
        let lang = self.cell_lang(c);
        let text = super::table::cell_text(c);
        let x = parse_number(&text, lang.as_deref())?;
        if !scans_text(lang.as_deref()) {
            return Some(SideCell { x, look: None, parens: false });
        }
        let t = text.trim();
        let parens = t.starts_with('(') && t.ends_with(')');
        let (_, seen) = scan_operand(t);
        let look = Look { currency: seen.currency, thousands: seen.thousands, parens, decimals: if seen.currency { seen.decimals } else { 0 } };
        Some(SideCell { x, look: Some(look), parens })
    }

    /// The number in the cell covering grid column `g` of row `r`, counted against the budget.
    fn number_at(&self, r: usize, g: usize) -> Result<Option<f64>, CmdError> {
        self.visit(1)?;
        let t = self.table()?;
        let ci = match t.cell_at_grid(r, g) {
            Some(ci) => ci,
            None => return Ok(None),
        };
        let c = match t.rows.get(r).and_then(|row| row.cells.get(ci)) {
            Some(c) => c,
            None => return Ok(None),
        };
        Ok(self.cell_number(c))
    }

    /// As [`Here::side_cell`] for the cell covering grid column `g` of row `r`, counted against
    /// the budget.
    fn side_at(&self, r: usize, g: usize) -> Result<Option<SideCell>, CmdError> {
        self.visit(1)?;
        let t = self.table()?;
        let cell = t.cell_at_grid(r, g).and_then(|ci| t.rows.get(r).and_then(|row| row.cells.get(ci)));
        Ok(cell.and_then(|c| self.side_cell(c)))
    }
}

/// A number in an ABOVE/LEFT/RIGHT/BELOW run (see [`Here::side_cell`]).
#[derive(Clone, Copy, Debug)]
struct SideCell {
    /// The cell's number.
    x: f64,
    /// The look it gives the result.
    look: Option<Look>,
    /// Whether the number is in parentheses.
    parens: bool,
}

enum Val {
    /// A number, with the look of the operands it came from (none for literals and functions).
    Num(f64, Option<Look>),
    /// The numbers of a range or ABOVE/LEFT/RIGHT/BELOW, with the look of the cells they came from.
    List(Vec<f64>, Option<Look>),
}

fn eval(e: &Expr, here: &Here) -> Result<Val, CmdError> {
    Ok(match e {
        Expr::Num(x) => Val::Num(*x, None),
        Expr::Cell(row, col, name) => {
            let (x, look) = here.cell_operand(*row, *col, name)?;
            Val::Num(x, look)
        }
        Expr::Range(r0, g0, r1, g1) => {
            // Only the part of the range inside the table holds cells.
            let t = here.table()?;
            let mut v = Vec::new();
            let mut look = None;
            let (r_end, g_end) = (t.rows.len().min(r1.saturating_add(1)), t.cols().min(g1.saturating_add(1)));
            for row in *r0..r_end {
                let mut last_ci = None;
                for g in *g0..g_end {
                    let ci = t.cell_at_grid(row, g);
                    if ci.is_some() && ci == last_ci {
                        continue;
                    }
                    last_ci = ci;
                    if let Some((x, l)) = here.range_cell(row, g)? {
                        v.push(x);
                        look = merge_looks(look, l);
                    }
                }
            }
            Val::List(v, look)
        }
        Expr::Side(side) => {
            let (v, look) = side_numbers(here, *side)?;
            Val::List(v, look)
        }
        Expr::Neg(inner) => {
            let (x, look) = scalar_look(eval(inner, here)?)?;
            Val::Num(-x, look)
        }
        Expr::Bin(op, a, b) => {
            let ((x, la), (y, lb)) = (scalar_look(eval(a, here)?)?, scalar_look(eval(b, here)?)?);
            let look = merge_looks(la, lb);
            let value = match op {
                '+' => x + y,
                '-' => x - y,
                '*' => x * y,
                '/' => {
                    if y == 0.0 {
                        return Err(CmdError::Failed("division by zero".into()));
                    }
                    x / y
                }
                _ => {
                    let p = x.powf(y);
                    finite(p)?
                }
            };
            Val::Num(value, look)
        }
        Expr::Cmp(op, a, b) => {
            let (x, y) = (scalar(eval(a, here)?)?, scalar(eval(b, here)?)?);
            let holds = match *op {
                "=" => x == y,
                "<>" => x != y,
                "<" => x < y,
                "<=" => x <= y,
                ">" => x > y,
                _ => x >= y,
            };
            Val::Num(truth(holds), None)
        }
        Expr::Call(name, args) => {
            let (x, look) = call(name, args, here)?;
            Val::Num(x, look)
        }
        Expr::Bookmark(name) => {
            let (x, look) = here.bookmark_number(name)?;
            Val::Num(x, look)
        }
    })
}

/// A truth value as a formula reads it: 1 for true, 0 for false.
fn truth(holds: bool) -> f64 {
    if holds { 1.0 } else { 0.0 }
}

/// A value that must be one number (arithmetic and single-argument functions).
fn scalar(v: Val) -> Result<f64, CmdError> {
    scalar_look(v).map(|(x, _)| x)
}

/// As [`scalar`], with the number's look.
fn scalar_look(v: Val) -> Result<(f64, Option<Look>), CmdError> {
    match v {
        Val::Num(x, look) => Ok((finite(x)?, look)),
        Val::List(..) => Err(syntax("a range can only be used inside a function".into())),
    }
}

/// The first run of numbers next to the formula's cell on one side (see the module note): the
/// cells are skipped while they hold no number, then summed while they do.
/// A cell with `$` or a thousands separator gives the sum its look. Walking away from the formula's
/// cell, a number in parentheses counts as negative and gives the parentheses look while every
/// cell taken before it was in parentheses too; after a cell that is not, it ends the run (as in
/// Word: `(5)`, `(10)` above sum to `(15)`, but `10`, `(5)`, `3` above sum to `3`).
fn side_numbers(here: &Here, side: Side) -> Result<(Vec<f64>, Option<Look>), CmdError> {
    let t = here.table()?;
    let mut out = Vec::new();
    let mut look = None;
    let mut started = false;
    let mut all_parens = true;
    let mut take = |value: Option<SideCell>| -> bool {
        match value {
            Some(cell) if !cell.parens || all_parens => {
                out.push(cell.x);
                look = merge_looks(look, cell.look);
                started = true;
                all_parens &= cell.parens;
                true
            }
            Some(_) => false,
            None => !started,
        }
    };
    match side {
        Side::Above => {
            for row in (0..here.r).rev() {
                if !take(here.side_at(row, here.g)?) {
                    break;
                }
            }
        }
        Side::Below => {
            for row in (here.r + 1)..t.rows.len() {
                if !take(here.side_at(row, here.g)?) {
                    break;
                }
            }
        }
        Side::Left | Side::Right => {
            let Some(row) = t.rows.get(here.r) else { return Ok((out, look)) };
            here.visit(row.cells.len())?;
            let cells: Vec<Option<SideCell>> = row.cells.iter().map(|c| here.side_cell(c)).collect();
            let order: Vec<usize> = if side == Side::Left { (0..here.c).rev().collect() } else { ((here.c + 1)..cells.len()).collect() };
            for ci in order {
                if !take(cells.get(ci).copied().flatten()) {
                    break;
                }
            }
        }
    }
    Ok((out, look))
}

/// Gather a function's arguments: numbers and lists (ranges and keywords) become one list, with
/// the union of their looks (see [`merge_looks`]).
fn gather_look(args: &[Expr], here: &Here) -> Result<(Vec<f64>, Option<Look>), CmdError> {
    let mut all = Vec::new();
    let mut look = None;
    for a in args {
        match eval(a, here)? {
            Val::Num(x, l) => {
                all.push(finite(x)?);
                look = merge_looks(look, l);
            }
            Val::List(v, l) => {
                all.extend(v);
                look = merge_looks(look, l);
            }
        }
    }
    Ok((all, look))
}

/// As [`gather_look`], without the look.
fn gather(args: &[Expr], here: &Here) -> Result<Vec<f64>, CmdError> {
    gather_look(args, here).map(|(v, _)| v)
}

/// A function's result and its look: SUM, COUNT, AVERAGE, MAX, MIN and PRODUCT carry the union of
/// their arguments' looks (`SUM($5, 3)` is `$8.00`, `COUNT($5, 3)` is `$2.00`, as in Word); the
/// other functions give a plain number.
fn call(name: &str, args: &[Expr], here: &Here) -> Result<(f64, Option<Look>), CmdError> {
    if matches!(name, "SUM" | "COUNT" | "AVERAGE" | "MAX" | "MIN" | "PRODUCT") && !args.is_empty() {
        let (v, look) = gather_look(args, here)?;
        let x = match name {
            "SUM" => v.iter().sum(),
            "COUNT" => v.len() as f64,
            _ if v.is_empty() => {
                if name == "PRODUCT" {
                    1.0
                } else {
                    0.0
                }
            }
            "AVERAGE" => v.iter().sum::<f64>() / v.len() as f64,
            "MAX" => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            "MIN" => v.iter().copied().fold(f64::INFINITY, f64::min),
            _ => v.iter().product(),
        };
        return Ok((x, look));
    }
    call_plain(name, args, here).map(|x| (x, None))
}

/// The one number a single-argument function takes.
fn one(name: &str, args: &[Expr], here: &Here) -> Result<f64, CmdError> {
    match args {
        [a] => scalar(eval(a, here)?),
        _ => Err(CmdError::Failed(format!("{name} takes one argument"))),
    }
}

fn call_plain(name: &str, args: &[Expr], here: &Here) -> Result<f64, CmdError> {
    let needs_one = matches!(
        name,
        "SUM" | "COUNT" | "AVERAGE" | "MAX" | "MIN" | "PRODUCT" | "ABS" | "INT" | "SIGN" | "ROUND" | "MOD" | "AND" | "OR" | "NOT" | "IF" | "DEFINED"
    );
    if args.is_empty() && needs_one {
        return Err(syntax(format!("{name}() needs an argument")));
    }
    match name {
        "TRUE" | "FALSE" if args.is_empty() => Ok(truth(name == "TRUE")),
        "TRUE" | "FALSE" => Err(CmdError::Failed(format!("{name} takes no arguments"))),
        "NOT" => Ok(truth(one(name, args, here)? == 0.0)),
        "AND" | "OR" => {
            let v = gather(args, here)?;
            if v.is_empty() {
                return Err(CmdError::Failed(format!("{name} has no numbers to test")));
            }
            Ok(truth(if name == "AND" { v.iter().all(|x| *x != 0.0) } else { v.iter().any(|x| *x != 0.0) }))
        }
        // Only the branch taken is worked out, so an error in the other one doesn't count.
        "IF" => match args {
            [cond, yes, no] => {
                if scalar(eval(cond, here)?)? != 0.0 {
                    scalar(eval(yes, here)?)
                } else {
                    scalar(eval(no, here)?)
                }
            }
            _ => Err(CmdError::Failed("IF takes three arguments: a condition, the value when it is true, and the value when it is false".into())),
        },
        // DEFINED(cell) is 1 when the cell holds a value the formula can read, 0 when it doesn't.
        "DEFINED" => match args {
            [Expr::Cell(row, col, _)] => Ok(truth(matches!(here.number_at(*row, *col), Ok(Some(_))))),
            [a] => Ok(truth(eval(a, here).is_ok())),
            _ => Err(CmdError::Failed("DEFINED takes one argument".into())),
        },
        // SUM, COUNT, AVERAGE, MAX, MIN and PRODUCT are worked out in `call`, with their look.
        "ABS" => Ok(one(name, args, here)?.abs()),
        "INT" => Ok(one(name, args, here)?.floor()),
        "SIGN" => {
            let x = one(name, args, here)?;
            Ok(if x == 0.0 { 0.0 } else { x.signum() })
        }
        "ROUND" => match args {
            [a, b] => {
                let x = scalar(eval(a, here)?)?;
                let digits = scalar(eval(b, here)?)?.clamp(-15.0, 15.0).trunc();
                let scale = 10f64.powi(digits as i32);
                let scaled = x * scale;
                if !scaled.is_finite() {
                    // Too large to have a fractional part at this precision.
                    return finite(x);
                }
                finite(scaled.round() / scale)
            }
            _ => Err(CmdError::Failed("ROUND takes two arguments".into())),
        },
        "MOD" => match args {
            [a, b] => {
                let (x, y) = (scalar(eval(a, here)?)?, scalar(eval(b, here)?)?);
                if y == 0.0 {
                    return Err(CmdError::Failed("division by zero".into()));
                }
                Ok(x - y * (x / y).floor())
            }
            _ => Err(CmdError::Failed("MOD takes two arguments".into())),
        },
        _ => Err(CmdError::Failed(format!("unknown formula function `{name}`"))),
    }
}

/// The bookmark names the Formula dialog's Paste bookmark list offers: the body's bookmarks sorted
/// by name ignoring case, each name once (the first in document order, the one a formula reads),
/// without the hidden ones (a name starting with `_`).
pub fn paste_bookmark_names(doc: &wordcraft_doc::Document) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (name, _) in doc.bookmarks() {
        if name.is_empty() || name.starts_with('_') || names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            continue;
        }
        names.push(name);
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_result_drops_trailing_zeros() {
        assert_eq!(format_result(2.5), "2.5");
        assert_eq!(format_result(4.375), "4.38");
        assert_eq!(format_result(0.1 + 0.2), "0.3");
        assert_eq!(format_result(5.0), "5");
        assert_eq!(format_result(-2.5), "-2.5");
        assert_eq!(format_result(-0.001), "0", "a result that rounds to zero is 0");
        assert_eq!(format_plain(3.5, Some("en-GB")), "3.5");
        assert_eq!(format_plain(3.5, Some("pl-PL")), "3,50", "other languages keep two decimals");
    }

    fn table(rows: &[&[&str]]) -> Table {
        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
        let mut t = Table::new(rows.len(), cols, 468.0);
        for (r, row) in rows.iter().enumerate() {
            for (c, text) in row.iter().enumerate() {
                if let Some(cl) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                    cl.blocks = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(text, Default::default()))];
                }
            }
        }
        t
    }

    fn ev(t: &Table, r: usize, c: usize, f: &str) -> Result<f64, CmdError> {
        evaluate(t, r, c, f)
    }

    #[test]
    fn sign_of_zero_is_zero() {
        let t = table(&[&["0", "5", "-5"]]);
        assert_eq!(ev(&t, 0, 0, "=SIGN(0)"), Ok(0.0));
        assert_eq!(ev(&t, 0, 0, "=SIGN(-0)"), Ok(0.0));
        assert_eq!(ev(&t, 0, 1, "=SIGN(A1)"), Ok(0.0), "a cell holding 0");
        assert_eq!(ev(&t, 0, 0, "=SIGN(5)"), Ok(1.0));
        assert_eq!(ev(&t, 0, 0, "=SIGN(-5)"), Ok(-1.0));
    }

    #[test]
    fn cell_references_count_from_one_and_from_a() {
        assert_eq!(cell_ref("A1"), Some((0, 0)));
        assert_eq!(cell_ref("B4"), Some((3, 1)));
        assert_eq!(cell_ref("AA2"), Some((1, 26)));
        assert_eq!(cell_ref("A0"), None);
        assert_eq!(cell_ref("1A"), None);
    }

    #[test]
    fn a_run_of_numbers_above_is_summed_after_skipping_cells_without_numbers() {
        // Observed in Word: the cells above are skipped while empty or text, then the numbers are
        // summed up to the next cell that holds none.
        let t = table(&[&["5"], &["x"], &["2"], &[""]]);
        assert_eq!(ev(&t, 3, 0, "=SUM(ABOVE)"), Ok(2.0), "text stops the run; the 5 above it is not counted");
        assert_eq!(ev(&t, 2, 0, "=SUM(ABOVE)"), Ok(5.0), "text directly above is skipped, so the run is the 5 above it");
        let t = table(&[&["5"], &[""], &[""], &[""]]);
        assert_eq!(ev(&t, 3, 0, "=SUM(ABOVE)"), Ok(5.0), "empty cells are skipped up to the 5");
        let t = table(&[&["5"], &["3"], &[""], &["9"], &[""]]);
        assert_eq!(ev(&t, 4, 0, "=SUM(ABOVE)"), Ok(9.0), "the run is the 9 alone: the blank cuts it off");
        let t = table(&[&["x"], &["5"], &[""]]);
        assert_eq!(ev(&t, 2, 0, "=SUM(ABOVE)"), Ok(5.0));
    }

    #[test]
    fn a_function_needs_its_arguments() {
        let t = table(&[&["1"], &[""]]);
        assert!(ev(&t, 1, 0, "=SUM()").is_err(), "empty parentheses are a syntax error, as in Word");
        assert!(ev(&t, 1, 0, "=SUM").is_err());
    }

    #[test]
    fn unknown_functions_are_errors_not_sums() {
        let t = table(&[&["1"], &["4"], &["7"], &[""]]);
        let err = ev(&t, 3, 0, "=MEDIAN(ABOVE)").err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.contains("MEDIAN"), "{err}");
    }

    #[test]
    fn round_large_values_return_input() {
        let t = table(&[&["1", "2"], &["3", "4"], &["", ""]]);
        let big = format!("1{}", "0".repeat(300));
        assert_eq!(ev(&t, 2, 0, &format!("=ROUND({big},15)")), Ok(1e300));
        assert_eq!(ev(&t, 2, 0, &format!("=ROUND({big},2)")), Ok(1e300));
    }

    #[test]
    fn ranges_and_references_and_operators() {
        let t = table(&[&["1", "2"], &["3", "4"], &["", ""]]);
        assert_eq!(ev(&t, 2, 0, "=SUM(A1:B2)"), Ok(10.0));
        assert_eq!(ev(&t, 2, 1, "=A1*B2+1"), Ok(5.0));
        assert_eq!(ev(&t, 2, 0, "=(A1+A2)/2"), Ok(2.0));
        assert_eq!(ev(&t, 2, 0, "=2^3-(-1)"), Ok(9.0));
        assert_eq!(ev(&t, 2, 0, "=AVERAGE(A1:B2)"), Ok(2.5));
        assert_eq!(ev(&t, 2, 0, "=MAX(A1:B2)"), Ok(4.0));
        assert_eq!(ev(&t, 2, 0, "=ROUND(2.5, 0)"), Ok(3.0));
        assert_eq!(ev(&t, 2, 0, "=MOD(7, 3)"), Ok(1.0));
    }

    #[test]
    fn range_over_spanning_cells_counts_each_cell_once() {
        let mut t = table(&[&["50", "10"], &["", ""]]);
        t.rows[0].cells[0].props.span = 2;
        assert_eq!(t.cols(), 3);
        assert_eq!(ev(&t, 1, 0, "=SUM(A1:B1)"), Ok(50.0), "spanning cell A1:B1 is evaluated once");
        assert_eq!(ev(&t, 1, 0, "=COUNT(A1:B1)"), Ok(1.0), "spanning cell A1:B1 has count 1");
        assert_eq!(ev(&t, 1, 0, "=SUM(A1:C1)"), Ok(60.0), "spanning cell A1:B1 plus cell C1 = 60");
    }

    #[test]
    fn division_by_zero_and_bad_syntax_are_errors() {
        let t = table(&[&["1"], &["0"]]);
        assert!(ev(&t, 1, 0, "=A1/A2").is_err());
        assert!(ev(&t, 1, 0, "=SUM(A1").is_err());
        assert!(ev(&t, 1, 0, "=A1 A1").is_err());
        assert!(ev(&t, 1, 0, "=A1:").is_err());
        assert!(ev(&t, 1, 0, &format!("={}", "(".repeat(64))).is_err(), "deep nesting is refused");
    }

    #[test]
    fn numbers_read_currency_and_separators_but_not_letters() {
        assert_eq!(parse_number("$1,200", None), Some(1200.0));
        assert_eq!(parse_number("B/. 1,234.50", None), Some(1234.5), "the Panamanian balboa");
        assert_eq!(parse_number("1,234.50 B/.", None), Some(1234.5));
        assert_eq!(parse_number("(B/. 1,234.50)", None), Some(-1234.5), "an accounting negative");
        assert_eq!(parse_number("B/.1,234.50", None), None, "Word needs a space after the sign");
        assert_eq!(parse_number("1/.5", None), None, "a fraction is not a balboa amount");
        // The formula reads the cell in the document's language. With the document's default text
        // as es-PA and the editing language as pl-PL the choice is observable: `1,250.50` is a
        // number under es-PA and not under pl-PL (whose decimal separator is `,`).
        let balboa = table(&[&["B/. 1,234.50"], &["(B/. 1,234.50)"], &[""]]);
        let mut doc = wordcraft_doc::Document::new();
        std::sync::Arc::make_mut(&mut doc.styles).default_chr.lang = Some("es-PA".into());
        assert_eq!(
            evaluate_with(&balboa, Some(&doc), Some("pl-PL"), 2, 0, "=SUM(ABOVE)"),
            Ok(0.0),
            "a balboa cell sums in the document's language, and its negative in parentheses cancels it",
        );
        assert_eq!(evaluate_with(&balboa, Some(&doc), Some("pl-PL"), 2, 0, "=COUNT(ABOVE)"), Ok(2.0), "both balboa cells are numbers",);
        // And the other way around: a balboa cell written with Polish separators is read only when
        // the document language reaches the parser, not when it falls back to English.
        let mut polish = wordcraft_doc::Document::new();
        std::sync::Arc::make_mut(&mut polish.styles).default_chr.lang = Some("pl-PL".into());
        let balboa_pl = table(&[&["B/. 1 250,50"], &[""]]);
        assert_eq!(
            evaluate_with(&balboa_pl, Some(&polish), Some("en-US"), 1, 0, "=SUM(ABOVE)"),
            Ok(1250.5),
            "the cell's language selects the separators; English does not read `1 250,50`",
        );
        assert_eq!(parse_number(" 12% ", None), Some(12.0));
        assert_eq!(parse_number("(5)", None), Some(-5.0));
        assert_eq!(parse_number("x9", None), None);
        assert_eq!(parse_number("1a", None), None);
        assert_eq!(parse_number("inf", None), None);
        assert_eq!(parse_number("", None), None);
        assert_eq!(parse_number("3,50", Some("pl-PL")), Some(3.5));
    }

    #[test]
    fn comparisons_give_one_or_zero_and_do_not_chain() {
        let t = table(&[&["5"], &["3"]]);
        assert_eq!(ev(&t, 1, 0, "=A1>A2"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=A1<=A2"), Ok(0.0));
        assert_eq!(ev(&t, 1, 0, "=A1<>A2"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=A2>=3"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=A1=5"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=A1<A2<1"), Err(CmdError::Failed("syntax error: unexpected text after the formula".into())));
    }

    #[test]
    fn if_and_or_not_and_the_truth_values() {
        let t = table(&[&["5", "x"], &["3", ""]]);
        assert_eq!(ev(&t, 1, 0, "=IF(A1>A2, 10, 20)"), Ok(10.0));
        assert_eq!(ev(&t, 1, 0, "=IF(A1<A2, 10, 20)"), Ok(20.0));
        assert_eq!(ev(&t, 1, 0, "=IF(A1>A2, 1, B1+1)"), Ok(1.0), "the branch not taken is not worked out");
        assert!(ev(&t, 1, 0, "=IF(A1<A2, 1, 1/0)").is_err(), "the branch taken is");
        assert_eq!(ev(&t, 1, 0, "=IF(A1<A2, 1, B1+1)"), Ok(1.0), "text in a cell counts as 0, as in Word");
        assert_eq!(ev(&t, 1, 0, "=AND(A1>1, A2>1)"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=AND(A1>1, A2>4)"), Ok(0.0));
        assert_eq!(ev(&t, 1, 0, "=OR(A1<1, A2>1)"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=OR(A1<1, A2<1)"), Ok(0.0));
        assert_eq!(ev(&t, 1, 0, "=NOT(A1<1)"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=TRUE"), Ok(1.0));
        assert_eq!(ev(&t, 1, 0, "=FALSE()"), Ok(0.0));
        assert!(ev(&t, 1, 0, "=TRUE(1)").is_err());
        assert!(ev(&t, 1, 0, "=IF(A1>A2, 1)").is_err(), "IF takes its three arguments");
        assert!(ev(&t, 1, 0, "=AND()").is_err());
    }

    #[test]
    fn defined_says_whether_a_cell_holds_a_value() {
        let t = table(&[&["5", "x", ""]]);
        assert_eq!(ev(&t, 0, 0, "=DEFINED(A1)"), Ok(1.0));
        assert_eq!(ev(&t, 0, 0, "=DEFINED(B1)"), Ok(0.0), "text isn't a value");
        assert_eq!(ev(&t, 0, 0, "=DEFINED(C1)"), Ok(0.0), "an empty cell isn't");
        assert_eq!(ev(&t, 0, 0, "=IF(DEFINED(C1), C1, 7)"), Ok(7.0));
    }

    #[test]
    fn a_numeric_picture_formats_the_result() {
        assert_eq!(format_picture(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(format_picture(0.256, "0%"), "26%");
        assert_eq!(format_picture(2.0, "#.##"), "2");
        assert_eq!(format_picture(0.05, "0.0"), "0.1", "halves round away from zero");
        assert_eq!(format_picture(-5.0, "$#,##0"), "-$5", "one section: a minus in front");
        assert_eq!(format_picture(-5.0, "$#,##0.00;($#,##0.00)"), "($5.00)", "the second section is the negative form");
        assert_eq!(format_picture(0.0, "0.00;-;zero"), "zero");
        assert_eq!(format_picture(7.0, "000"), "007");
        assert_eq!(format_picture(-0.001, "0.00"), "0.00", "a rounded zero has no sign");
    }

    #[test]
    fn a_field_switch_picks_the_picture_and_the_formula_stays_apart_from_it() {
        let t = table(&[&["1234.5"], &[""]]);
        assert_eq!(split_switch("=A1 \\# \"0.00\""), ("=A1", Some("\\# \"0.00\"")));
        assert_eq!(split_switch("=A1"), ("=A1", None));
        let shown = |f: &str| evaluate_field(&t, 1, 0, f).unwrap_or_else(|e| panic!("{f}: {e}"));
        assert_eq!(shown("=A1 \\# \"#,##0.00\""), "1,234.50");
        assert_eq!(shown("=A1"), "1234.5", "the cell's one decimal, as in Word");
        assert_eq!(shown("=A1 \\# 0"), "1235", "a bare picture word");
        assert_eq!(shown("=A1 \\# \"\""), "1234.5", "an empty picture is none");
    }

    #[test]
    fn a_format_switch_writes_the_result_as_text_and_sets_its_capitals() {
        let t = table(&[&["790"], &["1.5"], &[""]]);
        assert_eq!(split_switch("=A1 \\* Caps \\# 0"), ("=A1", Some("\\* Caps \\# 0")));
        let shown = |f: &str| evaluate_field(&t, 2, 0, f).unwrap_or_else(|e| panic!("{f}: {e}"));
        assert_eq!(shown("=A1 \\* CardText"), "seven hundred ninety");
        assert_eq!(shown("=A1 \\* CardText \\* Caps"), "Seven Hundred Ninety");
        assert_eq!(shown("=A1 \\* OrdText \\* FirstCap"), "Seven hundred ninetieth");
        assert_eq!(shown("=A1 \\* roman"), "dccxc");
        assert_eq!(shown("=A1 \\* MERGEFORMAT"), "790", "a switch that formats nothing");
        assert_eq!(shown("=A1 \\# \"#,##0.00\" \\* MERGEFORMAT"), "790.00");
        assert_eq!(shown("=A1 \\* MERGEFORMAT \\# 0.0"), "790.0", "a picture after a format switch");
        assert_eq!(shown("=A1 \\# \"0 \\* CardText items\""), "790 \\* CardText items", "a switch inside the picture is text");
        assert_eq!(shown("=A2 \\* CardText"), "1.5", "a fraction keeps its digits");
        assert_eq!(shown("=A1-800 \\* CardText"), "-10", "a negative number keeps its digits");
        assert_eq!(shown("=A1*1266 \\* CardText"), "1000140", "a number past CardText keeps its digits");
        assert_eq!(shown("=A1*1266 \\* OrdText \\* Upper"), "1000140");
        assert_eq!(shown("=A1*64 \\* roman"), "50560", "a number past roman keeps its digits");
        assert_eq!(shown("=A1-790 \\* roman"), "0", "roman has no zero");
        assert_eq!(shown("=A1 \\* alphabetic"), "790", "a number past alphabetic keeps its digits");
    }

    #[test]
    fn dollar_text_and_hex_switches_write_the_result() {
        let t = table(&[&["9.20", "5.35"], &["458", "-1"], &["", ""]]);
        let shown = |f: &str| evaluate_field(&t, 2, 0, f).unwrap_or_else(|e| panic!("{f}: {e}"));
        assert_eq!(shown("= 9.20 + 5.35 \\* DollarText \\* Upper"), "FOURTEEN AND 55/100");
        assert_eq!(shown("=A1+B1 \\* DollarText"), "fourteen and 55/100");
        assert_eq!(shown("=A2 \\* DollarText \\* FirstCap"), "Four hundred fifty-eight and 00/100");
        assert_eq!(shown("=0.999 \\* DollarText"), "one and 00/100");
        assert_eq!(shown("=A2 \\* Hex"), "1CA");
        assert_eq!(shown("=A2 \\* Hex \\* Lower"), "1ca");
        assert_eq!(shown("=A2*0 \\* Hex"), "0");
        assert_eq!(shown("=B2 \\* DollarText"), shown("=B2"), "a negative number keeps its digits");
        assert_eq!(shown("=B2 \\* Hex"), shown("=B2"));
        assert_eq!(shown("=A2/4 \\* Hex"), shown("=A2/4"), "a fraction keeps its digits");
        assert_eq!(shown("=A2*2200 \\* DollarText"), shown("=A2*2200"), "a number past DollarText keeps its digits");
        assert_eq!(shown("=A2 \\* ArabicDash"), "- 458 -");
    }

    #[test]
    fn a_huge_range_over_a_small_table_reads_only_the_table() {
        let t = table(&[&["1", "2", "3"], &["4", "5", "6"], &["", "", ""]]);
        // Forty ranges reaching to the last possible row and column would be about 80 million
        // grid positions; clamped to the table they are 6 numbers each, well inside the budget.
        let f = format!("=SUM({})", ["A1:BK32767"; 40].join(","));
        assert!(f.len() <= MAX_FORMULA);
        assert_eq!(ev(&t, 2, 0, &f).unwrap(), 21.0 * 40.0);
    }

    #[test]
    fn an_evaluation_past_the_cell_budget_is_an_error() {
        let mut t = Table::new(1000, 10, 468.0);
        t.rows[0].cells[0] = Cell::with_text("1");
        // Each range reads 10,000 cells, so eleven of them go past the 100,000-cell budget.
        let ok = format!("=SUM({})", ["A1:J1000"; 10].join(","));
        assert_eq!(ev(&t, 0, 1, &ok).unwrap(), 10.0);
        let too_much = format!("=SUM({})", ["A1:J1000"; 11].join(","));
        let err = ev(&t, 0, 1, &too_much).unwrap_err();
        assert_eq!(err, CmdError::Failed("Formula too complex".into()));
        assert_eq!(evaluate_field(&t, 0, 1, &too_much).unwrap_err().to_string(), "Formula too complex");
    }

    #[test]
    fn deep_power_and_sign_chains_are_rejected() {
        let t = table(&[&["1"]]);
        assert_eq!(ev(&t, 0, 0, "=2^3^2").unwrap(), 512.0);
        assert_eq!(ev(&t, 0, 0, "=-+-+2").unwrap(), 2.0);
        for f in [format!("={}1", "1^".repeat(40)), format!("={}1", "+".repeat(40)), format!("={}1", "-+".repeat(20))] {
            let err = ev(&t, 0, 0, &f).unwrap_err();
            assert!(err.to_string().contains("nested too deeply"), "{f}: {err}");
        }
    }

    /// A body of one paragraph per (bookmark name, text), each wrapped in its bookmark.
    fn marked(marks: &[(&str, &str)]) -> wordcraft_doc::Document {
        use wordcraft_doc::{CharProps, InlineObject, StoryRef};
        let text: Vec<&str> = marks.iter().map(|m| m.1).collect();
        let mut doc = wordcraft_doc::Document::from_text(&text.join("\n"));
        for (path, (name, _)) in doc.para_paths(StoryRef::Body).into_iter().zip(marks) {
            let p = doc.para_mut(StoryRef::Body, &path).unwrap();
            p.insert_object(p.len(), InlineObject::BookmarkEnd { name: (*name).into() }, &CharProps::default()).unwrap();
            p.insert_object(0, InlineObject::BookmarkStart { name: (*name).into() }, &CharProps::default()).unwrap();
        }
        doc
    }

    #[test]
    fn a_bookmark_that_is_not_a_number_counts_as_zero() {
        // Not measured: Word's documented behaviour, from memory.
        let doc = marked(&[("Word", "abc"), ("Blank", "x")]);
        assert_eq!(evaluate_text_field(&doc, None, "= Word + 3").as_deref(), Ok("3"));
        assert_eq!(evaluate_text_field(&doc, None, "= Blank * 2 + 1").as_deref(), Ok("1"));
    }

    #[test]
    fn a_bookmark_name_may_hold_underscores() {
        assert_eq!(tokenize("Net_Total*2").unwrap().first(), Some(&Tok::Name("NET_TOTAL".into())));
        let doc = marked(&[("Net_Total", "7.5"), ("a_b_c", "2")]);
        assert_eq!(evaluate_text_field(&doc, None, "= Net_Total * 2").as_deref(), Ok("15"));
        // The result keeps the decimals an operand wrote, as Word shows `1,234.50 + 3` as `1,237.5`.
        assert_eq!(evaluate_text_field(&doc, None, "= A_B_C + net_total").as_deref(), Ok("9.5"));
    }

    #[test]
    fn a_formula_outside_a_table_reads_no_cells() {
        let doc = marked(&[("A1", "4")]);
        assert!(evaluate_text_field(&doc, None, "=SUM(ABOVE)").is_err());
        assert!(evaluate_text_field(&doc, None, "=LEFT").is_err());
        assert!(evaluate_text_field(&doc, None, "=SUM(A1:B2)").is_err());
        let empty = wordcraft_doc::Document::from_text("x");
        assert_eq!(evaluate_text_field(&empty, None, "=A1").map_err(|e| e.to_string()), Err("Undefined Bookmark, A1".to_string()));
        assert_eq!(evaluate_text_field(&empty, None, r#"= 2 + 3 \# "0.0""#).as_deref(), Ok("5.0"));
    }

    #[test]
    fn table_formulas_work_out_as_before() {
        let t = table(&[&["1", "2"], &["3", "4"], &["", ""]]);
        assert_eq!(ev(&t, 2, 0, "=SUM(ABOVE)"), Ok(4.0));
        assert_eq!(ev(&t, 2, 1, "=SUM(A1:B2)*2"), Ok(20.0));
        assert_eq!(evaluate_field(&t, 2, 1, "=B1+B2").as_deref(), Ok("6"));
        // A name shaped like a cell reference stays one in a table, even with a bookmark so named.
        let doc = marked(&[("B1", "100"), ("Extra", "10")]);
        assert_eq!(evaluate_with(&t, Some(&doc), None, 2, 1, "=B1 + Extra"), Ok(12.0));
        assert!(ev(&t, 2, 1, "=Extra").is_err(), "no document, so no bookmarks");
    }

    /// The field `instr` in the cell below a one-cell table row holding `text` (`A1`).
    fn cell_field(text: &str, instr: &str) -> String {
        let t = table(&[&[text], &[""]]);
        evaluate_field(&t, 1, 0, instr).unwrap_or_else(|e| format!("error: {e}"))
    }

    #[test]
    fn every_number_in_a_cells_text_counts() {
        // Measured in Word 16.0.20527 (en-US) with the text in cell A1.
        for (text, instr, want) in [
            ("3 apples 4", "= A1", "7"),
            ("a 2, b 3", "= A1", "5"),
            ("12abc", "= A1", "12"),
            ("abc", "= A1 + 1", "1"),
            ("", "= A1 + 1", "1"),
            ("5-3", "= A1", "2"),
            ("10 - 4", "= A1", "6"),
            ("-5", "= A1 + 1", "-4"),
            ("50%", "= A1", "0.5"),
            ("3.4.5", "= A1", "3.9"),
            ("1.234,50", "= A1", "51.234"),
            ("1 000", "= A1", "1"),
            ("1e3", "= A1", "0"),
            ("1.5", "= A1 * 2", "3"),
        ] {
            assert_eq!(cell_field(text, instr), want, "{text:?} {instr}");
        }
    }

    #[test]
    fn a_result_keeps_its_cells_look() {
        for (text, instr, want) in [
            ("Total: 1,234.50 USD", "= A1", "1,234.5"),
            ("1,234.50", "= A1", "1,234.5"),
            ("1,234.50", "= A1 * 2", "2,469"),
            ("1234.5", "= A1", "1234.5"),
            ("$5", "= A1", "$5.00"),
            ("$5", "= A1 + 1", "$6.00"),
            ("(5)", "= A1", "(5)"),
            ("(5)", "= A1 + 1", "(4)"),
            ("12", "= A1 / 7", "1.71"),
            ("12", "= A1 * 1000", "12000"),
        ] {
            assert_eq!(cell_field(text, instr), want, "{text:?} {instr}");
        }
    }

    #[test]
    fn a_numeric_picture_wins_over_a_cells_look() {
        assert_eq!(cell_field("$5", r#"= A1 + 1 \# "0.00""#), "6.00");
        assert_eq!(cell_field("(5)", r#"= A1 \# "0.00""#), "-5.00");
        assert_eq!(cell_field("1,234.50", r#"= A1 \# "0""#), "1235");
    }

    #[test]
    fn looks_merge_as_a_union_and_a_currency_negative_shows_in_parentheses() {
        let dollar = Look { currency: true, ..Look::default() };
        let comma = Look { thousands: true, decimals: 2, ..Look::default() };
        let merged = merge_looks(Some(dollar), Some(comma));
        assert_eq!(merged, Some(Look { currency: true, thousands: true, parens: false, decimals: 2 }), "union of both");
        assert_eq!(merge_looks(None, Some(dollar)), Some(dollar), "a literal keeps the other look");
        assert_eq!(merge_looks(None, None), None);
        assert_eq!(format_look(-5.0, dollar), "($5.00)", "a currency negative is in parentheses");
        assert_eq!(format_look(0.125, Look { decimals: 3, ..dollar }), "$0.125", "more decimals are kept");
        assert_eq!(format_look(5.0, Look { parens: true, ..Look::default() }), "5", "a positive result has no parentheses");
    }

    #[test]
    fn a_range_leaves_out_cells_without_digits_and_side_runs_take_dollar_and_comma_looks() {
        // A cell without a digit is not a number of the range, so COUNT and AVERAGE skip it.
        let t = table(&[&["$4"], &["x"], &[""], &["2"], &[""]]);
        assert_eq!(evaluate_field(&t, 4, 0, "=COUNT(A1:A4)"), Ok("$2.00".into()), "COUNT carries the $");
        assert_eq!(evaluate_field(&t, 4, 0, "=AVERAGE(A1:A4)"), Ok("$3.00".into()));
        // A run below and to the right reads like one above and to the left.
        let t = table(&[&["", "1,000", "(5)"], &["$3", "", ""], &["1", "", ""]]);
        assert_eq!(evaluate_field(&t, 0, 0, "=SUM(BELOW)"), Ok("$4.00".into()));
        assert_eq!(evaluate_field(&t, 0, 0, "=SUM(RIGHT)"), Ok("1,000".into()), "the (5) ends the run");
    }

    #[test]
    fn a_side_run_counts_leading_parenthesised_cells() {
        // Word-measured: cells top to bottom, the formula cell right below (and the same row
        // read the other ways: LEFT, RIGHT and BELOW walk away from the formula cell too).
        let rows: [(&[&str], &str); 13] = [
            (&["(10)"], "(10)"),
            (&["(5)", "(10)"], "(15)"),
            (&["$5", "(10)"], "($5.00)"),
            (&["$5", "(10)", "(2)"], "($7.00)"),
            (&["10", "(5)"], "5"),
            (&["(5)", "10"], "10"),
            (&["10", "(5)", "3"], "3"),
            (&["10", "-5", "3"], "8"),
            (&["5", "-10"], "-5"),
            (&["$5", "-10"], "($5.00)"),
            (&["$0.125", "1"], "$1.125"),
            (&["1,250", "100"], "1,350"),
            (&["$1250.50", "3.25"], "$1253.75"),
        ];
        for (cells, want) in rows {
            let n = cells.len();
            let mut column: Vec<Vec<&str>> = cells.iter().map(|c| vec![*c]).collect();
            column.push(vec![""]);
            let column: Vec<&[&str]> = column.iter().map(Vec::as_slice).collect();
            assert_eq!(evaluate_field(&table(&column), n, 0, "=SUM(ABOVE)"), Ok(want.into()), "ABOVE {cells:?}");
            let mut below: Vec<Vec<&str>> = vec![vec![""]];
            below.extend(cells.iter().rev().map(|c| vec![*c]));
            let below: Vec<&[&str]> = below.iter().map(Vec::as_slice).collect();
            assert_eq!(evaluate_field(&table(&below), 0, 0, "=SUM(BELOW)"), Ok(want.into()), "BELOW {cells:?}");
            let mut row: Vec<&str> = cells.to_vec();
            row.push("");
            assert_eq!(evaluate_field(&table(&[&row]), 0, n, "=SUM(LEFT)"), Ok(want.into()), "LEFT {cells:?}");
            let mut right: Vec<&str> = vec![""];
            right.extend(cells.iter().rev());
            assert_eq!(evaluate_field(&table(&[&right]), 0, 0, "=SUM(RIGHT)"), Ok(want.into()), "RIGHT {cells:?}");
        }
    }

    #[test]
    fn a_reference_outside_the_table_is_still_an_error() {
        let t = table(&[&["1"], &[""]]);
        assert!(evaluate_field(&t, 1, 0, "= C9").is_err());
    }

    #[test]
    fn operand_text_reading_handles_odd_text() {
        assert_eq!(scan_operand("").0, 0.0);
        assert_eq!(scan_operand("(").0, 0.0);
        assert_eq!(scan_operand(".").0, 0.0);
        assert_eq!(scan_operand("1e").0, 1.0);
        assert_eq!(scan_operand("2e-").0, 2.0);
        assert_eq!(scan_operand("1e+5 3").0, 3.0);
        assert_eq!(scan_operand("1,").0, 1.0);
        assert_eq!(scan_operand("1,23").0, 24.0);
        assert_eq!(scan_operand("1,2345").0, 2346.0);
        assert!(scan_operand(&"9".repeat(400)).0.is_infinite());
        let t = table(&[&[&"9".repeat(400)], &[""]]);
        assert!(evaluate_field(&t, 1, 0, "= A1").is_err(), "too large to show");
    }

    #[test]
    fn bookmark_operands_count_against_the_visit_budget() {
        let doc = marked(&[("N", "1")]);
        let here = Here::new(None, Some(&doc), None, 0, 0, 0);
        here.visits.set(MAX_CELL_VISITS - 1);
        assert_eq!(eval(&Expr::Bookmark("N".into()), &here).ok().map(|v| scalar(v).ok()), Some(Some(1.0)));
        assert_eq!(here.visits.get(), MAX_CELL_VISITS);
        assert!(eval(&Expr::Bookmark("N".into()), &here).is_err(), "past the budget");
    }
}
