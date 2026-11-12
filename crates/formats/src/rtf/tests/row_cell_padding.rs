//! A row's own default cell padding (RTF 1.9.1, "Table Definitions": `\trpaddlN`, `\trpaddtN`, `\trpaddbN`,
//! `\trpaddrN` with their unit words `\trpaddflN`… = 3, and the gap `\trgaphN`). Word reads them per row, and
//! unlike a cell's `\clpadl`/`\clpadt` the row words are not swapped. The document has one default per table (its
//! first row's), so a later row whose padding differs hands the differing sides to its cells as their own margins;
//! a side a cell sets itself (`\clpad*`) is the cell's. Once a row states its left or its right padding in twips the
//! gap no longer stands in for the other side. Every expected value is what Word shows for the same rows.

use super::*;
use wordcraft_doc::Document;

/// Word's default cell padding (top, left, bottom, right), points.
const WORD_DEFAULT: [f32; 4] = [0.0, 5.4, 0.0, 5.4];

fn open(rows: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Arial;}}}}\fs24\pard P\par {rows}\pard Q\par}}").as_bytes()).unwrap()
}

/// One row of two cells: the row words `def` after `\trowd`, then the words of its first and second cell.
fn row_with(def: &str, cell_a: &str, cell_b: &str) -> String {
    format!(r"\trowd{def}{cell_a}\cellx2880{cell_b}\cellx5760\pard\intbl A\cell B\cell\row")
}

fn row(def: &str) -> String {
    row_with(def, "", "")
}

fn table(d: &Document) -> &wordcraft_doc::Table {
    let mut tables = d.body.iter().filter_map(|b| b.as_table());
    let t = tables.next().expect("a table");
    assert!(tables.next().is_none(), "the rows are one table");
    t
}

/// The table's default cell padding (top, left, bottom, right), points: its own, else Word's default.
fn padding(d: &Document) -> [f32; 4] {
    table(d).props.cell_margins.unwrap_or(WORD_DEFAULT).map(|v| (v * 100.0).round() / 100.0)
}

/// The margins cell `c` of row `r` sets itself: [top, left, bottom, right] in points.
fn sides(d: &Document, r: usize, c: usize) -> [Option<f32>; 4] {
    let p = &table(d).rows[r].cells[c].props;
    let own = p.margins.map(|m| m.map(Some)).or(p.partial_margins).unwrap_or([None; 4]);
    own.map(|s| s.map(|v| (v * 100.0).round() / 100.0))
}

/// Both cells of row `r` set exactly `want` themselves.
fn assert_row(d: &Document, r: usize, want: [Option<f32>; 4]) {
    for c in 0..2 {
        assert_eq!(sides(d, r, c), want, "row {r} cell {c} [top, left, bottom, right]");
    }
}

const NONE: [Option<f32>; 4] = [None; 4];
const LEFT: &str = r"\trgaph0\trpaddl720\trpaddfl3";
const TOP: &str = r"\trgaph0\trpaddt720\trpaddft3";
const BOTTOM: &str = r"\trgaph0\trpaddb720\trpaddfb3";
const RIGHT: &str = r"\trgaph0\trpaddr720\trpaddfr3";
const BARE: &str = r"\trgaph0";

fn five_rows() -> String {
    [BARE, LEFT, TOP, BOTTOM, BARE].map(row).concat()
}

fn assert_five_rows(d: &Document) {
    assert_eq!(table(d).rows.len(), 5);
    assert_eq!(padding(d), [0.0; 4], "the first row's padding is the table's");
    assert_row(d, 0, NONE);
    assert_row(d, 1, [None, Some(36.0), None, None]);
    assert_row(d, 2, [Some(36.0), None, None, None]);
    assert_row(d, 3, [None, None, Some(36.0), None]);
    assert_row(d, 4, NONE);
}

// ---- each row has its own padding ----

