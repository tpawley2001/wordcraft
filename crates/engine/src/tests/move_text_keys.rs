//! F2 / Shift+F2 (Windows and Linux): move or copy the selection to where the caret is put next,
//! confirmed with Enter, as Word's "Move to where?" / "Copy to where?" does. One undo step; the
//! clipboard is not used.

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;

use crate::{Session, cmd};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn text(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body)
}

fn id(k: &str) -> Option<&'static str> {
    cmd::registry().by_shortcut(k).map(|c| c.id)
}

fn doc(t: &str) -> Session {
    Session::new(wordcraft_doc::Document::from_text(t))
}

#[test]
fn f2_and_shift_f2_keys() {
    if cfg!(target_os = "macos") {
        assert_ne!(id("F2"), Some("edit.moveText"));
        assert_ne!(id("Shift+F2"), Some("edit.copyText"));
    } else {
        assert_eq!(id("F2"), Some("edit.moveText"));
        assert_eq!(id("Shift+F2"), Some("edit.copyText"));
    }
}

#[test]
fn f2_then_enter_moves_the_selection_in_one_undo_step() {
    let mut s = doc("one two three");
    run(&mut s, "select.text", json!({"text": "two "}));
    let r = run(&mut s, "edit.moveText", json!({}));
    assert_eq!(r["status"], "Move to where?");
    assert_eq!(text(&s), "one two three", "nothing moves before Enter");
    run(&mut s, "caret.docStart", json!({}));
    let depth = s.undo_depth();
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(text(&s), "two one three");
    assert_eq!(s.undo_depth(), depth + 1, "one undo step");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(text(&s), "one two three");
}

#[test]
fn shift_f2_copies_and_keeps_the_source() {
    let mut s = doc("one two three");
    run(&mut s, "select.text", json!({"text": "one "}));
    let r = run(&mut s, "edit.copyText", json!({}));
    assert_eq!(r["status"], "Copy to where?");
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "edit.moveTextHere", json!({}));
    assert_eq!(text(&s), "one two threeone ");
}

#[test]
fn moved_text_keeps_its_formatting() {
    let mut s = doc("plain bold");
    run(&mut s, "select.text", json!({"text": "bold"}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "edit.moveText", json!({}));
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "edit.moveTextHere", json!({}));
    assert_eq!(text(&s), "boldplain ");
    let p = s.doc.para(StoryRef::Body, &s.doc.para_paths(StoryRef::Body)[0]).unwrap();
    assert_eq!(p.props_at(0).bold, Some(true), "moved text stays bold");
    assert_ne!(p.props_of_char("bold".len()).bold, Some(true), "the text after it is not bold");
}

#[test]
fn cancel_or_an_edit_drops_the_pending_move() {
    let mut s = doc("one two three");
    run(&mut s, "select.text", json!({"text": "two "}));
    run(&mut s, "edit.moveText", json!({}));
    run(&mut s, "edit.moveTextCancel", json!({}));
    run(&mut s, "caret.docStart", json!({}));
    assert!(s.run("edit.moveTextHere", &json!({})).is_err(), "nothing pending after cancel");
    assert_eq!(text(&s), "one two three");

    run(&mut s, "select.text", json!({"text": "two "}));
    run(&mut s, "edit.moveText", json!({}));
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.insert", json!({"text": "!"}));
    assert!(s.run("edit.moveTextHere", &json!({})).is_err(), "typing cancels the pending move");
    assert_eq!(text(&s), "one two three!");
}

#[test]
fn enter_without_a_pending_move_is_a_new_paragraph() {
    let mut s = doc("one");
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(s.doc.para_paths(StoryRef::Body).len(), 2);
}

#[test]
fn a_caret_inside_the_source_changes_nothing() {
    let mut s = doc("one two three");
    run(&mut s, "select.text", json!({"text": "two three"}));
    run(&mut s, "edit.moveText", json!({}));
    // A selection inside the source: the text would go at its start, which is inside the source.
    run(&mut s, "select.text", json!({"text": "hre"}));
    let _ = s.run("edit.moveTextHere", &json!({}));
    assert_eq!(text(&s), "one two three");
}
