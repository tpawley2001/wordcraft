//! Paste Special › Paste link: text copied from a saved document is pasted as the result of a
//! LINK field that points at an `OLE_LINKn` bookmark around the copied text, as in Word.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use wordcraft_doc::{Document, Pos, StoryRef};

use crate::Session;
use crate::cmd::paste::{link_instr, link_result_text, result_text};

const REPORT: &str = "/home/ann/Report.docx";
const LINK_NOTE: &str = " Paste Link creates a shortcut to the source file. Changes to the source file will be reflected in your document.";

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// A document with `text` in one paragraph, saved at `path`.
fn saved(text: &str, path: &str) -> Session {
    let mut s = Session::new(Document::from_text(text));
    s.path = Some(PathBuf::from(path));
    s
}

/// Copy characters `from..to` of the first paragraph.
fn copy(s: &mut Session, from: usize, to: usize) {
    run(s, "select.range", json!({"anchor": Pos::body(0, from), "focus": Pos::body(0, to)}));
    run(s, "edit.copy", json!({}));
}

fn caret(s: &mut Session, off: usize) {
    run(s, "caret.set", json!({"pos": Pos::body(0, off)}));
}

/// The body's letters and spaces: field and bookmark markers left out.
fn letters(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect()
}

fn instrs(s: &Session) -> Vec<String> {
    s.doc.field_ranges(StoryRef::Body).into_iter().map(|f| f.instr.trim().to_string()).collect()
}

fn bookmark_names(s: &Session) -> Vec<String> {
    s.doc.bookmarks().into_iter().map(|(n, _)| n).collect()
}

fn ids(r: &Value, key: &str) -> Vec<String> {
    r[key].as_array().unwrap_or_else(|| panic!("no `{key}` in {r}")).iter().map(|f| f["id"].as_str().unwrap_or_default().to_string()).collect()
}

