//! `.doc` turned table cells: bits 2-4 of a cell's `TCGRF` word (the first word of its `TC80` in
//! `sprmTDefTable`, 0xD608) give its text flow, and `sprmTTextFlow` (0x7629) sets it for a range
//! of cells ([MS-DOC] 2.9.313 TCGRF, TextFlow, sprmTTextFlow). 3 runs bottom to top, 1 top to
//! bottom; 0 and the undefined values are horizontal.

use wordcraft_doc::props::{CharProps, TextDirection};
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];
const EDGES: [i16; 4] = [0, 3000, 3360, 3720];

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
    let cb = (body.len() + 1) as u16; // cb = bytes after cb + 1
    let mut v = 0xD608u16.to_le_bytes().to_vec();
    v.extend_from_slice(&cb.to_le_bytes());
    v.extend_from_slice(&body);
    v
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

fn directions(blocks: &Blocks) -> Vec<TextDirection> {
    table(blocks).rows[0].cells.iter().map(|c| c.props.text_direction).collect()
}

#[test]
fn tc80_text_flow_3_is_bottom_to_top() {
    // The header row of a report: a horizontal text cell, then two cells drawn bottom to top.
    let body = one_row(3, &[&def_table(&EDGES, &[0x0680, 0x068c, 0x068c])]);
    assert_eq!(directions(&body), vec![TextDirection::Horizontal, TextDirection::Up, TextDirection::Up]);
}

#[test]
fn tc80_text_flow_1_is_top_to_bottom() {
    let body = one_row(3, &[&def_table(&EDGES, &[0x0684, 0x0000, 0x0684])]);
    assert_eq!(directions(&body), vec![TextDirection::Down, TextDirection::Horizontal, TextDirection::Down]);
}

#[test]
fn tc80_undefined_text_flow_is_horizontal() {
    // Text flow values 2, 6 and 7 (bits 2-4) are not defined.
    let body = one_row(3, &[&def_table(&EDGES, &[2 << 2, 6 << 2, 7 << 2])]);
    assert_eq!(directions(&body), vec![TextDirection::Horizontal; 3]);
}

/// `sprmTTextFlow`: itcFirst, itcLim, then the text flow word.
fn text_flow_sprm(first: u8, lim: u8, flow: u16) -> Vec<u8> {
    let mut v = 0x7629u16.to_le_bytes().to_vec();
    v.extend([first, lim]);
    v.extend(flow.to_le_bytes());
    v
}

#[test]
fn text_flow_sprm_sets_its_cell_range() {
    let body = one_row(3, &[&def_table(&EDGES, &[0, 0, 0]), &text_flow_sprm(1, 3, 3)]);
    assert_eq!(directions(&body), vec![TextDirection::Horizontal, TextDirection::Up, TextDirection::Up]);
    // Applied after the definition: it turns a turned cell back too.
    let body = one_row(3, &[&def_table(&EDGES, &[0x068c, 0x068c, 0x068c]), &text_flow_sprm(0, 1, 0)]);
    assert_eq!(directions(&body), vec![TextDirection::Horizontal, TextDirection::Up, TextDirection::Up]);
    // A range past the row's cells is cut to them.
    let body = one_row(3, &[&def_table(&EDGES, &[0, 0, 0]), &text_flow_sprm(2, 255, 1)]);
    assert_eq!(directions(&body), vec![TextDirection::Horizontal, TextDirection::Horizontal, TextDirection::Down]);
}

#[test]
fn a_short_text_flow_sprm_is_ignored() {
    // Cut short: the operand ends before its text flow word.
    let mut short = 0x7629u16.to_le_bytes().to_vec();
    short.extend([1, 3, 3]);
    let body = one_row(3, &[&def_table(&EDGES, &[0, 0, 0]), &short]);
    assert_eq!(directions(&body), vec![TextDirection::Horizontal; 3]);
    // A value that is no text flow at all.
    let body = one_row(3, &[&def_table(&EDGES, &[0x068c, 0x068c, 0x068c]), &text_flow_sprm(0, 3, 0x0103)]);
    assert_eq!(directions(&body), vec![TextDirection::Up; 3]);
}
