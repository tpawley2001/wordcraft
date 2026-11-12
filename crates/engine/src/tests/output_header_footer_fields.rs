//! Output (PDF export, printing, page images) shows the fields in headers and footers with their
//! current values and keeps the body's and the notes' as stored. Expected results read from
//! Microsoft 365 Word's PDF of a probe document: core title `Ada`, and in the body, the header,
//! the footer, a footnote and an endnote a TITLE field and a `QUOTE "fresh<story>"` field, every
//! stored result stale.

use std::sync::Arc;

use serde_json::json;
use wordcraft_doc::{Block, CharProps, Document, InlineObject, Paragraph, PartKind, Pos, StoryRef};

use crate::cmd::references::output_fields;
use crate::{Selection, Session};

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
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

/// Puts `fields` at the start of the body's first paragraph, in order.
fn body_fields(doc: &mut Document, fields: Vec<InlineObject>) {
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let para = doc.para_mut(StoryRef::Body, &path).unwrap();
    for f in fields.into_iter().rev() {
        para.insert_object(0, f, &CharProps::default()).unwrap();
    }
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

/// The TITLE and QUOTE pair of the Word probe for one story, stale.
fn probe_pair(story: &str) -> Vec<InlineObject> {
    vec![field("TITLE", &format!("staleT{story}"), false), field(&format!("QUOTE \"fresh{story}\""), &format!("staleQ{story}"), false)]
}

/// The Word probe: (document, header, even-page header, footer, footnote, endnote).
fn probe() -> (Document, [StoryRef; 5]) {
    let mut doc = Document::from_text("Body text");
    doc.core.title = "Ada".into();
    body_fields(&mut doc, probe_pair("BODY"));
    let hdr = part(&mut doc, PartKind::Header, probe_pair("HDR"));
    let even = part(&mut doc, PartKind::Header, probe_pair("EVEN"));
    let ftr = part(&mut doc, PartKind::Footer, probe_pair("FTR"));
    let foot = part(&mut doc, PartKind::Footnote, probe_pair("FN"));
    let end = part(&mut doc, PartKind::Endnote, probe_pair("EN"));
    (doc, [hdr, even, ftr, foot, end])
}

/// Word's PDF of the probe: header `Ada` / `freshHDR`, footer `Ada` / `freshFTR`.
#[test]
fn output_shows_header_and_footer_fields_with_their_current_values() {
    let (doc, [hdr, even, ftr, ..]) = probe();
    let out = output_fields(&doc).expect("the header and footer fields change");
    assert_eq!(results(&out, hdr), ["Ada", "freshHDR"]);
    assert_eq!(results(&out, even), ["Ada", "freshEVEN"], "every header part, not only the first");
    assert_eq!(results(&out, ftr), ["Ada", "freshFTR"]);
}

/// Word's PDF of the probe keeps the body's, the footnote's and the endnote's stored results.
#[test]
fn output_keeps_body_and_note_fields_as_stored() {
    let (doc, [_, _, _, foot, end]) = probe();
    let out = output_fields(&doc).expect("the header and footer fields change");
    assert_eq!(results(&out, StoryRef::Body), ["staleTBODY", "staleQBODY"]);
    assert_eq!(results(&out, foot), ["staleTFN", "staleQFN"]);
    assert_eq!(results(&out, end), ["staleTEN", "staleQEN"]);
}

/// Output lays out a copy: the document itself keeps what it stored.
#[test]
fn the_document_keeps_its_stored_header_results() {
    let (doc, [hdr, _, ftr, ..]) = probe();
    let _ = output_fields(&doc);
    assert_eq!(results(&doc, hdr), ["staleTHDR", "staleQHDR"]);
    assert_eq!(results(&doc, ftr), ["staleTFTR", "staleQFTR"]);
}

/// A locked field is never updated; with nothing else to refresh, output copies nothing.
#[test]
fn locked_header_fields_keep_their_results() {
    let mut doc = Document::from_text("Body text");
    doc.core.title = "Ada".into();
    part(&mut doc, PartKind::Header, vec![field("TITLE", "staleT", true), field("QUOTE \"fresh\"", "staleQ", true)]);
    assert!(output_fields(&doc).is_none());
}

/// Header and footer fields that already show their current values need no copy.
#[test]
fn current_header_fields_copy_nothing() {
    let mut doc = Document::from_text("Body text");
    doc.core.title = "Ada".into();
    part(&mut doc, PartKind::Header, vec![field("TITLE", "Ada", false), field("QUOTE \"fresh\"", "fresh", false)]);
    part(&mut doc, PartKind::Footer, vec![field("TITLE", "Ada", false)]);
    assert!(output_fields(&doc).is_none());
}

/// A DATE field in a header shows the current date on output, as Update Field in that header
/// would store it (a real Word 97-2003 file: Word's PDF has today's date in the header); the
/// same field in the body keeps its stored result.
#[test]
fn header_date_fields_show_the_current_date() {
    let date = r#"DATE \@ "yyyy""#;
    let mut doc = Document::from_text("Body text");
    body_fields(&mut doc, vec![field(date, "stale", false)]);
    let hdr = part(&mut doc, PartKind::Header, vec![field(date, "stale", false)]);
    let ftr = part(&mut doc, PartKind::Footer, vec![field(date, "stale", false)]);

    // What F9 with the caret in the header stores: the year, four digits.
    let mut s = Session::new(doc.clone());
    let path = s.doc.para_paths(hdr).remove(0);
    s.sel = Selection::caret(Pos::new(hdr, path, 0));
    s.run("references.updateFields", &json!({})).unwrap();
    let want = results(&s.doc, hdr).remove(0);
    assert!(want.len() == 4 && want.chars().all(|c| c.is_ascii_digit()), "{want}");

    let out = output_fields(&doc).expect("the header and footer dates change");
    assert_eq!(results(&out, hdr), [want.as_str()]);
    assert_eq!(results(&out, ftr), [want.as_str()]);
    assert_eq!(results(&out, StoryRef::Body), ["stale"]);
}
