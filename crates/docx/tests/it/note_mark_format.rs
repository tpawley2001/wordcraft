//! A note reference with a character style and a size of its own (as the Word 97-2003 reader
//! gives it) saves both on its run (`w:rStyle`, `w:sz`, ECMA-376 Part 1 §17.3.2), and no
//! superscript of its own: the style already raises it.
use std::io::{Cursor, Read};

use wordcraft_doc::para::NoteKind;
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, InlineObject, PartKind, Pos, para_block};

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The `w:r` element holding the footnote reference.
fn reference_run(xml: &str) -> &str {
    let at = xml.find("<w:footnoteReference").expect("a footnote reference");
    let start = xml[..at].rfind("<w:r>").or_else(|| xml[..at].rfind("<w:r ")).expect("its run");
    let end = at + xml[at..].find("</w:r>").expect("run end");
    &xml[start..end]
}

#[test]
fn a_styled_10pt_reference_saves_its_style_and_size() {
    let mut d = Document::from_text("Text more");
    let id = d.add_part(PartKind::Footnote, vec![para_block(wordcraft_doc::Paragraph::with_text("Note", Default::default()))]);
    let mark = CharProps { style: Some("FootnoteReference".into()), size: Some(10.0), ..Default::default() };
    d.insert_object(&Pos::body(0, 4), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &mark).unwrap();
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    let run = reference_run(&xml);
    assert!(run.contains(r#"<w:rStyle w:val="FootnoteReference"/>"#), "{run}");
    assert!(run.contains(r#"<w:sz w:val="20"/>"#), "{run}");
    assert!(!run.contains("w:vertAlign"), "the style's superscript is not written again: {run}");
}
