//! Word's table keys Alt+Home / Alt+End (first / last cell in the row) and Alt+PageUp /
//! Alt+PageDown (first / last cell in the column).

use serde_json::{Value, json};
use wordcraft_doc::{Pos, StoryRef};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

fn caret_in(s: &mut Session, r: u32, c: u32) {
    s.sel = Selection::caret(Pos { story: StoryRef::Body, path: wordcraft_doc::Path(vec![0, r, c, 0]), off: 0 });
}

/// The (row, column) of the cell the caret is in.
fn caret_cell(s: &Session) -> (usize, usize) {
    let (_, r, c) = s.sel.focus.path.cell().expect("caret in a cell");
    (r, c)
}

/// A 3x4 table with text in every cell and the caret in the middle row, second column.
fn grid() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "insert.table", json!({"rows": 3, "cols": 4}));
    for i in 0..12 {
        if i > 0 {
            run(&mut s, "text.tab", json!({}));
        }
        run(&mut s, "text.insert", json!({"text": format!("c{i}")}));
    }
    caret_in(&mut s, 1, 1);
    s
}

const MOVES: [(&str, &str, (usize, usize)); 4] = [
    ("table.firstCellInRow", "Alt+Home", (1, 0)),
    ("table.lastCellInRow", "Alt+End", (1, 3)),
    ("table.firstCellInColumn", "Alt+PageUp", (0, 1)),
    ("table.lastCellInColumn", "Alt+PageDown", (2, 1)),
];

#[test]
fn each_key_goes_to_the_end_cell_of_the_row_or_column() {
    for (id, _, want) in MOVES {
        let mut s = grid();
        let before = s.doc.clone();
        run(&mut s, id, json!({}));
        assert_eq!(caret_cell(&s), want, "{id}");
        assert!(s.sel.is_collapsed(), "{id}: the caret is collapsed");
        assert_eq!(s.doc, before, "{id}: the document is unchanged");
    }
}

#[test]
fn the_keys_are_bound_as_in_word() {
    let s = grid();
    for (id, key, _) in MOVES {
        assert_eq!(s.keymap.resolve(&s.registry, key).map(|c| c.id), Some(id), "{key}");
    }
}

#[test]
fn a_selection_inside_the_table_collapses_into_the_target_cell() {
    let mut s = grid();
    run(&mut s, "table.selectRow", json!({}));
    run(&mut s, "table.lastCellInColumn", json!({}));
    assert!(s.sel.is_collapsed());
    assert_eq!(caret_cell(&s).0, 2, "the last row");
}

#[test]
fn already_at_the_end_the_caret_stays_in_that_cell() {
    let mut s = grid();
    caret_in(&mut s, 1, 3);
    run(&mut s, "table.lastCellInRow", json!({}));
    assert_eq!(caret_cell(&s), (1, 3));
    caret_in(&mut s, 0, 2);
    run(&mut s, "table.firstCellInColumn", json!({}));
    assert_eq!(caret_cell(&s), (0, 2));
}

#[test]
fn outside_a_table_nothing_moves() {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "text.insert", json!({"text": "Hello"}));
    let sel = s.sel.clone();
    for (id, _, _) in MOVES {
        let _ = s.run(id, &json!({}));
        assert_eq!(s.sel, sel, "{id}: the caret stays put outside a table");
        assert_eq!(s.doc.body.len(), 1, "{id}");
    }
}
