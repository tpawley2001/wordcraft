//! Table rows formatted as hidden text, row mark and all (`w:trPr/w:hidden`, `RowProps::hidden`),
//! laid out with hidden text not shown. Measured in Microsoft 365 Word's PDFs of small documents:
//! "AAA", a 3 x 2 table with single 0.5 pt borders, "ZZZ" (Arial 12 pt, no paragraph spacing).
//! A row is left out only when its row mark is hidden and every paragraph of its cells is hidden
//! too (text and paragraph mark); a table of such rows takes no room.
use crate::*;
use wordcraft_doc::props::{Border, Borders, HeightRule, LineSpacing};
use wordcraft_doc::{Block, Pos, Table, para_block};

fn chr() -> CharProps {
    CharProps { font: Some("Arial".into()), size: Some(12.0), ..Default::default() }
}

fn hidden_chr() -> CharProps {
    CharProps { hidden: Some(true), ..chr() }
}

fn para(text: &str) -> Paragraph {
    let mut p = if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, chr()) };
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p.mark = chr();
    p
}

/// An empty paragraph whose mark is hidden.
fn hidden_mark_para() -> Paragraph {
    let mut p = para("");
    p.mark = hidden_chr();
    p
}

/// How a row of the fixture is formatted.
#[derive(Clone, Copy, PartialEq)]
struct Row {
    /// The cells' text is hidden.
    text: bool,
    /// The cells' paragraph marks are hidden.
    marks: bool,
    /// The row mark is hidden.
    row: bool,
    /// The cells are empty.
    empty: bool,
    /// Only the first cell is formatted so; the second is plain.
    first_cell_only: bool,
}

const SHOWN: Row = Row { text: false, marks: false, row: false, empty: false, first_cell_only: false };
/// Probe 4: text, cell marks and row mark hidden.
const HIDDEN: Row = Row { text: true, marks: true, row: true, ..SHOWN };

fn cell_para(text: &str, r: Row) -> Paragraph {
    let text = if r.empty { "" } else { text };
    let mut p = para(text);
    if r.text && !text.is_empty() {
        p = Paragraph::with_text(text, hidden_chr());
        p.props = para("").props;
        p.mark = chr();
    }
    if r.marks {
        p.mark = hidden_chr();
    }
    p
}

fn table(rows: &[Row]) -> Table {
    let mut t = Table::new(rows.len(), 2, 150.0);
    t.props.style = None;
    t.props.borders = Some(Borders::all(Border::single(0.5)));
    for (ri, (row, r)) in t.rows.iter_mut().zip(rows).enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            let r = if r.first_cell_only && ci > 0 { SHOWN } else { *r };
            cell.blocks = vec![para_block(cell_para(&format!("x{ri}{ci}"), r))];
        }
        row.props.hidden = r.row;
    }
    t
}

fn tb(t: Table) -> std::sync::Arc<Block> {
    std::sync::Arc::new(Block::Table(t))
}

/// "AAA", a table with `rows`, "ZZZ".
fn doc(rows: &[Row]) -> Document {
    let mut d = Document::new();
    d.body = vec![para_block(para("AAA")), tb(table(rows)), para_block(para("ZZZ"))];
    d
}

fn lay_with(d: &Document, show_hidden: bool) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions { show_hidden, ..Default::default() })
}

fn top(l: &DocLayout, block: usize) -> f32 {
    l.caret(&Pos::body(block, 0)).unwrap().top
}

/// ZZZ's top minus AAA's top.
fn gap(d: &Document, show_hidden: bool) -> f32 {
    let l = lay_with(d, show_hidden);
    let last = d.body.len() - 1;
    top(&l, last) - top(&l, 0)
}

/// One text line: probe 1, no table.
fn line() -> f32 {
    let mut d = Document::new();
    d.body = vec![para_block(para("AAA")), para_block(para("ZZZ"))];
    gap(&d, false)
}

fn rules(l: &DocLayout) -> usize {
    l.pages[0].items.iter().filter(|it| matches!(it, Placed::Rule { .. })).count()
}

#[test]
fn a_table_of_hidden_rows_takes_no_room() {
    let line = line();
    assert!(line > 10.0);
    let d = doc(&[HIDDEN; 3]);
    let at = gap(&d, false);
    assert!((at - line).abs() < 0.1, "probe 4: ZZZ {at} below AAA, a line {line}");
    assert_eq!(rules(&lay_with(&d, false)), 0, "no table border is placed");
    assert!(rules(&lay_with(&doc(&[SHOWN; 3]), false)) > 0);
}

#[test]
fn a_table_of_hidden_rows_takes_no_room_in_compatibility_mode_15() {
    let mut d = doc(&[HIDDEN; 3]);
    d.settings.compat_mode = 15;
    let at = gap(&d, false);
    assert!((at - line()).abs() < 0.1, "probe 5: ZZZ {at} below AAA");
}

