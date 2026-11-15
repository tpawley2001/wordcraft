//! Save as RTF writes `\trhdr` (Repeat as header row) only for rows that repeat, and never makes a
//! cell's text bold on its own: the table look's Header Row (on by default) only picks which parts
//! of a table style apply, as in Word.

use super::*;

fn open(body: &str) -> wordcraft_doc::Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}{body}}}").as_bytes()).unwrap()
}

/// The written body from the first row definition on (the stylesheet has bold styles of its own).
fn body(rtf: &str) -> &str {
    &rtf[rtf.find(r"\trowd").expect("a table row")..]
}

const PLAIN: &str = r"\trowd\trgaph108\trleft0\cellx1440\cellx2880\pard\intbl A\cell B\cell\row\trowd\trgaph108\trleft0\cellx1440\cellx2880\pard\intbl C\cell D\cell\row\pard AFTER\par";

#[test]
fn a_plain_table_saves_without_a_header_row_or_bold() {
    let rtf = export(&open(PLAIN));
    let b = body(&rtf);
    assert!(!b.contains(r"\trhdr"), "{b}");
    assert!(!b.contains(r"\b "), "{b}");
    // And it reads back the same way: no row repeats.
    let back = import(rtf.as_bytes()).unwrap();
    let t = back.body.iter().find_map(|x| x.as_table()).unwrap();
    assert!(t.rows.iter().all(|r| !r.props.header), "{b}");
}

#[test]
fn the_look_alone_does_not_make_a_header_row() {
    // Header Row explicitly on in the look, no table style.
    let d = open(&PLAIN.replace(r"\trleft0\cellx1440", r"\trleft0\tbllkhdrrows\cellx1440"));
    let rtf = export(&d);
    let b = body(&rtf);
    assert!(!b.contains(r"\trhdr"), "{b}");
    assert!(!b.contains(r"\b "), "{b}");
}

#[test]
fn a_repeating_header_row_keeps_trhdr_without_forced_bold() {
    let d = open(&PLAIN.replacen(r"\trleft0\cellx1440", r"\trleft0\trhdr\cellx1440", 1));
    let rtf = export(&d);
    let b = body(&rtf);
    assert_eq!(b.matches(r"\trhdr").count(), 1, "{b}");
    assert!(!b.contains(r"\b "), "{b}");
    let back = import(rtf.as_bytes()).unwrap();
    let t = back.body.iter().find_map(|x| x.as_table()).unwrap();
    let hdr: Vec<bool> = t.rows.iter().map(|r| r.props.header).collect();
    assert_eq!(hdr, vec![true, false], "{b}");
}

#[test]
fn bold_text_in_a_header_row_stays_bold() {
    let d = open(&PLAIN.replacen(r"\trleft0\cellx1440", r"\trleft0\trhdr\cellx1440", 1).replacen(r"\intbl A\cell", r"\intbl {\b A}\cell", 1));
    let rtf = export(&d);
    let b = body(&rtf);
    assert_eq!(b.matches(r"\b ").count() + b.matches(r"\b\").count(), 1, "only A is bold: {b}");
}
