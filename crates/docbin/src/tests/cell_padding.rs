//! `.doc` table cell padding: `sprmTCellPaddingDefault` (0xD634) gives the table's default cell
//! margins, `sprmTCellPadding` (0xD632) a cell range's, and `sprmTDxaGapHalf` the left and right
//! ones the padding Prl leaves unset ([MS-DOC] 2.6.4, 2.9.43 CSSA).

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

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

/// A CSSA Prl (`op` 0xD634 or 0xD632): cb = 6, itcFirst, itcLim, grfbrc, ftsWidth, wWidth.
fn cssa(op: u16, first: u8, lim: u8, grfbrc: u8, fts: u8, width: u16) -> Vec<u8> {
    let mut operand = vec![6, first, lim, grfbrc, fts];
    operand.extend(width.to_le_bytes());
    super::grpprl(&[(op, &operand)])
}

fn gap_half(twips: i16) -> Vec<u8> {
    super::grpprl(&[(0x9602, &twips.to_le_bytes())])
}

/// One table of `cells` cells, 1 inch each, whose row-end TAP carries `tap`.
fn one_row(cells: usize, tap: &[u8]) -> Blocks {
    let edges: Vec<i16> = (0..=cells).map(|i| i as i16 * 1440).collect();
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(&edges));
    end.extend(tap);
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[], true)).collect();
    marks.push(mark("", &end, true));
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

#[test]
fn padding_default_on_all_sides_sets_the_table_cell_margins() {
    let body = one_row(2, &cssa(0xD634, 0, 1, 0x0F, 3, 60));
    let t = table(&body);
    assert_eq!(t.props.cell_margins, Some([3.0, 3.0, 3.0, 3.0]));
    assert!(t.rows[0].cells.iter().all(|c| c.props.partial_margins.is_none() && c.props.margins.is_none()));
}

#[test]
fn top_and_bottom_padding_take_left_and_right_from_the_gap_half() {
    let mut tap = cssa(0xD634, 0, 1, 0x05, 3, 40);
    tap.extend(gap_half(108));
    let body = one_row(2, &tap);
    assert_eq!(table(&body).props.cell_margins, Some([2.0, 5.4, 2.0, 5.4]));
}

#[test]
fn a_gap_half_alone_gives_left_and_right_margins() {
    let body = one_row(2, &gap_half(60));
    assert_eq!(table(&body).props.cell_margins, Some([0.0, 3.0, 0.0, 3.0]));
}

#[test]
fn a_padding_prl_with_no_width_type_is_ignored() {
    let mut tap = cssa(0xD634, 0, 1, 0x0F, 0, 60);
    tap.extend(gap_half(108));
    let body = one_row(2, &tap);
    assert_eq!(table(&body).props.cell_margins, Some([0.0, 5.4, 0.0, 5.4]));
}

#[test]
fn per_cell_padding_overrides_only_its_cells_and_sides() {
    let mut tap = cssa(0xD634, 0, 1, 0x0F, 3, 60);
    tap.extend(cssa(0xD632, 1, 2, 0x02, 3, 0));
    let body = one_row(3, &tap);
    let t = table(&body);
    assert_eq!(t.props.cell_margins, Some([3.0, 3.0, 3.0, 3.0]));
    let cells = &t.rows[0].cells;
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[1].props.partial_margins, Some([None, Some(0.0), None, None]));
    assert!(cells[0].props.partial_margins.is_none());
    assert!(cells[2].props.partial_margins.is_none());
}

#[test]
fn hostile_padding_is_clamped() {
    for width in [i16::MAX as u16, u16::MAX] {
        let body = one_row(1, &cssa(0xD634, 0, 1, 0x01, 3, width));
        let top = table(&body).props.cell_margins.map(|m| m[0]);
        assert!(top.is_some_and(|t| (0.0..=1584.0).contains(&t)), "{width}: {top:?}");
    }
}

#[test]
fn a_truncated_padding_prl_sets_nothing() {
    let tap = super::grpprl(&[(0xD634, &[3u8, 0, 1][..])]);
    let body = one_row(1, &tap);
    assert_eq!(table(&body).props.cell_margins, None);
}

#[test]
fn a_second_table_does_not_inherit_the_first_ones_padding() {
    let edges: [i16; 2] = [0, 1440];
    let mut padded = TTP.to_vec();
    padded.extend(super::t_def_table(&edges));
    padded.extend(cssa(0xD634, 0, 1, 0x0F, 3, 60));
    let mut plain = TTP.to_vec();
    plain.extend(super::t_def_table(&edges));
    let body = table::assemble(vec![
        mark("a", &[], true),
        mark("", &padded, true),
        ParaOut { para: Paragraph::with_text("between", CharProps::default()), end_cp: 0, row: None },
        mark("b", &[], true),
        mark("", &plain, true),
    ]);
    let tables: Vec<&Table> = body.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 2, "{body:?}");
    assert_eq!(tables[0].props.cell_margins, Some([3.0, 3.0, 3.0, 3.0]));
    assert_eq!(tables[1].props.cell_margins, None);
}
