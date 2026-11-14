//! Convert Table to Text as Word does it: Commas puts one space after each comma, and with Convert
//! nested tables (on by default) tables nested in the converted rows become text with the same
//! separator. Expected paragraphs measured in Microsoft 365 Word (Table.ConvertToText via COM),
//! 2026-11-14.

use serde_json::{Value, json};
use wordcraft_doc::{Path, Pos, StoryRef};

use crate::Session;

fn run(s: &mut Session, cmd: &str, v: Value) {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"));
}

fn caret(s: &mut Session, path: Vec<u32>) {
    s.sel = crate::Selection::caret(Pos { story: StoryRef::Body, path: Path(path), off: 0 });
}

/// A table typed cell by cell (`cols` columns), caret left in the last cell.
fn typed(cols: usize, cells: &[&str]) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "insert.table", json!({"rows": cells.len() / cols, "cols": cols}));
    for (k, c) in cells.iter().enumerate() {
        if k > 0 {
            run(&mut s, "text.tab", json!({}));
        }
        if !c.is_empty() {
            run(&mut s, "text.insert", json!({"text": c}));
        }
    }
    s
}

fn table_index(s: &Session) -> u32 {
    s.doc.body.iter().position(|b| b.as_table().is_some()).expect("a table") as u32
}

/// Every paragraph of the body from the converted table on, `T` for a table.
fn outline_from(s: &Session, at: usize) -> Vec<String> {
    s.doc.body.iter().skip(at).map(|b| b.as_para().map_or_else(|| "T".to_string(), |p| p.text.clone())).collect()
}

/// Word's probe table: `r1c1 | (nested 2x2 n11 n12 / n21 n22, then the cell's empty paragraph)`
/// over `r2c1 | r2c2`. Returns the session and the outer table's index.
fn with_nested() -> (Session, u32) {
    let mut s = typed(2, &["r1c1", "", "r2c1", "r2c2"]);
    let t = table_index(&s);
    caret(&mut s, vec![t, 0, 1, 0]);
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    for (k, c) in ["n11", "n12", "n21", "n22"].iter().enumerate() {
        if k > 0 {
            run(&mut s, "text.tab", json!({}));
        }
        run(&mut s, "text.insert", json!({"text": c}));
    }
    let outer = s.doc.body[t as usize].as_table().unwrap();
    let cell = &outer.rows[0].cells[1].blocks;
    assert!(cell.first().is_some_and(|b| b.as_table().is_some()), "fixture: the cell starts with the nested table");
    assert!(cell.last().and_then(|b| b.as_para()).is_some_and(|p| p.text.is_empty()), "fixture: the cell ends with an empty paragraph");
    assert_eq!(cell.len(), 2, "fixture: nested table + the cell's empty paragraph");
    caret(&mut s, vec![t, 0, 0, 0]);
    (s, t)
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn commas_put_a_space_after_each_comma() {
    let mut s = typed(2, &["a", "b", "c", "d"]);
    let t = table_index(&s) as usize;
    run(&mut s, "table.toText", json!({"separator": "comma"}));
    assert_eq!(outline_from(&s, t)[..2], strs(&["a, b", "c, d"]));
    // Empty cells: Word writes `r1c1, , ` and `, r2c2, r2c3`.
    let mut s = typed(3, &["r1c1", "", "", "", "r2c2", "r2c3"]);
    let t = table_index(&s) as usize;
    run(&mut s, "table.toText", json!({"separator": "comma"}));
    assert_eq!(outline_from(&s, t)[..2], strs(&["r1c1, , ", ", r2c2, r2c3"]));
    // Only Commas gets the space.
    let mut s = typed(3, &["r1c1", "r1c2", "r1c3"]);
    let t = table_index(&s) as usize;
    run(&mut s, "table.toText", json!({"separator": "other", "char": "-"}));
    assert_eq!(outline_from(&s, t)[..1], strs(&["r1c1-r1c2-r1c3"]));
}

#[test]
fn the_space_after_a_comma_takes_no_formatting() {
    let mut s = typed(2, &["a", "b"]);
    let t = table_index(&s);
    run(&mut s, "select.range", json!({"anchor": {"story": "body", "path": [t, 0, 0, 0], "off": 0}, "focus": {"story": "body", "path": [t, 0, 0, 0], "off": 1}}));
    run(&mut s, "format.bold", json!({}));
    caret(&mut s, vec![t, 0, 0, 0]);
    run(&mut s, "table.toText", json!({"separator": "comma"}));
    let p = s.doc.body[t as usize].as_para().unwrap();
    assert_eq!(p.text, "a, b");
    assert_eq!(p.props_of_char(0).bold, Some(true));
    assert_ne!(p.props_of_char(1).bold, Some(true), "the comma takes no formatting");
    assert_ne!(p.props_of_char(2).bold, Some(true), "the space takes no formatting");
}

#[test]
fn nested_tables_become_text_by_default() {
    let cases: [(Value, &[&str]); 4] = [
        (json!({}), &["r1c1\tn11\tn12", "n21\tn22", "", "r2c1\tr2c2"]),
        (json!({"separator": "tab", "nested": true}), &["r1c1\tn11\tn12", "n21\tn22", "", "r2c1\tr2c2"]),
        (json!({"separator": "comma"}), &["r1c1, n11, n12", "n21, n22", "", "r2c1, r2c2"]),
        (json!({"separator": "paragraph"}), &["r1c1", "n11", "n12", "n21", "n22", "", "r2c1", "r2c2"]),
    ];
    for (params, want) in cases {
        let (mut s, t) = with_nested();
        run(&mut s, "table.toText", params.clone());
        let got = outline_from(&s, t as usize);
        assert_eq!(got.get(..want.len()), Some(&strs(want)[..]), "{params}: {got:?}");
        assert!(s.doc.body.iter().all(|b| b.as_table().is_none()), "{params}: no table is left: {got:?}");
        assert!(!s.doc.has_unbalanced_controls(), "{params}");
    }
}

#[test]
fn nested_off_keeps_nested_tables_as_tables() {
    let (mut s, t) = with_nested();
    run(&mut s, "table.toText", json!({"separator": "tab", "nested": false}));
    let got = outline_from(&s, t as usize);
    assert_eq!(got.get(..4), Some(&strs(&["r1c1\t", "T", "", "r2c1\tr2c2"])[..]), "{got:?}");
}

#[test]
fn a_nested_table_after_text_in_the_cell_starts_a_new_line() {
    // Cell (1,2) holds `X`, then a 1x2 nested table `i1 | i2`, then its empty last paragraph.
    let mut s = typed(2, &["Name", "X"]);
    let t = table_index(&s);
    s.sel = crate::Selection::caret(Pos { story: StoryRef::Body, path: Path(vec![t, 0, 1, 0]), off: 1 });
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "insert.table", json!({"rows": 1, "cols": 2}));
    run(&mut s, "text.insert", json!({"text": "i1"}));
    run(&mut s, "text.tab", json!({}));
    run(&mut s, "text.insert", json!({"text": "i2"}));
    caret(&mut s, vec![t, 0, 0, 0]);
    run(&mut s, "table.toText", json!({}));
    let got = outline_from(&s, t as usize);
    assert_eq!(got.first().map(String::as_str), Some("Name\tX"), "{got:?}");
    assert_eq!(got.get(1).map(String::as_str), Some("i1\ti2"), "{got:?}");
    assert!(s.doc.body.iter().all(|b| b.as_table().is_none()), "{got:?}");
}

