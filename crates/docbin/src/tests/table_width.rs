//! `.doc` table preferred width: `sprmTTableWidth` (0xF614, [MS-DOC] 2.6.4) with a
//! `FtsWWidth_TablePart` operand (ftsWidth, wWidth) sets the table's `width` / `width_pct`.

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

/// A `sprmTTableWidth` Prl with the given operand bytes.
fn table_width(operand: &[u8]) -> Vec<u8> {
    super::grpprl(&[(0xF614, operand)])
}

/// The row-end TAP of a row of `cells` 1-inch cells, plus `tap`.
fn row_marks(cells: usize, tap: &[u8]) -> Vec<ParaOut> {
    let edges: Vec<i16> = (0..=cells).map(|i| i as i16 * 1440).collect();
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(&edges));
    end.extend(tap);
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[], true)).collect();
    marks.push(mark("", &end, true));
    marks
}

fn one_row(cells: usize, tap: &[u8]) -> Blocks {
    table::assemble(row_marks(cells, tap))
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn widths(operand: &[u8]) -> (Option<f32>, Option<f32>) {
    let body = one_row(2, &table_width(operand));
    let t = table(&body);
    (t.props.width, t.props.width_pct)
}

#[test]
fn twips_width_is_kept_in_points() {
    // 9750 twips = 487.5 pt.
    assert_eq!(widths(&[0x03, 0x16, 0x26]), (Some(487.5), None));
}

#[test]
fn system_twips_width_is_kept_in_points() {
    // ftsDxaSys (0x13), 1440 twips = 72 pt.
    assert_eq!(widths(&[0x13, 0xA0, 0x05]), (Some(72.0), None));
}

#[test]
fn percent_width_is_in_fiftieths() {
    assert_eq!(widths(&[0x02, 0x88, 0x13]), (None, Some(100.0)));
    assert_eq!(widths(&[0x02, 0xC4, 0x09]), (None, Some(50.0)));
}

#[test]
fn auto_and_nil_widths_stay_automatic() {
    assert_eq!(widths(&[0x01, 0x00, 0x00]), (None, None));
    assert_eq!(widths(&[0x00, 0x00, 0x00]), (None, None));
}

#[test]
fn a_negative_twips_width_clamps_to_zero() {
    assert_eq!(widths(&[0x03, 0xFF, 0xFF]), (Some(0.0), None));
}

#[test]
fn truncated_or_unknown_operands_are_ignored() {
    assert_eq!(widths(&[0x03, 0x16]), (None, None));
    assert_eq!(widths(&[]), (None, None));
    assert_eq!(widths(&[0x07, 0x16, 0x26]), (None, None));
}

#[test]
fn a_table_without_the_sprm_has_no_preferred_width() {
    let body = one_row(2, &[]);
    let t = table(&body);
    assert_eq!((t.props.width, t.props.width_pct), (None, None));
    // The grid still comes from the cell edges.
    assert_eq!(t.grid, vec![72.0, 72.0]);
}

#[test]
fn the_first_row_with_a_width_wins() {
    let mut marks = row_marks(2, &table_width(&[0x03, 0x16, 0x26]));
    marks.extend(row_marks(2, &table_width(&[0x03, 0xA0, 0x05])));
    let body = table::assemble(marks);
    let t = table(&body);
    assert_eq!(t.rows.len(), 2);
    assert_eq!((t.props.width, t.props.width_pct), (Some(487.5), None));
}
