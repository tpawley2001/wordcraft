//! Paragraph indents in character units (`\cufiN`, `\culiN`, `\curiN`, RTF 1.9.1; hundredths of a
//! character, like OOXML `w:firstLineChars`): read into the paragraph's own properties next to the
//! twips indents, reset by `\pard`, and written back.

use super::*;
use wordcraft_doc::Document;

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

/// (first, left, right) in characters, per paragraph.
fn chars(d: &Document) -> Vec<(Option<f32>, Option<f32>, Option<f32>)> {
    d.body.iter().filter_map(|b| b.as_para()).map(|p| (p.props.indent_first_chars, p.props.indent_left_chars, p.props.indent_right_chars)).collect()
}

#[test]
fn character_unit_indents_are_read() {
    let d = open(r"\pard\cufi200\culi350\curi100 a\par");
    assert_eq!(chars(&d), [(Some(2.0), Some(3.5), Some(1.0))]);
}

#[test]
fn a_negative_first_line_value_is_a_hanging_indent() {
    let d = open(r"\pard\cufi-200\culi200 a\par");
    assert_eq!(chars(&d), [(Some(-2.0), Some(2.0), None)]);
}

#[test]
fn twips_indents_are_kept_beside_the_character_units() {
    let d = open(r"\pard\fi420\li840\cufi200\culi400 a\par");
    let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
    assert_eq!((p.props.indent_first, p.props.indent_left), (Some(21.0), Some(42.0)));
    assert_eq!((p.props.indent_first_chars, p.props.indent_left_chars), (Some(2.0), Some(4.0)));
}

#[test]
fn pard_resets_and_later_paragraphs_carry_them() {
    let d = open(r"\pard\culi200 a\par b\par\pard c\par\pard\cufi0 d\par");
    assert_eq!(chars(&d), [(None, Some(2.0), None), (None, Some(2.0), None), (None, None, None), (Some(0.0), None, None)]);
}

#[test]
fn character_unit_indents_round_trip() {
    let d = open(r"\pard\cufi-150\culi250\curi75 a\par\pard plain\par\pard\cufi0 zero\par");
    let want = [(Some(-1.5), Some(2.5), Some(0.75)), (None, None, None), (Some(0.0), None, None)];
    assert_eq!(chars(&d), want);
    let rtf = export(&d);
    assert!(rtf.contains(r"\cufi-150") && rtf.contains(r"\culi250") && rtf.contains(r"\curi75"), "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(chars(&back), want);
}
