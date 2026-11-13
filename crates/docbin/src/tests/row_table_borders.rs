//! `.doc` table borders are per row: `sprmTTableBorders80` (0xD605) and `sprmTTableBorders`
//! (0xD613) sit in each row's TAP ([MS-DOC] 2.6.3). The table takes the first row's; a later row
//! whose borders differ keeps its own as row exceptions, and a row without either sprm has none.

use wordcraft_doc::props::{Border, BorderStyle, Borders, CharProps};
use wordcraft_doc::table::Table;
use wordcraft_doc::{Blocks, para::Paragraph};

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

const S: Border = Border { style: BorderStyle::Single, width: 0.5, color: None, space: 0.0 };

fn none() -> Border {
    Border::default()
}

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

/// A `sprmTDefTable` Prl for `cells` 1-inch cells, every TC80 all zero.
fn def_table(cells: usize) -> Vec<u8> {
    let mut body = vec![cells as u8];
    for i in 0..=cells {
        body.extend_from_slice(&((i as i16) * 1440).to_le_bytes());
    }
    body.extend(std::iter::repeat_n(0u8, 20 * cells));
    let mut v = 0xD608u16.to_le_bytes().to_vec();
    v.extend_from_slice(&((body.len() + 1) as u16).to_le_bytes());
    v.extend_from_slice(&body);
    v
}

/// `sprmTTableBorders80` with six 0.5 pt single Brc80s.
fn tb() -> Vec<u8> {
    let mut op = vec![0x18];
    for _ in 0..6 {
        op.extend([4, 1, 0, 0]);
    }
    super::grpprl(&[(0xD605, &op)])
}

/// `sprmTTableBorders` with six automatic-colour 1.5 pt single Brcs.
fn tb_new() -> Vec<u8> {
    let mut op = vec![0x30];
    for _ in 0..6 {
        op.extend([0, 0, 0, 0xFF, 0x0C, 1, 0, 0]);
    }
    super::grpprl(&[(0xD613, &op)])
}

/// The marks of one row of `cells` cells whose row-end TAP holds `tap` after the definition.
fn row(cells: usize, tap: &[Vec<u8>]) -> Vec<ParaOut> {
    let mut end = TTP.to_vec();
    end.extend(def_table(cells));
    for part in tap {
        end.extend_from_slice(part);
    }
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[])).collect();
    marks.push(mark("", &end));
    marks
}

fn rows(taps: &[Vec<Vec<u8>>]) -> Blocks {
    table::assemble(taps.iter().flat_map(|t| row(2, t)).collect())
}

fn tables(blocks: &Blocks) -> Vec<&Table> {
    blocks.iter().filter_map(|b| b.as_table()).collect()
}

fn the_table(blocks: &Blocks) -> &Table {
    let t = tables(blocks);
    assert_eq!(t.len(), 1, "{blocks:?}");
    t[0]
}

fn exception_borders(t: &Table, r: usize) -> Option<Borders> {
    t.rows[r].props.exceptions.as_ref().map(|e| {
        let mut rest = (**e).clone();
        rest.borders = None;
        assert_eq!(rest, Default::default(), "only the borders differ from the table");
        e.borders.unwrap_or_default()
    })
}

#[test]
fn every_row_with_the_same_borders_has_no_exception() {
    let blocks = rows(&[vec![tb()], vec![tb()]]);
    let t = the_table(&blocks);
    assert_eq!(t.props.borders, Some(Borders::all(S)));
    assert!(t.rows.iter().all(|r| r.props.exceptions.is_none()));
    assert!(t.rows.iter().flat_map(|r| &r.cells).all(|c| c.props.borders.is_none()));
}

#[test]
fn a_row_without_borders_under_a_bordered_row_has_none() {
    let blocks = rows(&[vec![tb()], vec![]]);
    let t = the_table(&blocks);
    assert_eq!(t.props.borders, Some(Borders::all(S)));
    assert_eq!(exception_borders(t, 0), None);
    assert_eq!(exception_borders(t, 1), Some(Borders::all(none())));
}

#[test]
fn a_bordered_row_under_a_borderless_first_row_keeps_its_own() {
    let blocks = rows(&[vec![], vec![tb()]]);
    let t = the_table(&blocks);
    assert_eq!(t.props.borders, None);
    assert_eq!(exception_borders(t, 0), None);
    assert_eq!(exception_borders(t, 1), Some(Borders::all(S)));
}

#[test]
fn a_form_whose_first_rows_have_borders() {
    let taps: Vec<Vec<Vec<u8>>> = (0..34).map(|i| if i < 3 { vec![tb()] } else { vec![] }).collect();
    let blocks = rows(&taps);
    let t = the_table(&blocks);
    assert_eq!(t.rows.len(), 34);
    for r in 0..3 {
        assert_eq!(exception_borders(t, r), None, "row {r}");
    }
    for r in 3..34 {
        assert_eq!(exception_borders(t, r), Some(Borders::all(none())), "row {r}");
    }
}

#[test]
fn the_later_of_the_two_border_sprms_wins() {
    let wide = Border { style: BorderStyle::Single, width: 1.5, color: None, space: 0.0 };
    let mut marks = row(2, &[tb(), tb_new()]);
    marks.push(ParaOut { para: Paragraph::with_text("between", CharProps::default()), end_cp: 0, row: None });
    marks.extend(row(2, &[tb_new(), tb()]));
    let blocks = table::assemble(marks);
    let t = tables(&blocks);
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].props.borders, Some(Borders::all(wide)));
    assert_eq!(t[1].props.borders, Some(Borders::all(S)));
}

#[test]
fn hostile_truncated_border_sprms_give_no_borders() {
    // (a) cb 0x18 but only 10 operand bytes, as the last sprm of the TAP.
    let short80 = super::grpprl(&[(0xD605, &[0x18, 4, 1, 0, 0, 4, 1, 0, 0, 4])]);
    let blocks = rows(&[vec![short80]]);
    assert_eq!(the_table(&blocks).props.borders, None);
    // (b) cb 0x30 but only 20 bytes.
    let mut op = vec![0x30];
    op.extend([0, 0, 0, 0xFF, 0x0C, 1, 0, 0, 0, 0, 0, 0xFF, 0x0C, 1, 0, 0, 0, 0, 0]);
    let short = super::grpprl(&[(0xD613, &op)]);
    let blocks = rows(&[vec![short]]);
    assert_eq!(the_table(&blocks).props.borders, None);
    // The operands themselves, decoded directly, give nothing either.
    let info = table::decode(&[TTP, &super::grpprl(&[(0xD613, &[0x30, 1, 2, 3])])[..]].concat());
    let blocks = table::assemble(vec![mark("a", &[]), ParaOut { row: Some(Box::new(info)), ..mark("", TTP) }]);
    assert_eq!(the_table(&blocks).props.borders, None);
}

#[test]
fn hostile_two_thousand_alternating_rows() {
    let taps: Vec<Vec<Vec<u8>>> = (0..2000).map(|i| if i % 2 == 0 { vec![tb()] } else { vec![] }).collect();
    let blocks = rows(&taps);
    let t = the_table(&blocks);
    assert_eq!(t.rows.len(), 2000);
    assert_eq!(t.rows.iter().filter(|r| r.props.exceptions.is_some()).count(), 1000);
}
