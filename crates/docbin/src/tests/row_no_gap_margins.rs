//! `.doc` rows that store no `sprmTDxaGapHalf` (0x9602): a row's table properties start from
//! zeroes ([MS-DOC] 2.6.3), so a row that defines its cells without a gap half or a cell padding
//! has no left or right cell margin, not the 5.4 pt default of a `.docx` table.

use wordcraft_doc::props::CharProps;
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

/// A depth-1 cell or row mark with extra PAPX sprms.
fn mark(text: &str, extra: &[u8]) -> ParaOut {
    let mut papx = vec![0x16, 0x24, 1]; // sprmPFInTable
    papx.extend([0x49, 0x66]); // sprmPItap
    papx.extend(1i32.to_le_bytes());
    papx.extend(extra);
    let mut row = table::decode(&papx);
    row.cell_mark = true;
    ParaOut { para: Paragraph::with_text(text, CharProps::default()), end_cp: 0, row: Some(Box::new(row)) }
}

fn gap_half(twips: i16) -> Vec<u8> {
    super::grpprl(&[(0x9602, &twips.to_le_bytes())])
}

/// A CSSA Prl (`op` 0xD634 or 0xD632): cb = 6, itcFirst, itcLim, grfbrc, ftsWidth (twips), wWidth.
fn cssa(op: u16, first: u8, lim: u8, grfbrc: u8, width: u16) -> Vec<u8> {
    let mut operand = vec![6, first, lim, grfbrc, 3];
    operand.extend(width.to_le_bytes());
    super::grpprl(&[(op, &operand)])
}

/// The row-end TAP of a row of `cells` 1-inch cells, with `tap` after its `sprmTDefTable`.
fn row_end(cells: usize, tap: &[u8]) -> Vec<u8> {
    let edges: Vec<i16> = (0..=cells).map(|i| i as i16 * 1440).collect();
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(&edges));
    end.extend(tap);
    end
}

/// The marks of one row of `cells` cells.
fn row(cells: usize, tap: &[u8]) -> Vec<ParaOut> {
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[])).collect();
    marks.push(mark("", &row_end(cells, tap)));
    marks
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

/// The left and right margins cell `cell` of row `r` resolves to.
fn sides(t: &Table, r: usize, cell: usize) -> (f32, f32) {
    let defaults = t.props.cell_margins.unwrap_or([5.4; 4]);
    let m = t.rows[r].cells[cell].props.resolved_margins(defaults);
    (m[1], m[3])
}

#[test]
fn a_row_with_no_gap_and_no_padding_has_zero_side_margins() {
    assert_eq!(table::decode(&row_end(2, &[])).cell_margins, Some([0.0; 4]));
    let body = table::assemble(row(2, &[]));
    let t = table(&body);
    assert_eq!(t.props.cell_margins, Some([0.0; 4]));
    assert!(t.rows[0].cells.iter().all(|c| c.props.partial_margins.is_none() && c.props.margins.is_none()));
}

#[test]
fn a_mark_without_a_cell_definition_still_has_no_margins() {
    // Only a row that defines its cells takes the zero gap half.
    assert_eq!(table::decode(TTP).cell_margins, None);
}

#[test]
fn a_stored_gap_half_still_gives_its_margins() {
    let body = table::assemble(row(2, &gap_half(108)));
    assert_eq!(table(&body).props.cell_margins, Some([0.0, 5.4, 0.0, 5.4]));
    let body = table::assemble(row(2, &gap_half(0)));
    assert_eq!(table(&body).props.cell_margins, Some([0.0; 4]));
}

#[test]
fn default_padding_without_a_gap_is_unchanged() {
    let body = table::assemble(row(2, &cssa(0xD634, 0, 1, 0x02, 72)));
    assert_eq!(table(&body).props.cell_margins, Some([0.0, 3.6, 0.0, 0.0]));
}

#[test]
fn per_cell_padding_keeps_its_own_sides_on_a_gapless_row() {
    let body = table::assemble(row(3, &cssa(0xD632, 1, 2, 0x02, 60)));
    let t = table(&body);
    assert_eq!(t.props.cell_margins, Some([0.0; 4]));
    assert_eq!(t.rows[0].cells[1].props.partial_margins, Some([None, Some(3.0), None, None]));
    assert_eq!(sides(t, 0, 0), (0.0, 0.0));
    assert_eq!(sides(t, 0, 1), (3.0, 0.0));
}

#[test]
fn rows_of_one_table_keep_their_own_margins() {
    let mut marks = row(2, &[]);
    marks.extend(row(2, &gap_half(108)));
    let body = table::assemble(marks);
    let t = table(&body);
    assert_eq!(t.rows.len(), 2);
    assert_eq!(t.props.cell_margins, Some([0.0; 4]));
    for c in 0..2 {
        assert_eq!(sides(t, 0, c), (0.0, 0.0), "row 1 cell {c}");
        assert_eq!(sides(t, 1, c), (5.4, 5.4), "row 2 cell {c}");
    }
}
