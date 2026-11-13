//! A table set to fit its contents whose cells' preferred widths (`w:tcW` dxa) do not add up to
//! the table's width (`w:tblW`). Word ignores the saved grid: each column starts from its
//! preferred width raised to its longest word, and when the columns ask for more than the table
//! has, each gives up room in proportion to its slack over its longest word; when they ask for
//! less, they widen in proportion. Measured in Microsoft 365 Word on synthetic DOCX files: Letter
//! page with 1 in margins, Times New Roman 12 pt, 5.4 pt cell margins, 4 columns and 2 rows, every
//! cell 2222 / 3610 / 2995 / 3610 dxa, table 8827 dxa, saved grid 2204 / 1407 / 1310 / 4321 dxa.
//! The rule itself is checked against Word's widths from Word's own word widths
//! (`rule_matches_word`). Laid out, the columns are compared with Word's within 0.6 pt when every
//! cell is short; when a long word decides a column, the test font's word is wider than Word's, so
//! the expected widths come from the rule fed the test font's narrowest widths.

use super::*;

const SIDE: f32 = 5.4;
const PREF: [f32; 4] = [111.1, 180.5, 149.75, 180.5];
const GRID: [f32; 4] = [110.2, 70.35, 65.5, 216.05];
const SHORT: [&str; 4] = ["aa", "bb", "cc", "dd"];
const C3: &str = "This document is about document properties";
const LONG: &str = "/ns0:Properties[1]/ns0:Company[1] and more words here to wrap the cell text over lines";
const C4: &str = "CoverPageProperties[1]/ns0:CompanyAddress";
const C1: &str = "CharactersWithSpaces";

fn chr() -> wordcraft_doc::CharProps {
    wordcraft_doc::CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str) -> wordcraft_doc::Paragraph {
    let mut p = wordcraft_doc::Paragraph::with_text(text, chr());
    p.mark = chr();
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(wordcraft_doc::props::LineSpacing::Multiple(1.0));
    p
}

struct Probe {
    /// Row 2's texts (row 1 always holds the short ones).
    row2: [&'static str; 4],
    /// Every cell's preferred width (None: no `w:tcW`).
    pref: Option<[f32; 4]>,
    /// The table's width (`w:tblW` dxa; None: automatic).
    width: Option<f32>,
    grid: [f32; 4],
    fixed: bool,
    compat: u32,
}

const BASE: Probe = Probe { row2: SHORT, pref: Some(PREF), width: Some(441.35), grid: GRID, fixed: false, compat: 14 };

fn probe_doc(p: &Probe, tweak: impl Fn(&mut Table)) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = p.compat;
    let mut t = Table::new(2, 4, 441.35);
    t.props.style = None;
    t.props.fixed = p.fixed;
    t.props.width = p.width;
    t.props.cell_margins = Some([0.0, SIDE, 0.0, SIDE]);
    t.grid = p.grid.to_vec();
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.props.width = p.pref.and_then(|w| w.get(ci).copied());
            let text = if ri == 0 { SHORT[ci] } else { p.row2[ci] };
            cell.blocks = vec![wordcraft_doc::para_block(para(text))];
        }
    }
    tweak(&mut t);
    d.body = vec![wordcraft_doc::para_block(para("before")), Arc::new(Block::Table(t)), wordcraft_doc::para_block(para("after"))];
    d
}

/// The widths of the table's columns, from the cells of `row`.
fn widths_of(d: &Document, row: usize, cols: usize) -> Vec<f32> {
    crate::test_fonts();
    let l = lay(d);
    (0..cols).map(|c| cell_rect(&l, 0, row, c).expect("cell").w).collect()
}

fn columns(p: &Probe) -> Vec<f32> {
    widths_of(&probe_doc(p, |_| {}), 1, 4)
}

fn assert_near(got: &[f32], want: &[f32], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() <= 0.6, "{what}: got {got:?}, want {want:?}");
    }
}

/// The narrowest widths with the test font (longest word plus the cell margins), per column.
fn mins(p: &Probe) -> Vec<f32> {
    crate::test_fonts();
    let d = probe_doc(p, |_| {});
    let Some(Block::Table(t)) = d.body.get(1).map(|b| b.as_ref()) else { panic!("table") };
    crate::measure_table_columns(&d, t).iter().map(|c| c.0).collect()
}

