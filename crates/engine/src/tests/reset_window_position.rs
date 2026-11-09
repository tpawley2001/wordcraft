//! View › Window › Reset Window Position: while View Side by Side is on, it puts the two windows
//! back on equal halves of the screen. The engine only checks that Side by Side is on and asks the
//! front end to place the pair; Synchronous Scrolling and the pairing stay as they were.

use serde_json::json;

use crate::Session;
use crate::cmd::view::OtherWindow;

fn s() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.windows.available = true;
    s.windows.others = vec![OtherWindow { id: 7, title: "Notes".into() }, OtherWindow { id: 9, title: "Draft".into() }];
    s
}

#[test]
fn reset_window_position_needs_side_by_side() {
    let mut bare = Session::new(wordcraft_doc::Document::new());
    assert!(bare.run("view.resetWindowPosition", &json!({})).is_err(), "no window host");
    let mut s = s();
    let e = s.run("view.resetWindowPosition", &json!({})).unwrap_err().to_string();
    assert!(e.contains("Side by Side"), "says why: {e}");
    assert!(s.ui_requests.is_empty());
}

#[test]
fn reset_window_position_places_the_pair_and_keeps_scrolling() {
    let mut s = s();
    s.run("view.sideBySide", &json!({"window": 9})).unwrap();
    s.run("view.syncScroll", &json!({"value": false})).unwrap();
    s.ui_requests.clear();
    let r = s.run("view.resetWindowPosition", &json!({})).unwrap();
    assert_eq!(r["window"].as_u64(), Some(9));
    assert_eq!(std::mem::take(&mut s.ui_requests), [json!({"windows": "resetPosition", "with": 9})]);
    assert_eq!((s.windows.side_by_side, s.windows.sync_scroll), (Some(9), false), "pairing and scrolling unchanged");
    assert!(!s.dirty, "window commands never touch the document");
}

#[test]
fn reset_window_position_is_in_the_view_window_group() {
    let reg = crate::cmd::registry();
    let spec = reg.get("view.resetWindowPosition").expect("command registered");
    assert_eq!(spec.label, "Reset Window Position");
    assert_eq!(spec.location, "View › Window");
    assert_eq!(spec.shortcut, "", "Word has no shortcut for it");
    assert!(!spec.mutates);
}