#[test]
fn cells_we_already_convert_like_word_stay_the_same() {
    // Two paragraphs in one cell: `r1c1<T>p1`, `p2<T>r1c3`.
    let mut s = typed(3, &["r1c1", "p1", "r1c3"]);
    let t = table_index(&s);
    s.sel = crate::Selection::caret(Pos { story: StoryRef::Body, path: Path(vec![t, 0, 1, 0]), off: 2 });
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "p2"}));
    caret(&mut s, vec![t, 0, 0, 0]);
    run(&mut s, "table.toText", json!({"separator": "tab"}));
    assert_eq!(outline_from(&s, t as usize)[..2], strs(&["r1c1\tp1", "p2\tr1c3"]));
}

#[test]
fn undo_brings_back_the_table_with_its_nested_table_in_one_step() {
    let (mut s, t) = with_nested();
    let before = s.doc.body[t as usize].as_table().cloned().unwrap();
    let depth = s.undo_depth();
    run(&mut s, "table.toText", json!({}));
    assert_eq!(s.undo_depth(), depth + 1);
    run(&mut s, "edit.undo", json!({}));
    let back = s.doc.body[t as usize].as_table().expect("the table is back");
    assert_eq!(back.rows.len(), before.rows.len());
    assert!(back.rows[0].cells[1].blocks.first().is_some_and(|b| b.as_table().is_some()), "the nested table is back");
}

#[test]
fn tables_nested_as_deep_as_the_model_allows_all_become_text() {
    use wordcraft_doc::table::MAX_TABLE_DEPTH;
    // MAX_TABLE_DEPTH levels, each table nested in the first cell of the one above.
    let mut s = Session::new(wordcraft_doc::Document::new());
    for _ in 0..MAX_TABLE_DEPTH {
        run(&mut s, "insert.table", json!({"rows": 1, "cols": 1}));
    }
    run(&mut s, "text.insert", json!({"text": "deep"}));
    let t = table_index(&s);
    caret(&mut s, vec![t, 0, 0, 0]);
    // Run on a small stack, as a UI thread might have.
    let handle = std::thread::Builder::new()
        .stack_size(1 << 20)
        .spawn(move || {
            run(&mut s, "table.toText", json!({}));
            let text: String = s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
            let tables_left = s.doc.body.iter().filter(|b| b.as_table().is_some()).count();
            (text, tables_left)
        })
        .unwrap();
    let (text, tables_left) = handle.join().expect("no panic or stack overflow");
    assert_eq!(tables_left, 0, "every level became text");
    assert!(text.contains("deep"), "the text is kept: {text:?}");
}