/// What the rule gives with the test font's word widths.
fn rule(p: &Probe, w: f32) -> Vec<f32> {
    crate::table::share_preferred(&p.pref.expect("preferred widths"), &mins(p), w).expect("fits")
}

const SHORT_MIN: [f32; 4] = [21.46, 22.8, 21.46, 22.8];

#[test]
fn rule_matches_word() {
    let cases: [(&str, [f32; 4], [f32; 4], f32, [f32; 4]); 9] = [
        ("a1", PREF, SHORT_MIN, 441.35, [80.8, 127.1, 106.3, 127.3]),
        ("a3", PREF, [21.46, 22.8, 21.46, 180.9], 441.35, [68.0, 104.6, 88.0, 180.9]),
        ("a4", PREF, [21.46, 180.9, 21.46, 22.8], 441.35, [68.0, 180.8, 88.0, 104.7]),
        ("a5", PREF, SHORT_MIN, 478.9, [87.0, 138.3, 115.3, 138.3]),
        ("a6", [50.0; 4], SHORT_MIN, 441.35, [110.3, 110.4, 110.5, 110.3]),
        ("a8", PREF, [21.46, 22.8, 21.46, 180.9], 441.35, [68.0, 104.6, 88.0, 180.9]),
        ("b2", PREF, [21.46, 22.8, 58.8, 235.4], 441.35, [48.8, 70.8, 86.3, 235.6]),
        ("b3", PREF, [227.8, 22.8, 21.46, 22.8], 441.35, [228.2, 74.8, 63.7, 74.8]),
        ("b4", PREF, [58.8; 4], 441.35, [86.7, 123.7, 107.3, 123.8]),
    ];
    for (name, pref, min, w, word) in cases {
        let got = crate::table::share_preferred(&pref, &min, w).expect(name);
        assert_near(&got, &word, name);
    }
    // b1: the narrowest widths don't fit, so the rule stands aside.
    assert!(crate::table::share_preferred(&PREF, &[119.3, 72.2, 58.8, 235.4], 441.35).is_none());
}

#[test]
fn a1_short_cells_share_the_excess_by_slack() {
    assert_near(&columns(&BASE), &[80.8, 127.1, 106.3, 127.3], "a1");
}

#[test]
fn a2_compat_mode_15_same_widths() {
    assert_near(&columns(&Probe { compat: 15, ..BASE }), &[80.7, 127.1, 106.5, 127.1], "a2");
}

/// Word: 68.0 / 104.6 / 88.0 / 180.9.
#[test]
fn a3_long_word_in_column_4() {
    let p = Probe { row2: ["aa", "bb", "cc", LONG], ..BASE };
    let got = columns(&p);
    assert_near(&got, &rule(&p, 441.35), "a3");
    // The long word's column is exactly its narrowest width.
    assert!((got[3] - mins(&p)[3]).abs() < 0.1, "{got:?}");
}

/// Word: 68.0 / 180.8 / 88.0 / 104.7.
#[test]
fn a4_long_word_in_column_2() {
    let p = Probe { row2: ["aa", LONG, "cc", "dd"], ..BASE };
    assert_near(&columns(&p), &rule(&p, 441.35), "a4");
}

/// An automatic table width takes the available width. Word's table is 478.9 pt here (the text
/// width plus the cell margins, a rule of modes below 15 that is not this change's); ours is the
/// text width, 468 pt, so the columns are Word's in the same proportions.
#[test]
fn a5_automatic_table_width_takes_the_room() {
    let got = columns(&Probe { width: None, ..BASE });
    let want = [87.0f32, 138.3, 115.3, 138.3].map(|w| w * 468.0 / 478.9);
    assert_near(&got, &want, "a5");
}

#[test]
fn a6_narrow_preferred_widths_widen_to_the_table() {
    assert_near(&columns(&Probe { pref: Some([50.0; 4]), ..BASE }), &[110.3, 110.4, 110.5, 110.3], "a6");
}

/// Fixed layout is out of this change's scope: the widths stay what they were.
#[test]
fn a7_fixed_layout_unchanged() {
    let got = columns(&Probe { fixed: true, ..BASE });
    let k = 441.35 / GRID.iter().sum::<f32>();
    assert_near(&got, &GRID.map(|w| w * k), "a7");
}

