//! View › Window › Split (#3254): two panes on one document. The engine keeps whether the window is
//! split and the top pane's share of the height; the desktop app draws the panes. Remove Split is
//! Alt+Shift+C (Word's ClosePane, Linux/Windows) or Ctrl+Alt+S again. Repeat is also on Alt+Enter.

use serde_json::{Value, json};

use crate::{CmdError, Session, cmd};

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id(k: &str) -> Option<&'static str> {
    cmd::registry().by_shortcut(k).map(|c| c.id)
}

#[test]
fn split_toggles_with_an_even_ratio_and_no_undo_step() {
    let mut s = s();
    assert_eq!(s.view.split, None);
    let undo = s.undo_depth();
    let out = run(&mut s, "view.split", json!({}));
    assert_eq!(out, json!({"split": true, "ratio": 0.5}));
    assert_eq!(s.view.split, Some(0.5));
    let out = run(&mut s, "view.split", json!({}));
    assert_eq!(out, json!({"split": false, "ratio": null}));
    assert_eq!(s.view.split, None);
    assert_eq!(s.undo_depth(), undo, "splitting the window isn't an edit");
}

#[test]
fn value_and_ratio_set_the_split() {
    let mut s = s();
    let out = run(&mut s, "view.split", json!({"ratio": 0.25}));
    assert_eq!(out, json!({"split": true, "ratio": 0.25}));
    // Turning it on again keeps the dragged ratio.
    run(&mut s, "view.split", json!({"value": true}));
    assert_eq!(s.view.split, Some(0.25));
    run(&mut s, "view.split", json!({"ratio": 0.9}));
    assert_eq!(s.view.split, Some(0.9));
    run(&mut s, "view.split", json!({"value": false, "ratio": 0.4}));
    assert_eq!(s.view.split, None, "value false wins over a ratio");
    run(&mut s, "view.split", json!({"value": false}));
    assert_eq!(s.view.split, None);
    run(&mut s, "view.split", json!({"value": true}));
    assert_eq!(s.view.split, Some(0.5));
}

#[test]
fn a_ratio_out_of_range_is_refused_and_changes_nothing() {
    let mut s = s();
    run(&mut s, "view.split", json!({"ratio": 0.3}));
    for bad in [json!(0.05), json!(0.95), json!(-1.0), json!(2), json!("half")] {
        let r = s.run("view.split", &json!({"ratio": bad}));
        assert!(matches!(r, Err(CmdError::Params(_))), "ratio {bad}: {r:?}");
        assert_eq!(s.view.split, Some(0.3), "ratio {bad} left the split alone");
    }
    // The ends of the range are allowed.
    run(&mut s, "view.split", json!({"ratio": 0.1}));
    assert_eq!(s.view.split, Some(0.1));
}

#[test]
fn remove_split_ends_the_split_and_is_harmless_without_one() {
    let mut s = s();
    let out = run(&mut s, "view.removeSplit", json!({}));
    assert_eq!(out, json!({"split": false}));
    run(&mut s, "view.split", json!({"ratio": 0.6}));
    let undo = s.undo_depth();
    let out = run(&mut s, "view.removeSplit", json!({}));
    assert_eq!(out, json!({"split": false}));
    assert_eq!(s.view.split, None);
    assert_eq!(s.undo_depth(), undo);
    // A new split starts even again.
    run(&mut s, "view.split", json!({}));
    assert_eq!(s.view.split, Some(0.5));
}

#[test]
fn split_survives_in_the_view_state_json() {
    let mut s = s();
    run(&mut s, "view.split", json!({"ratio": 0.75}));
    let state = run(&mut s, "view.state", json!({}));
    assert_eq!(state["split"], json!(0.75));
    // Older saved view state without the field reads as one pane.
    let mut old = state.clone();
    old.as_object_mut().unwrap().remove("split");
    let v: crate::ViewState = serde_json::from_value(old).unwrap();
    assert_eq!(v.split, None);
}

#[test]
fn split_keys_and_repeat_on_alt_enter() {
    assert_eq!(id("Mod+Alt+S"), Some("view.split"));
    assert_eq!(id("F4"), Some("edit.repeat"));
    if cfg!(target_os = "macos") {
        assert_eq!(id("Alt+Shift+C"), None);
        assert_eq!(id("Alt+Enter"), None);
    } else {
        assert_eq!(id("Alt+Shift+C"), Some("view.removeSplit"));
        assert_eq!(id("Alt+Enter"), Some("edit.repeat"));
    }
}

#[test]
fn alt_enter_repeats_the_last_command() {
    if cfg!(target_os = "macos") {
        return;
    }
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "ab"}));
    let before = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
    let rep = id("Alt+Enter").expect("Alt+Enter is bound");
    run(&mut s, rep, json!({}));
    let after = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
    assert_eq!(after.trim_end(), format!("{}ab", before.trim_end()), "repeat typed the text again");
}
