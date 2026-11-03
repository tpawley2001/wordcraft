//! Word's Insert Cells and Delete Cells options: Shift cells right / down / left / up, entire row / column.

use serde_json::{Value, json};
use wordcraft_doc::{Pos, Rgb, StoryRef, Table};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn table(s: &Session) -> &Table {
    s.doc.body.iter().find_map(|b| b.as_table()).expect("a table")
}

/// Every cell's text, row by row.
fn cells(s: &Session) -> Vec<Vec<String>> {
    table(s)
        .rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()).collect())
        .collect()
}

fn caret_in(s: &mut Session, r: u32, c: u32) {
    s.sel = Selection::caret(Pos { story: StoryRef::Body, path: wordcraft_doc::Path(vec![0, r, c, 0]), off: 0 });
}

/// A 3x3 table A1 B1 C1 / A2 B2 C2 / A3 B3 C3 with the caret in B2.
fn grid() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "insert.table", json!({"rows": 3, "cols": 3}));
    for (i, t) in ["A1", "B1", "C1", "A2", "B2", "C2", "A3", "B3", "C3"].iter().enumerate() {
        if i > 0 {
            run(&mut s, "text.tab", json!({}));
        }
        run(&mut s, "text.insert", json!({"text": t}));
    }
    caret_in(&mut s, 1, 1);
    s
}

fn rows(v: &[&[&str]]) -> Vec<Vec<String>> {
    v.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect()
}

fn caret_cell(s: &Session) -> (u32, u32) {
    let (_, r, c) = s.sel.focus.path.cell().expect("caret in a cell");
    (r as u32, c as u32)
}

#[test]
fn delete_cells_shift_left_is_the_default_and_keeps_todays_behaviour() {
    let mut s = grid();
    run(&mut s, "table.deleteCells", json!({}));
    assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["A2", "C2"], &["A3", "B3", "C3"]]));
    let mut s = grid();
    run(&mut s, "table.deleteCells", json!({"shift": "left"}));
    assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["A2", "C2"], &["A3", "B3", "C3"]]));
}

#[test]
fn delete_cells_shift_up_moves_the_column_up_and_keeps_the_row_count() {
    let mut s = grid();
    run(&mut s, "table.deleteCells", json!({"shift": "up"}));
    assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["A2", "B3", "C2"], &["A3", "", "C3"]]));
    assert_eq!(caret_cell(&s), (1, 1));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(cells(&s), cells(&grid()), "one undo step restores the table");
}

#[test]
fn delete_cells_entire_row_and_column() {
    let mut s = grid();
    run(&mut s, "table.deleteCells", json!({"shift": "row"}));
    assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["A3", "B3", "C3"]]));
    let mut s = grid();
    run(&mut s, "table.deleteCells", json!({"shift": "column"}));
    assert_eq!(cells(&s), rows(&[&["A1", "C1"], &["A2", "C2"], &["A3", "C3"]]));
}

#[test]
fn insert_cells_shift_down_is_the_default_and_adds_a_row_at_the_bottom() {
    for v in [json!({}), json!({"shift": "down"})] {
        let mut s = grid();
        run(&mut s, "table.insertCells", v.clone());
        assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["A2", "", "C2"], &["A3", "B2", "C3"], &["", "B3", ""]]), "{v}");
        assert_eq!(caret_cell(&s), (1, 1), "{v}");
        run(&mut s, "edit.undo", json!({}));
        assert_eq!(cells(&s), cells(&grid()), "{v}: one undo step restores the table");
    }
}

#[test]
fn insert_cells_shift_right_adds_a_cell_to_that_row_only() {
    let mut s = grid();
    // Give the caret cell a distinctive look; the new cell copies it.
    let (tp, _, _) = s.sel.focus.path.cell().unwrap();
    let t = s.doc.table_mut(StoryRef::Body, &tp).unwrap();
    t.rows[1].cells[1].props.shading = Rgb::parse("FFCC00");
    t.rows[1].cells[1].props.width = Some(97.0);
    run(&mut s, "table.insertCells", json!({"shift": "right"}));
    assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["A2", "", "B2", "C2"], &["A3", "B3", "C3"]]));
    let new = &table(&s).rows[1].cells[1];
    assert_eq!(new.props.shading, Rgb::parse("FFCC00"));
    assert_eq!(new.props.width, Some(97.0));
    assert_eq!(caret_cell(&s), (1, 1));
}

#[test]
fn insert_cells_entire_row_and_column() {
    let mut s = grid();
    run(&mut s, "table.insertCells", json!({"shift": "row"}));
    assert_eq!(cells(&s), rows(&[&["A1", "B1", "C1"], &["", "", ""], &["A2", "B2", "C2"], &["A3", "B3", "C3"]]));
    let mut s = grid();
    run(&mut s, "table.insertCells", json!({"shift": "column"}));
    assert_eq!(cells(&s), rows(&[&["A1", "", "B1", "C1"], &["A2", "", "B2", "C2"], &["A3", "", "B3", "C3"]]));
}

#[test]
fn unknown_shift_values_are_refused() {
    let mut s = grid();
    assert!(s.run("table.insertCells", &json!({"shift": "left"})).is_err());
    assert!(s.run("table.deleteCells", &json!({"shift": "down"})).is_err());
    assert_eq!(cells(&s), cells(&grid()));
}

#[test]
fn content_moves_are_refused_under_track_changes() {
    for (id, shift) in [("table.deleteCells", "up"), ("table.insertCells", "right"), ("table.insertCells", "down")] {
        let mut s = grid();
        run(&mut s, "review.trackChanges", json!({"value": true}));
        assert!(s.run(id, &json!({"shift": shift})).is_err(), "{id} {shift}");
        assert_eq!(cells(&s), cells(&grid()), "{id} {shift} changed the table");
    }
}

#[test]
fn insert_cells_is_in_the_catalog() {
    let mut s = grid();
    assert!(s.run("table.insertCells", &json!({})).is_ok());
    let found = crate::catalog::entries().iter().any(|e| e.3 == "table.insertCells");
    assert!(found, "Insert Cells is listed in the feature catalog");
}