/// The saved grid plays no part: the same widths as a3 (Word: 68.0 / 104.6 / 88.0 / 180.9).
#[test]
fn a8_saved_grid_plays_no_part() {
    let p = Probe { row2: ["aa", "bb", "cc", LONG], grid: [110.35; 4], ..BASE };
    let a3 = columns(&Probe { row2: ["aa", "bb", "cc", LONG], ..BASE });
    assert_near(&columns(&p), &a3, "a8");
    assert_near(&columns(&p), &rule(&p, 441.35), "a8");
}

/// Word: 48.8 / 70.8 / 86.3 / 235.6.
#[test]
fn b2_two_long_words() {
    let p = Probe { row2: ["aa", "bb", C3, C4], ..BASE };
    assert_near(&columns(&p), &rule(&p, 441.35), "b2");
}

/// Word: 228.2 / 74.8 / 63.7 / 74.8.
#[test]
fn b3_one_word_wider_than_its_preferred_width() {
    let p = Probe { row2: ["CharactersWithSpacesCharactersWithSpaces", "bb", "cc", "dd"], ..BASE };
    let got = columns(&p);
    assert_near(&got, &rule(&p, 441.35), "b3");
    assert!((got[1] - got[3]).abs() < 0.1, "{got:?}");
}

/// Word: 86.7 / 123.7 / 107.3 / 123.8.
#[test]
fn b4_medium_words_everywhere() {
    let row2 = [C3, C3, C3, "This document is about document properties This document is about document properties"];
    let p = Probe { row2, ..BASE };
    assert_near(&columns(&p), &rule(&p, 441.35), "b4");
}

/// When even the longest words don't fit in the table's width the columns are their minimums,
/// scaled into the room together, as before this change (Word: 117.0 / 73.7 / 57.8 / 230.4 of a
/// 478.9 pt table).
#[test]
fn b1_minimums_wider_than_the_room_unchanged() {
    let p = Probe { row2: [C1, "Normal.dotm", C3, C4], ..BASE };
    let got = columns(&p);
    let m = mins(&p);
    let k = 468.0 / m.iter().sum::<f32>();
    assert_near(&got, &m.iter().map(|w| w * k).collect::<Vec<_>>(), "b1");
}

/// Tables with no preferred cell widths keep the content-only autofit.
#[test]
fn no_cell_widths_unchanged() {
    let got = columns(&Probe { pref: None, width: None, ..BASE });
    let content: Vec<(f32, f32)> = {
        let d = probe_doc(&Probe { pref: None, width: None, ..BASE }, |_| {});
        let Some(Block::Table(t)) = d.body.get(1).map(|b| b.as_ref()) else { panic!("table") };
        crate::measure_table_columns(&d, t)
    };
    assert_near(&got, &content.iter().map(|c| c.1).collect::<Vec<_>>(), "no tcW");
}

/// Percentage cell widths keep their own path: the same widths as with the saved grid alone.
#[test]
fn pct_cells_unchanged() {
    let pct = |t: &mut Table| {
        for row in &mut t.rows {
            for cell in &mut row.cells {
                cell.props.width = None;
                cell.props.width_pct = Some(25.0);
            }
        }
    };
    let with = widths_of(&probe_doc(&BASE, pct), 1, 4);
    let other_grid = Probe { grid: [110.35; 4], ..BASE };
    let without = widths_of(&probe_doc(&other_grid, pct), 1, 4);
    // Still grid-driven (the grid changes the result), so not the preferred-width rule.
    assert!(with.iter().zip(&without).any(|(a, b)| (a - b).abs() > 1.0), "{with:?} {without:?}");
}

/// A row with a spanning cell keeps today's handling: the saved grid, scaled to the table.
#[test]
fn spanned_row_unchanged() {
    let span = |t: &mut Table| {
        if let Some(row) = t.rows.get_mut(1) {
            row.cells.truncate(3);
            if let Some(c) = row.cells.get_mut(0) {
                c.props.span = 2;
                c.props.width = Some(PREF[0] + PREF[1]);
            }
            if let Some(c) = row.cells.get_mut(1) {
                c.props.width = Some(PREF[2]);
            }
            if let Some(c) = row.cells.get_mut(2) {
                c.props.width = Some(PREF[3]);
            }
        }
    };
    let got = widths_of(&probe_doc(&BASE, span), 0, 4);
    let k = 441.35 / GRID.iter().sum::<f32>();
    assert_near(&got, &GRID.map(|w| w * k), "span");
}
