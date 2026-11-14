//! Ctrl+Shift+Enter in a table splits it (in the first row it adds an empty paragraph above
//! the table), and Alt+Shift+Up / Alt+Shift+Down move the whole table row, as in Word.
//! Expected results measured in Microsoft 365 Word 16.0.20527 (InsertColumnBreak,
//! OutlineMoveUp / OutlineMoveDown and Split Table run with the caret in a cell).

use serde_json::{Value, json};
use wordcraft_doc::{Block, StoryRef};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The body as one string per block: a paragraph's text, or `T[..]` listing each row's
/// first-cell text (its paragraphs joined by `/`).
fn shape(s: &Session) -> Vec<String> {
    s.doc
        .body
        .iter()
        .map(|b| match &**b {
            Block::Para(p) => p.text.clone(),
            Block::Table(t) => {
                let rows: Vec<String> = t
                    .rows
                    .iter()
                    .map(|r| r.cells[0].blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect::<Vec<_>>().join("/"))
                    .collect();
                format!("T[{}]", rows.join(","))
            }
        })
        .collect()
}

/// `before`, a table of `rows` rows (cell 1 = r1.., cell 2 = x), then an empty paragraph.
fn doc(rows: usize) -> Session {
    let mut text = String::from("before\n");
    for r in 1..=rows {
        text.push_str(&format!("r{r}\tx\n"));
    }
    let mut s = Session::new(wordcraft_doc::Document::from_text(&text));
    let anchor = json!({"story": "body", "path": [1], "off": 0});
    let focus = json!({"story": "body", "path": [rows], "off": 4});
    run(&mut s, "select.range", json!({"anchor": anchor, "focus": focus}));
    run(&mut s, "table.fromText", json!({}));
    s
}

/// Caret at the start of paragraph `para` of cell (`row`, `col`) of the table (body block 1).
fn caret(s: &mut Session, row: usize, col: usize, para: usize) {
    run(s, "caret.set", json!({"pos": {"story": "body", "path": [1, row, col, para], "off": 0}}));
}

fn v(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| x.to_string()).collect()
}

#[test]
fn setup_builds_before_table_and_final_paragraph() {
    let s = doc(4);
    assert_eq!(shape(&s), v(&["before", "T[r1,r2,r3,r4]", ""]));
}

#[test]
fn ctrl_shift_enter_in_row_three_splits_the_table() {
    // A1
    let mut s = doc(4);
    caret(&mut s, 2, 0, 0);
    run(&mut s, "text.columnBreak", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r2]", "", "T[r3,r4]", ""]));
    assert_eq!(s.sel.focus.story, StoryRef::Body);
    assert_eq!(s.sel.focus.path.0.len(), 1, "caret in the new paragraph between the tables, not in a cell");
    assert_eq!(s.sel.focus.path.last(), 2);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r2,r3,r4]", ""]));
}

#[test]
fn ctrl_shift_enter_in_row_two_of_three_splits_the_table() {
    // A2
    let mut s = doc(3);
    caret(&mut s, 1, 1, 0);
    run(&mut s, "text.columnBreak", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1]", "", "T[r2,r3]", ""]));
}

#[test]
fn ctrl_shift_enter_in_the_first_row_adds_a_paragraph_above_the_table() {
    // A3
    let mut s = doc(4);
    caret(&mut s, 0, 0, 0);
    run(&mut s, "text.columnBreak", json!({}));
    assert_eq!(shape(&s), v(&["before", "", "T[r1,r2,r3,r4]", ""]));
    assert_eq!(s.sel.focus.path.0.len(), 1);
    assert_eq!(s.sel.focus.path.last(), 1);
}

#[test]
fn split_table_in_the_first_row_adds_a_paragraph_above_the_table() {
    // A4
    let mut s = doc(4);
    caret(&mut s, 0, 1, 0);
    run(&mut s, "table.splitTable", json!({}));
    assert_eq!(shape(&s), v(&["before", "", "T[r1,r2,r3,r4]", ""]));
}

#[test]
fn ctrl_shift_enter_outside_a_table_still_inserts_a_column_break() {
    let mut s = Session::new(wordcraft_doc::Document::from_text("one two"));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 3}}));
    run(&mut s, "text.columnBreak", json!({}));
    assert!(s.doc.plain_text(StoryRef::Body).contains(wordcraft_doc::para::COLUMN_BREAK));
}

#[test]
fn alt_shift_up_moves_the_row_up() {
    // B1
    let mut s = doc(4);
    caret(&mut s, 2, 1, 0);
    run(&mut s, "para.moveUp", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r3,r2,r4]", ""]));
    // The caret stays in the moved row's second cell.
    assert_eq!(s.sel.focus.path.0, vec![1, 1, 1, 0]);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r2,r3,r4]", ""]));
}

/// Row 2's first cell holds three paragraphs p1, p2, p3.
fn doc_with_three_paragraph_cell() -> Session {
    let mut s = doc(3);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [1, 1, 0, 0], "off": 0}}));
    run(&mut s, "select.range", json!({"anchor": {"story": "body", "path": [1, 1, 0, 0], "off": 0}, "focus": {"story": "body", "path": [1, 1, 0, 0], "off": 2}}));
    run(&mut s, "text.insert", json!({"text": "p1\np2\np3", "paragraphs": true}));
    assert_eq!(shape(&s), v(&["before", "T[r1,p1/p2/p3,r3]", ""]));
    s
}

#[test]
fn alt_shift_up_moves_the_whole_row_from_any_paragraph_of_a_cell() {
    // B2
    for para in 0..3 {
        let mut s = doc_with_three_paragraph_cell();
        caret(&mut s, 1, 0, para);
        run(&mut s, "para.moveUp", json!({}));
        assert_eq!(shape(&s), v(&["before", "T[p1/p2/p3,r1,r3]", ""]), "caret in p{}", para + 1);
    }
}

#[test]
fn alt_shift_down_moves_the_whole_row_from_the_last_paragraph_of_a_cell() {
    // B3
    let mut s = doc_with_three_paragraph_cell();
    caret(&mut s, 1, 0, 2);
    run(&mut s, "para.moveDown", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r3,p1/p2/p3]", ""]));
}

#[test]
fn alt_shift_up_in_the_first_row_moves_it_above_the_paragraph_before_the_table() {
    // B4
    let mut s = doc(4);
    caret(&mut s, 0, 0, 0);
    run(&mut s, "para.moveUp", json!({}));
    assert_eq!(shape(&s), v(&["T[r1]", "before", "T[r2,r3,r4]", ""]));
}

#[test]
fn alt_shift_down_in_the_last_row_moves_it_below_the_paragraph_after_the_table() {
    // B5
    let mut s = doc(4);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [2], "off": 0}}));
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r2,r3,r4]", "", ""]));
    caret(&mut s, 3, 0, 0);
    run(&mut s, "para.moveDown", json!({}));
    assert_eq!(shape(&s), v(&["before", "T[r1,r2,r3]", "", "T[r4]", ""]));
}

#[test]
fn alt_shift_up_with_nothing_above_the_table_is_refused() {
    let mut s = doc(2);
    caret(&mut s, 0, 0, 0);
    run(&mut s, "para.moveUp", json!({}));
    assert_eq!(shape(&s), v(&["T[r1]", "before", "T[r2]", ""]));
    let before = shape(&s);
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0, 0, 0, 0], "off": 0}}));
    assert!(s.run("para.moveUp", &json!({})).is_err(), "nothing above the first table");
    assert_eq!(shape(&s), before);
}
