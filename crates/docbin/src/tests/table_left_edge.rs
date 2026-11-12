//! `.doc` table position: the row's first cell edge (`rgdxaCenter[0]` of `sprmTDefTable`, or
//! `sprmTDxaLeft` less `sprmTDxaGapHalf`) plus the table's default left cell margin is the
//! table's indent, also when the PAPX has no `sprmTDxaLeft` ([MS-DOC] 2.6.4, 2.9.320). Word 2000
//! and later write rows that way; Word draws their edge at the margin plus `rgdxaCenter[0]`.

use wordcraft_doc::props::{Align, CharProps};
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

fn def_table(edges: &[i16]) -> Vec<u8> {
    super::t_def_table(edges)
}

fn gap_half(twips: i16) -> Vec<u8> {
    super::grpprl(&[(0x9602, &twips.to_le_bytes())])
}

fn dxa_left(twips: i16) -> Vec<u8> {
    super::grpprl(&[(0x9601, &twips.to_le_bytes())])
}

/// `sprmTCellPaddingDefault` for the left side only: cb = 6, itcFirst, itcLim, grfbrc (2 = left),
/// ftsWidth (3 = twips), wWidth.
fn left_padding(twips: u16) -> Vec<u8> {
    let mut operand = vec![6, 0, 1, 0x02, 3];
    operand.extend(twips.to_le_bytes());
    super::grpprl(&[(0xD634, &operand)])
}

/// One table of one row whose row-end TAP is `tap` (the sprms in the order given), with as many
/// cells as `cells`.
fn one_row(cells: usize, tap: &[&[u8]]) -> Blocks {
    let mut end = TTP.to_vec();
    for part in tap {
        end.extend_from_slice(part);
    }
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[], true)).collect();
    marks.push(mark("", &end, true));
    table::assemble(marks)
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn indent(cells: usize, tap: &[&[u8]]) -> Option<f32> {
    table(&one_row(cells, tap)).props.indent
}

fn assert_indent(got: Option<f32>, want: f32, what: &str) {
    match got {
        Some(v) => assert!((v - want).abs() < 0.01, "{what}: indent {v}, expected {want}"),
        None => panic!("{what}: no indent, expected {want}"),
    }
}

#[test]
fn first_cell_edge_alone_indents_the_table() {
    // Measured in Word: both tables' edges stand 9 pt right of the margin and their text
    // starts 12.6 pt and 14.4 pt right of it (the edge plus the 72- and 108-twip cell margin).
    assert_indent(indent(2, &[&def_table(&[180, 947, 9900]), &gap_half(72)]), 12.6, "gap half 72");
    assert_indent(indent(2, &[&def_table(&[180, 1019, 9900]), &gap_half(108)]), 14.4, "gap half 108");
}

#[test]
fn an_ordinary_table_has_no_indent() {
    // The first cell's edge one cell margin left of the margin: its text starts at the margin.
    assert_eq!(indent(2, &[&def_table(&[-108, 4212, 8532]), &gap_half(108)]), None);
    assert_eq!(indent(2, &[&gap_half(108), &def_table(&[-108, 4212, 8532])]), None, "gap half first");
}

#[test]
fn an_outdented_table_has_a_negative_indent() {
    // Edge 540 twips left of the margin, text 432 twips (21.6 pt) left of it.
    assert_indent(indent(2, &[&def_table(&[-540, 3240, 5040]), &gap_half(108)]), -21.6, "outdent");
}

#[test]
fn dxa_left_and_the_edge_it_made_agree() {
    // Word's own order when a row carries both: sprmTDxaLeft 252 = the edge (180) + gap half (72).
    assert_indent(indent(2, &[&dxa_left(252), &gap_half(72), &def_table(&[180, 947, 9900])]), 12.6, "both");
}

#[test]
fn dxa_left_after_the_edges_moves_the_row() {
    // No gap half: the edge and the text both start 720 twips in.
    assert_indent(indent(2, &[&def_table(&[0, 1440, 2880]), &dxa_left(720)]), 36.0, "no gap half");
    // With a gap half the edge is at 612 twips and the text at 720.
    assert_indent(indent(2, &[&def_table(&[0, 1440, 2880]), &gap_half(108), &dxa_left(720)]), 36.0, "gap half 108");
}

#[test]
fn the_default_left_padding_is_the_margin_added_to_the_edge() {
    // sprmTCellPaddingDefault's left side (144 twips) wins over the gap half (72): 180 + 144.
    let got = indent(2, &[&def_table(&[180, 947, 9900]), &gap_half(72), &left_padding(144)]);
    assert_indent(got, 16.2, "left padding 144");
}

#[test]
fn a_centred_table_keeps_both_its_alignment_and_its_edge() {
    let jc_center = super::grpprl(&[(0x5400, &1u16.to_le_bytes())]);
    let body = one_row(2, &[&def_table(&[180, 947, 9900]), &gap_half(72), &jc_center]);
    let t = table(&body);
    assert_eq!(t.props.align, Some(Align::Center));
    assert_indent(t.props.indent, 12.6, "centred");
}

#[test]
fn the_table_takes_its_first_row_s_edge_and_keeps_its_column_widths() {
    // Two rows; the second starts further right. The table's indent is the first row's, and the
    // first row's cells keep their widths (767 and 8953 twips). The rows share one grid of both
    // rows' edges: the second row starts one column in.
    let row = |edges: &[i16], gap: i16| {
        let mut end = TTP.to_vec();
        end.extend(def_table(edges));
        end.extend(gap_half(gap));
        vec![mark("a", &[], true), mark("b", &[], true), mark("", &end, true)]
    };
    let mut marks = row(&[180, 947, 9900], 72);
    marks.extend(row(&[360, 947, 9900], 72));
    let body = table::assemble(marks);
    let t = table(&body);
    assert_indent(t.props.indent, 12.6, "first row");
    assert_eq!(t.rows.len(), 2);
    assert_eq!(t.grid.len(), 3, "{:?}", t.grid);
    assert!((t.grid[0] - 9.0).abs() < 0.01 && (t.grid[1] - 29.35).abs() < 0.01 && (t.grid[2] - 447.65).abs() < 0.01, "{:?}", t.grid);
    let first: Vec<f32> = t.rows[0].cells.iter().filter_map(|c| c.props.width).collect();
    assert!(first.len() == 2 && (first[0] - 38.35).abs() < 0.01 && (first[1] - 447.65).abs() < 0.01, "{first:?}");
    assert_eq!(t.rows[0].cells.iter().map(|c| c.span()).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(t.rows[1].props.grid_before, 1);
}
