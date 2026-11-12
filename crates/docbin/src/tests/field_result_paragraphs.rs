//! A field's result that runs over several paragraphs keeps its paragraphs ([MS-DOC] §2.8.25
//! PlcFld: the field characters sit in the ordinary text, so paragraph marks between the separator
//! and the end are ordinary paragraph marks). An index, a table of authorities and a bibliography
//! are range fields like a table of contents (`FieldStart` ... entries ... `FieldEnd`); any other
//! field becomes one when its result reaches a paragraph mark. A result inside one paragraph stays
//! one inline field object.

use super::{grpprl, read, struct_doc, structured_spec};
use wordcraft_doc::para::InlineObject;

const BOLD: u16 = 0x0835; // sprmCFBold

fn open(text: String) -> wordcraft_doc::Document {
    let text: &'static str = Box::leak(text.into_boxed_str());
    let spec = structured_spec(text, text.chars().count() as u32);
    read(&struct_doc(&spec)).expect("opens")
}

fn paras(doc: &wordcraft_doc::Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

fn texts(doc: &wordcraft_doc::Document) -> Vec<String> {
    paras(doc).iter().map(|p| p.text.clone()).collect()
}

/// The start and end markers of the document's range fields, in order, as (is start, code).
fn markers(doc: &wordcraft_doc::Document) -> Vec<(bool, String)> {
    paras(doc)
        .iter()
        .flat_map(|p| &p.objects)
        .filter_map(|o| match o {
            InlineObject::FieldStart { instr, .. } => Some((true, instr.clone())),
            InlineObject::FieldEnd => Some((false, String::new())),
            _ => None,
        })
        .collect()
}

fn inline_fields(doc: &wordcraft_doc::Document) -> usize {
    paras(doc).iter().flat_map(|p| &p.objects).filter(|o| matches!(o, InlineObject::Field { .. })).count()
}

#[test]
fn index_entries_stay_paragraphs() {
    let doc = open("\u{13} INDEX \\c \"2\" \u{14}Apple, 3\rBorax, 10\u{15}\rBody\r".to_string());
    assert_eq!(texts(&doc), ["\u{FFFC}Apple, 3", "Borax, 10\u{FFFC}", "Body"]);
    assert_eq!(markers(&doc), [(true, r#"INDEX \c "2""#.to_string()), (false, String::new())]);
    assert_eq!(inline_fields(&doc), 0);
    let ps = paras(&doc);
    assert!(matches!(ps.first().and_then(|p| p.objects.first()), Some(InlineObject::FieldStart { .. })));
    assert!(matches!(ps.get(1).and_then(|p| p.objects.last()), Some(InlineObject::FieldEnd)));
}

#[test]
fn authorities_bibliography_and_any_letter_case_are_range_fields_too() {
    for code in [r#"TOA \h \c "1" \p"#, "BIBLIOGRAPHY", "index \\e \"\t\"", "Index"] {
        let doc = open(format!("\u{13} {code} \u{14}One, 1\rTwo, 2\u{15}\rBody\r"));
        assert_eq!(texts(&doc), ["\u{FFFC}One, 1", "Two, 2\u{FFFC}", "Body"], "{code}");
        assert_eq!(markers(&doc), [(true, code.to_string()), (false, String::new())], "{code}");
        assert_eq!(inline_fields(&doc), 0, "{code}");
    }
}

#[test]
fn an_index_with_no_result_is_a_whole_range_field() {
    for text in ["\u{13}INDEX\u{15}\r", "\u{13}INDEX\u{14}\u{15}\r"] {
        let doc = open(text.to_string());
        let ps = paras(&doc);
        assert_eq!(ps.len(), 1, "{text:?}");
        let objs = ps.first().map(|p| p.objects.as_slice()).unwrap_or_default();
        assert!(matches!(objs, [InlineObject::FieldStart { .. }, InlineObject::FieldEnd]), "{text:?}: {objs:?}");
    }
}

#[test]
fn an_index_left_open_at_the_story_end_is_closed() {
    let doc = open("\u{13}INDEX \u{14}Apple\rBorax\r".to_string());
    let m = markers(&doc);
    assert_eq!(m.iter().filter(|(start, _)| *start).count(), 1, "{m:?}");
    assert_eq!(m.iter().filter(|(start, _)| !*start).count(), 1, "{m:?}");
    assert_eq!(m.last().map(|(start, _)| *start), Some(false), "the end comes last: {m:?}");
    let all: String = texts(&doc).concat();
    assert!(all.contains("Apple") && all.contains("Borax"), "{all:?}");
}

#[test]
fn any_field_becomes_a_range_at_a_paragraph_mark_in_its_result() {
    let doc = open("\u{13}INCLUDETEXT \"a.doc\" \u{14}One\rTwo\u{15}\rBody\r".to_string());
    assert_eq!(texts(&doc), ["\u{FFFC}One", "Two\u{FFFC}", "Body"]);
    assert_eq!(markers(&doc), [(true, r#"INCLUDETEXT "a.doc""#.to_string()), (false, String::new())]);
    assert_eq!(inline_fields(&doc), 0);
}

#[test]
fn text_around_a_field_over_two_paragraphs_stays_in_place() {
    let doc = open("See \u{13}QUOTE x\u{14}ab\rcd\u{15} end\r".to_string());
    assert_eq!(texts(&doc), ["See \u{FFFC}ab", "cd\u{FFFC} end"]);
    assert_eq!(markers(&doc), [(true, "QUOTE x".to_string()), (false, String::new())]);
}

#[test]
fn the_result_before_the_paragraph_mark_keeps_its_formatting() {
    // CPs: \x13=0 "QUOTE x"=1..=7 \x14=8 a=9 b=10 \r=11 c=12 d=13 \x15=14 \r=15
    let text = "\u{13}QUOTE x\u{14}ab\rcd\u{15}\r";
    let mut spec = structured_spec(text, text.chars().count() as u32);
    spec.chpx_runs = vec![(0, Vec::new()), (9, grpprl(&[(BOLD, &[0x01])])), (11, Vec::new())];
    let doc = read(&struct_doc(&spec)).expect("opens");
    assert_eq!(texts(&doc), ["\u{FFFC}ab", "cd\u{FFFC}"]);
    let ps = paras(&doc);
    let first = ps.first().expect("first paragraph");
    // "ab" starts after the 3-byte object character.
    let at = first.text.find("ab").expect("result text");
    let mut start = 0;
    let mut bold = None;
    for r in &first.runs {
        if at >= start && at < start + r.len {
            bold = r.props.bold;
        }
        start += r.len;
    }
    assert_eq!(bold, Some(true), "{:?}", first.runs);
    let second = ps.get(1).expect("second paragraph");
    assert!(second.runs.iter().all(|r| r.props.bold != Some(true)), "{:?}", second.runs);
}

#[test]
fn a_result_inside_one_paragraph_stays_one_inline_field() {
    for (text, shown, result) in [("x \u{13}PAGE\u{14}12\u{15} y\r", "x \u{FFFC} y", "12"), ("\u{13}QUOTE x\u{14}abc\u{15}\r", "\u{FFFC}", "abc")] {
        let doc = open(text.to_string());
        assert_eq!(texts(&doc), [shown], "{text:?}");
        assert!(markers(&doc).is_empty(), "{text:?}");
        let ps = paras(&doc);
        let objs = ps.first().map(|p| p.objects.as_slice()).unwrap_or_default();
        assert!(matches!(objs, [InlineObject::Field { result: r, .. }] if r == result), "{text:?}: {objs:?}");
    }
}
