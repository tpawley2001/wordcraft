//! View › Window: Arrange All, View Side by Side, Synchronous Scrolling and Reset Window Position
//! (gap #27), as in Word for Windows. The engine keeps the side-by-side pair and the scrolling flag;
//! the window layer moves the windows from the UI requests these commands push.

use serde_json::{Value, json};

use crate::{CmdError, Session};

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Three documents a, b, c (c active).
fn three() -> (Session, u64, u64, u64) {
    let mut s = s();
    let a = s.document_id();
    run(&mut s, "file.new", json!({}));
    let b = s.document_id();
    run(&mut s, "file.new", json!({}));
    let c = s.document_id();
    (s, a, b, c)
}

fn requests(s: &mut Session) -> Vec<Value> {
    std::mem::take(&mut s.ui_requests)
}

#[test]
fn arrange_all_asks_the_windows_to_tile_and_lists_the_documents() {
    let (mut s, a, b, c) = three();
    requests(&mut s);
    let undo = s.undo_depth();
    let out = run(&mut s, "view.arrangeAll", json!({}));
    let ids: Vec<u64> = out["documents"].as_array().unwrap().iter().map(|d| d["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, vec![a, b, c]);
    assert!(requests(&mut s).contains(&json!({"open": "arrangeAll"})));
    assert_eq!(s.document_id(), c, "the active window keeps focus");
    assert_eq!(s.undo_depth(), undo, "no undo step");
}

#[test]
fn side_by_side_needs_a_second_document() {
    let mut s = s();
    assert!(matches!(s.run("view.sideBySide", &json!({})), Err(CmdError::Disabled(_))));
    assert!(matches!(s.run("view.syncScroll", &json!({})), Err(CmdError::Disabled(_))));
    assert!(matches!(s.run("view.resetWindowPosition", &json!({})), Err(CmdError::Disabled(_))));
}

#[test]
fn two_documents_pair_active_left_and_turn_on_synchronous_scrolling() {
    let mut s = s();
    let a = s.document_id();
    run(&mut s, "file.new", json!({}));
    let b = s.document_id();
    requests(&mut s);
    let out = run(&mut s, "view.sideBySide", json!({}));
    assert_eq!(out["sideBySide"], json!({"left": b, "right": a, "syncScroll": true}));
    assert!(requests(&mut s).contains(&json!({"sideBySide": {"left": b, "right": a}})));

    // Synchronous Scrolling toggles while side by side is on.
    assert_eq!(run(&mut s, "view.syncScroll", json!({}))["syncScroll"], json!(false));
    assert_eq!(run(&mut s, "view.syncScroll", json!({"value": true}))["syncScroll"], json!(true));

    run(&mut s, "view.resetWindowPosition", json!({}));
    assert!(requests(&mut s).contains(&json!({"open": "resetWindowPosition"})));

    // Toggling again ends it; the windows stay where they are.
    let out = run(&mut s, "view.sideBySide", json!({}));
    assert_eq!(out["sideBySide"], Value::Null);
    assert!(requests(&mut s).contains(&json!({"sideBySide": null})));
    assert!(matches!(s.run("view.syncScroll", &json!({})), Err(CmdError::Disabled(_))));
    assert!(matches!(s.run("view.resetWindowPosition", &json!({})), Err(CmdError::Disabled(_))));
}

#[test]
fn three_documents_ask_which_one_unless_an_id_is_given() {
    let (mut s, a, b, c) = three();
    requests(&mut s);
    let out = run(&mut s, "view.sideBySide", json!({}));
    assert_eq!(out["sideBySide"], Value::Null, "nothing pairs before the user chooses");
    assert_eq!(out["choose"], json!([a, b]));
    assert!(requests(&mut s).contains(&json!({"open": "sideBySide"})));
    assert!(matches!(s.run("view.syncScroll", &json!({})), Err(CmdError::Disabled(_))));

    let out = run(&mut s, "view.sideBySide", json!({"value": true, "id": a}));
    assert_eq!(out["sideBySide"], json!({"left": c, "right": a, "syncScroll": true}));
}

#[test]
fn bad_partner_ids_are_parameter_errors() {
    let (mut s, _a, _b, c) = three();
    assert!(matches!(s.run("view.sideBySide", &json!({"value": true, "id": c})), Err(CmdError::Params(_))));
    assert!(matches!(s.run("view.sideBySide", &json!({"value": true, "id": 999_999})), Err(CmdError::Params(_))));
}

#[test]
fn closing_either_document_of_the_pair_ends_side_by_side() {
    let (mut s, a, _b, c) = three();
    run(&mut s, "view.sideBySide", json!({"value": true, "id": a}));
    run(&mut s, "file.closeDocument", json!({"id": a, "force": true}));
    assert!(matches!(s.run("view.syncScroll", &json!({})), Err(CmdError::Disabled(_))));
    s.activate(c).unwrap();
    // Two documents left: pairing again works with the default partner.
    let out = run(&mut s, "view.sideBySide", json!({"value": true}));
    assert_eq!(out["sideBySide"]["left"], json!(c));
}
