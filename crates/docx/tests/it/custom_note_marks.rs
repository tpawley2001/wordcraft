//! A footnote with a custom reference mark, as the Word 97-2003 reader makes it (`NoteRef` with
//! `custom` set, the note's own text starting with the mark as plain text), saves as
//! `w:footnoteReference w:customMarkFollows="1"` followed by the mark in the same run
//! (ECMA-376 Part 1 §17.11.14) and reads back the same.
use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use wordcraft_doc::para::NoteKind;
use wordcraft_doc::{Document, InlineObject, PartKind, Pos, para_block};

fn unzip(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            parts.insert(f.name().to_string(), s);
        }
    }
    parts
}

/// `See* here` with a `*` footnote whose text is `* Note body`.
fn custom_note_doc() -> Document {
    let mut d = Document::from_text("See here");
    let id = d.add_part(PartKind::Footnote, vec![para_block(wordcraft_doc::Paragraph::with_text("* Note body", Default::default()))]);
    d.insert_object(&Pos::body(0, 3), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: "*".into() }, &Default::default()).unwrap();
    d
}

/// The body paragraph's text and note references, and each referenced note's text.
fn notes_view(d: &Document) -> (String, Vec<(NoteKind, String, String)>) {
    let p = d.body.first().and_then(|b| b.as_para()).unwrap();
    let notes = p
        .objects
        .iter()
        .filter_map(|o| match o {
            InlineObject::NoteRef { kind, id, custom } => {
                let text = d.parts.get(id)?.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<Vec<_>>().join("\n");
                Some((*kind, custom.clone(), text))
            }
            _ => None,
        })
        .collect();
    (p.text.clone(), notes)
}

#[test]
fn a_custom_mark_footnote_saves_and_reads_back() {
    let d = custom_note_doc();
    let bytes = wordcraft_docx::write(&d).unwrap();
    let parts = unzip(&bytes);
    let footnotes = parts.get("word/footnotes.xml").expect("footnotes part");
    assert!(footnotes.contains("Note body"), "{footnotes}");
    // The note's text keeps its own `*`; no automatic number is added to it.
    assert!(!footnotes.contains("<w:footnoteRef/>"), "{footnotes}");
    let document = parts.get("word/document.xml").unwrap();
    let at = document.find(r#"w:customMarkFollows="1""#).expect("custom mark reference");
    let after = document.get(at..).unwrap();
    let t = after.find("<w:t").expect("mark text");
    assert!(after.get(t..).unwrap().starts_with(r#"<w:t xml:space="preserve">*</w:t>"#), "{after}");
    // The mark is in the reference's run.
    assert!(!after.get(..t).unwrap().contains("</w:r>"), "{after}");

    let again = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(notes_view(&again), notes_view(&d));
    assert_eq!(notes_view(&d), ("See\u{FFFC} here".into(), vec![(NoteKind::Footnote, "*".into(), "* Note body".into())]));
}
