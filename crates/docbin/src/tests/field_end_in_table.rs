//! A field whose end the story's field table ([MS-DOC] 2.8.25 Plcfld) records at a paragraph mark
//! ends there: the character is the field's end mark, not a paragraph break, so the paragraph runs
//! on to the next mark. Only that case is new; every other mismatch between the table and the text
//! is read from the text as before.

use super::{StructSpec, plc, read, struct_doc, structured_spec};
use wordcraft_doc::para::InlineObject;

/// FIB pair of the main text's field table (PlcfFldMom) and of the headers' (PlcfFldHdr).
const MOM: usize = 16;
const HDR: usize = 17;

/// A Plcfld with one entry `(cp, fldch, grffld)` per `entries`, then `limit` as the last CP.
fn plcfld(entries: &[(u32, u8, u8)], limit: u32) -> Vec<u8> {
    let mut cps: Vec<u32> = entries.iter().map(|e| e.0).collect();
    cps.push(limit);
    let data: Vec<u8> = entries.iter().flat_map(|e| [e.1, e.2]).collect();
    plc(&cps, &data)
}

fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

fn len(text: &str) -> u32 {
    text.chars().count() as u32
}

/// The document for main text `text` with field table `pairs` (none when empty).
fn open(text: &str, pairs: Vec<(usize, Vec<u8>)>) -> wordcraft_doc::Document {
    let text = leak(text);
    let spec = StructSpec { extra_pairs: pairs, ..structured_spec(text, len(text)) };
    read(&struct_doc(&spec)).expect("opens")
}

fn body(doc: &wordcraft_doc::Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

fn part_paras(doc: &wordcraft_doc::Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.parts.values().flat_map(|p| p.blocks.iter()).filter_map(|b| b.as_para()).collect()
}

/// The paragraph's inline fields as (code, result, whether its code spans paragraphs).
fn fields(p: &wordcraft_doc::Paragraph) -> Vec<(String, String, bool)> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { instr, result, code_paras, .. } => Some((instr.clone(), result.clone(), code_paras.is_some())),
            _ => None,
        })
        .collect()
}

fn page_field() -> Vec<(String, String, bool)> {
    vec![("PAGE".to_string(), "1".to_string(), false)]
}

/// No range-field markers anywhere in these paragraphs.
fn no_markers(ps: &[&wordcraft_doc::Paragraph]) -> bool {
    ps.iter().flat_map(|p| &p.objects).all(|o| !matches!(o, InlineObject::FieldStart { .. } | InlineObject::FieldEnd))
}

const FOOTER: &str = "03300-\u{13} PAGE \u{14}1\rCAST\r\r";
const BODY: &str = "A\u{13} PAGE \u{14}1\rB\r";

/// Begin, separator and end entries for [`BODY`]-shaped text: the end at `end` (none when `None`).
fn body_table(text: &str, end: Option<u32>) -> Vec<(usize, Vec<u8>)> {
    let mut entries = vec![(1, 0x13, 0x21), (8, 0x14, 0xFF)];
    entries.extend(end.map(|cp| (cp, 0x15, 0x80)));
    vec![(MOM, plcfld(&entries, len(text)))]
}

fn footer_doc(with_end: bool) -> wordcraft_doc::Document {
    let mut entries = vec![(6, 0x13, 0x21), (13, 0x14, 0xFF)];
    if with_end {
        entries.push((15, 0x15, 0x80));
    }
    let spec = StructSpec {
        header_stories: vec![FOOTER],
        extra_pairs: vec![(HDR, plcfld(&entries, len(FOOTER)))],
        ..structured_spec("Body\r", 5)
    };
    read(&struct_doc(&spec)).expect("opens")
}

#[test]
fn row1_a_header_story_field_ended_at_a_paragraph_mark_keeps_one_paragraph() {
    // Without the end entry the result runs over the mark: `CAST` is a paragraph of its own.
    let before = footer_doc(false);
    assert_eq!(part_paras(&before).get(1).map(|p| p.text.as_str()), Some("CAST"));
    let doc = footer_doc(true);
    let ps = part_paras(&doc);
    assert_eq!(ps.len(), 1, "{:?}", ps.iter().map(|p| &p.text).collect::<Vec<_>>());
    let p = ps.first().expect("a paragraph");
    assert_eq!(p.text, "03300-\u{FFFC}CAST");
    assert_eq!(fields(p), page_field());
}

#[test]
fn row2_a_main_text_field_ended_at_a_paragraph_mark_joins_the_paragraphs() {
    let before = open(BODY, body_table(BODY, None));
    let doc = open(BODY, body_table(BODY, Some(10)));
    let ps = body(&doc);
    assert_eq!(body(&before).get(1).map(|p| p.text.as_str()), Some("B"));
    assert!(ps.len() < body(&before).len());
    assert_eq!(ps.len(), 1);
    let p = ps.first().expect("a paragraph");
    assert_eq!(p.text, "A\u{FFFC}B");
    assert_eq!(fields(p), page_field());
    assert!(no_markers(&ps));
}

/// The body paragraphs as the model holds them, for comparing with today's reading.
fn model(doc: &wordcraft_doc::Document) -> Vec<String> {
    body(doc).iter().map(|p| format!("{:?} {:?} {:?}", p.text, p.runs, p.objects)).collect()
}

#[test]
fn row3_without_an_end_entry_the_result_runs_over_the_mark() {
    let doc = open(BODY, body_table(BODY, None));
    assert_eq!(model(&doc), model(&open(BODY, vec![])));
    assert!(body(&doc).len() >= 2);
}

#[test]
fn row4_an_end_entry_at_a_field_end_character_is_read_as_before() {
    let text = "A\u{13} PAGE \u{14}1\u{15}B\r";
    let doc = open(text, body_table(text, Some(10)));
    assert_eq!(model(&doc), model(&open(text, vec![])));
    let ps = body(&doc);
    assert_eq!(ps.len(), 1);
    let p = ps.first().expect("a paragraph");
    assert_eq!(p.text, "A\u{FFFC}B");
    assert_eq!(fields(p), page_field());
}

#[test]
fn row5_an_end_entry_at_a_letter_changes_nothing() {
    let doc = open(BODY, body_table(BODY, Some(11)));
    assert_eq!(model(&doc), model(&open(BODY, body_table(BODY, None))));
}

#[test]
fn row6_the_joined_paragraph_holds_the_whole_field_for_a_docx_save() {
    // An inline field object is written to DOCX as `w:fldChar` begin / separate / end runs inside
    // its paragraph, so the save keeps all three in the one footer paragraph (and reads back as
    // row 1); range-field markers would split them across paragraphs.
    let doc = footer_doc(true);
    let ps = part_paras(&doc);
    assert_eq!(ps.len(), 1);
    assert!(no_markers(&ps));
    let p = ps.first().expect("a paragraph");
    assert_eq!(fields(p), page_field());
    let at = p.text.chars().position(|c| c == '\u{FFFC}');
    assert_eq!(at, Some(6));
    assert!(p.text.ends_with("CAST"));
}

#[test]
fn no_open_field_leaves_the_mark_a_paragraph_mark() {
    let text = "A\rB\r";
    let doc = open(text, vec![(MOM, plcfld(&[(1, 0x15, 0x80)], len(text)))]);
    assert_eq!(model(&doc), model(&open(text, vec![])));
    assert_eq!(body(&doc).len(), 2);
}

