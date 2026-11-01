//! Paragraph frames ([MS-DOC] §2.6.2 sprmPPc, sprmPDxaAbs, sprmPDyaAbs, sprmPDxaWidth,
//! sprmPWHeightAbs, sprmPDxaFromText, sprmPDyaFromText): read into `ParaProps.frame`, from styles
//! and from direct paragraph formatting, merged one field at a time along the chain. Values are
//! shaped like the framed cover-page styles of real standards templates (anchored to the page
//! vertically and the margin horizontally, 56.75 pt down, 510.3 pt wide, exactly 39.7 pt tall).

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::TextFrame;

const P_PC: u16 = 0x261B;
const P_DXA_ABS: u16 = 0x8418;
const P_DYA_ABS: u16 = 0x8419;
const P_DXA_WIDTH: u16 = 0x841A;
const P_WHEIGHT_ABS: u16 = 0x442B;
const P_DYA_FROM_TEXT: u16 = 0x842E;
const P_DXA_FROM_TEXT: u16 = 0x842F;

/// pcVert = 1 (page) in bits 4-5, pcHorz = 1 (margin) in bits 6-7.
const PC_PAGE_V_MARGIN_H: u8 = 0x50;

fn framed_style_papx() -> Vec<u8> {
    grpprl(&[
        (P_PC, &[PC_PAGE_V_MARGIN_H]),
        (P_DYA_ABS, &1135i16.to_le_bytes()),
        (P_DXA_WIDTH, &10206i16.to_le_bytes()),
        (P_WHEIGHT_ABS, &794u16.to_le_bytes()),
    ])
}

/// Normal (istd 0) and Framed (istd 1, based on Normal); one paragraph per entry.
fn doc(text: &'static str, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Framed", framed_style_papx(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: paras,
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn style_frame(doc: &Document, name: &str) -> Option<TextFrame> {
    doc.styles.styles.iter().find(|s| s.name == name).expect("style").para.frame.clone()
}

fn para_frame(doc: &Document, i: usize) -> Option<TextFrame> {
    doc.body.iter().filter_map(|b| b.as_para()).nth(i).expect("paragraph").props.frame.clone()
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn style_frame_is_read() {
    let d = doc("Title\r", vec![(0, 1, Vec::new())]);
    let f = style_frame(&d, "Framed").expect("Framed style has a frame");
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("margin", "page"));
    assert!(close(f.y, 56.75), "y {}", f.y);
    assert!(close(f.x, 0.0) && f.x_align.is_none(), "x {} {:?}", f.x, f.x_align);
    assert!(f.w.is_some_and(|w| close(w, 510.3)), "w {:?}", f.w);
    assert!(f.h.is_some_and(|h| close(h, 39.7)) && f.h_exact, "h {:?} exact {}", f.h, f.h_exact);
}

#[test]
fn no_frame_sprms_means_no_frame() {
    let d = doc("Plain\rTitle\r", vec![(0, 0, Vec::new()), (6, 1, Vec::new())]);
    assert_eq!(style_frame(&d, "Normal"), None);
    assert_eq!(para_frame(&d, 0), None);
    // Inherits from its style; no direct frame sprm, so no direct frame.
    assert_eq!(para_frame(&d, 1), None);
}

#[test]
fn alignment_codes_in_direct_formatting() {
    let papx = grpprl(&[(P_PC, &[0x60]), (P_DXA_ABS, &(-4i16).to_le_bytes()), (P_DYA_ABS, &(-8i16).to_le_bytes())]);
    let d = doc("Centred\r", vec![(0, 0, papx)]);
    let f = para_frame(&d, 0).expect("direct frame");
    // 0x60: pcVert = 2 (paragraph -> "text"), pcHorz = 1 (margin).
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("margin", "text"));
    assert_eq!((f.x_align.as_deref(), f.y_align.as_deref()), (Some("center"), Some("center")));
    assert!(close(f.x, 0.0) && close(f.y, 0.0));
    assert_eq!((f.w, f.h), (None, None));
}

