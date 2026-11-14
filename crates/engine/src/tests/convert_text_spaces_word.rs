//! Convert Text to Table keeps every space, tab and no-break space in the cells, as Word does;
//! only one ordinary space right after each comma goes with the comma separator.
//! Expected cells measured in Microsoft 365 Word 16.0.20527 (Range.ConvertToTable), 2026-11-14.

use serde_json::{Value, json};

use crate::Session;

fn cells(text: &str, params: Value) -> Vec<Vec<String>> {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({ "text": text })).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s.run("table.fromText", &params).unwrap();
    let t = s.doc.body.iter().find_map(|b| b.as_table()).expect("a table");
    t.rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect()).collect())
        .collect()
}

#[test]
fn comma_drops_one_space_after_each_comma_only() {
    let comma = json!({ "separator": "comma" });
    assert_eq!(cells("a, b ,c\nd,e", comma.clone()), [["a", "b ", "c"], ["d", "e", ""]]);
    assert_eq!(cells("  a  ,  b  \n d", comma.clone()), [["  a  ", " b  "], [" d", ""]]);
    assert_eq!(cells("a,\tb", comma.clone()), [["a", "\tb"]]);
    assert_eq!(cells("a,\u{a0}b", comma), [["a", "\u{a0}b"]]);
}

#[test]
fn tab_keeps_every_space() {
    let tab = json!({ "separator": "tab" });
    assert_eq!(cells("  a  \t  b  ", tab.clone()), [["  a  ", "  b  "]]);
    assert_eq!(cells("a\t   \tb", tab.clone()), [["a", "   ", "b"]]);
    assert_eq!(cells("\ta\tb", tab), [["", "a", "b"]]);
}

#[test]
fn other_character_keeps_every_space() {
    assert_eq!(cells("  a  -  b  ", json!({ "separator": "other", "char": "-" })), [["  a  ", "  b  "]]);
    assert_eq!(cells("a ; b", json!({ "separator": "other", "char": ";" })), [["a ", " b"]]);
}

#[test]
fn paragraph_keeps_every_space() {
    assert_eq!(cells("  p1  \n  p2  ", json!({ "separator": "paragraph" })), [["  p1  "], ["  p2  "]]);
}
