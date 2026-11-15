//! `.doc` table styles ([MS-DOC] 2.9.260 STD, 2.9.269 StkTableGRLPUPX): a table style (stk 3)
//! holds three UPXs, table sprms, then a UpxPapx (istd + grpprl), then character sprms. A row
//! names its table style with `sprmTIstd` (0x563A), and `sprmTCellFHideMark` (0xD642) sets the
//! cells' Hide end-of-cell mark.

use wordcraft_doc::props::{Align, Border, BorderStyle, Borders};
use wordcraft_doc::styles::StyleKind;
use wordcraft_doc::{Document, Style};

use super::{StructSpec, grpprl, read, std_bytes, struct_doc_styled, structured_spec, t_def_table};

const T_ISTD: u16 = 0x563A;
const T_HIDE_MARK: u16 = 0xD642;
const T_BORDERS_80: u16 = 0xD605;
const T_PADDING_DEFAULT: u16 = 0xD634;
const C_BOLD: u16 = 0x0835;
const P_JC80: u16 = 0x2403;

const S: Border = Border { style: BorderStyle::Single, width: 0.5, color: None, space: 0.0 };

/// One table-style STD (stk 3, cupx 3, user-defined, based on nothing).
fn table_std(name: &str, tapx: &[u8], papx: &[u8], chpx: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; 10];
    v[0] = 0xFF;
    v[1] = 0x0F; // sti 0xFFF
    let w1: u16 = 3 | (0x0FFF << 4); // stk 3, istdBase none
    v[2..4].copy_from_slice(&w1.to_le_bytes());
    let w2: u16 = 3 | (0x0FFF << 4); // cupx 3, istdNext none
    v[4..6].copy_from_slice(&w2.to_le_bytes());
    let units: Vec<u16> = name.encode_utf16().collect();
    v.extend_from_slice(&(units.len() as u16).to_le_bytes());
    for u in &units {
        v.extend_from_slice(&u.to_le_bytes());
    }
    v.extend_from_slice(&0u16.to_le_bytes());
    let papx_upx = [0u16.to_le_bytes().as_slice(), papx].concat();
    for upx in [tapx, papx_upx.as_slice(), chpx] {
        v.extend_from_slice(&(upx.len() as u16).to_le_bytes());
        v.extend_from_slice(upx);
        if upx.len() % 2 == 1 {
            v.push(0);
        }
    }
    let cb = v.len() as u16;
    v[6..8].copy_from_slice(&cb.to_le_bytes()); // bchUpe = cbStd
    let mut out = cb.to_le_bytes().to_vec();
    out.extend_from_slice(&v);
    out
}

/// `sprmTTableBorders80` with six 0.5 pt single Brc80s.
fn borders80() -> Vec<u8> {
    let mut op = vec![0x18];
    for _ in 0..6 {
        op.extend([4, 1, 0, 0]);
    }
    grpprl(&[(T_BORDERS_80, &op)])
}

/// Slot 0 Normal, slot 1 table style "Grid A" with `tapx`, centred, bold; a 2 x 2 table ("a", "b" /
/// "c", "d") whose row-end TAPs carry `row_tap`, then a tail paragraph.
fn doc(tapx: &[u8], row_tap: &[u8]) -> Document {
    let styles = vec![
        std_bytes(1, 0x0FFF, "Normal", &[], &[]),
        table_std("Grid A", tapx, &grpprl(&[(P_JC80, &[1])]), &grpprl(&[(C_BOLD, &[1])])),
    ];
    let text = "a\u{7}b\u{7}c\u{7}d\u{7}tail\r";
    let in_tbl = grpprl(&[(0x2416, &[0x01])]);
    let end = [grpprl(&[(0x2416, &[0x01]), (0x2417, &[0x01])]), t_def_table(&[0, 2880, 5760]), row_tap.to_vec()].concat();
    let spec = StructSpec {
        text,
        papx_entries: vec![(0, 0, in_tbl.clone()), (2, 0, end.clone()), (4, 0, in_tbl), (6, 0, end), (8, 0, Vec::new())],
        ..structured_spec(text, text.chars().count() as u32)
    };
    read(&struct_doc_styled(&spec, &styles)).expect("opens")
}

