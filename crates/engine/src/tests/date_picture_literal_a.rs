//! Date-time pictures (`\@`, ECMA-376 §17.16.4.1): a bare lowercase `a`, and an uppercase `A`
//! followed by `M` that does not start `AM/PM`, are literal text. Expected results measured in
//! Microsoft 365 Word for Windows (QUOTE fields with `\@`, updated on stated times).

use crate::cmd::insert::format_date_parts;

fn at(picture: &str, hms: (u32, u32, u32)) -> String {
    format_date_parts(picture, 2026, 11, 14, hms, 20_406)
}

#[test]
fn a_bare_lowercase_a_is_literal() {
    for hms in [(9, 5, 0), (23, 5, 0), (0, 5, 0)] {
        assert_eq!(at("a", hms), "a", "a at {hms:?}");
    }
}

#[test]
fn an_uppercase_a_before_m_is_literal() {
    for (picture, hms, word) in [
        ("AM", (9, 5, 0), "A11"),
        ("AMPM", (9, 5, 0), "A11P11"),
        ("h AMPM", (13, 5, 0), "1 A11P11"),
        ("h AMPM", (9, 5, 0), "9 A11P11"),
    ] {
        assert_eq!(at(picture, hms), word, "{picture} at {hms:?}");
    }
}

#[test]
fn am_pm_spellings_word_reads_are_unchanged() {
    for (picture, hms, word) in [
        ("h:mm aM/pM", (13, 5, 0), "1:05 PM"),
        ("h:mm aM/pM", (9, 5, 0), "9:05 AM"),
        ("h:mm am/pm", (13, 5, 0), "1:05 PM"),
        ("h:mm AM/PM", (9, 5, 0), "9:05 AM"),
        ("h:mm a/p", (13, 5, 0), "1:05 PM"),
    ] {
        assert_eq!(at(picture, hms), word, "{picture} at {hms:?}");
    }
}
