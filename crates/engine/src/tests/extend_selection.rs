//! F8 extend mode and Shift+F8 shrink, as in Word: the first F8 turns extend mode on without
//! selecting, the arrow keys then extend, each further F8 grows the selection one step (word,
//! sentence, paragraph, section when there are several, document) and Shift+F8 goes back one step.

use serde_json::{Value, json};

use crate::{Session, cmd};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn doc() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("One two. Three four.\nFive."));
    run(&mut s, "caret.docStart", json!({}));
    s
}

fn f8(s: &mut Session, n: usize) {
    for _ in 0..n {
        run(s, "select.extend", json!({}));
    }
}

fn collapsed(s: &Session) -> bool {
    let (a, b) = s.sel.ordered();
    a == b
}

#[test]
fn the_first_f8_selects_nothing_and_later_presses_grow_the_selection() {
    let mut s = doc();
    f8(&mut s, 1);
    assert!(collapsed(&s), "first F8 only turns extend mode on");
    f8(&mut s, 1);
    assert_eq!(s.selected_text().trim_end(), "One");
    f8(&mut s, 1);
    assert_eq!(s.selected_text().trim_end(), "One two.");
    f8(&mut s, 1);
    assert_eq!(s.selected_text().trim_end(), "One two. Three four.");
    // One section: the next step is the whole document.
    f8(&mut s, 1);
    let (a, b) = s.sel.ordered();
    assert_eq!(a, s.doc.start_of(wordcraft_doc::StoryRef::Body));
    assert_eq!(b, s.doc.end_of(wordcraft_doc::StoryRef::Body));
    let whole = s.sel.ordered();
    f8(&mut s, 1);
    assert_eq!(s.sel.ordered(), whole, "F8 past the document changes nothing");
}

#[test]
fn shift_f8_shrinks_back_one_step_at_a_time_to_the_caret() {
    let mut s = doc();
    let caret = s.sel.ordered();
    f8(&mut s, 4); // on, word, sentence, paragraph
    assert_eq!(s.selected_text().trim_end(), "One two. Three four.");
    run(&mut s, "select.shrink", json!({}));
    assert_eq!(s.selected_text().trim_end(), "One two.");
    run(&mut s, "select.shrink", json!({}));
    assert_eq!(s.selected_text().trim_end(), "One");
    run(&mut s, "select.shrink", json!({}));
    assert_eq!(s.sel.ordered(), caret);
    run(&mut s, "select.shrink", json!({}));
    assert_eq!(s.sel.ordered(), caret, "nothing left to shrink");
}

#[test]
fn shift_f8_without_f8_history_changes_nothing() {
    let mut s = doc();
    run(&mut s, "select.word", json!({}));
    let before = s.sel.ordered();
    run(&mut s, "select.shrink", json!({}));
    assert_eq!(s.sel.ordered(), before);
}

#[test]
fn in_extend_mode_the_arrow_keys_extend_until_it_is_turned_off() {
    let mut s = doc();
    f8(&mut s, 1);
    for _ in 0..3 {
        run(&mut s, "caret.right", json!({}));
    }
    assert_eq!(s.selected_text(), "One");
    run(&mut s, "select.extend", json!({"on": false}));
    assert_eq!(s.selected_text(), "One", "turning the mode off keeps the selection");
    run(&mut s, "caret.right", json!({}));
    assert!(collapsed(&s), "a plain arrow moves the caret again");
}

#[test]
fn f8_with_text_already_selected_grows_at_once() {
    let mut s = doc();
    run(&mut s, "select.word", json!({}));
    f8(&mut s, 1);
    assert_eq!(s.selected_text().trim_end(), "One two.");
}

#[test]
fn extend_and_shrink_keys() {
    let id = |k: &str| cmd::registry().by_shortcut(k).map(|c| c.id);
    assert_eq!(id("F8"), Some("select.extend"));
    assert_eq!(id("Shift+F8"), Some("select.shrink"));
}
