//! `.doc` horizontal merges: each row's `sprmTDefTable` carries one `TC80` per cell after the cell
//! boundaries, and bits 0-1 of `TC80.tcgrf` (its first two bytes) say whether the cell is the
//! first of a horizontally merged set (fFirstMerged, bit 0) or is merged into the set to its left
//! (fMerged, bit 1) ([MS-DOC] 2.9.313 `TC80`, 2.9.314 `TCGRF`, 2.9.315 `TDefTableOperand`). Word
//! saves a merged row there, with no `sprmTMerge`.

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

/// `tcgrf` with a twips cell width (ftsWidth 3) and nothing else: an ordinary cell.
const P: u16 = 0x0600;
/// First cell of a horizontally merged set (fFirstMerged).
const F: u16 = 0x0601;
/// Merged into the set to its left (fMerged).
const M: u16 = 0x0602;
/// Both bits set.
const B: u16 = 0x0603;

/// A depth-1 mark with extra PAPX sprms; `cell_mark` says it was 0x07.
fn mark(text: &str, extra: &[u8], cell_mark: bool) -> ParaOut {
    let mut papx = vec![0x16, 0x24, 1]; // sprmPFInTable
    papx.extend([0x49, 0x66]); // sprmPItap
    papx.extend(1i32.to_le_bytes());
    papx.extend(extra);
    let mut row = table::decode(&papx);
    row.cell_mark = cell_mark;
    ParaOut { para: Paragraph::with_text(text, CharProps::default()), end_cp: 0, row: Some(Box::new(row)) }
}

/// A `sprmTDefTable` Prl: cb (bytes after it, plus one), itcMac, the boundaries, then one
/// 20-byte TC80 per entry of `tcgrf` (flags, a zero wWidth and four zero Brc80s) followed by
/// `stray` extra bytes. `tcgrf` may be shorter than the cell count.
fn def_table(edges: &[i16], tcgrf: &[u16], stray: usize) -> Vec<u8> {
    let mut body = vec![edges.len() as u8 - 1];
    for e in edges {
        body.extend_from_slice(&e.to_le_bytes());
    }
    for flags in tcgrf {
        body.extend_from_slice(&flags.to_le_bytes());
        body.extend_from_slice(&[0u8; 18]);
    }
    body.extend(std::iter::repeat_n(0xA5u8, stray));
    let cb = (body.len() + 1) as u16;
    let mut v = (0xD608u16).to_le_bytes().to_vec();
    v.extend_from_slice(&cb.to_le_bytes());
    v.extend_from_slice(&body);
    v
}

/// The definition of a row of `n` cells 1000 twips (50 pt) wide each.
fn def(n: usize, tcgrf: &[u16]) -> Vec<u8> {
    let edges: Vec<i16> = (0..=n as i16).map(|i| i * 1000).collect();
    def_table(&edges, tcgrf, 0)
}

/// `sprmTMerge`: itcFirst, itcLim.
fn t_merge(a: u8, b: u8) -> Vec<u8> {
    super::grpprl(&[(0x5624, &[a, b])])
}

/// One table; each row is its cell texts and the sprms of its row-end TAP, in order.
fn rows(rows: &[(&[&str], &[&[u8]])]) -> Blocks {
    let mut marks: Vec<ParaOut> = Vec::new();
    for (texts, tap) in rows {
        let mut end = TTP.to_vec();
        for part in *tap {
            end.extend_from_slice(part);
        }
        marks.extend(texts.iter().map(|t| mark(t, &[], true)));
        marks.push(mark("", &end, true));
    }
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn spans(t: &Table, row: usize) -> Vec<u32> {
    t.rows[row].cells.iter().map(|c| c.props.span).collect()
}

fn widths(t: &Table, row: usize) -> Vec<Option<f32>> {
    t.rows[row].cells.iter().map(|c| c.props.width).collect()
}

/// Each cell's paragraph texts.
fn texts(t: &Table, row: usize) -> Vec<Vec<String>> {
    t.rows[row].cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()).collect()
}

/// A row of `tcgrf.len()` cells named a, b, c… with the given flags and extra row sprms.
fn one_row(tcgrf: &[u16], extra: &[&[u8]]) -> Blocks {
    let names = ["a", "b", "c", "d", "e"];
    let d = def(tcgrf.len(), tcgrf);
    let mut tap: Vec<&[u8]> = vec![&d];
    tap.extend_from_slice(extra);
    rows(&[(&names[..tcgrf.len()], &tap)])
}

/// Check the single row's spans, widths (pt) and cell texts.
fn check(blocks: &Blocks, want_spans: &[u32], want_widths: &[f32], want_texts: &[&[&str]]) {
    let t = table(blocks);
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].cells.len(), want_spans.len());
    assert_eq!(spans(t, 0), want_spans);
    assert_eq!(widths(t, 0), want_widths.iter().map(|w| Some(*w)).collect::<Vec<_>>());
    let want: Vec<Vec<String>> = want_texts.iter().map(|c| c.iter().map(|s| s.to_string()).collect()).collect();
    assert_eq!(texts(t, 0), want);
}