#[test]
fn link_instr_follows_words_link_field() {
    let rows = [
        (REPORT, "OLE_LINK1", "formatted", r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \r"#),
        (REPORT, "OLE_LINK1", "rtf", r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \r"#),
        (REPORT, "OLE_LINK2", "html", r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK2" \a \h"#),
        (REPORT, "OLE_LINK1", "text", r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \t"#),
        (REPORT, "OLE_LINK1", "unicode", r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \u"#),
        (r"C:\Docs\My Report.docx", "OLE_LINK1", "rtf", r#"LINK Word.Document.12 "C:\\Docs\\My Report.docx" "OLE_LINK1" \a \r"#),
        (r"C:\Docs\old.DOC", "OLE_LINK3", "text", r#"LINK Word.Document.8 "C:\\Docs\\old.DOC" "OLE_LINK3" \a \t"#),
        (r#"/tmp/a "b".rtf"#, "OLE_LINK1", "unicode", r#"LINK Word.Document.8 "/tmp/a \"b\".rtf" "OLE_LINK1" \a \u"#),
    ];
    for (path, bookmark, fmt, want) in rows {
        assert_eq!(link_instr(Path::new(path), bookmark, fmt).as_deref(), Some(want), "{path} {fmt}");
    }
    assert_eq!(link_instr(Path::new(REPORT), "OLE_LINK1", "picture"), None);
    assert_eq!(link_instr(Path::new(REPORT), "OLE_LINK1", "bogus"), None);
}

#[test]
fn link_result_text_adds_words_paste_link_sentence() {
    for id in ["formatted", "rtf", "html", "text", "unicode"] {
        assert_eq!(link_result_text(id), format!("{}{LINK_NOTE}", result_text(id)), "{id}");
    }
    assert_eq!(link_result_text("picture"), "");
    assert_eq!(link_result_text("bogus"), "");
}

#[test]
fn a_copy_from_a_saved_document_lists_link_formats() {
    let mut s = saved("Alpha beta gamma", REPORT);
    copy(&mut s, 0, 5);
    let r = run(&mut s, "edit.pasteSpecial", json!({}));
    let formats = ids(&r, "formats");
    assert!(formats.iter().any(|f| f == "formatted") && formats.iter().any(|f| f == "text"), "{formats:?}");
    assert_eq!(ids(&r, "linkFormats"), formats, "no picture on the clipboard: the same list");
    let first = &r["linkFormats"][0];
    assert_eq!(first["result"].as_str().unwrap(), link_result_text(first["id"].as_str().unwrap()));
    assert_eq!(first["label"], r["formats"][0]["label"]);
    let req = s.ui_requests.iter().find(|q| q["open"] == "pasteSpecial").expect("dialog request");
    assert_eq!(req["linkFormats"].as_array().map(Vec::len), Some(formats.len()));
}

#[test]
fn paste_link_in_the_same_document_inserts_the_field_and_bookmarks_the_source() {
    let mut s = saved("Alpha beta gamma", REPORT);
    copy(&mut s, 0, 5);
    caret(&mut s, 16);
    let r = run(&mut s, "edit.pasteSpecial", json!({"as": "text", "link": true}));
    let want = r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \t"#;
    assert_eq!(r["pastedAs"], "text");
    assert_eq!(r["linked"], json!(true));
    assert_eq!(r["instr"], want);
    assert_eq!(r["bookmark"], "OLE_LINK1");
    assert_eq!(instrs(&s), [want]);
    assert_eq!(letters(&s), "Alpha beta gammaAlpha", "the result goes where the caret was");
    let marks = s.doc.bookmarks();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].0, "OLE_LINK1");
    assert_eq!(marks[0].1.off, 0, "the bookmark starts where the copied text starts");
}

#[test]
fn paste_link_into_another_document_bookmarks_the_source_document() {
    let mut s = saved("Source words", REPORT);
    let a = s.document_id();
    copy(&mut s, 0, 6);
    let b = s.add_document(Document::from_text("Target "));
    assert_ne!(a, b);
    caret(&mut s, 7);
    let r = run(&mut s, "edit.pasteSpecial", json!({"as": "formatted", "link": true}));
    let want = r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \r"#;
    assert_eq!(r["instr"], want);
    assert_eq!(instrs(&s), [want]);
    assert_eq!(letters(&s), "Target Source");
    assert!(bookmark_names(&s).is_empty(), "the bookmark belongs to the source document");
    let (marks, fields, text) = s.with_document(a, |s| (bookmark_names(s), instrs(s), letters(s))).unwrap();
    assert_eq!(marks, ["OLE_LINK1"]);
    assert!(fields.is_empty());
    assert_eq!(text, "Source words");
    assert!(s.document_dirty(a), "adding the bookmark changes the source document");
}

#[test]
fn each_format_writes_its_switch() {
    for (fmt, switch) in [("formatted", r"\r"), ("text", r"\t"), ("unicode", r"\u")] {
        let mut s = saved("Alpha beta gamma", REPORT);
        copy(&mut s, 6, 10);
        caret(&mut s, 16);
        let r = run(&mut s, "edit.pasteSpecial", json!({"as": fmt, "link": true}));
        let want = format!(r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a {switch}"#);
        assert_eq!(r["instr"], want.as_str(), "{fmt}");
        assert_eq!(letters(&s), "Alpha beta gammabeta", "{fmt}");
    }
}

#[test]
fn an_unsaved_source_has_no_paste_link() {
    let mut s = Session::new(Document::from_text("Alpha beta"));
    copy(&mut s, 0, 5);
    let r = run(&mut s, "edit.pasteSpecial", json!({}));
    assert!(!ids(&r, "formats").is_empty());
    assert!(ids(&r, "linkFormats").is_empty());
    caret(&mut s, 10);
    let e = s.run("edit.pasteSpecial", &json!({"as": "text", "link": true})).unwrap_err();
    assert!(e.to_string().contains("saved document"), "{e}");
    assert_eq!(letters(&s), "Alpha beta");
    assert!(bookmark_names(&s).is_empty() && instrs(&s).is_empty());
}

#[test]
fn cut_text_and_other_programs_text_have_no_paste_link() {
    let mut s = saved("Alpha beta gamma", REPORT);
    run(&mut s, "select.range", json!({"anchor": Pos::body(0, 0), "focus": Pos::body(0, 6)}));
    run(&mut s, "edit.cut", json!({}));
    let r = run(&mut s, "edit.pasteSpecial", json!({}));
    assert!(ids(&r, "linkFormats").is_empty(), "cut text is no longer in the source");

    let mut s = saved("Alpha beta gamma", REPORT);
    let r = run(&mut s, "edit.pasteSpecial", json!({"text": "from elsewhere"}));
    assert!(ids(&r, "linkFormats").is_empty(), "nothing copied here");
    copy(&mut s, 0, 5);
    let r = run(&mut s, "edit.pasteSpecial", json!({"text": "from elsewhere"}));
    assert!(ids(&r, "linkFormats").is_empty(), "another program copied since");
    let e = s.run("edit.pasteSpecial", &json!({"as": "text", "link": true, "text": "from elsewhere"})).unwrap_err();
    assert!(e.to_string().contains("saved document"), "{e}");
    assert!(bookmark_names(&s).is_empty() && instrs(&s).is_empty());
}

#[test]
fn an_edited_source_range_has_no_paste_link() {
    let mut s = saved("Alpha beta gamma", REPORT);
    copy(&mut s, 0, 5);
    caret(&mut s, 0);
    run(&mut s, "text.insert", json!({"text": "X"}));
    let r = run(&mut s, "edit.pasteSpecial", json!({}));
    assert!(ids(&r, "linkFormats").is_empty(), "characters 0..5 no longer read Alpha");
    assert!(s.run("edit.pasteSpecial", &json!({"as": "text", "link": true})).is_err());
    assert!(bookmark_names(&s).is_empty() && instrs(&s).is_empty());
}

#[test]
fn a_second_paste_link_reuses_the_bookmark_and_a_taken_name_is_skipped() {
    let mut s = saved("Source words", REPORT);
    let a = s.document_id();
    copy(&mut s, 0, 6);
    s.add_document(Document::from_text("Target "));
    caret(&mut s, 7);
    let first = run(&mut s, "edit.pasteSpecial", json!({"as": "text", "link": true}));
    caret(&mut s, 0);
    let second = run(&mut s, "edit.pasteSpecial", json!({"as": "text", "link": true}));
    assert_eq!(first["bookmark"], "OLE_LINK1");
    assert_eq!(second["bookmark"], "OLE_LINK1");
    assert_eq!(instrs(&s).len(), 2);
    assert_eq!(s.with_document(a, |s| bookmark_names(s)).unwrap(), ["OLE_LINK1"]);

    let mut s = saved("Alpha beta gamma", REPORT);
    run(&mut s, "select.range", json!({"anchor": Pos::body(0, 11), "focus": Pos::body(0, 16)}));
    run(&mut s, "insert.bookmark", json!({"name": "ole_link1"}));
    copy(&mut s, 0, 5);
    let b = s.add_document(Document::from_text("Target "));
    caret(&mut s, 7);
    let r = run(&mut s, "edit.pasteSpecial", json!({"as": "unicode", "link": true}));
    assert_eq!(r["bookmark"], "OLE_LINK2", "OLE_LINK1 is taken (names compare without case)");
    assert_eq!(r["instr"], r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK2" \a \u"#);
    assert!(s.is_open(b));
}
