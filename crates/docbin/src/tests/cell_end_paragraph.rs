//! The last paragraph of a `.doc` table cell is the one its cell mark ends ([MS-DOC] §2.4.3): a
//! cell written as `text`, 0x0D, cell mark holds two paragraphs, the second one empty, and Word
//! lays that one out as a blank line. An empty cell's only paragraph is its cell mark's own, with
//! that paragraph's style and formatting.

use wordcraft_doc::props::{Align, CharProps};
use wordcraft_doc::table::{Cell, Table};
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};
use crate::{fmt, sprm};

const P_IN_TABLE: u16 = 0x2416;
const P_TTP: u16 = 0x2417;
const P_INNER_CELL: u16 = 0x244B;
const P_INNER_TTP: u16 = 0x244C;
const P_ITAP: u16 = 0x6649;

/// What a paragraph mark at table depth `itap` ends.
#[derive(Clone, Copy)]
enum End {
    /// An ordinary paragraph inside a cell (0x0D).
    Para,
    /// A depth-1 cell (the 0x07 mark).
    Cell,
    /// A depth-1 row of `n` cells (0x07 with `sprmPFTtp` and the row's `sprmTDefTable`).
    Row(usize),
    /// A cell of a table at depth 2 (`sprmPFInnerTableCell`).
    InnerCell,
    /// A row of `n` cells of a table at depth 2 (`sprmPFInnerTtp` and its `sprmTDefTable`).
    InnerRow(usize),
}

fn mark(text: &str, itap: i32, end: End) -> ParaOut {
    let mut prls: Vec<(u16, Vec<u8>)> = vec![(P_IN_TABLE, vec![1]), (P_ITAP, itap.to_le_bytes().to_vec())];
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
    ParaOut { para, end_cp: 0, row: Some(Box::new(row)) }
}

/// `mark`, with the paragraph formatted the way the tests look for it afterwards: style
/// "Q Stem", centred, 6 pt after, a 14 pt paragraph mark.
fn styled(text: &str, itap: i32, end: End) -> ParaOut {
    let mut m = mark(text, itap, end);
    m.para.props.style = Some("Q Stem".to_string());
    m.para.props.align = Some(Align::Center);
    m.para.props.space_after = Some(6.0);
    m.para.mark.size = Some(14.0);
    m
}

fn only_table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "one table expected in {blocks:?}");
    tables[0]
}

/// The paragraphs of a cell; a nested table in it fails the test.
fn paras(c: &Cell) -> Vec<&Paragraph> {
    c.blocks.iter().map(|b| b.as_para().expect("a paragraph")).collect()
}

fn texts(c: &Cell) -> Vec<&str> {
    paras(c).iter().map(|p| p.text.as_str()).collect()
}

fn is_styled(p: &Paragraph) -> bool {
    p.props.style.as_deref() == Some("Q Stem")
        && p.props.align == Some(Align::Center)
        && p.props.space_after == Some(6.0)
        && p.mark.size == Some(14.0)
}

/// The corpus shape: `V`, cell mark, cell mark, `1.`, cell mark, `text`, 0x0D, cell mark, row mark.
#[test]
fn a_cell_written_as_text_paragraph_mark_cell_mark_has_an_empty_last_paragraph() {
    let body = table::assemble(vec![
        mark("V", 1, End::Cell),
        mark("", 1, End::Cell),
        mark("1.", 1, End::Cell),
        mark("text", 1, End::Para),
        mark("", 1, End::Cell),
        mark("", 1, End::Row(4)),
    ]);
    let t = only_table(&body);
    assert_eq!(t.rows.len(), 1);
    let cells = &t.rows[0].cells;
    assert_eq!(cells.len(), 4, "the row mark adds no cell");
    assert_eq!(texts(&cells[0]), ["V"]);
    assert_eq!(texts(&cells[1]), [""]);
    assert_eq!(texts(&cells[2]), ["1."]);
    assert_eq!(texts(&cells[3]), ["text", ""]);
}

