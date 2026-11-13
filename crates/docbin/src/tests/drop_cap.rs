//! Drop caps ([MS-DOC] §2.6.2 sprmPDcs, §2.9.61 DCS): Word 97-2003 stores the dropped letter as its
//! own short framed paragraph carrying a DCS (bits 0-2 fdct: 1 dropped, 2 in margin; bits 3-7 lines).
//! It opens as a drop cap on the paragraph it starts (one paragraph, no text frame), as the .docx
//! reader does for `w:framePr w:dropCap`, so the first lines wrap beside the letter as in Word.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;

const P_PC: u16 = 0x261B;
const P_WR: u16 = 0x2423;
const P_DCS: u16 = 0x442C;
const P_DXA_FROM_TEXT: u16 = 0x842F;

/// pcVert = 2 (text) in bits 4-5, pcHorz = 0 (column) in bits 6-7: how Word anchors a drop cap.
const PC_TEXT_V_COLUMN_H: u8 = 0x20;

fn dcs(fdct: u16, lines: u16) -> [u8; 2] {
    (fdct | (lines << 3)).to_le_bytes()
}

fn cap_papx(fdct: u16, lines: u16) -> Vec<u8> {
    grpprl(&[(P_PC, &[PC_TEXT_V_COLUMN_H]), (P_WR, &[2]), (P_DCS, &dcs(fdct, lines))])
}

/// Normal (istd 0) and "Cap" (istd 1, based on Normal, carrying `cap_style` as its PAPX).
fn doc_with_style(text: &'static str, cap_style: Vec<u8>, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Cap", cap_style, Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: paras,
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn doc(text: &'static str, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    doc_with_style(text, Vec::new(), paras)
}

fn paras(d: &Document) -> Vec<&wordcraft_doc::Paragraph> {
    d.body.iter().filter_map(|b| b.as_para()).collect()
}

#[test]
fn dropped_cap_merges_into_its_paragraph() {
    let d = doc("T\rhe United States\r", vec![(0, 0, cap_papx(1, 2)), (2, 0, Vec::new())]);
    let ps = paras(&d);
    assert_eq!(ps.len(), 1, "the cap paragraph merges into the paragraph it starts");
    assert_eq!(ps[0].text, "The United States");
    assert_eq!(ps[0].props.drop_cap, Some(2));
    assert_eq!(ps[0].props.drop_cap_margin, None);
    assert!(ps[0].props.frame.is_none(), "a drop cap is not a text frame");
}

#[test]
fn in_margin_cap_sets_margin() {
    let d = doc("T\rhe United States\r", vec![(0, 0, cap_papx(2, 3)), (2, 0, Vec::new())]);
    let ps = paras(&d);
    assert_eq!(ps.len(), 1);
    assert_eq!(ps[0].props.drop_cap, Some(3));
    assert_eq!(ps[0].props.drop_cap_margin, Some(true));
}

#[test]
fn distance_from_text_is_the_cap_distance() {
    let mut papx = cap_papx(1, 2);
    papx.extend(grpprl(&[(P_DXA_FROM_TEXT, &144i16.to_le_bytes())]));
    let d = doc("T\rhe United States\r", vec![(0, 0, papx), (2, 0, Vec::new())]);
    let ps = paras(&d);
    assert_eq!(ps.len(), 1);
    let dist = ps[0].props.drop_cap_distance.expect("distance");
    assert!((dist - 7.2).abs() < 0.01, "distance {dist}");
}

#[test]
fn fdct_none_keeps_an_ordinary_frame() {
    let d = doc("Boxed\rBody text\r", vec![(0, 0, cap_papx(0, 2)), (6, 0, Vec::new())]);
    let ps = paras(&d);
    assert_eq!(ps.len(), 2, "no drop cap: nothing merges");
    assert_eq!(ps[0].props.drop_cap, None);
    assert!(ps[0].props.frame.is_some(), "the frame sprms still make a text frame");
}

#[test]
fn lines_are_clamped_to_ten() {
    let d = doc("T\rhe United States\r", vec![(0, 0, cap_papx(1, 31)), (2, 0, Vec::new())]);
    assert_eq!(paras(&d)[0].props.drop_cap, Some(10));
}

#[test]
fn cap_as_the_last_paragraph_stays() {
    let d = doc("Body\rT\r", vec![(0, 0, Vec::new()), (5, 0, cap_papx(1, 2))]);
    let ps = paras(&d);
    let last = ps.last().expect("paragraph");
    assert_eq!(last.text, "T");
    assert_eq!(last.props.drop_cap, Some(2));
}

#[test]
fn drop_cap_from_the_paragraph_style() {
    let d = doc_with_style("T\rhe United States\r", cap_papx(1, 2), vec![(0, 1, Vec::new()), (2, 0, Vec::new())]);
    let ps = paras(&d);
    assert_eq!(ps.len(), 1, "a style's drop cap merges too");
    assert_eq!(ps[0].text, "The United States");
    assert_eq!(ps[0].props.drop_cap, Some(2));
    assert!(ps[0].props.frame.is_none());
}
