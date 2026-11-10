//! Section columns and vertical alignment ([MS-DOC] §2.6.4): sprmSCcolumns 0x500B, sprmSDxaColumns 0x900C,
//! sprmSFEvenlySpaced 0x3005, sprmSDxaColWidth 0xF203, sprmSDxaColSpacing 0xF204, sprmSLBetween 0x3019 and
//! sprmSVjc 0x301A, read into the same model fields as DOCX `w:cols` and `w:vAlign`.

use wordcraft_doc::section::{Columns, PageVAlign};

use super::{StructSpec, grpprl, one_section, read, struct_doc};

const S_CCOLUMNS: u16 = 0x500B;
const S_DXA_COLUMNS: u16 = 0x900C;
const S_F_EVENLY_SPACED: u16 = 0x3005;
const S_DXA_COL_WIDTH: u16 = 0xF203;
const S_DXA_COL_SPACING: u16 = 0xF204;
const S_L_BETWEEN: u16 = 0x3019;
const S_VJC: u16 = 0x301A;

fn u16b(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

/// sprmSDxaColWidth / sprmSDxaColSpacing operand: iCol, then the twips.
fn col(i: u8, twips: u16) -> Vec<u8> {
    let mut v = vec![i];
    v.extend_from_slice(&twips.to_le_bytes());
    v
}

/// Columns and vertical alignment of a one-section document whose Sepx holds `prls`.
fn section(prls: &[(u16, Vec<u8>)]) -> (Columns, PageVAlign) {
    let prls: Vec<(u16, &[u8])> = prls.iter().map(|(op, v)| (*op, v.as_slice())).collect();
    let spec = one_section("Text\r", grpprl(&prls), vec![(0, 0, Vec::new())]);
    let doc = read(&struct_doc(&spec)).expect("opens");
    (doc.last_section.columns.clone(), doc.last_section.valign)
}

fn columns(prls: &[(u16, Vec<u8>)]) -> Columns {
    section(prls).0
}

#[test]
fn no_column_sprms_leave_one_column_with_the_default_spacing() {
    let c = columns(&[]);
    assert_eq!((c.count, c.space, c.separator, c.widths.is_empty()), (1, 36.0, false, true));
}

#[test]
fn column_spacing_is_read() {
    let c = columns(&[(S_CCOLUMNS, u16b(1)), (S_DXA_COLUMNS, u16b(360))]);
    assert_eq!((c.count, c.space, c.widths.is_empty()), (2, 18.0, true));
}

#[test]
fn spacing_before_the_column_count_is_kept() {
    let c = columns(&[(S_DXA_COLUMNS, u16b(360)), (S_CCOLUMNS, u16b(1))]);
    assert_eq!((c.count, c.space), (2, 18.0));
}

#[test]
fn line_between_is_read() {
    let c = columns(&[(S_CCOLUMNS, u16b(1)), (S_L_BETWEEN, vec![1])]);
    assert_eq!((c.count, c.separator), (2, true));
    let c = columns(&[(S_L_BETWEEN, vec![1]), (S_CCOLUMNS, u16b(1))]);
    assert!(c.separator, "a line between read before the count is kept");
}

#[test]
fn unequal_columns_take_their_widths_and_spacing() {
    let c = columns(&[
        (S_CCOLUMNS, u16b(2)),
        (S_F_EVENLY_SPACED, vec![0]),
        (S_DXA_COL_WIDTH, col(0, 2880)),
        (S_DXA_COL_WIDTH, col(1, 1440)),
        (S_DXA_COL_WIDTH, col(2, 2160)),
        (S_DXA_COL_SPACING, col(0, 720)),
        (S_DXA_COL_SPACING, col(1, 360)),
    ]);
    assert_eq!(c.count, 3);
    assert_eq!(c.widths, vec![(144.0, 36.0), (72.0, 18.0), (108.0, 0.0)]);
}

#[test]
fn widths_read_before_the_count_still_apply() {
    let c = columns(&[
        (S_DXA_COL_WIDTH, col(0, 2880)),
        (S_DXA_COL_WIDTH, col(1, 1440)),
        (S_DXA_COL_SPACING, col(0, 720)),
        (S_F_EVENLY_SPACED, vec![0]),
        (S_CCOLUMNS, u16b(1)),
    ]);
    assert_eq!(c.widths, vec![(144.0, 36.0), (72.0, 0.0)]);
}

#[test]
fn a_missing_column_width_keeps_equal_columns() {
    let c = columns(&[(S_CCOLUMNS, u16b(1)), (S_F_EVENLY_SPACED, vec![0]), (S_DXA_COL_WIDTH, col(0, 2880))]);
    assert_eq!((c.count, c.widths.is_empty()), (2, true));
}

#[test]
fn evenly_spaced_columns_ignore_column_widths() {
    let c = columns(&[(S_CCOLUMNS, u16b(1)), (S_F_EVENLY_SPACED, vec![1]), (S_DXA_COL_WIDTH, col(0, 2880)), (S_DXA_COL_WIDTH, col(1, 1440))]);
    assert_eq!((c.count, c.widths.is_empty()), (2, true));
    // Evenly spaced is the default when the sprm is absent.
    let c = columns(&[(S_CCOLUMNS, u16b(1)), (S_DXA_COL_WIDTH, col(0, 2880)), (S_DXA_COL_WIDTH, col(1, 1440))]);
    assert!(c.widths.is_empty());
}

#[test]
fn out_of_range_columns_are_ignored_and_the_last_column_has_no_spacing() {
    let c = columns(&[
        (S_CCOLUMNS, u16b(1)),
        (S_F_EVENLY_SPACED, vec![0]),
        (S_DXA_COL_WIDTH, col(0, 2880)),
        (S_DXA_COL_WIDTH, col(1, 1440)),
        (S_DXA_COL_WIDTH, col(7, 999)),
        (S_DXA_COL_WIDTH, col(200, 999)),
        (S_DXA_COL_SPACING, col(1, 500)),
    ]);
    assert_eq!(c.widths, vec![(144.0, 0.0), (72.0, 0.0)]);
}

#[test]
fn short_operands_are_ignored() {
    let c = columns(&[(S_CCOLUMNS, u16b(1)), (S_DXA_COLUMNS, vec![0x68])]);
    assert_eq!((c.count, c.space), (2, 36.0));
}

#[test]
fn the_later_of_two_equal_sprms_wins() {
    let c = columns(&[
        (S_CCOLUMNS, u16b(1)),
        (S_F_EVENLY_SPACED, vec![0]),
        (S_DXA_COL_WIDTH, col(0, 2880)),
        (S_DXA_COL_WIDTH, col(1, 1440)),
        (S_DXA_COL_WIDTH, col(0, 2000)),
    ]);
    assert_eq!(c.widths, vec![(100.0, 0.0), (72.0, 0.0)]);
}

#[test]
fn vertical_alignment_is_read() {
    let v = |b: u8| section(&[(S_VJC, vec![b])]).1;
    assert_eq!(section(&[]).1, PageVAlign::Top);
    assert_eq!(v(0), PageVAlign::Top);
    assert_eq!(v(1), PageVAlign::Center);
    assert_eq!(v(2), PageVAlign::Justify);
    assert_eq!(v(3), PageVAlign::Bottom);
    assert_eq!(v(9), PageVAlign::Top);
}

#[test]
fn each_section_keeps_its_own_columns() {
    let first: Vec<(u16, Vec<u8>)> = vec![(S_CCOLUMNS, u16b(1)), (S_DXA_COLUMNS, u16b(360)), (S_L_BETWEEN, vec![1]), (S_VJC, vec![1])];
    let first: Vec<(u16, &[u8])> = first.iter().map(|(op, v)| (*op, v.as_slice())).collect();
    let spec = StructSpec {
        sections: vec![(6, grpprl(&first)), (12, grpprl(&[(S_DXA_COLUMNS, &1440u16.to_le_bytes())]))],
        papx_entries: vec![(0, 0, Vec::new()), (6, 0, Vec::new())],
        ..one_section("first\rsecond\r", Vec::new(), Vec::new())
    };
    let doc = read(&struct_doc(&spec)).expect("opens");
    let p0 = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let sec = p0.section.as_ref().expect("section on the first paragraph");
    assert_eq!((sec.columns.count, sec.columns.space, sec.columns.separator, sec.valign), (2, 18.0, true, PageVAlign::Center));
    let last = &doc.last_section;
    assert_eq!((last.columns.count, last.columns.space, last.columns.separator, last.valign), (1, 72.0, false, PageVAlign::Top));
}
