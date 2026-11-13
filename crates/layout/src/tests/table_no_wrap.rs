//! Cells set not to wrap (`w:noWrap`) in a table that fits its contents. Word keeps a column that
//! holds such a cell at least as wide as the cell's point preferred width (`w:tcW` dxa; its text
//! still wraps inside), or, with no point width, as its longest paragraph on one line plus the
//! cell margins, and lets the table run past its preferred width for it. Fixed layout ignores the
//! flag. Measured in Microsoft 365 Word on synthetic DOCX files: Letter page, 1 in margins, Times
//! New Roman 12 pt, 5.4 pt cell margins, 3 columns and 2 rows, table 211 pt, every cell
//! 47 / 110 / 54 pt, saved grid 52.2 / 120.6 / 54 pt. Laid out, the columns are compared with
//! Word's within 0.6 pt. The test font is not metric-compatible with Times New Roman: where a
//! column is as wide as a line of text, the expected width is that line in the test font, and the
//! mixed-rows texts are chosen so the test font gives Word's widths.

use super::*;

const SIDE: f32 = 5.4;
const PREF: [f32; 3] = [47.0, 110.0, 54.0];
const GRID: [f32; 3] = [52.2, 120.6, 54.0];
const ROW1: [&str; 3] = ["aa", "bb", "cc"];
const LONG: &str = "Almonds in shell and bitter ones";

fn chr(size: f32) -> wordcraft_doc::CharProps {
    wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() }
}

fn para(text: &str, size: f32) -> wordcraft_doc::Paragraph {
    let mut p = wordcraft_doc::Paragraph::with_text(text, chr(size));
    p.mark = chr(size);
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
    p
}

#[derive(Clone, Copy)]
struct Probe {
    /// Which columns' cells are set not to wrap.
    no_wrap: [bool; 3],
    /// Every cell's preferred width (None: automatic).
    pref: Option<[f32; 3]>,
    /// The table's width (None: automatic).
    width: Option<f32>,
    fixed: bool,
    compat: u32,
    /// How many times row 2's middle text is repeated.
    repeat: usize,
}

const N1: Probe = Probe { no_wrap: [false; 3], pref: Some(PREF), width: Some(211.0), fixed: false, compat: 11, repeat: 1 };
const N2: Probe = Probe { no_wrap: [true; 3], ..N1 };

fn probe_doc(p: &Probe, long: &str) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = p.compat;
    let mut t = Table::new(2, 3, 211.0);
    t.props.style = None;
    t.props.fixed = p.fixed;
    t.props.width = p.width;
    t.props.cell_margins = Some([0.0, SIDE, 0.0, SIDE]);
    t.grid = GRID.to_vec();
    let middle = vec![long; p.repeat].join(" ");
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.props.width = p.pref.map(|w| w[ci]);
            cell.props.no_wrap = p.no_wrap[ci];
            let text = match (ri, ci) {
                (0, _) => ROW1[ci],
                (_, 0) => "08021110",
                (_, 1) => middle.as_str(),
                _ => "0%",
            };
            cell.blocks = vec![wordcraft_doc::para_block(para(text, 12.0))];
        }
    }
    d.body = vec![wordcraft_doc::para_block(para("before", 12.0)), Arc::new(Block::Table(t)), wordcraft_doc::para_block(para("after", 12.0))];
    d
}

fn widths(d: &Document, row: usize, cols: usize) -> Vec<f32> {
    crate::test_fonts();
    let l = lay(d);
    (0..cols).map(|c| cell_rect(&l, 0, row, c).expect("cell").w).collect()
}

/// The column widths and the number of lines row 2's middle text takes.
fn run(p: &Probe) -> (Vec<f32>, usize) {
    crate::test_fonts();
    let d = probe_doc(p, LONG);
    let base = lines_of(&lay(&probe_doc(p, "x")));
    (widths(&d, 1, 3), lines_of(&lay(&d)) + 1 - base)
}

fn assert_near(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() <= 0.6, "{what}: got {got:?}, want {want:?}");
    }
}

#[test]
fn n1_no_cell_set_not_to_wrap_is_unchanged() {
    let (w, lines) = run(&N1);
    assert_near(&w, &[58.8, 107.1, 45.1], "n1");
    assert_eq!(lines, 2);
}

#[test]
fn n2_every_cell_keeps_its_preferred_width() {
    let (w, lines) = run(&N2);
    assert_near(&w, &[58.8, 110.1, 54.0], "n2");
    assert_eq!(lines, 2, "a cell with a point width still wraps inside it");
}

