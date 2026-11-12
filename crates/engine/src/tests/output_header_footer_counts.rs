//! Output (PDF export, printing, page images) shows the count, revision and property fields in
//! headers and footers with their current values and keeps the body's as stored. Expected results
//! read from Microsoft 365 Word's PDFs of probe documents: body text `marked text`, core title
//! `Real Title`, revision 7, and NUMWORDS, NUMCHARS, REVNUM, DOCPROPERTY Title and FILESIZE fields
//! in the header, the footer or the body, every stored result `STALE`.

use std::sync::Arc;

use wordcraft_doc::{Block, CharProps, Document, InlineObject, Paragraph, PartKind, StoryRef};

use crate::cmd::references::output_fields;

const CODES: [&str; 4] = ["NUMWORDS", "NUMCHARS", "REVNUM", "DOCPROPERTY Title"];
/// What Word's PDF shows for `CODES` in a header or footer of the probe.
const FRESH: [&str; 4] = ["2", "10", "7", "Real Title"];

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
}

/// `CODES` as fields, each showing `result(i)`.
fn fields(result: impl Fn(usize) -> &'static str, locked: bool) -> Vec<InlineObject> {
    CODES.iter().enumerate().map(|(i, code)| field(code, result(i), locked)).collect()
}

/// A new part of `kind` holding one paragraph with `fields`.
fn part(doc: &mut Document, kind: PartKind, fields: Vec<InlineObject>) -> StoryRef {
    let mut p = Paragraph::new();
    for f in fields {
        let off = p.len();
        p.insert_object(off, f, &CharProps::default()).unwrap();
    }
    StoryRef::Part(doc.add_part(kind, vec![Arc::new(Block::Para(p))]))
}

/// The results of the fields in `story`, in order.
fn results(doc: &Document, story: StoryRef) -> Vec<String> {
    let mut out = Vec::new();
    for path in doc.para_paths(story) {
        for o in &doc.para(story, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                out.push(result.clone());
            }
        }
    }
    out
}

/// The Word probe without its fields.
fn probe() -> Document {
    let mut doc = Document::from_text("marked text");
    doc.core.title = "Real Title".into();
    doc.core.revision = 7;
    doc
}

/// Word's PDF of the header probe and of the footer probe: `2`, `10`, `7`, `Real Title`.
#[test]
fn output_shows_header_and_footer_counts_and_properties() {
    let mut doc = probe();
    let hdr = part(&mut doc, PartKind::Header, fields(|_| "STALE", false));
    let even = part(&mut doc, PartKind::Header, fields(|_| "STALE", false));
    let ftr = part(&mut doc, PartKind::Footer, fields(|_| "STALE", false));
    let out = output_fields(&doc).expect("the header and footer fields change");
    assert_eq!(results(&out, hdr), FRESH);
    assert_eq!(results(&out, even), FRESH, "every header part, not only the first");
    assert_eq!(results(&out, ftr), FRESH);
}

/// Word's PDF of the body probe keeps every one of these fields as stored.
#[test]
fn output_keeps_the_body_fields_as_stored() {
    let mut doc = probe();
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let para = doc.para_mut(StoryRef::Body, &path).unwrap();
    for f in fields(|_| "STALE", false).into_iter().rev() {
        para.insert_object(0, f, &CharProps::default()).unwrap();
    }
    assert!(output_fields(&doc).is_none(), "nothing but body fields: no copy");
    part(&mut doc, PartKind::Header, vec![field("REVNUM", "STALE", false)]);
    let out = output_fields(&doc).expect("the header field changes");
    assert_eq!(results(&out, StoryRef::Body), ["STALE"; 4]);
}

/// Footnotes and endnotes keep theirs too, as for the other header and footer fields.
#[test]
fn output_keeps_note_fields_as_stored() {
    let mut doc = probe();
    let foot = part(&mut doc, PartKind::Footnote, fields(|_| "STALE", false));
    let end = part(&mut doc, PartKind::Endnote, fields(|_| "STALE", false));
    assert!(output_fields(&doc).is_none());
    part(&mut doc, PartKind::Footer, vec![field("REVNUM", "STALE", false)]);
    let out = output_fields(&doc).expect("the footer field changes");
    assert_eq!(results(&out, foot), ["STALE"; 4]);
    assert_eq!(results(&out, end), ["STALE"; 4]);
}

/// Word's PDF shows a header FILESIZE as stored.
#[test]
fn a_header_filesize_keeps_its_stored_result() {
    let mut doc = probe();
    part(&mut doc, PartKind::Header, vec![field("FILESIZE", "STALE", false)]);
    part(&mut doc, PartKind::Footer, vec![field("FILESIZE", "STALE", false)]);
    assert!(output_fields(&doc).is_none());
}

/// Output lays out a copy: the document itself keeps what it stored.
#[test]
fn the_document_keeps_its_stored_results() {
    let mut doc = probe();
    let hdr = part(&mut doc, PartKind::Header, fields(|_| "STALE", false));
    let out = output_fields(&doc).expect("the header fields change");
    assert_eq!(results(&out, hdr), FRESH);
    assert_eq!(results(&doc, hdr), ["STALE"; 4]);
}

/// A locked field is never updated; with nothing else to refresh, output copies nothing.
#[test]
fn locked_header_fields_keep_their_results() {
    let mut doc = probe();
    part(&mut doc, PartKind::Header, fields(|_| "STALE", true));
    assert!(output_fields(&doc).is_none());
}

/// Header and footer fields that already show their current values need no copy.
#[test]
fn current_header_fields_copy_nothing() {
    let mut doc = probe();
    part(&mut doc, PartKind::Header, fields(|i| FRESH[i], false));
    part(&mut doc, PartKind::Footer, fields(|i| FRESH[i], false));
    assert!(output_fields(&doc).is_none());
}
