//! Alt+Shift+Home / End / Page Up / Page Down in a table extend the selection, as whole cells, from
//! the anchor cell to the first or last cell of the row or column (Word's StartOfRow, EndOfRow,
//! StartOfColumn and EndOfColumn with extend); the plain Alt keys keep moving the caret.

use serde_json::json;
use wordcraft_doc::{Block, Document, Path, Pos, StoryRef, Table};

use crate::{Selection, Session, cmd};

fn session(table: Table, path: Vec<u32>) -> Session {
    let mut doc = Document::new();
    doc.body.insert(0, Block::Table(table).into());
    let mut s = Session::new(doc);
    s.sel = Selection::caret(Pos { story: StoryRef::Body, path: Path(path), off: 0 });
    s
}

fn cells(s: &Session) -> Vec<(usize, usize)> {
    let range = s.cell_range().expect("a whole-cell selection");
    let t = s.doc.table(range.story, &range.table).expect("the range's table");
    let mut v = range.cells(t);
    v.sort_unstable();
    v
}

const KEYS: &[(&str, &str)] = &[
    ("Alt+Shift+Home", "table.selectToRowStart"),
    ("Alt+Shift+End", "table.selectToRowEnd"),
    ("Alt+Shift+PageUp", "table.selectToColumnTop"),
    ("Alt+Shift+PageDown", "table.selectToColumnBottom"),
];

#[test]
fn shifted_alt_keys_run_the_extend_commands_and_plain_ones_still_move() {
    for &(key, id) in KEYS {
        assert_eq!(cmd::registry().by_shortcut(key).map(|c| c.id), Some(id), "{key}");
    }
    for (key, id) in [
        ("Alt+Home", "table.firstCellInRow"),
        ("Alt+End", "table.lastCellInRow"),
        ("Alt+PageUp", "table.firstCellInColumn"),
        ("Alt+PageDown", "table.lastCellInColumn"),
    ] {
        assert_eq!(cmd::registry().by_shortcut(key).map(|c| c.id), Some(id), "{key}");
    }
}

#[test]
fn each_key_selects_from_the_current_cell_to_the_edge() {
    let want: &[(&str, &[(usize, usize)])] = &[
        ("table.selectToRowStart", &[(1, 0), (1, 1), (1, 2)]),
        ("table.selectToRowEnd", &[(1, 2), (1, 3)]),
        ("table.selectToColumnTop", &[(0, 2), (1, 2)]),
        ("table.selectToColumnBottom", &[(1, 2), (2, 2)]),
    ];
    for &(id, cells_want) in want {
        let mut s = session(Table::new(3, 4, 400.0), vec![0, 1, 2, 0]);
        let before = s.doc.clone();
        let undo = s.undo_labels();
        s.run(id, &json!({})).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        assert_eq!(cells(&s), cells_want.to_vec(), "{id}");
        assert_eq!(s.doc, before, "{id} changes nothing");
        assert_eq!(s.undo_labels(), undo, "{id} adds no undo step");
    }
}

#[test]
fn a_second_key_extends_from_the_same_anchor() {
    let mut s = session(Table::new(3, 4, 400.0), vec![0, 1, 2, 0]);
    s.run("table.selectToRowEnd", &json!({})).unwrap();
    s.run("table.selectToColumnBottom", &json!({})).unwrap();
    assert_eq!(cells(&s), vec![(1, 2), (1, 3), (2, 2), (2, 3)]);
}

#[test]
fn a_nested_table_extends_inside_itself() {
    let mut outer = Table::new(3, 4, 400.0);
    outer.rows[1].cells[1].blocks.insert(0, Block::Table(Table::new(3, 4, 300.0)).into());
    let mut s = session(outer, vec![0, 1, 1, 0, 1, 1, 0]);
    s.run("table.selectToRowEnd", &json!({})).unwrap();
    let range = s.cell_range().expect("a whole-cell selection");
    assert_eq!(range.table, Path(vec![0, 1, 1, 0]));
    assert_eq!(cells(&s), vec![(1, 1), (1, 2), (1, 3)]);
}

#[test]
fn outside_a_table_the_commands_are_disabled() {
    let mut s = Session::new(Document::from_text("plain text"));
    let sel = s.sel.clone();
    for &(_, id) in KEYS {
        assert!(matches!(s.run(id, &json!({})), Err(crate::CmdError::Disabled(_))), "{id}");
        assert_eq!(s.sel, sel, "{id}");
    }
}