#[test]
fn plain_cells_stay_separate() {
    check(&one_row(&[P, P, P], &[]), &[1, 1, 1], &[50.0, 50.0, 50.0], &[&["a"], &["b"], &["c"]]);
}

#[test]
fn first_and_merged_cells_make_one_cell() {
    check(&one_row(&[F, M, P], &[]), &[2, 1], &[100.0, 50.0], &[&["a", "b"], &["c"]]);
    check(&one_row(&[F, M, M, P], &[]), &[3, 1], &[150.0, 50.0], &[&["a", "b", "c"], &["d"]]);
    check(&one_row(&[P, F, M], &[]), &[1, 2], &[50.0, 100.0], &[&["a"], &["b", "c"]]);
    check(&one_row(&[F, M, F, M], &[]), &[2, 2], &[100.0, 100.0], &[&["a", "b"], &["c", "d"]]);
}

#[test]
fn an_empty_merged_away_cell_adds_no_paragraph() {
    let d = def(3, &[F, M, P]);
    let blocks = rows(&[(&["A", "", "C"], &[&d])]);
    check(&blocks, &[2, 1], &[100.0, 50.0], &[&["A"], &["C"]]);
}

#[test]
fn rows_of_a_table_merge_on_their_own() {
    // A title row merging cells 0-1 and a body row merging cells 2-4, as a report stores them.
    let title = def(5, &[F, M, P, P, P]);
    let body = def(5, &[P, P, F, M, M]);
    let blocks = rows(&[(&["A", "", "B", "", ""], &[&title]), (&["", "L", "long text", "", ""], &[&body])]);
    let t = table(&blocks);
    assert_eq!(t.rows.len(), 2);
    assert_eq!(spans(t, 0), [2, 1, 1, 1]);
    assert_eq!(widths(t, 0), [Some(100.0), Some(50.0), Some(50.0), Some(50.0)]);
    assert_eq!(texts(t, 0)[0], ["A"]);
    assert_eq!(spans(t, 1), [1, 1, 3]);
    assert_eq!(widths(t, 1), [Some(50.0), Some(50.0), Some(150.0)]);
    assert_eq!(texts(t, 1)[2], ["long text"]);
}

#[test]
fn merged_cells_without_an_open_set_stay_separate() {
    // A merged cell with no first cell before it, or after an ordinary cell, merges nothing.
    check(&one_row(&[M, P, P], &[]), &[1, 1, 1], &[50.0, 50.0, 50.0], &[&["a"], &["b"], &["c"]]);
    check(&one_row(&[F, P, M], &[]), &[1, 1, 1], &[50.0, 50.0, 50.0], &[&["a"], &["b"], &["c"]]);
    // A first cell with no merged cell after it merges nothing.
    check(&one_row(&[F, P, P], &[]), &[1, 1, 1], &[50.0, 50.0, 50.0], &[&["a"], &["b"], &["c"]]);
}

#[test]
fn a_later_first_cell_starts_a_new_set() {
    check(&one_row(&[F, F, M], &[]), &[1, 2], &[50.0, 100.0], &[&["a"], &["b", "c"]]);
}

#[test]
fn a_cell_with_both_bits_counts_as_merged() {
    check(&one_row(&[F, B, P], &[]), &[2, 1], &[100.0, 50.0], &[&["a", "b"], &["c"]]);
}

#[test]
fn t_merge_alone_is_unchanged() {
    check(&one_row(&[P, P, P], &[&t_merge(0, 2)]), &[2, 1], &[100.0, 50.0], &[&["a", "b"], &["c"]]);
}

#[test]
fn t_merge_wins_over_an_overlapping_tc80_set() {
    check(&one_row(&[F, M, M, P], &[&t_merge(1, 3)]), &[1, 2, 1], &[50.0, 100.0, 50.0], &[&["a"], &["b", "c"], &["d"]]);
}

#[test]
fn t_merge_and_a_separate_tc80_set_both_apply() {
    check(&one_row(&[F, M, P, P], &[&t_merge(2, 4)]), &[2, 2], &[100.0, 100.0], &[&["a", "b"], &["c", "d"]]);
}

#[test]
fn incomplete_tc80s_count_only_when_whole() {
    // Two complete TC80s of three cells, then seven stray bytes: the merge still reads.
    let d = def_table(&[0, 1000, 2000, 3000], &[F, M], 7);
    let blocks = rows(&[(&["a", "b", "c"], &[&d])]);
    check(&blocks, &[2, 1], &[100.0, 50.0], &[&["a", "b"], &["c"]]);
    // One complete TC80 (a first cell) and the operand ends: nothing merges.
    let d = def_table(&[0, 1000, 2000, 3000], &[F], 0);
    let blocks = rows(&[(&["a", "b", "c"], &[&d])]);
    check(&blocks, &[1, 1, 1], &[50.0, 50.0, 50.0], &[&["a"], &["b"], &["c"]]);
}

#[test]
fn the_row_counts_its_tc80_merges() {
    let d = def(3, &[F, M, P]);
    assert_eq!(table::decode(&d).overrides(), 1);
    let d = def(3, &[P, P, P]);
    assert_eq!(table::decode(&d).overrides(), 0);
}
