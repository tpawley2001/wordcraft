//! Table Properties › Cell › Options… (Word's Cell Options): `table.properties` takes
//! `cellMargins` (a cell's own margins, `w:tcMar`, ECMA-376 Part 1 §17.4.42; `null` is Same as
//! the whole table) and `wrapText` (off is `w:noWrap`, §17.4.30), for the selected cells or the
//! caret's, in the same undo step as the dialog's other settings.

use std::io::Read;

use serde_json::json;
use wordcraft_doc::Document;
use wordcraft_doc::props::{CellProps, VAlign};
use wordcraft_engine::Session;

const COMMAND: &str = "table.properties";

/// A new document with a 2 × 2 table and the caret in one of its cells.
fn session() -> Session {
    let mut s = Session::new(Document::new());
    s.run("insert.table", &json!({"rows": 2, "cols": 2})).unwrap();
    s
}

/// The caret cell's (row, cell).
fn caret(s: &Session) -> (usize, usize) {
    let (_, r, c) = s.sel.focus.path.cell().expect("the caret is in a table");
    (r, c)
}

fn props(s: &Session, r: usize, c: usize) -> CellProps {
    let (tp, _, _) = s.sel.focus.path.cell().expect("the caret is in a table");
    s.doc.table(s.sel.focus.story, &tp).expect("a table").rows[r].cells[c].props.clone()
}

fn set(s: &mut Session, r: usize, c: usize, f: impl FnOnce(&mut CellProps)) {
    let (tp, _, _) = s.sel.focus.path.cell().expect("the caret is in a table");
    let story = s.sel.focus.story;
    f(&mut s.doc.table_mut(story, &tp).unwrap().rows[r].cells[c].props);
}

/// The table's default cell margins (top, left, bottom, right), as `table.cellMargins` reads them.
fn table_default(s: &Session) -> [f32; 4] {
    let (tp, _, _) = s.sel.focus.path.cell().expect("the caret is in a table");
    let t = s.doc.table(s.sel.focus.story, &tp).expect("a table");
    let style = t.props.style.as_deref().and_then(|id| s.doc.styles.table_style(id));
    t.props.cell_margins.or(style.and_then(|st| st.parts.cell_margins)).unwrap_or([0.0, 5.4, 0.0, 5.4])
}

/// Every cell of the table except `(r, c)`.
fn others(r: usize, c: usize) -> Vec<(usize, usize)> {
    [(0, 0), (0, 1), (1, 0), (1, 1)].into_iter().filter(|&x| x != (r, c)).collect()
}

fn saved_xml(s: &Session) -> String {
    let bytes = wordcraft_docx::write(&s.doc).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml
}

/// The cells' properties of the first table of the saved DOCX, read back, row by row.
fn reread(s: &Session) -> Vec<Vec<CellProps>> {
    let back = wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap()).unwrap();
    let t = back.body.iter().find_map(|b| b.as_table()).expect("the table is saved");
    t.rows.iter().map(|row| row.cells.iter().map(|c| c.props.clone()).collect()).collect()
}

fn near(got: Option<[f32; 4]>, want: [f32; 4]) -> bool {
    got.is_some_and(|g| g.iter().zip(want).all(|(a, b)| (a - b).abs() < 0.026))
}

#[test]
fn cell_margins_set_the_carets_cell_only() {
    let mut s = session();
    let (r, c) = caret(&s);
    assert!(!saved_xml(&s).contains("<w:tcMar"), "a new table's cells have no margins of their own");

    s.run(COMMAND, &json!({"cellMargins": {"top": 3.6, "left": 7.2, "bottom": 1.8, "right": 9.0}})).unwrap();
    let p = props(&s, r, c);
    assert_eq!(p.margins, Some([3.6, 7.2, 1.8, 9.0]), "top, left, bottom, right");
    assert_eq!(p.partial_margins, None);
    for (or, oc) in others(r, c) {
        assert_eq!(props(&s, or, oc).margins, None, "cell ({or}, {oc}) is not the caret's");
    }
    assert_eq!(saved_xml(&s).matches("<w:tcMar").count(), 1, "one cell has w:tcMar");
    let back = reread(&s);
    assert!(near(back[r][c].margins, [3.6, 7.2, 1.8, 9.0]), "read back: {:?}", back[r][c].margins);
}

#[test]
fn a_side_that_is_not_passed_keeps_the_tables_margin() {
    let mut s = session();
    let (r, c) = caret(&s);
    let d = table_default(&s);
    s.run(COMMAND, &json!({"cellMargins": {"left": 10.8}})).unwrap();
    assert_eq!(props(&s, r, c).margins, Some([d[0], 10.8, d[2], d[3]]));

    // A second call starts from the cell's own margins.
    s.run(COMMAND, &json!({"cellMargins": {"bottom": 2.0}})).unwrap();
    assert_eq!(props(&s, r, c).margins, Some([d[0], 10.8, 2.0, d[3]]));
}

#[test]
fn an_empty_object_pins_the_margins_the_cell_shows() {
    let mut s = session();
    let (r, c) = caret(&s);
    let d = table_default(&s);
    s.run(COMMAND, &json!({"cellMargins": {}})).unwrap();
    assert_eq!(props(&s, r, c).margins, Some(d));
}

