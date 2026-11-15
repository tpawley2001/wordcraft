//! `.doc` table-level shading and each row's width before / after from the row-end TAP:
//! `sprmTSetShdTable` (0xD660, cb then a 10-byte Shd) sets the table's fill, and
//! `sprmTWidthBefore` / `sprmTWidthAfter` (0xF617 / 0xF618, FtsWWidth_TablePart: ftsWidth then a
//! signed 16-bit width) set the row's `w_before` / `w_after` ([MS-DOC] 2.6.4, 2.9.105, 2.9.248).

use wordcraft_doc::Blocks;
use wordcraft_doc::Rgb;
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::props::CharProps;
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

/// `sprmTSetShdTable` with a full Shd: cvFore, cvBack (COLORREF: red, green, blue, fAuto), ipat.
fn shd_table(fore: [u8; 4], back: [u8; 4], ipat: u16) -> Vec<u8> {
    let mut operand = vec![10u8];
    operand.extend(fore);
    operand.extend(back);
    operand.extend(ipat.to_le_bytes());
    super::grpprl(&[(0xD660, &operand)])
}

/// `sprmTWidthBefore` (before = true) or `sprmTWidthAfter`.
fn width(before: bool, fts: u8, w: i16) -> Vec<u8> {
    let mut operand = vec![fts];
    operand.extend(w.to_le_bytes());
    super::grpprl(&[(if before { 0xF617 } else { 0xF618 }, &operand)])
}

/// Rows of two cells each; each row's row-end TAP is `TTP`, a `sprmTDefTable` and the extra
/// sprms given for that row.
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

const RED: [u8; 4] = [0xFF, 0x00, 0x00, 0x00];
const GOLD: [u8; 4] = [0xFF, 0xCC, 0x00, 0x00];
const AUTO: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

#[test]
fn set_shd_table_sets_the_table_fill() {
    // (cvFore, cvBack, ipat, expected table fill)
    let cases = [
        (AUTO, GOLD, 0u16, Some(Rgb(0xFF, 0xCC, 0x00))), // clear: the background shows
        (RED, GOLD, 1, Some(Rgb(0xFF, 0x00, 0x00))),     // solid: the foreground shows
        (RED, AUTO, 0, None),                            // automatic background: no fill
        (RED, GOLD, 0xFFFF, None),                       // nil Shd: no fill
    ];
    for (fore, back, ipat, want) in cases {
        let b = rows(&[shd_table(fore, back, ipat)]);
        assert_eq!(table(&b).props.shading, want, "fore {fore:?} back {back:?} ipat {ipat:#x}");
    }
}

#[test]
fn table_fill_is_not_copied_into_the_cells() {
    let b = rows(&[shd_table(AUTO, GOLD, 0), vec![]]);
    let t = table(&b);
    assert_eq!(t.props.shading, Some(Rgb(0xFF, 0xCC, 0x00)));
    for (r, row) in t.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            assert_eq!(cell.props.shading, None, "row {r} cell {c}");
        }
    }
}

#[test]
fn a_truncated_or_missing_table_shd_gives_no_fill() {
    let short = super::grpprl(&[(0xD660, &[4, 0xFF, 0xCC, 0x00, 0x00])]);
    assert_eq!(table(&rows(&[short])).props.shading, None, "4-byte Shd");
    assert_eq!(table(&rows(&[vec![]])).props.shading, None, "no sprm");
}

#[test]
fn width_before_and_after_are_read_per_row() {
    let mut row0 = width(true, 3, 1440);
    row0.extend(width(false, 3, 720));
    let b = rows(&[row0, vec![], width(false, 3, 300)]);
    let t = table(&b);
    let got: Vec<(Option<f32>, Option<f32>)> = t.rows.iter().map(|r| (r.props.w_before, r.props.w_after)).collect();
    assert_eq!(got, vec![(Some(72.0), Some(36.0)), (None, None), (None, Some(15.0))]);
}

#[test]
fn width_before_only_counts_in_twips() {
    // (ftsWidth, wWidth, expected w_before in points)
    let cases = [
        (3u8, 0i16, Some(0.0)),
        (3, 2880, Some(144.0)),
        (3, 32767, Some(1584.0)), // clamped to 22 inches
        (3, -10, None),           // negative width
        (0, 1440, None),          // ftsNil
        (1, 1440, None),          // ftsAuto
        (2, 1440, None),          // percent is not a valid unit for a table part
    ];
    for (fts, w, want) in cases {
        let b = rows(&[width(true, fts, w)]);
        assert_eq!(table(&b).rows[0].props.w_before, want, "ftsWidth {fts} wWidth {w}");
    }
}

#[test]
fn a_truncated_width_operand_is_ignored() {
    // sprmTWidthAfter is a fixed 3-byte sprm; the TAP ends after 2 operand bytes.
    let tap = super::grpprl(&[(0xF618, &[3, 0x05])]);
    let b = rows(&[tap]);
    assert_eq!(table(&b).rows[0].props.w_after, None);
}
