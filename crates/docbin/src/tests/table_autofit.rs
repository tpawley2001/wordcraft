//! `.doc` tables and `sprmTFAutofit` (0x3615): a table none of whose rows ask to autofit keeps
//! its stored cell widths (fixed column widths); a row with operand 1 makes it automatic.

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];
const INNER_CELL: &[u8] = &[0x4B, 0x24, 1];
const INNER_TTP: &[u8] = &[0x4C, 0x24, 1];
const THREE: &[i16] = &[0, 1620, 3249, 7740];

/// A mark at depth `itap` with extra PAPX sprms; `cell_mark` says it was 0x07.
fn mark(text: &str, itap: i32, extra: &[u8], cell_mark: bool) -> ParaOut {
    let mut papx = vec![0x16, 0x24, 1]; // sprmPFInTable
    papx.extend([0x49, 0x66]); // sprmPItap
    papx.extend(itap.to_le_bytes());
    papx.extend(extra);
    let mut row = table::decode(&papx);
    row.cell_mark = cell_mark;
    ParaOut { para: Paragraph::with_text(text, CharProps::default()), end_cp: 0, row: Some(Box::new(row)) }
}

/// `sprmTFAutofit` with `operand`, or nothing.
fn autofit(operand: Option<u8>) -> Vec<u8> {
    operand.map(|o| super::grpprl(&[(0x3615, &[o])])).unwrap_or_default()
}

/// The marks of one depth-1 row whose cells end at `edges` (twips), the row end carrying `extra`.
fn row(edges: &[i16], extra: &[u8]) -> Vec<ParaOut> {
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(edges));
    end.extend(extra);
    let mut marks: Vec<ParaOut> = (0..edges.len() - 1).map(|i| mark(&format!("c{i}"), 1, &[], true)).collect();
    marks.push(mark("", 1, &end, true));
    marks
}

fn tables(blocks: &Blocks) -> Vec<&Table> {
    blocks.iter().filter_map(|b| b.as_table()).collect()
}

fn two_rows(operand: Option<u8>) -> Blocks {
    let mut marks = row(THREE, &autofit(operand));
    marks.extend(row(THREE, &autofit(operand)));
    table::assemble(marks)
}

#[test]
fn a_table_without_the_autofit_flag_has_fixed_columns() {
    let body = two_rows(None);
    let t = tables(&body)[0];
    assert!(t.props.fixed);
    assert_eq!(t.grid, vec![81.0, 81.45, 224.55]);
}

#[test]
fn a_table_whose_rows_autofit_is_not_fixed() {
    let body = two_rows(Some(1));
    let t = tables(&body)[0];
    assert!(!t.props.fixed);
    assert_eq!(t.grid, vec![81.0, 81.45, 224.55]);
}

#[test]
fn an_autofit_flag_of_zero_leaves_the_table_fixed() {
    let body = two_rows(Some(0));
    assert!(tables(&body)[0].props.fixed);
}

#[test]
fn one_autofit_row_makes_the_table_automatic() {
    let mut marks = row(THREE, &[]);
    marks.extend(row(THREE, &autofit(Some(1))));
    let body = table::assemble(marks);
    assert!(!tables(&body)[0].props.fixed);
}

#[test]
fn a_row_with_more_cells_than_the_grid_stays_automatic() {
    // The second row's edges cannot go on one grid with the first's (they are out of order), so
    // the grid is the first row's two columns and cannot size the second row's three cells.
    let mut marks = row(&[0, 1440, 7200], &[]);
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(&[0, 3000, 1000, 7200]));
    marks.extend((0..3).map(|i| mark(&format!("d{i}"), 1, &[], true)));
    marks.push(mark("", 1, &end, true));
    let body = table::assemble(marks);
    let t = tables(&body)[0];
    assert_eq!((t.grid.len(), t.cols()), (2, 3));
    assert!(!t.props.fixed);
    assert_eq!((t.props.width, t.props.width_pct), (None, None));
}

#[test]
fn a_nested_table_without_the_autofit_flag_has_fixed_columns() {
    let mut inner_end = INNER_TTP.to_vec();
    inner_end.extend(super::t_def_table(&[0, 1440, 4320]));
    let mut outer_end = TTP.to_vec();
    outer_end.extend(super::t_def_table(&[0, 7200]));
    outer_end.extend(autofit(Some(1)));
    let body = table::assemble(vec![
        mark("a", 2, INNER_CELL, false),
        mark("b", 2, INNER_CELL, false),
        mark("", 2, &inner_end, false),
        mark("", 1, &[], true),
        mark("", 1, &outer_end, true),
    ]);
    let outer = tables(&body)[0];
    assert!(!outer.props.fixed);
    let inner = tables(&outer.rows[0].cells[0].blocks)[0];
    assert_eq!(inner.grid, [72.0, 144.0]);
    assert!(inner.props.fixed);
}
