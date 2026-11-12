//! Paste Special › Paste link: a paste that fails leaves the source document (and the copy's
//! link) as they were, and a paste over the copied text itself is refused rather than deleting
//! the text and bookmark the new LINK field would point at.

use std::path::PathBuf;

use serde_json::{Value, json};
use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Document, Pos, StoryRef};

use crate::{Selection, Session};

const REPORT: &str = "/home/ann/Report.docx";
const LINK1: &str = r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \t"#;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// A document with `text` in one paragraph, saved at `REPORT`.
fn saved(text: &str) -> Session {
    let mut s = Session::new(Document::from_text(text));
    s.path = Some(PathBuf::from(REPORT));
    s
}

/// Copy characters `from..to` of the first paragraph.
fn copy(s: &mut Session, from: usize, to: usize) {
    run(s, "select.range", json!({"anchor": Pos::body(0, from), "focus": Pos::body(0, to)}));
    run(s, "edit.copy", json!({}));
}

fn select(s: &mut Session, from: usize, to: usize) {
    s.sel = Selection { anchor: Pos::body(0, from), focus: Pos::body(0, to) };
}

fn caret(s: &mut Session, off: usize) {
    s.sel = Selection::caret(Pos::body(0, off));
}

/// Put characters `from..to` of the first paragraph in a rich text control locked against
/// editing, and the caret inside it.
fn caret_in_locked_control(s: &mut Session, from: usize, to: usize) {
    select(s, from, to);
    run(s, "developer.richText", json!({"tag": "ctl"}));
    run(s, "developer.properties", json!({"control": "ctl", "lockEdit": true}));
    caret(s, from + OBJ.len_utf8() + 1);
}

fn paste_link(s: &mut Session) -> Result<Value, crate::CmdError> {
    s.run("edit.pasteSpecial", &json!({"as": "text", "link": true}))
}

/// The body's letters and spaces: field, bookmark and control markers left out.
fn letters(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect()
}

fn instrs(s: &Session) -> Vec<String> {
    s.doc.field_ranges(StoryRef::Body).into_iter().map(|f| f.instr.trim().to_string()).collect()
}

fn bookmark_names(s: &Session) -> Vec<String> {
    s.doc.bookmarks().into_iter().map(|(n, _)| n).collect()
}

fn link_formats(s: &mut Session) -> usize {
    run(s, "edit.pasteSpecial", json!({}))["linkFormats"].as_array().map_or(0, Vec::len)
}

#[test]
fn a_failed_paste_link_leaves_another_source_document_as_it_was() {
    let mut s = saved("Source words");
    let a = s.document_id();
    copy(&mut s, 0, 6);
    assert!(!s.document_dirty(a), "the fixture's source starts unchanged");
    s.add_document(Document::from_text("Target words"));
    caret_in_locked_control(&mut s, 0, 6);
    let target = s.doc.clone();

    assert!(paste_link(&mut s).is_err(), "a control locked against editing takes no paste");
    assert_eq!(s.doc, target, "the target is as it was");
    let (marks, text) = s.with_document(a, |s| (bookmark_names(s), letters(s))).unwrap();
    assert!(marks.is_empty(), "the source keeps no bookmark from the failed paste: {marks:?}");
    assert_eq!(text, "Source words");
    assert!(!s.document_dirty(a), "the source is not marked changed");
    let marks = s
        .with_document(a, |s| {
            let _ = s.run("edit.redo", &json!({}));
            bookmark_names(s)
        })
        .unwrap();
    assert!(marks.is_empty(), "Redo in the source does not bring the bookmark back: {marks:?}");
    assert!(!s.document_dirty(a));
}

#[test]
fn paste_link_still_works_after_a_failed_try_into_another_document() {
    let mut s = saved("Source words");
    let a = s.document_id();
    copy(&mut s, 0, 6);
    s.add_document(Document::from_text("Target words"));
    caret_in_locked_control(&mut s, 0, 6);
    assert!(paste_link(&mut s).is_err());

    // The end of the paragraph, outside the control.
    run(&mut s, "caret.docEnd", json!({}));
    assert!(link_formats(&mut s) > 0, "Paste link is still offered for the same copy");
    let r = paste_link(&mut s).expect("Paste link at a place that takes it");
    assert_eq!(r["bookmark"], "OLE_LINK1", "the failed try used up no name");
    assert_eq!(instrs(&s), [LINK1]);
    assert_eq!(letters(&s), "Target wordsSource");
    assert_eq!(s.with_document(a, |s| bookmark_names(s)).unwrap(), ["OLE_LINK1"]);
    assert!(s.document_dirty(a), "a paste that worked changes the source, as before");
}

