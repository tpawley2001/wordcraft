//! Increase / Decrease Font Size (Ctrl+Shift+> / Ctrl+Shift+<) step as in Word: one point at a time
//! below 8 pt, past 10.5 in the list, by tens above 72 pt. Measured in Microsoft 365 Word 16.0.20527
//! (Windows, en-US) through `Font.Grow` / `Font.Shrink`, 2026-11-14.

use serde_json::json;
use wordcraft_doc::{Document, Pos};

use crate::Session;

fn at(size: f32) -> Session {
    let mut doc = Document::from_text("Sample text");
    let end = doc.para_at(&Pos::body(0, 0)).unwrap().len();
    doc.format_range(&Pos::body(0, 0), &Pos::body(0, end), &|c| {
        c.size = Some(size);
        c.size_cs = Some(size);
    })
    .unwrap();
    let mut s = Session::new(doc);
    s.run("select.all", &json!({})).unwrap();
    s
}

fn size_at(s: &Session, off: usize) -> f32 {
    let p = s.doc.para_at(&Pos::body(0, 0)).unwrap();
    s.doc.styles.resolve_char(p.props.style.as_deref(), p.props_of_char(off)).size
}

fn step(command: &str, from: f32) -> f32 {
    let mut s = at(from);
    s.run(command, &json!({})).unwrap();
    size_at(&s, 0)
}

/// (start size, Word's Increase Font Size, Word's Decrease Font Size).
const WORD: [(f32, f32, f32); 23] = [
    (1.0, 2.0, 1.0),
    (1.5, 2.5, 1.0),
    (2.0, 3.0, 1.0),
    (5.0, 6.0, 4.0),
    (6.0, 7.0, 5.0),
    (7.0, 8.0, 6.0),
    (7.5, 8.0, 6.5),
    (8.0, 9.0, 7.0),
    (8.5, 9.0, 8.0),
    (9.0, 10.0, 8.0),
    (10.0, 11.0, 9.0),
    (10.5, 11.0, 10.0),
    (11.0, 12.0, 10.0),
    (12.0, 14.0, 11.0),
    (13.0, 14.0, 12.0),
    (28.0, 36.0, 26.0),
    (72.0, 80.0, 48.0),
    (75.0, 80.0, 72.0),
    (80.0, 90.0, 72.0),
    (90.0, 100.0, 80.0),
    (1630.0, 1638.0, 1620.0),
    (1637.0, 1638.0, 1630.0),
    (1638.0, 1638.0, 1630.0),
];

#[test]
fn increase_font_size_steps_as_in_word() {
    let got: Vec<(f32, f32)> = WORD.iter().map(|&(from, _, _)| (from, step("format.growFont", from))).collect();
    let want: Vec<(f32, f32)> = WORD.iter().map(|&(from, up, _)| (from, up)).collect();
    assert_eq!(got, want);
}

#[test]
fn decrease_font_size_steps_as_in_word() {
    let got: Vec<(f32, f32)> = WORD.iter().map(|&(from, _, _)| (from, step("format.shrinkFont", from))).collect();
    let want: Vec<(f32, f32)> = WORD.iter().map(|&(from, _, down)| (from, down)).collect();
    assert_eq!(got, want);
}

#[test]
fn increase_from_one_point_reaches_eight_in_seven_steps_then_follows_the_list() {
    let mut s = at(1.0);
    let mut seen = Vec::new();
    for _ in 0..10 {
        s.run("format.growFont", &json!({})).unwrap();
        seen.push(size_at(&s, 0));
    }
    assert_eq!(seen, [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0]);
}

#[test]
fn each_run_of_a_mixed_selection_steps_from_its_own_size() {
    // "Sample" at 5 pt, " text" at 10.5 pt.
    let mut s = at(5.0);
    let end = s.doc.para_at(&Pos::body(0, 0)).unwrap().len();
    s.doc
        .format_range(&Pos::body(0, 6), &Pos::body(0, end), &|c| {
            c.size = Some(10.5);
            c.size_cs = Some(10.5);
        })
        .unwrap();
    s.run("select.all", &json!({})).unwrap();
    s.run("format.growFont", &json!({})).unwrap();
    assert_eq!((size_at(&s, 0), size_at(&s, 7)), (6.0, 11.0));
    s.run("format.shrinkFont", &json!({})).unwrap();
    s.run("format.shrinkFont", &json!({})).unwrap();
    assert_eq!((size_at(&s, 0), size_at(&s, 7)), (4.0, 9.0));
}

#[test]
fn grow_and_shrink_font_one_point_are_unchanged() {
    for (from, up, down) in [(10.5, 11.5, 9.5), (1.0, 2.0, 1.0), (7.5, 8.5, 6.5), (1638.0, 1638.0, 1637.0)] {
        assert_eq!(step("format.growFont1", from), up, "Grow Font 1 Point from {from}");
        assert_eq!(step("format.shrinkFont1", from), down, "Shrink Font 1 Point from {from}");
    }
}
