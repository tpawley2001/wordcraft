//! Go To (F5) › Line: `edit.goto {"what": "line", "n"?, "by"?}` goes to the start of the n-th layout
//! line of the main text, or `by` lines from the caret's line (clamped to the document).

use serde_json::{Value, json};
use wordcraft_doc::{Pos, StoryRef};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id} {v}: {e}"))
}

/// Body paragraph index and the text from the caret to the end of its paragraph.
fn at_caret(s: &Session) -> (usize, String) {
    assert_eq!(s.sel.focus.story, StoryRef::Body);
    assert!(s.sel.is_collapsed(), "Go To leaves a caret");
    let i = s.doc.para_paths(StoryRef::Body).iter().position(|p| *p == s.sel.focus.path).expect("caret in a body paragraph");
    let p = s.doc.para_at(&s.sel.focus).expect("caret paragraph");
    (i, p.text[s.sel.focus.off..].to_string())
}

/// Five lines: "alpha", "bravo", "charlie" in paragraph 0 (two manual line breaks), then
/// "delta" and "echo" as paragraphs 1 and 2. The caret starts at the top.
fn five_lines() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("alpha\ndelta\necho"));
    s.sel = Selection::caret(Pos::body(0, 5));
    run(&mut s, "text.lineBreak", json!({}));
    run(&mut s, "text.insert", json!({"text": "bravo"}));
    run(&mut s, "text.lineBreak", json!({}));
    run(&mut s, "text.insert", json!({"text": "charlie"}));
    s.sel = Selection::caret(Pos::body(0, 0));
    s
}

#[test]
fn line_by_number_counts_manual_line_breaks_and_paragraphs() {
    let mut s = five_lines();
    let r = run(&mut s, "edit.goto", json!({"what": "line", "n": 2}));
    let (para, rest) = at_caret(&s);
    assert_eq!(para, 0);
    assert!(rest.starts_with("bravo"), "line 2 starts at bravo: {rest:?}");
    assert_eq!((r["found"].as_bool(), r["what"].as_str(), r["index"].as_u64(), r["count"].as_u64()), (Some(true), Some("line"), Some(2), Some(5)));

    run(&mut s, "edit.goto", json!({"what": "line", "n": 4}));
    assert_eq!(at_caret(&s), (1, "delta".to_string()));
}

#[test]
fn line_number_is_clamped_to_the_document() {
    let mut s = five_lines();
    let r = run(&mut s, "edit.goto", json!({"what": "line", "n": 99}));
    assert_eq!(at_caret(&s), (2, "echo".to_string()));
    assert_eq!(r["index"].as_u64(), Some(5));
    let r = run(&mut s, "edit.goto", json!({"what": "line", "n": 0}));
    assert_eq!(at_caret(&s), (0, s.doc.para_at(&Pos::body(0, 0)).unwrap().text.clone()));
    assert_eq!(r["index"].as_u64(), Some(1));
    run(&mut s, "edit.goto", json!({"what": "line", "n": u64::MAX}));
    assert_eq!(at_caret(&s).0, 2);
}

#[test]
fn plus_and_minus_move_from_the_caret_line() {
    let mut s = five_lines();
    run(&mut s, "edit.goto", json!({"what": "line", "n": 2}));
    let r = run(&mut s, "edit.goto", json!({"what": "line", "by": 2}));
    assert_eq!(at_caret(&s), (1, "delta".to_string()));
    assert_eq!(r["index"].as_u64(), Some(4));
    let r = run(&mut s, "edit.goto", json!({"what": "line", "by": -1}));
    assert_eq!(at_caret(&s), (0, "charlie".to_string()));
    assert_eq!(r["index"].as_u64(), Some(3));
    // Next (no number) is one line down.
    run(&mut s, "edit.goto", json!({"what": "line"}));
    assert_eq!(at_caret(&s), (1, "delta".to_string()));
}

#[test]
fn relative_moves_stop_at_the_first_and_last_line() {
    let mut s = five_lines();
    run(&mut s, "edit.goto", json!({"what": "line", "by": -3}));
    assert_eq!(at_caret(&s).0, 0);
    assert_eq!(s.sel.focus.off, 0);
    run(&mut s, "edit.goto", json!({"what": "line", "by": 50}));
    assert_eq!(at_caret(&s), (2, "echo".to_string()));
    run(&mut s, "edit.goto", json!({"what": "line", "by": i64::MIN}));
    assert_eq!((at_caret(&s).0, s.sel.focus.off), (0, 0));
    run(&mut s, "edit.goto", json!({"what": "line", "by": i64::MAX}));
    assert_eq!(at_caret(&s).0, 2);
}

#[test]
fn by_zero_is_not_found_and_keeps_the_caret() {
    let mut s = five_lines();
    s.sel = Selection::caret(Pos::body(1, 2));
    let r = run(&mut s, "edit.goto", json!({"what": "line", "by": 0}));
    assert_eq!(r["found"].as_bool(), Some(false));
    assert_eq!(r["count"].as_u64(), Some(5));
    assert_eq!(s.sel.focus, Pos::body(1, 2));
}

#[test]
fn a_wrapped_paragraph_is_several_lines() {
    // One long paragraph that wraps, then "tail".
    let long = vec!["word"; 400].join(" ");
    let mut s = Session::new(wordcraft_doc::Document::from_text(&format!("{long}\ntail")));
    let r = run(&mut s, "edit.goto", json!({"what": "line", "n": 2}));
    let (para, _) = at_caret(&s);
    assert_eq!(para, 0, "line 2 is still in the long paragraph");
    let off = s.sel.focus.off;
    assert!(off > 0 && long[..off].ends_with(' '), "line 2 starts at a word: {off}");
    let count = r["count"].as_u64().unwrap();
    assert!(count >= 4, "400 words wrap onto several lines, plus tail: {count}");
    run(&mut s, "edit.goto", json!({"what": "line", "n": count}));
    assert_eq!(at_caret(&s), (1, "tail".to_string()));
}

#[test]
fn header_lines_do_not_count() {
    let mut s = five_lines();
    run(&mut s, "edit.goto", json!({"what": "line", "n": 1}));
    let before = run(&mut s, "edit.goto", json!({"what": "line", "n": 99}))["count"].as_u64();
    run(&mut s, "insert.header", json!({"text": "header one\nheader two"}));
    s.sel = Selection::caret(Pos::body(0, 0));
    let after = run(&mut s, "edit.goto", json!({"what": "line", "n": 99}))["count"].as_u64();
    assert_eq!(before, after);
    assert_eq!(at_caret(&s), (2, "echo".to_string()));
}
