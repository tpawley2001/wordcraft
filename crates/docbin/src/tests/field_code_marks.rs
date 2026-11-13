//! Paragraph marks inside a field's code: Word hides them with the code, so the paragraphs from
//! the field's begin to its separator or end show as one. The reader joins them into one
//! paragraph and keeps the marks on the field (`CodeParas`), the model the DOCX reader builds for
//! a code split over several `w:p`, so both formats lay out and save alike.

use wordcraft_doc::para::{CodeParas, InlineObject};
use wordcraft_doc::{Document, Paragraph};

use super::{RichSpec, grpprl, read, rich_doc};

const DYA_AFTER: u16 = 0xA414; // sprmPDyaAfter, twips

/// `text` with one paragraph entry per paragraph start, the n-th with `after[n]` twips after
/// (one entry for the whole text when `after` is empty).
fn open(text: &'static str, after: &[i16]) -> Document {
    let mut starts = vec![0u32];
    for (i, c) in text.chars().enumerate() {
        if !after.is_empty() && c == '\r' && i + 1 < text.chars().count() {
            starts.push(i as u32 + 1);
        }
    }
    let papx_entries =
        starts.iter().enumerate().map(|(i, &cp)| (cp, 0, after.get(i).map(|a| grpprl(&[(DYA_AFTER, &a.to_le_bytes())])).unwrap_or_default())).collect();
    let spec = RichSpec { text, styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())], fonts: vec!["Times New Roman"], chpx_runs: vec![(0, Vec::new())], papx_entries };
    read(&rich_doc(&spec)).expect("opens")
}

fn paras(doc: &Document) -> Vec<&Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

fn texts(doc: &Document) -> Vec<String> {
    paras(doc).iter().map(|p| p.plain_text()).collect()
}

/// The fields of `p`: (code, result, the marks inside the code).
fn fields(p: &Paragraph) -> Vec<(String, String, Option<CodeParas>)> {
    p.objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::Field { instr, result, code_paras, .. } => Some((instr.clone(), result.clone(), code_paras.as_deref().cloned())),
            _ => None,
        })
        .collect()
}

#[test]
fn paragraph_marks_in_a_code_join_the_paragraphs() {
    let doc = open("Head \u{13}tc \"A\r\rB\"\u{15}\rNext\r", &[120, 60, 240, 0]);
    assert_eq!(texts(&doc), ["Head ", "Next"]);
    let p = paras(&doc)[0];
    let f = fields(p);
    assert_eq!(f.len(), 1);
    let (code, result, marks) = &f[0];
    assert_eq!((code.as_str(), result.as_str()), ("tc \"A B\"", ""));
    let marks = marks.as_ref().expect("the marks are kept");
    assert_eq!(marks.marks.iter().map(|m| m.at).collect::<Vec<_>>(), [6, 6]);
    assert_eq!(marks.parts(code), Some(vec!["tc \"A ", "", "B\""]));
    // The first paragraph's space after is kept on the field; the joined paragraph has the last one's.
    assert_eq!(marks.first_after, Some(6.0));
    assert_eq!(marks.marks[0].props.space_after, Some(3.0));
    assert_eq!(marks.marks[1].props.space_after, Some(12.0));
    assert_eq!(p.props.space_after, Some(12.0));
}

#[test]
fn a_mark_in_a_field_nested_in_the_code_joins_too() {
    let doc = open("Head \u{13}IF \u{13}PAGE\r\u{14}1\u{15} = 1 \"a\"\r\"b\"\u{14}a\u{15}\rNext\r", &[]);
    assert_eq!(texts(&doc), ["Head a", "Next"]);
    let f = fields(paras(&doc)[0]);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].0, "IF PAGE 1 = 1 \"a\" \"b\"");
    assert_eq!(f[0].2.as_ref().map(|m| m.marks.len()), Some(2));
}

#[test]
fn a_mark_in_a_field_result_still_ends_its_paragraph() {
    let doc = open("Head \u{13}QUOTE x\u{14}A\rB\u{15}\rNext\r", &[]);
    assert_eq!(paras(&doc).len(), 3, "{:?}", texts(&doc));
    assert!(paras(&doc).iter().all(|p| fields(p).iter().all(|f| f.2.is_none())));
}

#[test]
fn a_code_without_marks_is_unchanged() {
    let doc = open("Head \u{13}tc \"A B\"\u{15}\rNext\r", &[120, 0]);
    assert_eq!(texts(&doc), ["Head ", "Next"]);
    let f = fields(paras(&doc)[0]);
    assert_eq!((f[0].0.as_str(), f[0].2.is_none()), ("tc \"A B\"", true));
    assert_eq!(paras(&doc)[0].props.space_after, Some(6.0));
}

#[test]
fn a_code_that_never_ends_joins_at_most_64_marks() {
    let text: &'static str = format!("Head \u{13}tc \"A{}", "\r".repeat(200)).leak();
    let doc = open(text, &[]);
    let ps = paras(&doc);
    // 64 marks joined into the first paragraph; the rest stay paragraphs.
    assert_eq!(ps.len(), 200 - 64, "{}", ps.len());
}

