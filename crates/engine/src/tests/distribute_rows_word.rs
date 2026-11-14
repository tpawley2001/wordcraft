//! Table Layout › Distribute Rows as in Word: the selected rows share their total laid-out height
//! equally, each set to that height At least; other rows keep theirs. Measured in Microsoft 365 Word
//! 16.0.20527 for Windows (`Rows.DistributeHeight`), 2026-11-14.

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_doc::props::HeightRule;

use crate::Session;

/// The body index of the (only) table.
fn table_at(s: &Session) -> usize {
    s.doc.body.iter().position(|b| b.as_table().is_some()).unwrap()
}

fn cell(s: &Session, row: usize) -> Value {
    json!({"story": "body", "path": [table_at(s), row, 0, 0], "off": 0})
}

/// A one-column table whose rows have these (height, rule) settings and empty cells.
fn table(rows: &[(f32, &str)]) -> Session {
    let mut s = Session::new(Document::new());
    s.run("insert.table", &json!({"rows": rows.len(), "cols": 1})).unwrap();
    for (k, (h, rule)) in rows.iter().enumerate() {
        let pos = cell(&s, k);
        s.run("caret.set", &json!({"pos": pos})).unwrap();
        s.run("table.rowHeight", &json!({"height": h, "rule": rule})).unwrap();
    }
    s
}

fn heights(s: &Session) -> Vec<(f32, HeightRule)> {
    let t = s.doc.body[table_at(s)].as_table().unwrap();
    t.rows.iter().map(|r| ((r.props.height.unwrap_or(0.0) * 100.0).round() / 100.0, r.props.height_rule)).collect()
}

#[test]
fn rows_share_their_total_height_as_at_least() {
    let mut s = table(&[(20.0, "exact"), (50.0, "exact"), (30.0, "atLeast")]);
    let pos = cell(&s, 0);
    s.run("caret.set", &json!({"pos": pos})).unwrap();
    s.run("table.distributeRows", &json!({})).unwrap();
    assert_eq!(heights(&s), [(33.33, HeightRule::AtLeast), (33.33, HeightRule::AtLeast), (33.33, HeightRule::AtLeast)]);
}

#[test]
fn only_the_selected_rows_are_distributed() {
    let mut s = table(&[(20.0, "exact"), (60.0, "exact"), (90.0, "exact")]);
    let (a, b) = (cell(&s, 0), cell(&s, 1));
    s.run("select.range", &json!({"anchor": a, "focus": b})).unwrap();
    s.run("table.distributeRows", &json!({})).unwrap();
    assert_eq!(heights(&s), [(40.0, HeightRule::AtLeast), (40.0, HeightRule::AtLeast), (90.0, HeightRule::Exact)]);
}

#[test]
fn distribute_rows_is_one_undo_step() {
    let mut s = table(&[(20.0, "exact"), (60.0, "exact")]);
    let before = s.doc.clone();
    s.run("table.distributeRows", &json!({})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before);
}
