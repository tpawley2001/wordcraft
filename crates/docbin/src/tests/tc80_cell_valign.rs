//! `.doc` cell vertical alignment stored in the row definition: `sprmTDefTable` (0xD608) carries
//! one 20-byte `TC80` per cell whose first word is the `TCGRF` flags; bits 7-8 are `vertAlign`
//! (0 top, 1 center, 2 bottom) ([MS-DOC] 2.9.325, 2.9.313, 2.9.314). Word 97-2003 files often
//! store the alignment only there, with no `sprmTVertAlign` (0xD62C).

use wordcraft_doc::props::{CharProps, VAlign};
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

/// A `sprmTDefTable` Prl: `edges` twip boundaries, then one TC80 per entry of `tcgrf` (flag
/// word, wWidth 0, four nil Brc80s). `tcgrf` may be shorter than the column count.
fn def_table(edges: &[i16], tcgrf: &[u16]) -> Vec<u8> {
    let mut body = vec![edges.len() as u8 - 1];
    for e in edges {
        body.extend_from_slice(&e.to_le_bytes());
    }
    for g in tcgrf {
        body.extend_from_slice(&g.to_le_bytes());
        body.extend_from_slice(&[0u8; 18]);
    }
    let cb = (body.len() + 1) as u16; // cb = bytes after cb + 1
    let mut v = 0xD608u16.to_le_bytes().to_vec();
    v.extend_from_slice(&cb.to_le_bytes());
    v.extend_from_slice(&body);
    v
}

/// `sprmTVertAlign`: cb = 3, itcFirst, itcLim, vertAlign.
fn vert_align(first: u8, lim: u8, va: u8) -> Vec<u8> {
    super::grpprl(&[(0xD62C, &[3, first, lim, va])])
}

/// One table of one row whose row-end TAP is `tap` (the sprms in the order given).
fn one_row(cells: usize, tap: &[&[u8]]) -> Blocks {
    let mut end = TTP.to_vec();
    for part in tap {
        end.extend_from_slice(part);
    }
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[])).collect();
    marks.push(mark("", &end));
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn valigns(blocks: &Blocks) -> Vec<Option<VAlign>> {
    table(blocks).rows[0].cells.iter().map(|c| c.props.valign).collect()
}

/// The four-cell row of the Word 2003 file this was found in: edges -5.4, 90, 238.95, 328.5, 477 pt.
const EDGES4: [i16; 5] = [-108, 1800, 4779, 6570, 9540];

#[test]
fn control_default_tc80s_leave_cells_without_alignment() {
    // TCGRF 0x0600 (ftsWidth 3, vertAlign 0) is what Word writes for a top-aligned cell.
    let blocks = one_row(4, &[&def_table(&EDGES4, &[0x0600; 4])]);
    assert_eq!(valigns(&blocks), [None; 4]);
    assert_eq!(table(&blocks).grid.len(), 4);
}

#[test]
fn control_sprm_t_vert_align_still_sets_its_range() {
    let blocks = one_row(4, &[&def_table(&EDGES4, &[0x0600; 4]), &vert_align(1, 3, 1)]);
    assert_eq!(valigns(&blocks), [None, Some(VAlign::Center), Some(VAlign::Center), None]);
}

#[test]
fn tc80_bottom_on_every_cell_of_the_row() {
    // TCGRF 0x0700: vertAlign 2. Word draws the four cells' last lines on one line.
    let blocks = one_row(4, &[&def_table(&EDGES4, &[0x0700; 4])]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Bottom); 4]);
    assert_eq!(table(&blocks).grid.len(), 4, "the grid is read as before");
}

#[test]
fn tc80_center_and_mixed_rows() {
    let blocks = one_row(2, &[&def_table(&[-108, 4779, 9540], &[0x0680, 0x0680])]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Center); 2]);
    let blocks = one_row(4, &[&def_table(&EDGES4, &[0x0600, 0x0700, 0x0600, 0x0600])]);
    assert_eq!(valigns(&blocks), [None, Some(VAlign::Bottom), None, None]);
    // The alignment bits do not depend on the width type or the other flags.
    let blocks = one_row(3, &[&def_table(&[0, 1440, 2880, 4320], &[0x0100, 0x0080, 0x2700])]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Bottom), Some(VAlign::Center), Some(VAlign::Bottom)]);
}

#[test]
fn undefined_value_three_is_no_alignment() {
    let blocks = one_row(2, &[&def_table(&[0, 1440, 2880], &[0x0780, 0x0180])]);
    assert_eq!(valigns(&blocks), [None; 2]);
}

#[test]
fn later_sprm_t_vert_align_overrides_its_cells_only() {
    let blocks = one_row(3, &[&def_table(&[0, 1440, 2880, 4320], &[0x0700; 3]), &vert_align(1, 2, 1)]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Bottom), Some(VAlign::Center), Some(VAlign::Bottom)]);
    let blocks = one_row(3, &[&def_table(&[0, 1440, 2880, 4320], &[0x0700; 3]), &vert_align(0, 3, 0)]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Top); 3], "an explicit top from the later sprm");
}

#[test]
fn definition_replaces_an_earlier_alignment() {
    // sprmTVertAlign first, then the definition: the definition's cells win.
    let blocks = one_row(2, &[&vert_align(0, 2, 1), &def_table(&[0, 1440, 2880], &[0x0700, 0x0600])]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Bottom), None]);
}

#[test]
fn short_tc80_array_reads_the_records_present() {
    // Three columns, one whole TC80: the other cells are default cells.
    let blocks = one_row(3, &[&def_table(&[0, 1440, 2880, 4320], &[0x0700])]);
    assert_eq!(valigns(&blocks), [Some(VAlign::Bottom), None, None]);
    assert_eq!(table(&blocks).grid.len(), 3);
    // A TC80 cut after its flag word is not a whole record: no alignment, no panic.
    let mut cut = def_table(&[0, 1440, 2880], &[]);
    cut.extend_from_slice(&0x0700u16.to_le_bytes());
    let cb = (cut.len() - 4 + 1) as u16;
    cut[2..4].copy_from_slice(&cb.to_le_bytes());
    let blocks = one_row(2, &[&cut]);
    assert_eq!(valigns(&blocks), [None; 2]);
}
