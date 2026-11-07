//! Word's StartOfWindow / EndOfWindow on Ctrl+Alt+PageUp / Ctrl+Alt+PageDown (Linux/Windows):
//! the caret goes to the start of the first line shown in the window, or the end of the last
//! one; Shift extends. The shell passes the window's top or bottom edge as `page` and `y`.

use serde_json::{Value, json};
use wordcraft_doc::Pos;

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

const LINES: usize = 40;

fn text(i: usize) -> String {
    format!("Line {i:02}")
}

/// 40 one-line paragraphs, caret at the start of the document.
fn doc() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    for i in 0..LINES {
        if i > 0 {
            run(&mut s, "text.newParagraph", json!({}));
        }
        run(&mut s, "text.insert", json!({"text": text(i)}));
    }
    run(&mut s, "caret.docStart", json!({}));
    s
}

/// (page, top, bottom) of paragraph `i`'s only line, measured from the layout.
fn line(s: &mut Session, i: usize) -> (usize, f32, f32) {
    let c = s.layout().caret_on(&Pos::body(i, 0), 0).expect("line laid out");
    (c.page, c.top, c.top + c.height)
}

#[test]
fn top_goes_to_the_start_of_the_first_whole_line() {
    let mut s = doc();
    let (page, top, _) = line(&mut s, 5);
    assert_eq!(page, 0);
    // The window's top edge exactly at line 5's top: line 5 is shown whole.
    run(&mut s, "caret.windowTop", json!({"page": 0, "y": top}));
    assert_eq!(s.sel.focus, Pos::body(5, 0));
    assert!(s.sel.is_collapsed());
    // The edge cuts line 5: the first whole line is line 6.
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "caret.windowTop", json!({"page": 0, "y": top + 2.0}));
    assert_eq!(s.sel.focus, Pos::body(6, 0));
}

#[test]
fn bottom_goes_to_the_end_of_the_last_whole_line() {
    let mut s = doc();
    let (_, _, bottom) = line(&mut s, 12);
    run(&mut s, "caret.windowBottom", json!({"page": 0, "y": bottom}));
    assert_eq!(s.sel.focus, Pos::body(12, text(12).len()));
    assert!(s.sel.is_collapsed());
    // The edge cuts line 12: the last whole line is line 11.
    run(&mut s, "caret.windowBottom", json!({"page": 0, "y": bottom - 2.0}));
    assert_eq!(s.sel.focus, Pos::body(11, text(11).len()));
}

#[test]
fn extend_keeps_the_anchor() {
    let mut s = doc();
    s.sel = Selection::caret(Pos::body(3, 2));
    let (_, _, bottom) = line(&mut s, 9);
    run(&mut s, "caret.windowBottom", json!({"page": 0, "y": bottom, "extend": true}));
    assert_eq!(s.sel.anchor, Pos::body(3, 2));
    assert_eq!(s.sel.focus, Pos::body(9, text(9).len()));
    let (_, top, _) = line(&mut s, 1);
    run(&mut s, "caret.windowTop", json!({"page": 0, "y": top, "extend": true}));
    assert_eq!(s.sel.anchor, Pos::body(3, 2));
    assert_eq!(s.sel.focus, Pos::body(1, 0));
}

#[test]
fn the_document_is_unchanged_and_bad_params_refuse() {
    let mut s = doc();
    let before = s.doc.clone();
    let (_, top, _) = line(&mut s, 4);
    run(&mut s, "caret.windowTop", json!({"page": 0, "y": top}));
    assert_eq!(s.doc, before);
    let sel = s.sel.clone();
    assert!(s.run("caret.windowTop", &json!({})).is_err(), "page and y are required");
    assert!(s.run("caret.windowBottom", &json!({"page": 0})).is_err(), "y is required");
    assert_eq!(s.sel, sel, "a refused move leaves the selection");
}

#[test]
fn the_keys_are_bound_as_in_word_for_windows() {
    let s = doc();
    for (key, id) in [("Mod+Alt+PageUp", "caret.windowTop"), ("Mod+Alt+PageDown", "caret.windowBottom")] {
        let got = s.keymap.resolve(&s.registry, key).map(|c| c.id);
        if cfg!(target_os = "macos") {
            assert_ne!(got, Some(id), "{key}: Word for Mac has no default key");
        } else {
            assert_eq!(got, Some(id), "{key}");
        }
    }
    // The table keys on Alt alone stay as they are.
    assert_eq!(s.keymap.resolve(&s.registry, "Alt+PageUp").map(|c| c.id), Some("table.firstCellInColumn"));
}
