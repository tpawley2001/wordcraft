//! Clipboard pane Options (Home › Clipboard › Options ▾), as in Word: Show Office Clipboard
//! Automatically, Show When Ctrl+C Pressed Twice, Collect Without Showing, and the copy status
//! "Item N of 24 Clipboard collected".

use serde_json::{Value, json};

use crate::Session;

fn s() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("one two three\n"));
    s.run("caret.docStart", &json!({})).unwrap();
    s
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn sel(s: &mut Session, a: usize, b: usize) {
    run(s, "select.range", json!({"anchor": {"block": 0, "off": a}, "focus": {"block": 0, "off": b}}));
}

fn copy(s: &mut Session, a: usize, b: usize) -> Value {
    sel(s, a, b);
    run(s, "edit.copy", json!({}))
}

#[test]
fn options_report_the_defaults() {
    let mut s = s();
    assert_eq!(
        run(&mut s, "edit.clipboardOptions", json!({})),
        json!({"showAutomatically": false, "showOnCopyTwice": false, "collectWithoutShowing": true, "showStatus": true})
    );
}

#[test]
fn options_set_only_the_given_keys_and_reject_non_bools() {
    let mut s = s();
    let r = run(&mut s, "edit.clipboardOptions", json!({"showOnCopyTwice": true}));
    assert_eq!(r["showOnCopyTwice"], true);
    assert_eq!(r["showStatus"], true, "other options unchanged");
    assert!(s.run("edit.clipboardOptions", &json!({"showStatus": "yes"})).is_err());
    assert_eq!(run(&mut s, "edit.clipboardOptions", json!({}))["showStatus"], true, "a bad value changes nothing");
}

#[test]
fn options_persist_in_prefs_and_old_prefs_get_defaults() {
    let mut s = s();
    run(&mut s, "edit.clipboardOptions", json!({"showAutomatically": true, "collectWithoutShowing": false}));
    let v = serde_json::to_value(&s.prefs).unwrap();
    assert_eq!(v["clipboard"]["showAutomatically"], true);
    assert_eq!(v["clipboard"]["collectWithoutShowing"], false);
    let back: crate::Prefs = serde_json::from_value(v).unwrap();
    assert_eq!(back, s.prefs);

    let mut old = serde_json::to_value(crate::Prefs::default()).unwrap();
    old.as_object_mut().unwrap().remove("clipboard");
    let old: crate::Prefs = serde_json::from_value(old).unwrap();
    assert_eq!(old, crate::Prefs::default());
}

#[test]
fn collecting_off_skips_copies_while_the_pane_is_hidden() {
    let mut s = s();
    run(&mut s, "edit.clipboardOptions", json!({"collectWithoutShowing": false}));
    let r = copy(&mut s, 0, 3);
    assert_eq!(r["text"], "one", "the clipboard itself still gets the copy");
    assert_eq!(s.clipboard_text, "one");
    assert!(s.clip_history.is_empty(), "hidden pane, collecting off: nothing collected");
    assert!(r.get("collected").is_none());

    run(&mut s, "edit.clipboardPane", json!({"value": true}));
    let r = copy(&mut s, 4, 7);
    assert_eq!(s.clip_history.len(), 1, "the shown pane collects");
    assert_eq!(r["collected"], json!({"item": 1, "of": 24}));
}

#[test]
fn copy_reports_the_collected_item_number_unless_status_is_off() {
    let mut s = s();
    assert_eq!(copy(&mut s, 0, 3)["collected"], json!({"item": 1, "of": 24}));
    assert_eq!(copy(&mut s, 4, 7)["collected"], json!({"item": 2, "of": 24}));
    // The same copy again collects nothing.
    assert!(run(&mut s, "edit.copy", json!({})).get("collected").is_none());
    run(&mut s, "edit.clipboardOptions", json!({"showStatus": false}));
    let r = copy(&mut s, 8, 13);
    assert!(r.get("collected").is_none(), "status off");
    assert_eq!(s.clip_history.len(), 3, "still collected");
}

#[test]
fn cut_reports_the_collected_item_too() {
    let mut s = s();
    sel(&mut s, 0, 4);
    let r = run(&mut s, "edit.cut", json!({}));
    assert_eq!(r["collected"], json!({"item": 1, "of": 24}));
}

#[test]
fn show_automatically_opens_the_pane_at_the_second_item() {
    let mut s = s();
    run(&mut s, "edit.clipboardOptions", json!({"showAutomatically": true}));
    let r = copy(&mut s, 0, 3);
    assert!(!s.view.clipboard_pane, "one item doesn't open it");
    assert_ne!(r.get("pane"), Some(&json!(true)));
    let r = copy(&mut s, 4, 7);
    assert!(s.view.clipboard_pane, "the second item opens it");
    assert_eq!(r["pane"], true);
}

#[test]
fn show_automatically_off_never_opens_the_pane() {
    let mut s = s();
    copy(&mut s, 0, 3);
    copy(&mut s, 4, 7);
    copy(&mut s, 8, 13);
    assert!(!s.view.clipboard_pane);
}

#[test]
fn copy_twice_opens_the_pane_when_the_option_is_on() {
    let mut s = s();
    copy(&mut s, 0, 3);
    run(&mut s, "edit.copy", json!({}));
    assert!(!s.view.clipboard_pane, "option off by default");

    run(&mut s, "edit.clipboardOptions", json!({"showOnCopyTwice": true}));
    copy(&mut s, 4, 7);
    assert!(!s.view.clipboard_pane, "a copy of a new selection is not a second press");
    let r = run(&mut s, "edit.copy", json!({}));
    assert!(s.view.clipboard_pane, "the same selection copied twice opens the pane");
    assert_eq!(r["pane"], true);
}

#[test]
fn copy_twice_needs_the_same_selection_and_no_edit_between() {
    let mut s = s();
    run(&mut s, "edit.clipboardOptions", json!({"showOnCopyTwice": true}));
    copy(&mut s, 0, 3);
    copy(&mut s, 4, 7);
    assert!(!s.view.clipboard_pane, "different selections");

    copy(&mut s, 8, 13);
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.insert", json!({"text": "!"}));
    sel(&mut s, 8, 13);
    run(&mut s, "edit.copy", json!({}));
    assert!(!s.view.clipboard_pane, "the document changed between the copies");
}
