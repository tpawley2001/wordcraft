//! `.doc` cells set not to wrap: bit 13 (`fNoWrap`, 0x2000) of the `TCGRF` flag word that opens
//! each 20-byte `TC80` of `sprmTDefTable` (0xD608) ([MS-DOC] 2.9.320, 2.9.313). The flag reaches
//! the model's `no_wrap`; the cell's width and vertical alignment are read as before.

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
/// word, wWidth 0, four nil Brc80s).
fn def_table(edges: &[i16], tcgrf: &[u16]) -> Vec<u8> {
    let mut body = vec![edges.len() as u8 - 1];
    for e in edges {
        body.extend_from_slice(&e.to_le_bytes());
    }
    for g in tcgrf {
        body.extend_from_slice(&g.to_le_bytes());
        body.extend_from_slice(&[0u8; 18]);
    }
    let cb = (body.len() + 1) as u16;
    let mut v = 0xD608u16.to_le_bytes().to_vec();
    v.extend_from_slice(&cb.to_le_bytes());
    v.extend_from_slice(&body);
    v
}

fn one_row(cells: usize, tap: &[u8]) -> Blocks {
    let mut end = TTP.to_vec();
    end.extend_from_slice(tap);
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[])).collect();
    marks.push(mark("", &end));
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

/// Each cell's (no_wrap, vertical alignment, width).
fn cells(blocks: &Blocks) -> Vec<(bool, Option<VAlign>, Option<f32>)> {
    table(blocks).rows[0].cells.iter().map(|c| (c.props.no_wrap, c.props.valign, c.props.width)).collect()
}

/// The price-list row: 47 / 110 / 54 pt.
const EDGES: [i16; 4] = [0, 940, 3140, 4220];

#[test]
fn no_wrap_bit_sets_the_cells_flag_only() {
    let blocks = one_row(3, &def_table(&EDGES, &[0x2700, 0x0600, 0x2600]));
    let got = cells(&blocks);
    assert_eq!(got.iter().map(|c| c.0).collect::<Vec<_>>(), [true, false, true]);
    // The same row without the bit: alignment and widths are unchanged.
    let plain = cells(&one_row(3, &def_table(&EDGES, &[0x0700, 0x0600, 0x0600])));
    assert_eq!(got.iter().map(|c| (c.1, c.2)).collect::<Vec<_>>(), plain.iter().map(|c| (c.1, c.2)).collect::<Vec<_>>());
    assert_eq!(got[0].1, Some(VAlign::Bottom));
    assert_eq!(got.iter().map(|c| c.2).collect::<Vec<_>>(), [Some(47.0), Some(110.0), Some(54.0)]);
    assert!(plain.iter().all(|c| !c.0));
}

#[test]
fn short_or_cut_tc80_array_does_not_panic() {
    // Three columns, one whole TC80: only that cell is read.
    let blocks = one_row(3, &def_table(&EDGES, &[0x2600]));
    assert_eq!(cells(&blocks).iter().map(|c| c.0).collect::<Vec<_>>(), [true, false, false]);
    // A TC80 cut after its flag word is not a whole record.
    let mut cut = def_table(&EDGES[..3], &[]);
    cut.extend_from_slice(&0x2600u16.to_le_bytes());
    let cb = (cut.len() - 4 + 1) as u16;
    cut[2..4].copy_from_slice(&cb.to_le_bytes());
    let blocks = one_row(2, &cut);
    assert_eq!(cells(&blocks).iter().map(|c| c.0).collect::<Vec<_>>(), [false; 2]);
    // An operand that claims more cells than it holds.
    let mut liar = def_table(&EDGES, &[0x2600]);
    if let Some(n) = liar.get_mut(4) {
        *n = 60;
    }
    let _ = one_row(3, &liar);
}