#[test]
fn right_inside_outside_and_bottom_codes() {
    let cases: [(i16, i16, &str, &str); 3] = [(-8, -12, "right", "bottom"), (-12, -16, "inside", "inside"), (-16, -20, "outside", "outside")];
    for (xas, yas, xa, ya) in cases {
        let papx = grpprl(&[(P_DXA_ABS, &xas.to_le_bytes()), (P_DYA_ABS, &yas.to_le_bytes())]);
        let d = doc("Box\r", vec![(0, 0, papx)]);
        let f = para_frame(&d, 0).expect("direct frame");
        assert_eq!((f.x_align.as_deref(), f.y_align.as_deref()), (Some(xa), Some(ya)), "XAS {xas} YAS {yas}");
    }
}

#[test]
fn direct_sprm_merges_with_style_frame() {
    // Only the horizontal position is direct; anchors, y, width and height come from the style.
    let papx = grpprl(&[(P_DXA_ABS, &1440i16.to_le_bytes())]);
    let d = doc("Title\r", vec![(0, 1, papx)]);
    let f = para_frame(&d, 0).expect("merged direct frame");
    assert!(close(f.x, 72.0) && f.x_align.is_none(), "x {} {:?}", f.x, f.x_align);
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("margin", "page"));
    assert!(close(f.y, 56.75), "y {}", f.y);
    assert!(f.w.is_some_and(|w| close(w, 510.3)), "w {:?}", f.w);
    assert!(f.h.is_some_and(|h| close(h, 39.7)) && f.h_exact);
}

#[test]
fn minimum_height_and_distance_from_text() {
    let papx = grpprl(&[
        (P_DYA_ABS, &2880i16.to_le_bytes()),
        (P_WHEIGHT_ABS, &(0x8000u16 | 400).to_le_bytes()),
        (P_DXA_FROM_TEXT, &180i16.to_le_bytes()),
        (P_DYA_FROM_TEXT, &0i16.to_le_bytes()),
    ]);
    let d = doc("Box\r", vec![(0, 0, papx)]);
    let f = para_frame(&d, 0).expect("direct frame");
    // Defaults where no sprm speaks: column horizontally, margin vertically.
    assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("text", "margin"));
    assert!(close(f.y, 144.0), "y {}", f.y);
    assert!(f.h.is_some_and(|h| close(h, 20.0)) && !f.h_exact, "h {:?} exact {}", f.h, f.h_exact);
    assert!(close(f.h_space, 9.0) && close(f.v_space, 0.0), "spaces {} {}", f.h_space, f.v_space);
}

#[test]
fn short_operand_does_not_panic() {
    // A two-byte operand cut to one byte at the end of the grpprl.
    let mut papx = grpprl(&[(P_PC, &[PC_PAGE_V_MARGIN_H])]);
    papx.extend_from_slice(&P_DXA_ABS.to_le_bytes());
    papx.push(0x05);
    let spec = RichSpec {
        text: "Cut\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(0, 0, papx)],
    };
    let d = read(&rich_doc(&spec)).expect("opens");
    assert_eq!(d.body.iter().filter_map(|b| b.as_para()).next().map(|p| p.text.as_str()), Some("Cut"));
}

#[test]
fn three_header_items_left_centre_right() {
    // A header written as three framed paragraphs anchored to their paragraph (pcVert 2) and the
    // margin (pcHorz 1), 1 twip down, at the left, centre and right: Word shows them on one line.
    let item = |xas: i16| grpprl(&[(P_DXA_ABS, &xas.to_le_bytes()), (P_DYA_ABS, &1i16.to_le_bytes()), (P_PC, &[0x60])]);
    let d = doc("Left\rMiddle\rRight\r", vec![(0, 0, item(0)), (5, 0, item(-4)), (12, 0, item(-8))]);
    let frames: Vec<TextFrame> = (0..3).map(|i| para_frame(&d, i).expect("frame")).collect();
    let aligns: Vec<Option<&str>> = frames.iter().map(|f| f.x_align.as_deref()).collect();
    assert_eq!(aligns, [None, Some("center"), Some("right")]);
    for f in &frames {
        assert_eq!((f.h_anchor.as_str(), f.v_anchor.as_str()), ("margin", "text"));
        assert!(close(f.x, 0.0) && close(f.y, 0.05) && f.y_align.is_none(), "{f:?}");
    }
}