#[test]
fn paste_link_still_works_after_a_failed_try_in_the_same_document() {
    let mut s = saved("Alpha beta gamma");
    copy(&mut s, 0, 5);
    caret_in_locked_control(&mut s, 11, 16);
    let before = s.doc.clone();

    assert!(paste_link(&mut s).is_err(), "a control locked against editing takes no paste");
    assert_eq!(s.doc, before, "the document is as it was");
    assert!(bookmark_names(&s).is_empty());

    caret(&mut s, 6);
    assert!(link_formats(&mut s) > 0, "Paste link is still offered for the same copy");
    let r = paste_link(&mut s).expect("Paste link at a place that takes it");
    assert_eq!(r["bookmark"], "OLE_LINK1");
    assert_eq!(instrs(&s), [LINK1]);
    assert_eq!(letters(&s), "Alpha Alphabeta gamma");
    assert_eq!(bookmark_names(&s), ["OLE_LINK1"]);
}

#[test]
fn a_failed_second_paste_link_keeps_the_first_ones_bookmark() {
    let mut s = saved("Source words");
    let a = s.document_id();
    copy(&mut s, 0, 6);
    s.add_document(Document::from_text("Target words"));
    run(&mut s, "caret.docEnd", json!({}));
    paste_link(&mut s).expect("the first Paste link");
    caret_in_locked_control(&mut s, 0, 6);

    assert!(paste_link(&mut s).is_err());
    assert_eq!(s.with_document(a, |s| bookmark_names(s)).unwrap(), ["OLE_LINK1"], "the bookmark the first paste's field uses stays");
    assert_eq!(instrs(&s), [LINK1]);
    run(&mut s, "caret.docEnd", json!({}));
    let r = paste_link(&mut s).expect("a third Paste link of the same copy");
    assert_eq!(r["bookmark"], "OLE_LINK1", "the same copy reuses its bookmark");
    assert_eq!(s.with_document(a, |s| bookmark_names(s)).unwrap(), ["OLE_LINK1"]);
}

#[test]
fn paste_link_over_the_copied_text_is_refused_and_changes_nothing() {
    for (from, to) in [(0, 5), (0, 10), (3, 8), (2, 3)] {
        let mut s = saved("Alpha beta gamma");
        copy(&mut s, 0, 5);
        select(&mut s, from, to);
        let before = s.doc.clone();
        assert!(paste_link(&mut s).is_err(), "{from}..{to} shares text with the copy");
        assert_eq!(s.doc, before, "{from}..{to}: the document is as it was");
        assert!(bookmark_names(&s).is_empty(), "{from}..{to}");
        assert!(instrs(&s).is_empty(), "{from}..{to}");

        // The copy can still be paste-linked somewhere else.
        caret(&mut s, 16);
        let r = paste_link(&mut s).unwrap_or_else(|e| panic!("{from}..{to}: Paste link at the end afterwards: {e}"));
        assert_eq!(r["bookmark"], "OLE_LINK1", "{from}..{to}");
        assert_eq!(letters(&s), "Alpha beta gammaAlpha", "{from}..{to}");
    }
}

#[test]
fn paste_link_over_other_text_in_the_same_document_replaces_it() {
    for (from, to, want) in [(6, 10, "Alpha Alpha gamma"), (5, 10, "AlphaAlpha gamma")] {
        let mut s = saved("Alpha beta gamma");
        copy(&mut s, 0, 5);
        select(&mut s, from, to);
        let r = paste_link(&mut s).unwrap_or_else(|e| panic!("{from}..{to}: {e}"));
        assert_eq!(r["bookmark"], "OLE_LINK1", "{from}..{to}");
        assert_eq!(letters(&s), want, "{from}..{to}");
        assert_eq!(instrs(&s), [LINK1], "{from}..{to}");
        let marks = s.doc.bookmarks();
        assert_eq!(marks.len(), 1, "{from}..{to}: the bookmark the field names is there");
        assert_eq!(marks[0].0, "OLE_LINK1");
        assert_eq!(marks[0].1.off, 0, "{from}..{to}: it still starts at the copied text");
    }
}

#[test]
fn a_second_paste_link_over_the_bookmarked_source_is_refused() {
    let mut s = saved("Alpha beta gamma");
    copy(&mut s, 0, 5);
    caret(&mut s, 16);
    paste_link(&mut s).expect("the first Paste link");
    assert_eq!(bookmark_names(&s), ["OLE_LINK1"]);
    // From the paragraph start to past `Alpha` and both bookmark marks.
    select(&mut s, 0, 5 + 2 * OBJ.len_utf8());
    let before = s.doc.clone();

    assert!(paste_link(&mut s).is_err(), "the selection holds the text the first field links to");
    assert_eq!(s.doc, before);
    assert_eq!(bookmark_names(&s), ["OLE_LINK1"]);
    assert_eq!(instrs(&s), [LINK1]);
}