#[test]
fn a_later_rows_left_padding_is_its_cells_own() {
    let d = open(&format!("{}{}", row(BARE), row(LEFT)));
    assert_eq!(table(&d).rows.len(), 2);
    assert_eq!(padding(&d), [0.0; 4]);
    assert_row(&d, 0, NONE);
    assert_row(&d, 1, [None, Some(36.0), None, None]);
}

#[test]
fn row_words_are_not_swapped_left_top_bottom_and_right_each_reach_their_side() {
    assert_five_rows(&open(&five_rows()));
    let d = open(&format!("{}{}", row(BARE), row(RIGHT)));
    assert_row(&d, 1, [None, None, None, Some(36.0)]);
}

#[test]
fn a_later_row_without_the_first_rows_padding_sets_it_back() {
    let d = open(&format!("{}{}", row(LEFT), row(BARE)));
    assert_eq!(padding(&d), [0.0, 36.0, 0.0, 0.0]);
    assert_row(&d, 0, NONE);
    assert_row(&d, 1, [None, Some(0.0), None, None]);
}

#[test]
fn a_later_rows_gap_is_its_left_and_right_padding() {
    let d = open(&format!("{}{}", row(r"\trgaph108"), row(r"\trgaph300")));
    assert_eq!(table(&d).props.cell_margins, None, "Word's own gap in the first row sets nothing");
    assert_row(&d, 0, NONE);
    assert_row(&d, 1, [None, Some(15.0), None, Some(15.0)]);
}

#[test]
fn a_cells_own_side_wins_over_its_rows_side_by_side() {
    // Row 2 states left 18 pt; its second cell states its own left (`\clpadt`, Word's swap) 12 pt.
    let second = row_with(r"\trgaph0\trpaddl360\trpaddfl3", "", r"\clpadt240\clpadft3");
    let d = open(&format!("{}{second}", row(LEFT)));
    assert_eq!(padding(&d), [0.0, 36.0, 0.0, 0.0]);
    assert_eq!(sides(&d, 1, 0), [None, Some(18.0), None, None]);
    assert_eq!(sides(&d, 1, 1), [None, Some(12.0), None, None]);

    // Row 2 states all four sides 18 pt; a cell's own top (`\clpadl`) 0 replaces only the top.
    let all = r"\trgaph0\trpaddl360\trpaddfl3\trpaddt360\trpaddft3\trpaddb360\trpaddfb3\trpaddr360\trpaddfr3";
    let d = open(&format!("{}{}", row(BARE), row_with(all, r"\clpadl0\clpadfl3", "")));
    assert_eq!(sides(&d, 1, 0), [Some(0.0), Some(18.0), Some(18.0), Some(18.0)]);
    assert_eq!(sides(&d, 1, 1), [Some(18.0); 4]);
}

#[test]
fn the_first_rows_cells_keep_only_what_they_set_themselves() {
    let first = row_with(LEFT, r"\clpadt240\clpadft3", "");
    let d = open(&format!("{first}{}", row(LEFT)));
    assert_eq!(padding(&d), [0.0, 36.0, 0.0, 0.0]);
    assert_eq!(sides(&d, 0, 0), [None, Some(12.0), None, None]);
    assert_eq!(sides(&d, 0, 1), NONE);
    assert_row(&d, 1, NONE);
}

#[test]
fn a_row_without_its_own_definition_repeats_the_last_one() {
    let d = open(&format!(r"{}\pard\intbl C\cell D\cell\row", row(LEFT)));
    assert_eq!(table(&d).rows.len(), 2);
    assert_eq!(padding(&d), [0.0, 36.0, 0.0, 0.0]);
    assert_row(&d, 1, NONE);
}

#[test]
fn rows_with_the_same_padding_set_nothing_on_their_cells() {
    let def = r"\trgaph108\trpaddl300\trpaddfl3";
    let d = open(&[def, def, def].map(row).concat());
    for r in 0..3 {
        assert_row(&d, r, NONE);
    }
    let d = open(&[r"\trgaph108\trleft-108", r"\trgaph108\trleft-108\trpaddl108\trpaddr108\trpaddfl3\trpaddfr3"].map(row).concat());
    assert_eq!(table(&d).props.cell_margins, None);
    assert_row(&d, 1, NONE);
}

