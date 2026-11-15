//! `\trautofitN`: a table is AutoFit only when its first row says `\trautofit1`; without it Word
//! opens the table with fixed column widths (checked in Microsoft 365 Word, 2026-11-14).

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}\fs24{body}}}").as_bytes()).unwrap()
}

/// One table of two rows; `first` and `second` go after each row's `\trowd`.
fn rows(first: &str, second: &str) -> String {
    format!(
        r"\trowd\trgaph108{first}\cellx1440\cellx4000\pard\intbl A\cell B\cell\row\trowd\trgaph108{second}\cellx1440\cellx4000\pard\intbl C\cell D\cell\row\pard AFTER\par"
    )
}

fn fixed(d: &Document) -> bool {
    d.body.iter().find_map(|b| b.as_table()).expect("a table").props.fixed
}

#[test]
fn a_table_without_autofit_is_fixed() {
    assert!(fixed(&open(&rows("", ""))));
}

#[test]
fn autofit_one_is_autofit() {
    assert!(!fixed(&open(&rows(r"\trautofit1", r"\trautofit1"))));
}

#[test]
fn bare_autofit_word_counts_as_on() {
    assert!(!fixed(&open(&rows(r"\trautofit", r"\trautofit"))));
}

#[test]
fn autofit_zero_is_fixed() {
    assert!(fixed(&open(&rows(r"\trautofit0", r"\trautofit0"))));
}

#[test]
fn the_first_row_decides() {
    assert!(!fixed(&open(&rows(r"\trautofit1", ""))));
    assert!(fixed(&open(&rows("", r"\trautofit1"))));
}

#[test]
fn the_setting_does_not_carry_into_the_next_table() {
    let body = format!(r"{}\pard BETWEEN\par{}", rows(r"\trautofit1", r"\trautofit1"), rows("", ""));
    let d = open(&body);
    let tables: Vec<bool> = d.body.iter().filter_map(|b| b.as_table()).map(|t| t.props.fixed).collect();
    assert_eq!(tables, [false, true]);
}

#[test]
fn autofit_tables_write_autofit_on_every_row_and_read_back() {
    let rtf = export(&open(&rows(r"\trautofit1", r"\trautofit1")));
    assert_eq!(rtf.matches(r"\trautofit1").count(), 2, "{rtf}");
    assert!(!fixed(&import(rtf.as_bytes()).unwrap()));
}

#[test]
fn fixed_tables_write_no_autofit_word_and_read_back_fixed() {
    let rtf = export(&open(&rows("", "")));
    assert!(!rtf.contains(r"\trautofit"), "{rtf}");
    assert!(fixed(&import(rtf.as_bytes()).unwrap()));
}