#[test]
fn the_empty_last_paragraph_keeps_its_own_style_and_mark_formatting() {
    let body = table::assemble(vec![mark("text", 1, End::Para), styled("", 1, End::Cell), mark("b", 1, End::Cell), mark("", 1, End::Row(2))]);
    let cells = &only_table(&body).rows[0].cells;
    assert_eq!(cells.len(), 2);
    let first = paras(&cells[0]);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].text, "text");
    assert!(!is_styled(first[0]), "the text paragraph keeps its own (plain) formatting");
    assert_eq!(first[1].text, "");
    assert!(is_styled(first[1]), "{:?}", first[1]);
    assert!(first[1].props.frame.is_none());
}

#[test]
fn every_empty_paragraph_before_the_cell_mark_counts() {
    let body = table::assemble(vec![
        mark("a", 1, End::Para),
        mark("", 1, End::Para),
        mark("", 1, End::Cell),
        mark("b", 1, End::Cell),
        mark("", 1, End::Row(2)),
    ]);
    let cells = &only_table(&body).rows[0].cells;
    assert_eq!(cells.len(), 2);
    assert_eq!(texts(&cells[0]), ["a", "", ""]);
    assert_eq!(texts(&cells[1]), ["b"]);
}

#[test]
fn an_empty_cell_keeps_the_paragraph_of_its_cell_mark() {
    let body = table::assemble(vec![mark("a", 1, End::Cell), styled("", 1, End::Cell), mark("", 1, End::Row(2))]);
    let cells = &only_table(&body).rows[0].cells;
    assert_eq!(cells.len(), 2);
    let empty = paras(&cells[1]);
    assert_eq!(empty.len(), 1, "an empty cell holds one paragraph");
    assert_eq!(empty[0].text, "");
    assert!(is_styled(empty[0]), "{:?}", empty[0]);
    assert!(!is_styled(paras(&cells[0])[0]));
}

#[test]
fn a_row_of_empty_cells_keeps_one_paragraph_each_and_no_extra_cell() {
    let body = table::assemble(vec![
        styled("", 1, End::Cell),
        styled("", 1, End::Cell),
        mark("", 1, End::Row(2)),
        mark("x", 1, End::Cell),
        mark("y", 1, End::Cell),
        mark("", 1, End::Row(2)),
    ]);
    let t = only_table(&body);
    assert_eq!(t.rows.len(), 2);
    for c in &t.rows[0].cells {
        assert_eq!(texts(c), [""]);
        assert!(is_styled(paras(c)[0]));
    }
    assert_eq!(t.rows[0].cells.len(), 2);
    assert_eq!(t.rows[1].cells.len(), 2);
    assert_eq!(texts(&t.rows[1].cells[0]), ["x"]);
    assert_eq!(texts(&t.rows[1].cells[1]), ["y"]);
}

/// A nested table's cells end with `sprmPFInnerTableCell` marks: the same rule.
#[test]
fn a_nested_cell_keeps_its_empty_last_paragraph() {
    let body = table::assemble(vec![
        mark("in", 2, End::Para),
        styled("", 2, End::InnerCell),
        mark("", 2, End::InnerRow(1)),
        mark("after", 1, End::Cell),
        mark("", 1, End::Row(1)),
    ]);
    let outer = only_table(&body);
    assert_eq!(outer.rows[0].cells.len(), 1);
    let blocks = &outer.rows[0].cells[0].blocks;
    assert_eq!(blocks.len(), 2, "{blocks:?}");
    let inner = blocks[0].as_table().expect("the nested table");
    assert_eq!(inner.rows.len(), 1);
    assert_eq!(inner.rows[0].cells.len(), 1);
    let inner_paras = paras(&inner.rows[0].cells[0]);
    assert_eq!(inner_paras.len(), 2);
    assert_eq!(inner_paras[0].text, "in");
    assert_eq!(inner_paras[1].text, "");
    assert!(is_styled(inner_paras[1]));
    assert_eq!(blocks[1].as_para().map(|p| p.text.as_str()), Some("after"));
}
