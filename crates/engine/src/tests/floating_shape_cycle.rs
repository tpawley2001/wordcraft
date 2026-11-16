//! Word's Ctrl+Alt+5 (Linux/Windows): select a floating shape, then Tab / Shift+Tab cycle through the
//! floating shapes in document order (`select.nextFloatingShape`, `backward` for Shift+Tab). Inline
//! pictures and shapes hidden in the Selection Pane are skipped; the cycle wraps around.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;
use crate::cmd::objects::object_selection;

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

/// "Alpha" then four rectangles at the end of the paragraph, in this document order:
/// 100 pt wide (Square), 120 pt (In Line with Text), 140 pt (Square), 160 pt (In Front of Text).
fn doc() -> Session {
    let mut s = Session::new(Document::from_text("Alpha"));
    for (w, wrap) in [(100, "square"), (120, "inline"), (140, "square"), (160, "inFrontOfText")] {
        run(&mut s, "caret.docEnd", json!({}));
        run(&mut s, "insert.shape", json!({"kind": "rectangle", "width": w, "height": 30}));
        run(&mut s, "arrange.wrap", json!({"wrap": wrap}));
    }
    run(&mut s, "caret.docStart", json!({}));
    s
}

/// Width (pt) of the object selected now, rounded; None for a text selection or caret.
fn selected_width(s: &Session) -> Option<i32> {
    object_selection(s).and_then(|(_, o)| o.frame()).map(|(w, _, _)| w.round() as i32)
}

fn next(s: &mut Session, backward: bool) -> Option<i32> {
    run(s, "select.nextFloatingShape", json!({"backward": backward}));
    selected_width(s)
}

#[test]
fn ctrl_alt_5_then_tab_cycles_floating_shapes_in_document_order() {
    let mut s = doc();
    assert_eq!(selected_width(&s), None, "starts with a caret");
    let got: Vec<_> = (0..5).map(|_| next(&mut s, false)).collect();
    assert_eq!(got, [Some(100), Some(140), Some(160), Some(100), Some(140)], "the inline 120 pt shape is skipped; wraps");
}

#[test]
fn shift_tab_cycles_backwards() {
    let mut s = doc();
    let got: Vec<_> = (0..4).map(|_| next(&mut s, true)).collect();
    assert_eq!(got, [Some(160), Some(140), Some(100), Some(160)]);
    // Forward again from the selected 160 pt shape wraps to the first.
    assert_eq!(next(&mut s, false), Some(100));
}

#[test]
fn from_an_inline_object_it_starts_at_the_first_floating_shape() {
    let mut s = doc();
    let mut found = false;
    for i in 0..4 {
        run(&mut s, "arrange.selectObject", json!({"index": i}));
        if selected_width(&s) == Some(120) {
            found = true;
            break;
        }
    }
    assert!(found, "the inline 120 pt rectangle is selectable");
    assert_eq!(next(&mut s, false), Some(100));
}

#[test]
fn shapes_hidden_in_the_selection_pane_are_skipped() {
    let mut s = doc();
    let idx = (0..4u64)
        .find(|&i| {
            run(&mut s, "arrange.selectObject", json!({"index": i}));
            selected_width(&s) == Some(140)
        })
        .expect("the 140 pt rectangle is in the Selection Pane");
    run(&mut s, "arrange.setVisible", json!({"index": idx, "visible": false}));
    run(&mut s, "caret.docStart", json!({}));
    let got: Vec<_> = (0..3).map(|_| next(&mut s, false)).collect();
    assert_eq!(got, [Some(100), Some(160), Some(100)]);
}

#[test]
fn no_floating_shape_fails_and_keeps_the_selection() {
    let mut s = Session::new(Document::from_text("Just text"));
    run(&mut s, "caret.docEnd", json!({}));
    let before = s.sel.clone();
    assert!(s.run("select.nextFloatingShape", &json!({})).is_err());
    assert_eq!(s.sel, before);
}

#[test]
fn ctrl_alt_5_is_bound_on_linux_and_windows() {
    let id = crate::cmd::registry().by_shortcut("Mod+Alt+5").map(|c| c.id);
    if cfg!(target_os = "macos") {
        assert_ne!(id, Some("select.nextFloatingShape"));
    } else {
        assert_eq!(id, Some("select.nextFloatingShape"));
    }
}
