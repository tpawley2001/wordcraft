//! Section page borders ([MS-DOC] §2.6.4): sprmSBrcTop80..sprmSBrcRight80 (0x702B-0x702E, a Brc80
//! each) and sprmSBrcTop..sprmSBrcRight (0xD234-0xD237, a BrcOperand each), read into
//! `SectionProps::page_borders` as DOCX `w:pgBorders` is (ECMA-376 Part 1 §17.6.10).

use wordcraft_doc::Rgb;
use wordcraft_doc::props::{Border, BorderStyle, Borders};

use super::{grpprl, one_section, read, struct_doc};

const S_BRC_TOP80: u16 = 0x702B;
const S_BRC_LEFT80: u16 = 0x702C;
const S_BRC_BOTTOM80: u16 = 0x702D;
const S_BRC_RIGHT80: u16 = 0x702E;
const S_BRC_TOP: u16 = 0xD234;

/// The page borders of a one-section document whose Sepx holds `prls`.
fn borders(prls: &[(u16, Vec<u8>)]) -> Option<Borders> {
    let prls: Vec<(u16, &[u8])> = prls.iter().map(|(op, v)| (*op, v.as_slice())).collect();
    let spec = one_section("Text\r", grpprl(&prls), vec![(0, 0, Vec::new())]);
    read(&struct_doc(&spec)).expect("opens").last_section.page_borders
}

fn border(style: BorderStyle, width: f32, color: Option<Rgb>, space: f32) -> Border {
    Border { style, width, color, space }
}

#[test]
fn no_border_sprms_leave_page_borders_off() {
    assert_eq!(borders(&[]), None);
}

#[test]
fn brc80_sides_are_read_with_palette_colour_and_space() {
    // dptLineWidth 12 (1.5 pt), brcType 1 single, ico 6 red, dptSpace 24.
    let op = vec![0x0C, 0x01, 0x06, 0x18];
    let b =
        borders(&[(S_BRC_TOP80, op.clone()), (S_BRC_LEFT80, op.clone()), (S_BRC_BOTTOM80, op.clone()), (S_BRC_RIGHT80, op)]).expect("page borders");
    let want = Some(border(BorderStyle::Single, 1.5, Some(Rgb(255, 0, 0)), 24.0));
    assert_eq!((b.top, b.left, b.bottom, b.right), (want, want, want, want));
    assert_eq!((b.between, b.inside_v), (None, None));
}

#[test]
fn brc_operand_side_is_read_with_its_full_colour() {
    // cb 8; cv 00 80 00 00 (green); dptLineWidth 16 (2 pt); brcType 3 double; dptSpace 4.
    let b = borders(&[(S_BRC_TOP, vec![8, 0x00, 0x80, 0x00, 0x00, 0x10, 0x03, 0x04, 0x00])]).expect("page borders");
    assert_eq!(b.top, Some(border(BorderStyle::Double, 2.0, Some(Rgb(0, 128, 0)), 4.0)));
    assert_eq!((b.left, b.bottom, b.right), (None, None, None));
}

#[test]
fn the_last_sprm_for_a_side_wins() {
    let b = borders(&[(S_BRC_TOP80, vec![0x08, 0x01, 0x01, 0x00]), (S_BRC_TOP, vec![8, 0, 0, 255, 0, 0x18, 0x01, 0x00, 0])]).expect("page borders");
    assert_eq!(b.top, Some(border(BorderStyle::Single, 3.0, Some(Rgb(0, 0, 255)), 0.0)));
}

#[test]
fn a_nil_border_alone_is_no_page_border() {
    assert_eq!(borders(&[(S_BRC_TOP80, vec![0xFF; 4])]), None);
}

#[test]
fn truncated_or_malformed_operands_are_ignored() {
    assert_eq!(borders(&[(S_BRC_TOP80, vec![0x0C, 0x01]), (S_BRC_TOP, vec![7, 0, 0, 0, 0, 0x10, 0x03, 0x04, 0])]), None);
}
