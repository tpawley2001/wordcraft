//! Sort Text by Number and by Date as Word does (Home › Sort, Sort by Paragraphs, Type Number / Date),
//! measured in Microsoft 365 Word 16.0.20527 for Windows through `Range.Sort`, 2026-11-14.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

/// Paragraphs from `lines` (one per line), English (US), all selected.
fn paras(lines: &[&str]) -> Session {
    let mut s = Session::new(Document::from_text(&lines.join("\n")));
    s.run("review.language", &json!({"lang": "en-US", "default": true})).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s
}

fn sorted(lines: &[&str], keys: Value) -> Vec<String> {
    let mut s = paras(lines);
    s.run("para.sort", &json!({"keys": keys})).unwrap_or_else(|e| panic!("para.sort {keys}: {e}"));
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()
}

const NUMBERS: &[&str] =
    &["10", "9", "-1", "1.5", "$3", "(2)", "abc", "1,000", "2e3", "5%", "x7y", "3-4", "0.5", ".25", "7 apples", "b", "12 13", "-0.5", "+4", "8."];

#[test]
fn paragraphs_without_a_number_sort_below_every_number() {
    assert_eq!(
        sorted(NUMBERS, json!([{"type": "number"}])),
        ["abc", "2e3", "x7y", "b", "+4", "(2)", "-1", "3-4", "-0.5", "5%", ".25", "0.5", "1.5", "$3", "7 apples", "8.", "9", "10", "12 13", "1,000"]
    );
    assert_eq!(
        sorted(NUMBERS, json!([{"type": "number", "descending": true}])),
        ["1,000", "12 13", "10", "9", "8.", "7 apples", "$3", "1.5", "0.5", ".25", "5%", "-0.5", "-1", "3-4", "(2)", "abc", "2e3", "x7y", "b", "+4"]
    );
}

/// Reference values each text was sorted among in Word.
const REFS: &[&str] = &[
    "-1000", "-10", "-5", "-2.5", "-1.5", "-1", "-0.75", "-0.25", "0", "0.03", "0.07", "0.3", "1", "2.5", "3.5", "4.5", "7", "11.5", "12.06", "12.2",
    "12.5", "13", "100", "1212", "1214", "5000",
];

/// (text, the reference just before it, the one just after it) after an ascending Number sort of the text
/// followed by [`REFS`], as Word placed it.
const WORD: &[(&str, Option<&str>, Option<&str>)] = &[
    ("12 13", Some("13"), Some("100")),
    ("3-4", Some("-1.5"), Some("-1")),
    ("+4", None, Some("-1000")),
    ("5%", Some("0.03"), Some("0.07")),
    ("2e3", None, Some("-1000")),
    ("1 000", Some("0.3"), Some("1")),
    ("1.5.3", Some("1"), Some("2.5")),
    ("4+", None, Some("-1000")),
    ("(2", None, Some("-1000")),
    ("-$3", Some("-5"), Some("-2.5")),
    ("$-3", None, Some("-1000")),
    ("3,5", Some("7"), Some("11.5")),
    ("x7y", None, Some("-1000")),
    ("abc 5", Some("4.5"), Some("7")),
    ("5 abc", Some("4.5"), Some("7")),
    ("4-", None, Some("-1000")),
    ("10-3-2", Some("4.5"), Some("7")),
    ("2*3", Some("4.5"), Some("7")),
    ("1/2", Some("0.3"), Some("1")),
    ("12.5%", Some("0.07"), Some("0.3")),
    ("(3)%", Some("-0.25"), Some("0")),
    ("-(3)", Some("2.5"), Some("3.5")),
    ("3)", None, Some("-1000")),
    ("7.", Some("4.5"), Some("7")),
    ("..5", Some("0.3"), Some("1")),
    ("1,2,3", Some("4.5"), Some("7")),
    ("1,23", Some("13"), Some("100")),
    ("12,345.6", Some("5000"), None),
    ("$1,000", Some("100"), Some("1212")),
    ("-", None, Some("-1000")),
    (".", None, Some("-1000")),
    ("0", Some("-0.25"), Some("0")),
    ("00012", Some("11.5"), Some("12.06")),
];

#[test]
fn a_number_key_reads_each_paragraph_as_word_does() {
    let mut wrong = Vec::new();
    for &(text, lo, hi) in WORD {
        let mut lines = vec![text];
        lines.extend_from_slice(REFS);
        let out = sorted(&lines, json!([{"type": "number"}]));
        let i = out.iter().position(|t| t == text).unwrap();
        let got = (i.checked_sub(1).map(|k| out[k].as_str()), out.get(i + 1).map(String::as_str));
        if got != (lo, hi) {
            wrong.push(format!("{text:?}: Word between {lo:?} and {hi:?}, ours {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

const DATES: &[&str] = &[
    "1/2/2020", "2020-01-03", "Jan 5, 2020", "5 January 2020", "12/31/99", "abc", "3/4/2021", "2019", "June 2018", "1/2/30", "1/2/29", "2021-13-01", "b",
];

#[test]
fn a_date_key_reads_word_date_forms_and_puts_the_rest_below() {
    assert_eq!(
        sorted(DATES, json!([{"type": "date"}])),
        [
            "abc", "2019", "2021-13-01", "b", "1/2/30", "12/31/99", "June 2018", "1/2/2020", "2020-01-03", "Jan 5, 2020", "5 January 2020", "3/4/2021",
            "1/2/29"
        ]
    );
    assert_eq!(
        sorted(DATES, json!([{"type": "date", "descending": true}])),
        [
            "1/2/29", "3/4/2021", "Jan 5, 2020", "5 January 2020", "2020-01-03", "1/2/2020", "June 2018", "12/31/99", "1/2/30", "abc", "2019", "2021-13-01",
            "b"
        ]
    );
}

#[test]
fn a_first_number_over_twelve_is_the_day() {
    assert_eq!(
        sorted(&["2/30/2026", "2/3/2026", "13/1/2026", "1/31/2026"], json!([{"type": "date"}])),
        ["2/30/2026", "13/1/2026", "1/31/2026", "2/3/2026"]
    );
}
