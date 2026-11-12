//! Update Field (F9) on a cross-reference whose bookmark is gone: Word replaces the result with
//! `Error! Reference source not found.` (REF) or `Error! Bookmark not defined.` (PAGEREF,
//! NOTEREF) instead of keeping the old result (ECMA-376 §17.16.5.45, §17.16.5.51, §17.16.5.40).

use serde_json::{Value, json};
use wordcraft_doc::{Block, CharProps, Document, InlineObject, Paragraph, PartKind, Pos, StoryRef};

use crate::Session;

const REF_ERROR: &str = "Error! Reference source not found.";
const BOOKMARK_ERROR: &str = "Error! Bookmark not defined.";

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e:?}"))
}

fn field(instr: &str, locked: bool) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: "stale".into(), locked, dirty: false, code: false, form: String::new() }
}

/// A paragraph holding one unlocked field per code, each stored with the result "stale".
fn field_para(codes: &[&str]) -> Paragraph {
    let mut p = Paragraph::new();
    for instr in codes {
        let off = p.len();
        p.insert_object(off, field(instr, false), &CharProps::default()).unwrap();
    }
    p
}

/// "Target text here." with "Target text" bookmarked as `Mark`, then a paragraph "Refs: " with
/// the caret at its end, where `insert` puts fields.
fn session() -> Session {
    let mut s = Session::new(Document::from_text("Target text here.\nRefs: "));
    run(&mut s, "select.text", json!({"text": "Target text"}));
    run(&mut s, "insert.bookmark", json!({"name": "Mark"}));
    run(&mut s, "caret.docEnd", json!({}));
    s
}

/// One field per code at the caret, each stored with the result "stale".
fn insert(s: &mut Session, codes: &[&str]) {
    for instr in codes {
        run(s, "insert.field", json!({"instr": instr, "result": "stale"}));
    }
}

fn update(s: &mut Session) {
    run(s, "references.updateFields", json!({}));
}

/// The story's field results, in document order.
fn results(s: &Session, story: StoryRef) -> Vec<String> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(story) {
        for o in &s.doc.para(story, &path).unwrap().objects {
            if let InlineObject::Field { result, .. } = o {
                out.push(result.clone());
            }
        }
    }
    out
}