#[test]
fn sides_a_file_set_one_by_one_are_kept_and_the_rest_filled_in() {
    let mut s = session();
    let (r, c) = caret(&s);
    let d = table_default(&s);
    set(&mut s, r, c, |p| p.partial_margins = Some([None, Some(9.0), None, None]));
    s.run(COMMAND, &json!({"cellMargins": {"top": 2.0}})).unwrap();
    let p = props(&s, r, c);
    assert_eq!(p.margins, Some([2.0, 9.0, d[2], d[3]]));
    assert_eq!(p.partial_margins, None, "all four sides are the cell's own now");
}

#[test]
fn null_is_same_as_the_whole_table() {
    let mut s = session();
    let (r, c) = caret(&s);
    s.run(COMMAND, &json!({"cellMargins": {"top": 4.0, "left": 4.0, "bottom": 4.0, "right": 4.0}})).unwrap();
    assert!(saved_xml(&s).contains("<w:tcMar"));
    s.run(COMMAND, &json!({"cellMargins": null})).unwrap();
    assert_eq!(props(&s, r, c).margins, None);
    assert!(!saved_xml(&s).contains("<w:tcMar"), "ticking Same as the whole table removes w:tcMar");

    // Margins a file gave side by side go too.
    set(&mut s, r, c, |p| p.partial_margins = Some([None, Some(9.0), None, None]));
    assert!(saved_xml(&s).contains("<w:tcMar"));
    s.run(COMMAND, &json!({"cellMargins": null})).unwrap();
    let p = props(&s, r, c);
    assert_eq!((p.margins, p.partial_margins), (None, None));
    assert!(!saved_xml(&s).contains("<w:tcMar"));
}

#[test]
fn margins_are_clamped_to_zero_and_two_inches() {
    let mut s = session();
    let (r, c) = caret(&s);
    s.run(COMMAND, &json!({"cellMargins": {"top": -5.0, "left": 5000.0, "bottom": 144.0, "right": 0.0}})).unwrap();
    assert_eq!(props(&s, r, c).margins, Some([0.0, 144.0, 144.0, 0.0]));
}

#[test]
fn a_cell_margins_value_that_is_no_object_is_refused() {
    for bad in [json!(5), json!("wide"), json!([1, 2, 3, 4]), json!(true)] {
        let mut s = session();
        let (r, c) = caret(&s);
        assert!(s.run(COMMAND, &json!({"cellMargins": bad, "wrapText": false})).is_err(), "{bad} is not a set of margins");
        let p = props(&s, r, c);
        assert_eq!((p.margins, p.no_wrap), (None, false), "a refused call changes nothing");
    }
}

#[test]
fn wrap_text_off_writes_no_wrap_and_on_removes_it() {
    let mut s = session();
    let (r, c) = caret(&s);
    assert!(!saved_xml(&s).contains("w:noWrap"));

    s.run(COMMAND, &json!({"wrapText": false})).unwrap();
    assert!(props(&s, r, c).no_wrap);
    for (or, oc) in others(r, c) {
        assert!(!props(&s, or, oc).no_wrap, "cell ({or}, {oc}) is not the caret's");
    }
    assert_eq!(saved_xml(&s).matches("<w:noWrap/>").count(), 1);
    assert!(reread(&s)[r][c].no_wrap, "it reads back");

    s.run(COMMAND, &json!({"wrapText": true})).unwrap();
    assert!(!props(&s, r, c).no_wrap);
    assert!(!saved_xml(&s).contains("w:noWrap"));
}

#[test]
fn every_selected_cell_is_set() {
    let mut s = session();
    let (r, _) = caret(&s);
    let other_row = 1 - r;
    s.run("table.selectRow", &json!({})).unwrap();
    s.run(COMMAND, &json!({"cellMargins": {"top": 1.0, "left": 2.0, "bottom": 3.0, "right": 4.0}, "wrapText": false})).unwrap();
    for c in 0..2 {
        let p = props(&s, r, c);
        assert_eq!(p.margins, Some([1.0, 2.0, 3.0, 4.0]), "selected cell ({r}, {c})");
        assert!(p.no_wrap, "selected cell ({r}, {c})");
        let q = props(&s, other_row, c);
        assert_eq!((q.margins, q.no_wrap), (None, false), "cell ({other_row}, {c}) is outside the selection");
    }
    assert_eq!(saved_xml(&s).matches("<w:tcMar").count(), 2);
    assert_eq!(saved_xml(&s).matches("<w:noWrap/>").count(), 2);
}

#[test]
fn it_is_one_undo_step_with_the_other_cell_settings() {
    let mut s = session();
    let (r, c) = caret(&s);
    let before = props(&s, r, c);
    s.run(COMMAND, &json!({"valign": "center", "cellMargins": {"left": 12.0}, "wrapText": false})).unwrap();
    let p = props(&s, r, c);
    assert_eq!(p.valign, Some(VAlign::Center));
    assert!(p.margins.is_some() && p.no_wrap);

    s.run("edit.undo", &json!({})).unwrap();
    let p = props(&s, r, c);
    assert_eq!((p.valign, p.margins, p.no_wrap), (before.valign, None, false), "one undo takes all three back");
}