fn grid_a(d: &Document) -> &Style {
    d.styles.styles.iter().find(|s| s.name == "Grid A").expect("the table style is in the style sheet")
}

fn table(d: &Document) -> &wordcraft_doc::table::Table {
    d.body.iter().find_map(|b| b.as_table()).expect("table")
}

fn hide_marks(d: &Document) -> Vec<Vec<bool>> {
    table(d).rows.iter().map(|r| r.cells.iter().map(|c| c.props.hide_mark).collect()).collect()
}

#[test]
fn a_table_style_keeps_its_borders() {
    let d = doc(&borders80(), &[]);
    let s = grid_a(&d);
    assert_eq!(s.kind, StyleKind::Table);
    assert_eq!(s.table.as_ref().and_then(|t| t.borders), Some(Borders::all(S)));
}

#[test]
fn a_table_style_keeps_its_text_formatting() {
    let d = doc(&borders80(), &[]);
    let s = grid_a(&d);
    assert_eq!(s.chr.bold, Some(true));
    assert_eq!(s.para.align, Some(Align::Center));
}

#[test]
fn a_table_style_keeps_its_cell_margins() {
    // CSSAOperand: cb 6, itcFirst 0, itcLim 4, all four sides, ftsDxa, 72 twips = 3.6 pt.
    let tapx = grpprl(&[(T_PADDING_DEFAULT, &[6, 0, 4, 0x0F, 3, 72, 0])]);
    let d = doc(&tapx, &[]);
    let m = grid_a(&d).table.as_ref().and_then(|t| t.cell_margins).expect("cell margins");
    for (got, want) in m.iter().zip([3.6f32; 4]) {
        assert!((got - want).abs() < 0.01, "{m:?}");
    }
}

#[test]
fn an_empty_table_upx_gives_a_style_without_borders() {
    let d = doc(&[], &[]);
    let s = grid_a(&d);
    assert_eq!(s.kind, StyleKind::Table);
    assert_eq!(s.table.as_ref().and_then(|t| t.borders), None);
}

#[test]
fn a_row_names_its_table_style() {
    let d = doc(&borders80(), &grpprl(&[(T_ISTD, &1u16.to_le_bytes())]));
    let id = grid_a(&d).id.clone();
    assert_eq!(table(&d).props.style.as_deref(), Some(id.as_str()));
}

#[test]
fn a_row_naming_a_paragraph_style_gets_no_table_style() {
    let d = doc(&borders80(), &grpprl(&[(T_ISTD, &0u16.to_le_bytes())]));
    assert_eq!(table(&d).props.style, None);
}

#[test]
fn a_row_naming_a_style_past_the_end_gets_no_table_style() {
    let d = doc(&borders80(), &grpprl(&[(T_ISTD, &9u16.to_le_bytes())]));
    assert_eq!(table(&d).props.style, None);
}

#[test]
fn hide_mark_on_every_cell() {
    let d = doc(&[], &grpprl(&[(T_HIDE_MARK, &[3, 0, 2, 1])]));
    assert_eq!(hide_marks(&d), [[true, true], [true, true]]);
}

#[test]
fn hide_mark_on_the_second_cell_only() {
    let d = doc(&[], &grpprl(&[(T_HIDE_MARK, &[3, 1, 2, 1])]));
    assert_eq!(hide_marks(&d), [[false, true], [false, true]]);
}

#[test]
fn without_hide_mark_no_cell_hides_its_mark() {
    let d = doc(&[], &[]);
    assert_eq!(hide_marks(&d), [[false, false], [false, false]]);
}