#[test]
fn each_rows_padding_survives_a_save_and_reopen() {
    let rtf = export(&open(&five_rows()));
    // The differing sides go out as the cells' own: left as `\clpadt`, top as `\clpadl` (Word's swap).
    for word in [r"\clpadt720\clpadft3", r"\clpadl720\clpadfl3", r"\clpadb720\clpadfb3"] {
        assert_eq!(rtf.matches(word).count(), 2, "{word} once per cell of its row:\n{rtf}");
    }
    assert!(!rtf.contains(r"\clpadr"), "{rtf}");
    assert_five_rows(&import(rtf.as_bytes()).unwrap());
}

#[test]
fn hostile_later_rows_stay_finite_and_within_22_inches() {
    for def in [
        r"\trpaddl2147483647\trpaddfl3",
        r"\trgaph2147483647\trpaddt-2147483647\trpaddft3",
        r"\trpaddfr2147483647\trpaddr300\trpaddb99999999\trpaddfb3",
    ] {
        let d = open(&format!("{}{}", row(BARE), row(def)));
        assert_eq!(table(&d).rows.len(), 2, "{def}");
        for c in 0..2 {
            assert!(sides(&d, 1, c).iter().flatten().all(|v| v.is_finite() && (0.0..=1584.0).contains(v)), "{def}: {:?}", sides(&d, 1, c));
        }
    }
}

// ---- the gap against stated sides ----

fn assert_padding(def: &str, want: [f32; 4]) {
    assert_eq!(padding(&open(&row(def))), want, "{def}: padding [top, left, bottom, right]");
}

#[test]
fn the_gap_is_left_and_right_while_neither_is_stated() {
    assert_padding(r"\trgaph300", [0.0, 15.0, 0.0, 15.0]);
    assert_padding(r"\trgaph300\trpaddt720\trpaddft3", [36.0, 15.0, 0.0, 15.0]);
    assert_padding(r"\trgaph300\trpaddb720\trpaddfb3", [0.0, 15.0, 36.0, 15.0]);
    assert_padding(r"\trgaph108\trpaddt720\trpaddft3", [36.0, 5.4, 0.0, 5.4]);
    // A left or right value without the twips unit is not stated.
    assert_padding(r"\trgaph300\trpaddl100", [0.0, 15.0, 0.0, 15.0]);
    assert_padding(r"\trgaph300\trpaddr100\trpaddfr0", [0.0, 15.0, 0.0, 15.0]);
}

#[test]
fn a_stated_left_leaves_the_right_without_the_gap() {
    assert_padding(r"\trgaph108\trpaddl720\trpaddfl3", [0.0, 36.0, 0.0, 0.0]);
    assert_padding(r"\trgaph300\trpaddl100\trpaddfl3", [0.0, 5.0, 0.0, 0.0]);
    assert_padding(r"\trgaph300\trpaddl700\trpaddfl3", [0.0, 35.0, 0.0, 0.0]);
    assert_padding(r"\trgaph300\trpaddl0\trpaddfl3", [0.0; 4]);
    assert_padding(r"\trgaph300\trpaddfl3\trpaddl100", [0.0, 5.0, 0.0, 0.0]);
}

#[test]
fn a_stated_right_leaves_the_left_without_the_gap() {
    assert_padding(r"\trgaph108\trpaddr720\trpaddfr3", [0.0, 0.0, 0.0, 36.0]);
    assert_padding(r"\trgaph108\trpaddr100\trpaddfr3", [0.0, 0.0, 0.0, 5.0]);
    assert_padding(r"\trgaph300\trpaddr100\trpaddfr3", [0.0, 0.0, 0.0, 5.0]);
    assert_padding(r"\trgaph300\trpaddr300\trpaddfr3", [0.0, 0.0, 0.0, 15.0]);
    assert_padding(r"\trgaph300\trpaddr700\trpaddfr3\trpaddt720\trpaddft3", [36.0, 0.0, 0.0, 35.0]);
}
