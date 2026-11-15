//! `.doc` table style options and direction from the row-end TAP: `sprmTTlp` (0x740A, a TLP:
//! `itl` then `grfatl`, the Fatl bits that `w:tblLook w:val` also uses) sets the table's look,
//! and `sprmTFBiDi` (0x560B, 1 = right-to-left) sets `bidi_visual` ([MS-DOC] 2.6.4, 2.9.69, 2.9.322).

use wordcraft_doc::Blocks;
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::props::{CharProps, TableLook};
use wordcraft_doc::table::Table;

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

/// A depth-1 mark with extra PAPX sprms.
fn mark(text: &str, extra: &[u8]) -> ParaOut {
    let mut papx = vec![0x16, 0x24, 1]; // sprmPFInTable
    papx.extend([0x49, 0x66]); // sprmPItap
    papx.extend(1i32.to_le_bytes());
    papx.extend(extra);
    let mut row = table::decode(&papx);
    row.cell_mark = true;
    ParaOut { para: Paragraph::with_text(text, CharProps::default()), end_cp: 0, row: Some(Box::new(row)) }
}

/// `sprmTTlp` with itl = 0 and the given grfatl.
fn tlp(grfatl: u16) -> Vec<u8> {
    let mut operand = 0u16.to_le_bytes().to_vec();
    operand.extend(grfatl.to_le_bytes());
    super::grpprl(&[(0x740A, &operand)])
}

fn bidi(v: i16) -> Vec<u8> {
    super::grpprl(&[(0x560B, &v.to_le_bytes())])
}

/// Rows of two cells each; each row's row-end TAP is `sprmTTableHeader`-free `TTP`, a
/// `sprmTDefTable` and the extra sprms given for that row.
fn rows(taps: &[Vec<u8>]) -> Blocks {
    let mut marks = Vec::new();
    for (r, extra) in taps.iter().enumerate() {
        marks.push(mark(&format!("r{r}c0"), &[]));
        marks.push(mark(&format!("r{r}c1"), &[]));
        let mut end = TTP.to_vec();
        end.extend(super::t_def_table(&[0, 2880, 5760]));
        end.extend_from_slice(extra);
        marks.push(mark("", &end));
    }
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn look(h: bool, t: bool, br: bool, fc: bool, lc: bool, bc: bool) -> TableLook {
    TableLook { header_row: h, total_row: t, banded_rows: br, first_column: fc, last_column: lc, banded_columns: bc }
}

#[test]
fn tlp_bits_set_the_table_look() {
    // (grfatl, header, total, banded rows, first col, last col, banded cols)
    let cases = [
        (0x04A0u16, look(true, false, true, true, false, false)),
        (0x0660, look(true, true, false, false, false, false)),
        (0x0180, look(false, false, true, true, true, true)),
        // Old AutoFormat bits (borders, shading, font, color, best fit) do not change the look.
        (0x001F, look(false, false, true, false, false, true)),
    ];
    for (bits, want) in cases {
        let b = rows(&[tlp(bits)]);
        assert_eq!(table(&b).props.look, want, "grfatl {bits:#06x}");
    }
}

#[test]
fn a_truncated_or_missing_tlp_keeps_the_default_look() {
    let short = super::grpprl(&[(0x740A, &[0, 0])]);
    assert_eq!(table(&rows(&[short])).props.look, TableLook::default(), "2-byte operand");
    assert_eq!(table(&rows(&[Vec::new()])).props.look, TableLook::default(), "no sprmTTlp");
}

#[test]
fn tfbidi_makes_the_table_right_to_left() {
    assert!(table(&rows(&[bidi(1)])).props.bidi_visual);
    assert!(!table(&rows(&[bidi(0)])).props.bidi_visual);
    assert!(!table(&rows(&[Vec::new()])).props.bidi_visual);
}

#[test]
fn the_first_row_sets_look_and_direction() {
    let first = [tlp(0x0660), bidi(1)].concat();
    let b = rows(&[first, Vec::new()]);
    let t = table(&b);
    assert_eq!(t.rows.len(), 2);
    assert_eq!(t.props.look, look(true, true, false, false, false, false));
    assert!(t.props.bidi_visual);
}