#[test]
fn a_hidden_middle_row_is_left_out() {
    let d = doc(&[SHOWN, HIDDEN, SHOWN]);
    let two = doc(&[SHOWN, SHOWN]);
    let (at, want) = (gap(&d, false), gap(&two, false));
    assert!((at - want).abs() < 0.1, "probe 6: ZZZ {at} below AAA, two rows {want}");
    let l = lay_with(&d, false);
    let lt = lay_with(&two, false);
    let cell = |l: &DocLayout, r: u32| l.caret(&Pos { story: StoryRef::Body, path: Path(vec![1, r, 0, 0]), off: 0 }).unwrap().top;
    let pitch = cell(&lt, 1) - cell(&lt, 0);
    assert!(pitch > 10.0);
    assert!((cell(&l, 2) - cell(&l, 0) - pitch).abs() < 0.1, "row 3 one row below row 1");
}

#[test]
fn rows_with_anything_shown_keep_their_room() {
    // Probe 7: row mark alone; 8: text hidden, cell marks shown; 10: one cell shown; 12: cells
    // empty, cell marks shown. Each lays out as the same table without the row flag.
    let cases = [
        ("7", Row { row: true, ..SHOWN }),
        ("8", Row { text: true, row: true, ..SHOWN }),
        ("10", Row { first_cell_only: true, ..HIDDEN }),
        ("12", Row { row: true, empty: true, ..SHOWN }),
    ];
    for (name, r) in cases {
        let at = gap(&doc(&[r; 3]), false);
        let want = gap(&doc(&[Row { row: false, ..r }; 3]), false);
        assert!((at - want).abs() < 0.01, "probe {name}: ZZZ {at} below AAA, without the row flag {want}");
        assert!(at > line() + 5.0, "probe {name}: the rows take room");
    }
    // Probe 7 is the table of probe 2.
    let at = gap(&doc(&[Row { row: true, ..SHOWN }; 3]), false);
    assert!((at - gap(&doc(&[SHOWN; 3]), false)).abs() < 0.01);
}

#[test]
fn empty_cells_with_hidden_marks_and_a_hidden_row_mark_take_no_room() {
    // Probe 9.
    let at = gap(&doc(&[Row { row: true, marks: true, empty: true, ..SHOWN }; 3]), false);
    assert!((at - line()).abs() < 0.1, "probe 9: ZZZ {at} below AAA");
}

#[test]
fn an_exact_row_height_does_not_keep_a_hidden_row() {
    let mut d = doc(&[HIDDEN; 3]);
    if let Some(Block::Table(t)) = d.body.get_mut(1).map(std::sync::Arc::make_mut) {
        for row in &mut t.rows {
            row.props.height = Some(30.0);
            row.props.height_rule = HeightRule::Exact;
        }
    }
    let at = gap(&d, false);
    assert!((at - line()).abs() < 0.1, "probe 11: ZZZ {at} below AAA");
}

#[test]
fn two_hidden_tables_and_hidden_paragraphs_take_no_room() {
    // Probe 13: AAA, an empty paragraph, table, hidden mark, table, hidden mark, ZZZ.
    let mut d = Document::new();
    d.body = vec![
        para_block(para("AAA")),
        para_block(para("")),
        tb(table(&[HIDDEN; 3])),
        para_block(hidden_mark_para()),
        tb(table(&[HIDDEN; 3])),
        para_block(hidden_mark_para()),
        para_block(para("ZZZ")),
    ];
    let at = gap(&d, false);
    let line = line();
    assert!((at - 2.0 * line).abs() < 0.2, "probe 13: ZZZ {at} below AAA, two lines {}", 2.0 * line);
}

#[test]
fn hidden_rows_are_laid_out_with_hidden_text_shown() {
    let at = gap(&doc(&[HIDDEN; 3]), true);
    let want = gap(&doc(&[Row { row: false, ..HIDDEN }; 3]), true);
    assert!((at - want).abs() < 0.01, "ZZZ {at} below AAA, without the row flag {want}");
    assert!(at > line() + 30.0);
}

#[test]
fn twenty_hidden_tables_leave_the_page_its_room() {
    // How many lines fit on the first page.
    let mut probe = Document::new();
    probe.body = (0..200).map(|i| para_block(para(&format!("Line {i}")))).collect();
    let pl = lay_with(&probe, false);
    let per_page = (0..200).take_while(|&i| pl.caret(&Pos::body(i, 0)).is_some_and(|c| c.page == 0)).count();
    assert!(per_page > 10, "{per_page} lines a page");
    let mut d = Document::new();
    d.body = (0..per_page - 6).map(|i| para_block(para(&format!("Filler {i}")))).collect();
    for _ in 0..20 {
        d.body.push(tb(table(&[HIDDEN; 3])));
        d.body.push(para_block(hidden_mark_para()));
    }
    let lb = wordcraft_doc::para::LINE_BREAK;
    d.body.push(para_block(para(&format!("One{lb}Two{lb}Three{lb}Four"))));
    let l = lay_with(&d, false);
    let end = d.body.len() - 1;
    let caret = l.caret(&Pos::body(end, d.body[end].as_para().unwrap().text.len())).unwrap();
    assert_eq!(caret.page, 0, "the four-line paragraph stays on page 1");
    assert_eq!(l.pages.len(), 1);
}
