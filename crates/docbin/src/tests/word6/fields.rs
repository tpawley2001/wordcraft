//! Word 6.0/95 fields whose result runs over several paragraphs: the result keeps its paragraphs
//! between `FieldStart` and `FieldEnd` markers, as the Word 97 reader keeps them.

use super::{plain, read};
use wordcraft_doc::InlineObject;

fn open(text: &str) -> wordcraft_doc::Document {
    read(&plain(text).build()).expect("Word 6.0 opens")
}

fn paras(doc: &wordcraft_doc::Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

/// Each body paragraph's text without object placeholders.
fn texts(doc: &wordcraft_doc::Document) -> Vec<String> {
    paras(doc).iter().map(|p| p.text.replace('\u{FFFC}', "")).collect()
}

fn starts(p: &wordcraft_doc::Paragraph) -> Vec<String> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::FieldStart { instr, .. } => Some(instr.clone()),
            _ => None,
        })
        .collect()
}

fn ends(p: &wordcraft_doc::Paragraph) -> usize {
    p.objects.iter().filter(|o| matches!(o, InlineObject::FieldEnd)).count()
}

fn all_starts(doc: &wordcraft_doc::Document) -> Vec<String> {
    paras(doc).iter().flat_map(|p| starts(p)).collect()
}

fn all_ends(doc: &wordcraft_doc::Document) -> usize {
    paras(doc).iter().map(|p| ends(p)).sum()
}

#[test]
fn a_table_of_contents_keeps_its_entry_paragraphs() {
    let d = open("Before\r\x13 TOC \\o \x14One\t1\rTwo\t2\x15\rAfter\r");
    assert_eq!(texts(&d), ["Before", "One\t1", "Two\t2", "After"]);
    let p = paras(&d);
    let s = starts(p[1]);
    assert_eq!(s.len(), 1, "{s:?}");
    assert!(s[0].starts_with("TOC"), "{s:?}");
    assert_eq!(ends(p[2]), 1);
    assert_eq!((all_starts(&d).len(), all_ends(&d)), (1, 1));
}

#[test]
fn any_field_whose_result_reaches_a_paragraph_mark_keeps_its_paragraphs() {
    let d = open("\x13 REF bm \x14Line A\rLine B\x15\r");
    assert_eq!(texts(&d), ["Line A", "Line B"]);
    let p = paras(&d);
    assert_eq!(starts(p[0]), ["REF bm"]);
    assert_eq!(ends(p[1]), 1);
}

#[test]
fn a_one_paragraph_field_stays_an_inline_field() {
    let d = open("\x13 PAGE \x141\x15 of 3\r");
    assert_eq!(paras(&d).len(), 1);
    assert_eq!((all_starts(&d).len(), all_ends(&d)), (0, 0));
}

#[test]
fn a_field_nested_in_a_table_of_contents_stays_inside_its_entry() {
    let d = open("\x13 TOC \x14One\t\x13 PAGEREF _Toc1 \x141\x15\rTwo\t2\x15\r");
    let t = texts(&d);
    assert_eq!(t.len(), 2, "{t:?}");
    assert!(t[0].starts_with("One"), "{t:?}");
    assert_eq!(t[1], "Two\t2");
    let s = all_starts(&d);
    assert_eq!(s.len(), 1, "{s:?}");
    assert!(s[0].starts_with("TOC"), "{s:?}");
    assert_eq!(all_ends(&d), 1);
}

#[test]
fn a_field_left_open_at_the_end_is_closed() {
    let d = open("\x13 TOC \x14One\rTwo\r");
    let t = texts(&d).join("|");
    assert!(t.contains("One") && t.contains("Two"), "{t}");
    assert_eq!(all_starts(&d).len(), all_ends(&d));
}
