//! `.doc` table preferred width and rows of different cell counts: the grid comes from the
//! table's first `sprmTDefTable`, so a table whose later row has more cells than that grid
//! cannot carry a preferred width (`sprmTTableWidth`, 0xF614) and stays automatic.

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

/// 7200 twips (360 pt) as an ftsDxa `sprmTTableWidth` operand.
const W_7200_TWIPS: &[u8] = &[0x03, 0x20, 0x1C];
/// 100 % (5000 fiftieths) as an ftsPercent operand.
const W_100_PERCENT: &[u8] = &[0x02, 0x88, 0x13];
/// An automatic width (ftsAuto).
const W_AUTO: &[u8] = &[0x01, 0x00, 0x00];

const ONE_CELL: &[i16] = &[0, 7200];
const FIVE_CELLS: &[i16] = &[0, 1440, 2880, 4320, 5760, 7200];

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

/// The marks of one row whose cells end at `edges` (twips), with `width` as the row's
/// `sprmTTableWidth` operand when given.
fn row_marks(edges: &[i16], width: Option<&[u8]>) -> Vec<ParaOut> {
    let cells = edges.len() - 1;
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(edges));
    if let Some(operand) = width {
        end.extend(super::grpprl(&[(0xF614, operand)]));
    }
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[], true)).collect();
    marks.push(mark("", &end, true));
    marks
}

/// A two-row table: `first` then `second` cell edges, the width on the first row.
fn two_rows(first: &[i16], second: &[i16], width: Option<&[u8]>) -> Blocks {
    let mut marks = row_marks(first, width);
    marks.extend(row_marks(second, None));
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn cell_counts(t: &Table) -> Vec<usize> {
    t.rows.iter().map(|r| r.cells.len()).collect()
}

#[test]
fn a_later_row_with_more_cells_than_the_grid_leaves_a_twips_width_automatic() {
    let body = two_rows(ONE_CELL, FIVE_CELLS, Some(W_7200_TWIPS));
    let t = table(&body);
    assert_eq!(cell_counts(t), vec![1, 5]);
    assert_eq!(t.grid, vec![360.0]);
    assert_eq!((t.props.width, t.props.width_pct), (None, None));
}

#[test]
fn a_later_row_with_more_cells_than_the_grid_leaves_a_percent_width_automatic() {
    let body = two_rows(ONE_CELL, FIVE_CELLS, Some(W_100_PERCENT));
    let t = table(&body);
    assert_eq!(cell_counts(t), vec![1, 5]);
    assert_eq!((t.props.width, t.props.width_pct), (None, None));
}

#[test]
fn a_first_row_that_defines_every_column_keeps_the_width() {
    let body = two_rows(FIVE_CELLS, ONE_CELL, Some(W_7200_TWIPS));
    let t = table(&body);
    assert_eq!(cell_counts(t), vec![5, 1]);
    assert_eq!(t.grid, vec![72.0; 5]);
    assert_eq!((t.props.width, t.props.width_pct), (Some(360.0), None));
}

#[test]
fn rows_of_equal_cell_counts_keep_the_width() {
    let body = two_rows(FIVE_CELLS, FIVE_CELLS, Some(W_100_PERCENT));
    let t = table(&body);
    assert_eq!(cell_counts(t), vec![5, 5]);
    assert_eq!((t.props.width, t.props.width_pct), (None, Some(100.0)));
}

#[test]
fn an_uneven_table_without_a_width_or_with_an_automatic_one_is_unchanged() {
    for width in [None, Some(W_AUTO)] {
        let body = two_rows(ONE_CELL, FIVE_CELLS, width);
        let t = table(&body);
        assert_eq!(cell_counts(t), vec![1, 5]);
        assert_eq!(t.grid, vec![360.0]);
        assert_eq!((t.props.width, t.props.width_pct), (None, None));
    }
}

#[test]
fn the_rows_and_grid_of_an_uneven_table_are_not_rebuilt() {
    // The guard only drops the preferred width: cells, spans and the first-row grid stay.
    let body = two_rows(ONE_CELL, FIVE_CELLS, Some(W_7200_TWIPS));
    let t = table(&body);
    assert_eq!(t.cols(), 5);
    assert!(t.rows.iter().all(|r| r.cells.iter().all(|c| c.span() == 1)));
    let text: Vec<String> = t.rows[1].cells.iter().map(|c| c.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect()).collect();
    assert_eq!(text, vec!["c0", "c1", "c2", "c3", "c4"]);
}
