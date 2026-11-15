//! Output (PDF export, printing, page images) shows an unlocked PRINTDATE field as the time of
//! output: printing or exporting is itself a print. Measured with Microsoft 365 Word's PDF export
//! of three probe documents whose body held `PRINTDATE \@ "yyyy"` with a stored result of `2030`
//! and no last-printed date: Word printed the export year. The document keeps its stored result.

use std::sync::Arc;

use chrono::Datelike;
use wordcraft_doc::{Block, CharProps, Document, InlineObject, Paragraph, PartKind, StoryRef};

use crate::cmd::references::output_fields;

const PRINTDATE: &str = r#"PRINTDATE \@ "yyyy""#;

fn field(instr: &str, result: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked, dirty: false, code: false, form: String::new(), code_paras: None }
}

fn body_with(f: InlineObject) -> Document {
    let mut doc = Document::from_text("Printed: ");
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let para = doc.para_mut(StoryRef::Body, &path).unwrap();
    let end = para.len();
    para.insert_object(end, f, &CharProps::default()).unwrap();
    doc
}

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

fn this_year() -> String {
    chrono::Local::now().year().to_string()
}

#[test]
fn body_printdate_shows_the_output_year() {
    let doc = body_with(field(PRINTDATE, "2030", false));
    let out = output_fields(&doc).expect("the stale PRINTDATE changes on output");
    assert_eq!(results(&out, StoryRef::Body), [this_year()]);
    assert_eq!(results(&doc, StoryRef::Body), ["2030"], "the document keeps its stored result");
}

#[test]
fn locked_printdate_keeps_its_result() {
    let doc = body_with(field(PRINTDATE, "2030", true));
    let out = output_fields(&doc).unwrap_or_else(|| doc.clone());
    assert_eq!(results(&out, StoryRef::Body), ["2030"]);
}

#[test]
fn header_printdate_shows_the_output_year() {
    let mut doc = Document::from_text("Body text");
    let mut p = Paragraph::new();
    p.insert_object(0, field(PRINTDATE, "2030", false), &CharProps::default()).unwrap();
    let hdr = StoryRef::Part(doc.add_part(PartKind::Header, vec![Arc::new(Block::Para(p))]));
    let out = output_fields(&doc).expect("the header's stale PRINTDATE changes on output");
    assert_eq!(results(&out, hdr), [this_year()]);
    assert_eq!(results(&doc, hdr), ["2030"]);
}

#[test]
fn current_printdate_copies_nothing() {
    let doc = body_with(field(PRINTDATE, &this_year(), false));
    assert!(output_fields(&doc).is_none());
}
