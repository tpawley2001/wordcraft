//! Insert Table's AutoFit behavior, as Word's Insert Table dialog offers it: Fixed column width
//! (Auto or a width), AutoFit to contents and AutoFit to window, in one undo step.

use serde_json::{Value, json};
use wordcraft_doc::{Document, Path, StoryRef, Table};

use crate::Session;

fn session() -> Session {
    let mut s = Session::new(Document::new());
    s.run("document.setText", &json!({"text": "Before"})).unwrap();
    s
}

fn inserted(s: &Session, r: &Value) -> Table {
    let path: Vec<u32> = serde_json::from_value(r["table"].clone()).unwrap();
    s.doc.table(StoryRef::Body, &Path(path)).cloned().expect("the inserted table")
}

/// `insert.table` with `extra` params, the table it made and the undo steps it took.
fn insert(extra: Value) -> (Table, usize) {
    let mut s = session();
    let depth = s.undo_depth();
    let mut v = json!({"rows": 2, "cols": 3});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    let r = s.run("insert.table", &v).unwrap();
    (inserted(&s, &r), s.undo_depth() - depth)
}

/// The table `insert.table` then `table.autofit {mode}` makes today.
fn insert_then_autofit(mode: &str) -> Table {
    let mut s = session();
    let r = s.run("insert.table", &json!({"rows": 2, "cols": 3})).unwrap();
    s.run("table.autofit", &json!({"mode": mode})).unwrap();
    inserted(&s, &r)
}

fn same_layout(a: &Table, b: &Table) {
    assert_eq!(a.grid, b.grid, "grid");
    assert_eq!(a.props.fixed, b.props.fixed, "fixed layout");
    assert_eq!(a.props.width, b.props.width, "preferred width");
    assert_eq!(a.props.width_pct, b.props.width_pct, "preferred width %");
}

#[test]
fn insert_table_autofit_to_window_matches_table_autofit_window() {
    let (t, steps) = insert(json!({"autofit": "window"}));
    same_layout(&t, &insert_then_autofit("window"));
    assert_eq!(steps, 1, "one undo step");
}

#[test]
fn insert_table_autofit_to_contents_matches_table_autofit_contents() {
    let (t, steps) = insert(json!({"autofit": "contents"}));
    same_layout(&t, &insert_then_autofit("contents"));
    assert_eq!(steps, 1, "one undo step");
}

#[test]
fn insert_table_fixed_auto_matches_table_autofit_fixed() {
    let (t, steps) = insert(json!({"autofit": "fixed"}));
    same_layout(&t, &insert_then_autofit("fixed"));
    assert!(t.props.fixed);
    assert_eq!(steps, 1, "one undo step");
}

#[test]
fn insert_table_fixed_column_width_sets_every_column() {
    let (t, steps) = insert(json!({"autofit": "fixed", "colWidth": 72}));
    assert_eq!(t.grid, vec![72.0, 72.0, 72.0]);
    assert!(t.props.fixed);
    assert_eq!(steps, 1, "one undo step");
}

/// Guard: without `autofit` the table is today's: equal columns across the text width.
#[test]
fn insert_table_without_autofit_is_unchanged() {
    let (t, steps) = insert(json!({}));
    let tw = session().doc.sections().first().map(|(_, sp)| sp.text_width()).unwrap();
    assert_eq!(t.grid.len(), 3);
    for g in &t.grid {
        assert!((g - tw / 3.0).abs() < 0.01, "{g} vs {}", tw / 3.0);
    }
    assert_eq!(steps, 1, "one undo step");
}
