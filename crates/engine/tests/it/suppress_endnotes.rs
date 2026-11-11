//! Page Setup › Layout › Suppress endnotes: `layout.suppressEndnotes` sets the section's
//! `w:noEndnote` (ECMA-376 Part 1 §17.11.16). On writes the element, off removes it.

use std::io::Read;

use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_doc::section::NotePos;
use wordcraft_engine::{Session, io};

const COMMAND: &str = "layout.suppressEndnotes";

fn session(text: &str) -> Session {
    let mut s = Session::new(Document::new());
    s.run("document.setText", &json!({"text": text})).unwrap();
    s
}

/// The caret section's Suppress endnotes.
fn no_endnote(s: &Session) -> Option<bool> {
    wordcraft_engine::cmd::page::sect(s).no_endnote
}

/// The inside of each `w:sectPr` of the saved DOCX, in document order.
fn saved_sections(s: &Session) -> Vec<String> {
    let bytes = wordcraft_docx::write(&s.doc).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml.split("<w:sectPr").skip(1).map(|x| x.split("</w:sectPr>").next().unwrap_or_default().to_string()).collect()
}

#[test]
fn on_writes_the_element_and_off_removes_it() {
    let mut s = session("Text");
    assert_eq!(no_endnote(&s), None);
    assert_eq!(saved_sections(&s).len(), 1);
    assert!(!saved_sections(&s)[0].contains("w:noEndnote"), "a new document has none");

    s.run(COMMAND, &json!({"value": true})).unwrap();
    assert_eq!(no_endnote(&s), Some(true));
    let saved = saved_sections(&s);
    assert_eq!(saved.len(), 1);
    assert!(saved[0].contains("<w:noEndnote/>"), "on is the bare element: {}", saved[0]);
    // It reads back on.
    let back = wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap()).unwrap();
    assert_eq!(back.last_section.no_endnote, Some(true));

    s.run(COMMAND, &json!({"value": false})).unwrap();
    assert_eq!(no_endnote(&s), None, "off leaves no element, as Word does after unticking");
    assert!(!saved_sections(&s)[0].contains("w:noEndnote"));
}

#[test]
fn off_clears_an_explicit_false() {
    let mut s = session("Text");
    s.doc.last_section.no_endnote = Some(false);
    assert!(saved_sections(&s)[0].contains("w:noEndnote"), "the explicit off is in the file before");
    s.run(COMMAND, &json!({"value": false})).unwrap();
    assert_eq!(no_endnote(&s), None);
    assert!(!saved_sections(&s)[0].contains("w:noEndnote"));
}

#[test]
fn without_a_value_it_toggles() {
    let mut s = session("Text");
    s.run(COMMAND, &json!({})).unwrap();
    assert_eq!(no_endnote(&s), Some(true));
    s.run(COMMAND, &json!({})).unwrap();
    assert_eq!(no_endnote(&s), None);
    // An explicit off counts as off: toggling turns it on.
    s.doc.last_section.no_endnote = Some(false);
    s.run(COMMAND, &json!({})).unwrap();
    assert_eq!(no_endnote(&s), Some(true));
}

#[test]
fn it_is_one_undo_step() {
    let mut s = session("Text");
    let depth = s.undo_depth();
    s.run(COMMAND, &json!({"value": true})).unwrap();
    assert_eq!(s.undo_depth(), depth + 1);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(no_endnote(&s), None);
    s.run("edit.redo", &json!({})).unwrap();
    assert_eq!(no_endnote(&s), Some(true));
}

/// The command doesn't depend on where the endnotes go, and never moves them.
#[test]
fn it_leaves_the_endnote_placement_alone() {
    for pos in ["docEnd", "sectEnd"] {
        let mut s = session("Text");
        s.run("references.noteOptions", &json!({"kind": "endnote", "pos": pos})).unwrap();
        let want = if pos == "sectEnd" { NotePos::SectEnd } else { NotePos::DocEnd };
        let placed = |s: &Session| s.doc.note_options(&wordcraft_engine::cmd::page::sect(s), true).pos;
        assert_eq!(placed(&s), want, "{pos}: set up");
        s.run(COMMAND, &json!({"value": true})).unwrap();
        assert_eq!(no_endnote(&s), Some(true), "{pos}");
        assert_eq!(placed(&s), want, "{pos}: placement unchanged");
    }
}

/// Two sections, the caret in the first: only that section changes, in the model and in the file.
#[test]
fn only_the_carets_section_changes() {
    const RTF: &str = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Times New Roman;}}
\sectd\pard\plain First section\par\sect
\sectd\pard\plain Second section\par}";
    let mut s = Session::new(io::open_bytes("two.rtf", RTF.as_bytes()).unwrap());
    assert_eq!(s.doc.sections().len(), 2);
    // The caret starts in the first section: a section command lands there.
    s.run("layout.differentFirstPage", &json!({"value": true})).unwrap();
    assert!(s.doc.sections()[0].1.title_page && !s.doc.last_section.title_page, "the caret is in section 1");

    s.run(COMMAND, &json!({"value": true})).unwrap();
    assert_eq!(s.doc.sections()[0].1.no_endnote, Some(true));
    assert_ne!(s.doc.last_section.no_endnote, Some(true), "section 2 is untouched");
    let saved = saved_sections(&s);
    assert_eq!(saved.len(), 2);
    assert!(saved[0].contains("<w:noEndnote/>"), "section 1: {}", saved[0]);
    assert!(!saved[1].contains("<w:noEndnote/>"), "section 2: {}", saved[1]);
}