/// (field code, Word's result after Update Field when no bookmark `Gone` exists)
const MISSING: [(&str, &str); 14] = [
    (r"REF Gone", REF_ERROR),
    (r"REF Gone \h", REF_ERROR),
    (r#"REF "Gone" \h"#, REF_ERROR),
    (r"REF Gone \* MERGEFORMAT", REF_ERROR),
    (r"REF Gone \p \h", REF_ERROR),
    (r"REF Gone \r \h", REF_ERROR),
    (r"REF Gone \n \h", REF_ERROR),
    (r"REF Gone \w \h", REF_ERROR),
    (r"REF Gone \r \t \h", REF_ERROR),
    (r"PAGEREF Gone \h", BOOKMARK_ERROR),
    (r#"PAGEREF "Gone" \* Roman"#, BOOKMARK_ERROR),
    (r"PAGEREF Gone \p \h", BOOKMARK_ERROR),
    (r"NOTEREF Gone \h", BOOKMARK_ERROR),
    (r"NOTEREF Gone \f \p \h", BOOKMARK_ERROR),
];

#[test]
fn a_missing_bookmark_gives_words_error_text() {
    let mut s = session();
    let codes: Vec<&str> = MISSING.iter().map(|(c, _)| *c).collect();
    insert(&mut s, &codes);
    update(&mut s);
    let got = results(&s, StoryRef::Body);
    for ((code, want), got) in MISSING.iter().zip(&got) {
        assert_eq!(got, want, "{code}");
    }
    assert_eq!(got.len(), MISSING.len());
    // A second update changes nothing.
    update(&mut s);
    assert_eq!(results(&s, StoryRef::Body), got);
}

#[test]
fn an_existing_bookmark_updates_as_before() {
    let mut s = session();
    insert(&mut s, &[r"REF Mark \h", r"PAGEREF Mark \h", r"REF Mark \p \h", r"REF Gone \h"]);
    update(&mut s);
    assert_eq!(results(&s, StoryRef::Body), ["Target text", "1", "above", REF_ERROR]);
}

#[test]
fn deleting_the_bookmark_breaks_its_references_and_undo_restores_them() {
    let mut s = session();
    insert(&mut s, &[r"REF Mark \h", r"PAGEREF Mark \h"]);
    update(&mut s);
    assert_eq!(results(&s, StoryRef::Body), ["Target text", "1"]);
    run(&mut s, "insert.deleteBookmark", json!({"name": "Mark"}));
    update(&mut s);
    assert_eq!(results(&s, StoryRef::Body), [REF_ERROR, BOOKMARK_ERROR]);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(results(&s, StoryRef::Body), ["Target text", "1"], "undo of Update Field");
}

#[test]
fn a_name_given_a_value_by_set_is_not_missing() {
    let mut s = session();
    insert(&mut s, &[r#"SET who "Ada Lovelace""#, "REF who", "REF nobody"]);
    update(&mut s);
    let got = results(&s, StoryRef::Body);
    assert_eq!((got[1].as_str(), got[2].as_str()), ("Ada Lovelace", REF_ERROR));
}

#[test]
fn a_field_without_a_bookmark_name_says_no_bookmark_name_given() {
    let mut s = session();
    insert(&mut s, &["REF", r"REF \h", "PAGEREF", r"PAGEREF \h", "NOTEREF"]);
    update(&mut s);
    assert_eq!(results(&s, StoryRef::Body), ["Error! No bookmark name given."; 5]);
}

#[test]
fn a_locked_field_keeps_its_result() {
    let mut doc = Document::from_text("Body");
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let p = doc.para_mut(StoryRef::Body, &path).unwrap();
    for (instr, locked) in [("REF Gone", true), ("PAGEREF Gone", true), ("REF Gone", false)] {
        let off = p.len();
        p.insert_object(off, field(instr, locked), &CharProps::default()).unwrap();
    }
    let mut s = Session::new(doc);
    update(&mut s);
    assert_eq!(results(&s, StoryRef::Body), ["stale", "stale", REF_ERROR]);
}

/// A bookmark in a header exists: a body field that refers to it is not a broken reference.
#[test]
fn a_bookmark_outside_the_body_is_not_missing() {
    let mut doc = Document::from_text("Body ");
    let path = doc.para_paths(StoryRef::Body).remove(0);
    let p = doc.para_mut(StoryRef::Body, &path).unwrap();
    for instr in [r"REF HdrMark \h", r"PAGEREF HdrMark \h", r"REF Gone \h"] {
        let off = p.len();
        p.insert_object(off, field(instr, false), &CharProps::default()).unwrap();
    }
    let mut h = Paragraph::with_text("Header text", CharProps::default());
    let end = h.len();
    h.insert_object(end, InlineObject::BookmarkEnd { name: "HdrMark".into() }, &CharProps::default()).unwrap();
    h.insert_object(0, InlineObject::BookmarkStart { name: "HdrMark".into() }, &CharProps::default()).unwrap();
    doc.add_part(PartKind::Header, vec![std::sync::Arc::new(Block::Para(h))]);
    let mut s = Session::new(doc);
    update(&mut s);
    let got = results(&s, StoryRef::Body);
    assert!(!got[0].starts_with("Error!") && !got[1].starts_with("Error!"), "{got:?}");
    assert_eq!(got[2], REF_ERROR);
}

/// Fields in a header are updated the same way when the caret is in it, or with `all`.
#[test]
fn fields_in_a_header_show_the_error_too() {
    let codes = [r"REF Gone \h", r"PAGEREF Gone \h", r"NOTEREF Gone \h"];
    let want = [REF_ERROR, BOOKMARK_ERROR, BOOKMARK_ERROR];
    let make = || {
        let mut doc = Document::from_text("Body");
        let hdr = doc.add_part(PartKind::Header, vec![std::sync::Arc::new(Block::Para(field_para(&codes)))]);
        (Session::new(doc), StoryRef::Part(hdr))
    };
    // The caret in the header.
    let (mut s, story) = make();
    let path = s.doc.para_paths(story).remove(0);
    s.sel = crate::session::Selection::caret(Pos::new(story, path, 0));
    update(&mut s);
    assert_eq!(results(&s, story), want);
    // The caret in the body: only `all` reaches the header.
    let (mut s, story) = make();
    update(&mut s);
    assert_eq!(results(&s, story), ["stale"; 3]);
    run(&mut s, "references.updateFields", json!({"all": true}));
    assert_eq!(results(&s, story), want);
}
