//! Output (PDF export, printing, page images) shows an unlocked SAVEDATE field as the time of
//! output, as Microsoft 365 Word's PDF export does: probe documents with `SAVEDATE \@ "yyyy-MM-dd"`
//! (stored `CACHED`), `SAVEDATE \@ "yyyy-MM-dd HH:mm"` (stored `0000`) and `SAVEDATE \@ "yyyy"`
//! (stored `0000` / `2030`) printed the export date and time, not the stored result, the
//! last-saved property (2003) or the file's time on disk (2001). The document keeps its stored result.

use std::sync::Arc;

use chrono::{Datelike, Local};
use wordcraft_doc::{Block, CharProps, Document, InlineObject, Paragraph, PartKind, StoryRef};

use crate::cmd::references::output_fields;

const SAVEDATE: &str = r#"SAVEDATE \@ "yyyy""#;

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
fn body_savedate_shows_the_output_year() {
    let doc = body_with(field(SAVEDATE, "2030", false));
    let out = output_fields(&doc).expect("the stale SAVEDATE changes on output");
    assert_eq!(results(&out, StoryRef::Body), [this_year()]);
    assert_eq!(results(&doc, StoryRef::Body), ["2030"], "the document keeps its stored result");
}

#[test]
fn locked_savedate_keeps_its_result() {
    let doc = body_with(field(SAVEDATE, "2030", true));
    let out = output_fields(&doc).unwrap_or_else(|| doc.clone());
    assert_eq!(results(&out, StoryRef::Body), ["2030"]);
}

#[test]
fn header_savedate_shows_the_output_year() {
    let mut doc = Document::from_text("Body text");
    let mut p = Paragraph::new();
    p.insert_object(0, field(SAVEDATE, "2030", false), &CharProps::default()).unwrap();
    let hdr = StoryRef::Part(doc.add_part(PartKind::Header, vec![Arc::new(Block::Para(p))]));
    let out = output_fields(&doc).expect("the header's stale SAVEDATE changes on output");
    assert_eq!(results(&out, hdr), [this_year()]);
    assert_eq!(results(&doc, hdr), ["2030"]);
}

#[test]
fn current_savedate_copies_nothing() {
    let doc = body_with(field(SAVEDATE, &this_year(), false));
    assert!(output_fields(&doc).is_none());
}

#[test]
fn savedate_without_a_picture_shows_todays_date() {
    let doc = body_with(field("SAVEDATE", "1/1/2001", false));
    let out = output_fields(&doc).expect("the stale SAVEDATE changes on output");
    let today = Local::now().date_naive();
    let want = format!("{}/{}/{}", today.month(), today.day(), today.year());
    assert_eq!(results(&out, StoryRef::Body), [want]);
}

#[test]
fn savedate_and_printdate_both_show_the_output_year() {
    let mut doc = body_with(field(SAVEDATE, "0000", false));
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let para = doc.para_mut(StoryRef::Body, &path).unwrap();
    let end = para.len();
    para.insert_object(end, field(r#"PRINTDATE \@ "yyyy""#, "2030", false), &CharProps::default()).unwrap();
    let out = output_fields(&doc).expect("both stale fields change on output");
    assert_eq!(results(&out, StoryRef::Body), [this_year(), this_year()]);
}
