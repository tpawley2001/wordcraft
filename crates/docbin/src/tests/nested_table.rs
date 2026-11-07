//! Nested tables in `.doc` ([MS-DOC] §2.4.3): a paragraph's depth is `sprmPItap`; a cell of a
//! table at depth 2 or more ends with an ordinary paragraph mark carrying
//! `sprmPFInnerTableCell`, and its row with one carrying `sprmPFInnerTtp`. Depth-1 cells and rows
//! end with the 0x07 cell mark. A nested table opens as a table inside its parent's cell.

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Block, Blocks, para::Paragraph};

use crate::table::{self, ParaOut};
use crate::{fmt, sprm};

const P_IN_TABLE: u16 = 0x2416;
const P_TTP: u16 = 0x2417;
const P_INNER_CELL: u16 = 0x244B;
const P_INNER_TTP: u16 = 0x244C;
const P_ITAP: u16 = 0x6649;

/// A paragraph mark at table depth `itap` (0 = outside any table): `end` says what it ends.
#[derive(Clone, Copy)]
enum End {
    /// An ordinary paragraph (inside a cell, or outside a table).
    Para,
    /// A depth-1 cell (the 0x07 mark).
    Cell,
    /// A depth-1 row of `n` cells (0x07 with `sprmPFTtp` and the row's `sprmTDefTable`).
    Row(usize),
    /// A cell of a table at depth >= 2 (`sprmPFInnerTableCell`).
    InnerCell,
    /// A row of `n` cells of a table at depth >= 2 (`sprmPFInnerTtp` and its `sprmTDefTable`).
    InnerRow(usize),
}

fn mark(text: &str, itap: i32, end: End) -> ParaOut {
    let mut prls: Vec<(u16, Vec<u8>)> = Vec::new();
    if itap > 0 {
        prls.push((P_IN_TABLE, vec![1]));
        prls.push((P_ITAP, itap.to_le_bytes().to_vec()));
    }
    match end {
        End::Row(_) => prls.push((P_TTP, vec![1])),
        End::InnerCell => prls.push((P_INNER_CELL, vec![1])),
        End::InnerRow(_) => prls.push((P_INNER_TTP, vec![1])),
        End::Para | End::Cell => {}
    }
    let mut papx: Vec<u8> = prls.iter().flat_map(|(op, v)| op.to_le_bytes().into_iter().chain(v.iter().copied())).collect();
    // Word writes the row's cell edges on its row-end mark: one inch per cell here.
    if let End::Row(n) | End::InnerRow(n) = end {
        let edges: Vec<i16> = (0..=n).map(|i| i as i16 * 1440).collect();
        papx.extend(super::t_def_table(&edges));
    }
    let mut para = Paragraph::with_text(text, CharProps::default());
    for prl in sprm::iter(&papx).filter(|p| p.sgc() == 1) {
        fmt::apply_para(&mut para.props, &prl);
    }
    let mut row = table::decode(&papx);
    row.cell_mark = matches!(end, End::Cell | End::Row(_));
    let row = row.is_table().then(|| Box::new(row));
    ParaOut { para, end_cp: 0, row }
}

/// The text of a paragraph block, or `None` for a table.
fn text(b: &Block) -> Option<&str> {
    b.as_para().map(|p| p.text.as_str())
}

fn only_table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "one table expected in {blocks:?}");
    tables[0]
}

/// The texts of a table's cells, row by row (a nested table shows as "<table>").
fn grid_text(t: &Table) -> Vec<Vec<Vec<String>>> {
    t.rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.blocks.iter().map(|b| text(b).map_or_else(|| "<table>".to_string(), str::to_string)).collect()).collect())
        .collect()
}

/// Inner 1x2 table ("i1" "i2") then "after" in the outer table's first cell; "b" in its second.
fn one_level(inner_rows: usize) -> Vec<ParaOut> {
    let mut v = Vec::new();
    for r in 0..inner_rows {
        v.push(mark(&format!("i{}1", r + 1), 2, End::InnerCell));
        v.push(mark(&format!("i{}2", r + 1), 2, End::InnerCell));
        v.push(mark("", 2, End::InnerRow(2)));
    }
    v.push(mark("after", 1, End::Cell));
    v.push(mark("b", 1, End::Cell));
    v.push(mark("", 1, End::Row(2)));
    v.push(mark("tail", 0, End::Para));
    v
}

#[test]
fn a_table_in_a_cell_opens_as_a_table_in_that_cell() {
    let body = table::assemble(one_level(1));
    assert_eq!(body.len(), 2, "{body:?}");
    assert_eq!(body.last().and_then(|b| text(b)), Some("tail"));
    let outer = only_table(&body);
    assert_eq!(grid_text(outer), [[vec!["<table>", "after"], vec!["b"]]]);
    let inner = only_table(&outer.rows[0].cells[0].blocks);
    assert_eq!(grid_text(inner), [[vec!["i11"], vec!["i12"]]]);
}

#[test]
fn every_inner_row_end_closes_a_row_of_the_inner_table() {
    let body = table::assemble(one_level(2));
    let outer = only_table(&body);
    let inner = only_table(&outer.rows[0].cells[0].blocks);
    assert_eq!(grid_text(inner), [[vec!["i11"], vec!["i12"]], [vec!["i21"], vec!["i22"]]]);
}

#[test]
fn three_levels_nest_three_tables() {
    let body = table::assemble(vec![
        mark("deep", 3, End::InnerCell),
        mark("", 3, End::InnerRow(1)),
        mark("mid", 2, End::InnerCell),
        mark("", 2, End::InnerRow(1)),
        mark("top", 1, End::Cell),
        mark("", 1, End::Row(1)),
    ]);
    let outer = only_table(&body);
    assert_eq!(grid_text(outer), [[vec!["<table>", "top"]]]);
    let mid = only_table(&outer.rows[0].cells[0].blocks);
    assert_eq!(grid_text(mid), [[vec!["<table>", "mid"]]]);
    let deep = only_table(&mid.rows[0].cells[0].blocks);
    assert_eq!(grid_text(deep), [[vec!["deep"]]]);
}

#[test]
fn an_inner_row_that_never_ends_keeps_its_text_in_the_outer_cell() {
    let body = table::assemble(vec![
        mark("i1", 2, End::InnerCell),
        mark("i2", 2, End::InnerCell),
        mark("after", 1, End::Cell),
        mark("b", 1, End::Cell),
        mark("", 1, End::Row(2)),
    ]);
    let outer = only_table(&body);
    assert_eq!(grid_text(outer), [[vec!["i1", "i2", "after"], vec!["b"]]]);
}

#[test]
fn a_table_without_depth_sprms_is_unchanged() {
    let body = table::assemble(vec![mark("a", 1, End::Cell), mark("b", 1, End::Cell), mark("", 1, End::Row(2)), mark("tail", 0, End::Para)]);
    assert_eq!(body.len(), 2);
    assert_eq!(grid_text(only_table(&body)), [[vec!["a"], vec!["b"]]]);
}