#[test]
fn n3_only_the_middle_column() {
    let (w, lines) = run(&Probe { no_wrap: [false, true, false], ..N2 });
    assert_near(&w, &[58.8, 110.1, 42.2], "n3");
    assert_eq!(lines, 2);
}

#[test]
fn n4_automatic_table_width() {
    let (w, lines) = run(&Probe { width: None, ..N2 });
    assert_near(&w, &[58.8, 110.1, 54.0], "n4");
    assert_eq!(lines, 2);
}

#[test]
fn n5_automatic_cell_widths_keep_the_text_on_one_line() {
    let p = Probe { pref: None, ..N2 };
    let (w, lines) = run(&p);
    // Word: 58.8 / 166.5 / 26.8, the middle text 155.7 pt on one line in Times New Roman.
    let d = probe_doc(&p, LONG);
    let Some(Block::Table(t)) = d.body.get(1).map(|b| b.as_ref()) else { panic!("table") };
    let one_line = crate::measure_table_columns(&d, t)[1].1;
    assert!(one_line > 166.5, "the test font's line is wider than Word's: {one_line}");
    assert_near(&w, &[58.8, one_line, 26.8], "n5");
    assert_eq!(lines, 1);
}

#[test]
fn n6_fixed_layout_ignores_the_flag() {
    let fixed = Probe { fixed: true, ..N2 };
    let (w, lines) = run(&fixed);
    let (plain, plain_lines) = run(&Probe { no_wrap: [false; 3], ..fixed });
    assert_eq!(w, plain);
    assert_eq!(lines, plain_lines);
    assert_near(&w, &[48.6, 112.2, 50.2], "n6");
}

#[test]
fn n7_compat_mode_15_same_widths() {
    let (w, lines) = run(&Probe { compat: 15, ..N2 });
    assert_near(&w, &[58.8, 110.0, 54.0], "n7");
    assert_eq!(lines, 2);
}

#[test]
fn n8_long_text_still_wraps_in_its_preferred_width() {
    let (w, lines) = run(&Probe { repeat: 5, ..N2 });
    assert_near(&w, &[58.8, 110.1, 54.0], "n8");
    assert_eq!(lines, 10);
}

/// A table saved by Word: 0.85 pt cell margins, 9.5 pt text, 15 rows all set not to wrap; row 1
/// with point widths 47 / 110 / 54, the others 47 / automatic / automatic. Its longest text in
/// column 2 is 118.9 pt on one line. Word: 52.2 / 120.6 / 54.0 (table 226.8, past its 211): column
/// 2 is that text unwrapped plus the margins, column 3 row 1's preferred width. The texts here
/// give the same widths in the test font (column 1's longest word 50.5 pt, column 2's line
/// 119.1 pt).
#[test]
fn mixed_rows_take_the_unwrapped_text_and_row_ones_widths() {
    crate::test_fonts();
    const NARROW: f32 = 0.85;
    let mut d = Document::new();
    d.settings.compat_mode = 11;
    let mut t = Table::new(15, 3, 211.0);
    t.props.style = None;
    t.props.width = Some(211.0);
    t.props.cell_margins = Some([0.0, NARROW, 0.0, NARROW]);
    t.grid = GRID.to_vec();
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.props.no_wrap = true;
            cell.props.width = if ri == 0 || ci == 0 { Some(PREF[ci]) } else { None };
            let text = match (ri, ci) {
                (0, 0) => "Tariff",
                (0, 1) => "Description",
                (0, 2) => "Duty",
                (_, 0) => "0802111000-",
                (1, 1) => "Almonds in shell bitter ones",
                (_, 1) => "Hazelnuts",
                _ => "0%",
            };
            cell.blocks = vec![wordcraft_doc::para_block(para(text, 9.5))];
        }
    }
    d.body = vec![wordcraft_doc::para_block(para("before", 12.0)), Arc::new(Block::Table(t)), wordcraft_doc::para_block(para("after", 12.0))];
    let Some(Block::Table(t)) = d.body.get(1).map(|b| b.as_ref()) else { panic!("table") };
    let cols = crate::measure_table_columns(&d, t);
    let w = widths(&d, 1, 3);
    assert_near(&w, &[cols[0].0, cols[1].1, 54.0], "mixed (test font)");
    assert_near(&w, &[52.2, 120.6, 54.0], "mixed");
    let l = lay(&d);
    let row1: Vec<f32> = (0..3).map(|c| cell_rect(&l, 0, 0, c).expect("cell").w).collect();
    assert_eq!(row1, w, "one grid");
}
