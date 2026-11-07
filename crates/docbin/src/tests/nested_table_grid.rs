//! Nested `.doc` tables: an inner row's TAP sizes the inner table, and hostile depths are
//! capped without losing text.

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

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

const INNER_CELL: &[u8] = &[0x4B, 0x24, 1];
const INNER_TTP: &[u8] = &[0x4C, 0x24, 1];
const TTP: &[u8] = &[0x17, 0x24, 1];

fn tables(blocks: &Blocks) -> Vec<&Table> {
    blocks.iter().filter_map(|b| b.as_table()).collect()
}

#[test]
fn an_inner_rows_tdeftable_gives_the_inner_table_its_grid() {
    let mut inner_end = INNER_TTP.to_vec();
    inner_end.extend(super::t_def_table(&[0, 1440, 4320])); // 72 pt and 144 pt columns
    let mut outer_end = TTP.to_vec();
    outer_end.extend(super::t_def_table(&[0, 7200]));
    let body = table::assemble(vec![
        mark("a", 2, INNER_CELL, false),
        mark("b", 2, INNER_CELL, false),
        mark("", 2, &inner_end, false),
        mark("", 1, &[], true),
        mark("", 1, &outer_end, true),
    ]);
    let outer = tables(&body);
    assert_eq!(outer.len(), 1, "{body:?}");
    assert_eq!(outer[0].grid, [360.0]);
    let inner = tables(&outer[0].rows[0].cells[0].blocks);
    assert_eq!(inner.len(), 1);
    assert_eq!(inner[0].grid, [72.0, 144.0]);
    assert_eq!(inner[0].rows[0].cells.len(), 2);
}

#[test]
fn a_hostile_depth_keeps_its_text_and_the_outer_table() {
    let body = table::assemble(vec![
        mark("deep", i32::MAX, INNER_CELL, false),
        mark("", i32::MAX, INNER_TTP, false),
        mark("top", 1, &[], true),
        mark("", 1, TTP, true),
    ]);
    // Walk down the first cells to the deepest table; the deep text sits in its cell.
    let mut blocks = &body;
    let mut depth = 0;
    while let Some(t) = tables(blocks).first() {
        depth += 1;
        blocks = &t.rows[0].cells[0].blocks;
    }
    // Past the cap no inner row closes, so every level degrades into the outer cell.
    assert_eq!(depth, 1);
    let texts: Vec<&str> = blocks.iter().filter_map(|b| b.as_para()).map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["deep", "", "top"]);
}
