//! A document whose last paragraph ends at the closing brace, with no `\par`: Word gives that
//! paragraph the paragraph and character properties in force at the brace, exactly as if the
//! file had ended `\par}` (measured in Word 16.0.20527: indent, space before, alignment and bold
//! all kept; a trailing `\pard` resets the paragraph properties only). No empty paragraph is added.

use super::*;
use wordcraft_doc::{Document, props::Align};

fn open(body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

/// (text, left indent, space before, alignment) of every body paragraph.
fn paras(d: &Document) -> Vec<(String, Option<f32>, Option<f32>, Align)> {
    d.body
        .iter()
        .filter_map(|b| b.as_para())
        .map(|p| (p.text.to_string(), p.props.indent_left, p.props.space_before, p.props.align.unwrap_or_default()))
        .collect()
}

/// Whether every run of body paragraph `i` that holds text is bold.
fn all_bold(d: &Document, i: usize) -> bool {
    let p = d.body.iter().filter_map(|b| b.as_para()).nth(i).unwrap();
    p.runs.iter().filter(|r| r.len > 0).all(|r| r.props.bold == Some(true))
}

#[test]
fn single_paragraph_without_par_keeps_indent_spacing_alignment_and_bold() {
    let with_par = open(r"\pard\li2880\sb1440\qc\b\fs24 Lonely last paragraph\par");
    let without = open(r"\pard\li2880\sb1440\qc\b\fs24 Lonely last paragraph");
    assert_eq!(paras(&without), [("Lonely last paragraph".to_string(), Some(144.0), Some(72.0), Align::Center)]);
    assert_eq!(paras(&without), paras(&with_par));
    assert!(all_bold(&without, 0));
}

#[test]
fn second_paragraph_without_par_keeps_its_own_properties() {
    let d = open(r"\pard\li720 First paragraph\par\pard\li2880\qr\b Second paragraph");
    let p = paras(&d);
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].1, p[0].3), (Some(36.0), Align::Left));
    assert_eq!((p[1].0.as_str(), p[1].1, p[1].3), ("Second paragraph", Some(144.0), Align::Right));
    assert!(all_bold(&d, 1));
    assert_eq!(p, paras(&open(r"\pard\li720 First paragraph\par\pard\li2880\qr\b Second paragraph\par")));
}

#[test]
fn trailing_pard_resets_paragraph_properties_but_not_bold() {
    let d = open(r"\pard\li720 First paragraph\par\pard\li2880\qr\b Second paragraph\pard");
    let p = paras(&d);
    assert_eq!(p.len(), 2);
    assert_eq!((p[1].0.as_str(), p[1].1.unwrap_or(0.0), p[1].3), ("Second paragraph", 0.0, Align::Left));
    assert!(all_bold(&d, 1));
}

#[test]
fn a_group_closed_before_the_end_still_restores_its_state() {
    // Only the properties current at the end of the input carry over: a group that closed earlier
    // restores the outer state as before.
    let d = open(r"\pard\li720 {\qc\li2880 inside} after");
    let p = paras(&d);
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].1, p[0].3), (Some(36.0), Align::Left));
}

#[test]
fn truncated_file_keeps_the_last_paragraph_properties() {
    // No closing brace at all.
    let d = import(br"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}\pard\li2880\qc Cut short").unwrap();
    let p = paras(&d);
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].0.as_str(), p[0].1, p[0].3), ("Cut short", Some(144.0), Align::Center));
}
