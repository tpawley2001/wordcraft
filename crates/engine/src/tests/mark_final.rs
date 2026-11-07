//! Mark as Final (File › Info › Protect Document): the `_MarkAsFinal` custom property (`vt:bool`
//! true, ECMA-376 Part 1 §22.3) and Status `Final` (`cp:contentStatus`). A marked document takes
//! no edits until the mark is removed (Mark as Final again, or Edit Anyway); it is not
//! protection, so `w:documentProtection` stays as it was.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::{CmdError, Session};

fn s() -> Session {
    Session::new(Document::from_text("Body"))
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The stored (kind, value) of `_MarkAsFinal`.
fn mark(s: &Session) -> Option<(String, String)> {
    s.doc.custom_props.iter().find(|p| p.name.eq_ignore_ascii_case("_MarkAsFinal")).map(|p| (p.kind.clone(), p.value.clone()))
}

fn flat(doc: &Document) -> String {
    String::from_utf8(wordcraft_docx::write_flat(doc).expect("flat OPC writes")).expect("utf-8")
}

fn body(s: &Session) -> String {
    s.doc.plain_text(wordcraft_doc::StoryRef::Body).trim_end_matches(['\n', '\r']).to_string()
}

fn refused(s: &mut Session, id: &str, v: Value) -> bool {
    matches!(s.run(id, &v), Err(CmdError::Disabled(m)) if m.contains("marked as final"))
}

#[test]
fn marking_sets_the_property_and_status_final() {
    let mut s = s();
    let r = run(&mut s, "file.markAsFinal", json!({}));
    assert_eq!(r["final"], json!(true));
    assert_eq!(mark(&s), Some(("bool".to_string(), "true".to_string())));
    let xml = flat(&s.doc);
    assert!(xml.contains("<cp:contentStatus>Final</cp:contentStatus>"), "Status Final is saved");
    assert!(xml.contains("_MarkAsFinal"), "the custom property is saved");
    assert!(!xml.contains("documentProtection"), "not document protection");
    assert!(s.doc.settings.protection.is_none());
}

#[test]
fn a_marked_document_takes_no_edits() {
    let mut s = s();
    run(&mut s, "file.markAsFinal", json!({"final": true}));
    assert!(refused(&mut s, "text.insert", json!({"text": "x"})), "typing");
    assert!(refused(&mut s, "format.bold", json!({})), "formatting");
    assert!(refused(&mut s, "text.backspace", json!({})), "deleting");
    assert_eq!(body(&s), "Body");
    let why = s.why_disabled("text.insert").unwrap_or_default();
    assert!(why.contains("marked as final"), "ribbon greys typing out: {why:?}");
    // Reading and moving around still work.
    run(&mut s, "caret.right", json!({}));
    assert!(s.why_disabled("caret.right").is_none());
    assert!(s.why_disabled("file.markAsFinal").is_none(), "the toggle itself stays available");
}

#[test]
fn marking_again_or_edit_anyway_removes_the_mark() {
    for params in [json!({}), json!({"final": false})] {
        let mut s = s();
        run(&mut s, "file.markAsFinal", json!({}));
        let r = run(&mut s, "file.markAsFinal", params.clone());
        assert_eq!(r["final"], json!(false), "{params}");
        assert_eq!(mark(&s), None, "{params}: the property is removed");
        run(&mut s, "caret.docEnd", json!({}));
        run(&mut s, "text.insert", json!({"text": "!"}));
        assert_eq!(body(&s), "Body!", "{params}");
    }
}

#[test]
fn final_false_on_an_unmarked_document_changes_nothing() {
    let mut s = s();
    let r = run(&mut s, "file.markAsFinal", json!({"final": false}));
    assert_eq!(r["final"], json!(false));
    assert_eq!(mark(&s), None);
    run(&mut s, "text.insert", json!({"text": "x"}));
}

/// Files marked by Word open read-only; `_MarkAsFinal` false (or not a Yes/No property) doesn't.
#[test]
fn an_opened_file_follows_its_property() {
    for (kind, value, marked) in [("bool", "true", true), ("bool", "1", true), ("bool", "false", false), ("lpwstr", "true", false)] {
        let mut doc = Document::from_text("Body");
        doc.set_custom_prop_typed("_MarkAsFinal", kind, value);
        let mut s = Session::new(doc);
        assert_eq!(refused(&mut s, "text.insert", json!({"text": "x"})), marked, "{kind} {value}");
    }
}

#[test]
fn the_mark_and_status_survive_a_save_and_reopen() {
    let mut s = s();
    run(&mut s, "file.markAsFinal", json!({}));
    let doc = wordcraft_docx::read_xml(flat(&s.doc).as_bytes()).expect("reads back");
    assert!(flat(&doc).contains("<cp:contentStatus>Final</cp:contentStatus>"), "Status kept through open and save");
    let mut s = Session::new(doc);
    assert!(refused(&mut s, "text.insert", json!({"text": "x"})));
}

/// Status is an ordinary core property: `file.properties` sets it and a file's own value
/// survives open and save (it used to be dropped).
#[test]
fn status_is_a_document_property() {
    let mut s = s();
    run(&mut s, "file.properties", json!({"status": "Draft"}));
    let xml = flat(&s.doc);
    assert!(xml.contains("<cp:contentStatus>Draft</cp:contentStatus>"), "{}", &xml[..xml.len().min(400)]);
    let back = wordcraft_docx::read_xml(xml.as_bytes()).expect("reads back");
    assert!(flat(&back).contains("<cp:contentStatus>Draft</cp:contentStatus>"));
}

/// The other core properties Word writes survive open and save unchanged.
#[test]
fn other_core_properties_survive_a_round_trip() {
    let mut s = s();
    run(&mut s, "file.properties", json!({"status": "Draft"}));
    let xml = flat(&s.doc);
    let core_end = "</cp:coreProperties>";
    assert!(xml.contains(core_end), "a core properties part is written");
    let extra = "<cp:lastPrinted>2026-11-02T09:30:00Z</cp:lastPrinted><dc:language>en-GB</dc:language><cp:version>2.1</cp:version><dc:identifier>DOC-77</dc:identifier><cp:contentType>Memo</cp:contentType>";
    let xml = xml.replacen(core_end, &format!("{extra}{core_end}"), 1);
    let back = flat(&wordcraft_docx::read_xml(xml.as_bytes()).expect("reads"));
    for want in [
        "<cp:lastPrinted>2026-11-02T09:30:00Z</cp:lastPrinted>",
        "<dc:language>en-GB</dc:language>",
        "<cp:version>2.1</cp:version>",
        "<dc:identifier>DOC-77</dc:identifier>",
        "<cp:contentType>Memo</cp:contentType>",
    ] {
        assert!(back.contains(want), "{want} kept");
    }
}
