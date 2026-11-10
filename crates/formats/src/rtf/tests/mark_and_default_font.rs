//! The default font (`\deff`) whatever its pitch, and the paragraph mark's formatting of an
//! empty paragraph (the character formatting in effect at its `\par`), as Word reads them: an
//! empty line takes its height from its mark, so a mark Word keeps and we drop moves every line
//! after it (measured in Word 16 on these inputs).

use super::*;
use wordcraft_doc::{Document, Paragraph, PartKind};

const FONTS: &str = r"{\fonttbl{\f0\froman Times New Roman;}{\f1\fswiss Arial;}{\f6\fmodern Courier New;}}";

fn open(deff: u32, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff{deff}{FONTS}{body}}}").as_bytes()).unwrap()
}

fn paras(d: &Document) -> Vec<&Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

/// The mark's font and size of each body paragraph.
fn marks(d: &Document) -> Vec<(Option<String>, Option<f32>)> {
    paras(d).iter().map(|p| (p.mark.font.clone(), p.mark.size)).collect()
}

#[test]
fn monospaced_deff_is_the_default_font() {
    // Word shows "AAA" in Courier New: \plain resets to \deff even when it is monospaced.
    let d = open(6, r"\pard\plain AAA\par");
    assert_eq!(d.styles.default_chr.font.as_deref(), Some("Courier New"));
}

#[test]
fn plain_text_in_a_monospaced_default_font_keeps_it() {
    // The run never names a font: it is in the default font, not in some other font.
    let d = open(6, r"\pard\plain AAA\par");
    let p = paras(&d)[0];
    assert_eq!(p.text, "AAA");
    // Its font: the run's own, else the document default.
    let fonts: Vec<_> = p.runs.iter().map(|r| r.props.font.clone().or_else(|| d.styles.default_chr.font.clone())).collect();
    assert!(!fonts.is_empty() && fonts.iter().all(|f| f.as_deref() == Some("Courier New")), "{fonts:?}");
}

#[test]
fn deff_proportional_still_works() {
    let d = open(1, r"\pard\plain AAA\par");
    assert_eq!(d.styles.default_chr.font.as_deref(), Some("Arial"));
}

#[test]
fn empty_paragraph_keeps_its_mark_size_and_font() {
    // Word: the empty line is 41.4 pt (Times New Roman 36 pt), 13.59 pt (Courier New 12 pt).
    // (Default font Arial, so neither mark font is the default.)
    let d = open(1, r"\pard\plain\f0\fs72 \par\pard\plain\f6\fs24 \par\pard\plain\f0\fs24 BBB\par");
    let m = marks(&d);
    assert_eq!(m[0], (Some("Times New Roman".to_string()), Some(36.0)));
    assert_eq!(m[1], (Some("Courier New".to_string()), Some(12.0)));
}

#[test]
fn empty_paragraph_mark_from_a_group() {
    // `{\f1\fs48 \par}`: the formatting in effect at the \par, even inside a group (Word: a 27.6 pt
    // Arial 24 pt line).
    let d = open(0, r"\pard\plain{\f1\fs48 \par}\pard\plain\f0\fs24 BBB\par");
    assert_eq!(marks(&d)[0], (Some("Arial".to_string()), Some(24.0)));
}

#[test]
fn paragraph_with_text_gets_no_mark() {
    // Word: a bigger mark doesn't grow a line that has text; we keep such a paragraph as it is.
    let d = open(0, r"\pard\plain\f0\fs24 AAA{\fs72\par}\pard\plain\f0\fs24 BBB\par");
    let p = paras(&d);
    assert_eq!(p[0].text, "AAA");
    assert_eq!(p[0].mark.size, None);
}

#[test]
fn empty_paragraph_mark_round_trips() {
    let d = open(0, r"\pard\plain{\f1\fs48 \par}\pard\plain\f6\fs24 \par\pard\plain\f0\fs24 BBB\par");
    let back = import(export(&d).as_bytes()).unwrap();
    let m = marks(&back);
    assert_eq!(m[0], (Some("Arial".to_string()), Some(24.0)));
    assert_eq!(m[1], (Some("Courier New".to_string()), Some(12.0)));
}

#[test]
fn empty_header_paragraph_keeps_its_mark() {
    // The court-transcript header: one empty Courier New 12 pt paragraph (the page number sits in a
    // text box). Word starts the body below a 13.59 pt header line, not a default-font one.
    let d = open(6, r"\sectd\headery360{\header\pard\plain\f6\fs24 \par}\pard\plain\f6\fs24 BODY\par");
    let header = d.parts.values().find(|p| p.kind == PartKind::Header).expect("header");
    let p = header.blocks.iter().find_map(|b| b.as_para()).expect("header paragraph");
    assert!(p.text.is_empty(), "{:?}", p.text);
    // Courier New is also the default font here: the mark may name it or leave it to the default.
    let font = p.mark.font.clone().or_else(|| d.styles.default_chr.font.clone());
    assert_eq!((font.as_deref(), p.mark.size), (Some("Courier New"), Some(12.0)));
}
