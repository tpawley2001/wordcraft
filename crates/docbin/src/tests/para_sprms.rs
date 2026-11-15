//! Paragraph settings stored as PAP sprms ([MS-DOC] §2.6.2) that the model, the DOCX writer and
//! layout already know: Suppress line numbers (sprmPFNoLineNumb 0x240C), Don't hyphenate
//! (sprmPFNoAutoHyph 0x242A), the East Asian line-break flags (0x2433-0x2438), text alignment
//! (sprmPWAlignFont 0x4439), Mirror indents (sprmPFMirrorIndents 0x2470) and paragraph shading
//! (sprmPShd80 0x442D, sprmPShd 0xC64D). Each is read from direct formatting and from styles.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::props::{ParaProps, Rgb, TextAlignment};
use wordcraft_doc::Document;

const P_F_NO_LINE_NUMB: u16 = 0x240C;
const P_F_NO_AUTO_HYPH: u16 = 0x242A;
const P_F_KINSOKU: u16 = 0x2433;
const P_F_WORD_WRAP: u16 = 0x2434;
const P_F_OVERFLOW_PUNCT: u16 = 0x2435;
const P_F_TOP_LINE_PUNCT: u16 = 0x2436;
const P_F_AUTO_SPACE_DE: u16 = 0x2437;
const P_F_AUTO_SPACE_DN: u16 = 0x2438;
const P_W_ALIGN_FONT: u16 = 0x4439;
const P_F_MIRROR_INDENTS: u16 = 0x2470;
const P_SHD80: u16 = 0x442D;
const P_SHD: u16 = 0xC64D;

/// Normal (istd 0) and "Quiet" (istd 1, on Normal, with `style_papx`); one paragraph per entry
/// of `paras` (start CP, istd, grpprl).
fn doc(text: &'static str, style_papx: Vec<u8>, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Quiet", style_papx, Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: paras,
    };
    read(&rich_doc(&spec)).expect("opens")
}

/// One Normal paragraph "One" with `papx` as its direct formatting.
fn one(papx: Vec<u8>) -> ParaProps {
    para(&doc("One\r", Vec::new(), vec![(0, 0, papx)]), 0)
}

fn para(doc: &Document, i: usize) -> ParaProps {
    doc.body.iter().filter_map(|b| b.as_para()).nth(i).expect("paragraph").props.clone()
}

fn style_para(doc: &Document, name: &str) -> ParaProps {
    doc.styles.styles.iter().find(|s| s.name == name).expect("style").para.clone()
}

/// A variable-length sprm (spra 6): opcode, cb, operand.
fn sprm_cb(op: u16, body: &[u8]) -> Vec<u8> {
    let mut v = op.to_le_bytes().to_vec();
    v.push(body.len() as u8);
    v.extend_from_slice(body);
    v
}

/// A 10-byte `Shd` ([MS-DOC] §2.9.248): cvFore, cvBack (COLORREF r, g, b, fAuto), ipat.
fn shd(fore: Option<[u8; 3]>, back: Option<[u8; 3]>, ipat: u16) -> Vec<u8> {
    let cv = |c: Option<[u8; 3]>| match c {
        Some([r, g, b]) => [r, g, b, 0],
        None => [0, 0, 0, 0xFF],
    };
    let mut v = cv(fore).to_vec();
    v.extend_from_slice(&cv(back));
    v.extend_from_slice(&ipat.to_le_bytes());
    v
}

/// A 2-byte `Shd80` ([MS-DOC] §2.9.249): icoFore (bits 0-4), icoBack (5-9), ipat (10-15).
fn shd80(fore: u16, back: u16, ipat: u16) -> [u8; 2] {
    (fore | (back << 5) | (ipat << 10)).to_le_bytes()
}

#[test]
fn bool8_flags_read_on_from_direct_formatting() {
    let cases: [(u16, fn(&ParaProps) -> Option<bool>); 9] = [
        (P_F_NO_LINE_NUMB, |p| p.suppress_line_numbers),
        (P_F_NO_AUTO_HYPH, |p| p.suppress_hyphens),
        (P_F_KINSOKU, |p| p.kinsoku),
        (P_F_WORD_WRAP, |p| p.word_wrap),
        (P_F_OVERFLOW_PUNCT, |p| p.overflow_punct),
        (P_F_TOP_LINE_PUNCT, |p| p.top_line_punct),
        (P_F_AUTO_SPACE_DE, |p| p.auto_space_de),
        (P_F_AUTO_SPACE_DN, |p| p.auto_space_dn),
        (P_F_MIRROR_INDENTS, |p| p.mirror_indents),
    ];
    for (op, get) in cases {
        assert_eq!(get(&one(grpprl(&[(op, &[1])]))), Some(true), "sprm {op:#06X} = 1");
        assert_eq!(get(&one(grpprl(&[(op, &[0])]))), Some(false), "sprm {op:#06X} = 0 is an explicit off");
        assert_eq!(get(&one(Vec::new())), None, "no sprm {op:#06X}: nothing set");
    }
}

#[test]
fn any_non_zero_byte_turns_a_flag_on() {
    assert_eq!(one(grpprl(&[(P_F_NO_AUTO_HYPH, &[0x81])])).suppress_hyphens, Some(true));
}

#[test]
fn a_style_carries_the_flags_and_a_paragraph_can_switch_one_off() {
    let style = grpprl(&[(P_F_NO_AUTO_HYPH, &[1]), (P_F_NO_LINE_NUMB, &[1])]);
    // "One" uses the style as it is; "Two" switches Don't hyphenate off again.
    let d = doc("One\rTwo\r", style, vec![(0, 1, Vec::new()), (4, 1, grpprl(&[(P_F_NO_AUTO_HYPH, &[0])]))]);
    let s = style_para(&d, "Quiet");
    assert_eq!((s.suppress_hyphens, s.suppress_line_numbers), (Some(true), Some(true)));
    let r0 = d.styles.resolve_para(&para(&d, 0));
    assert!(r0.suppress_hyphens && r0.suppress_line_numbers, "first paragraph inherits both");
    let r1 = d.styles.resolve_para(&para(&d, 1));
    assert!(!r1.suppress_hyphens && r1.suppress_line_numbers, "second paragraph: hyphenation back on, line numbers still off");
}

#[test]
fn font_alignment_reads_each_value() {
    let table = [
        (0u16, Some(TextAlignment::Top)),
        (1, Some(TextAlignment::Center)),
        (2, Some(TextAlignment::Baseline)),
        (3, Some(TextAlignment::Bottom)),
        (4, Some(TextAlignment::Auto)),
        (9, None), // not a value the spec defines: ignored
    ];
    for (v, want) in table {
        assert_eq!(one(grpprl(&[(P_W_ALIGN_FONT, &v.to_le_bytes())])).text_alignment, want, "wAlignFont {v}");
    }
}

#[test]
fn shd80_clear_pattern_fills_with_the_background_colour() {
    // icoBack 7 = yellow, clear (ipat 0): how Word 97 writes a plain fill.
    assert_eq!(one(grpprl(&[(P_SHD80, &shd80(0, 7, 0))])).shading, Some(Rgb(255, 255, 0)));
}

#[test]
fn shd80_solid_pattern_fills_with_the_foreground_colour() {
    // icoFore 9 = dark blue, solid (ipat 1).
    assert_eq!(one(grpprl(&[(P_SHD80, &shd80(9, 0, 1))])).shading, Some(Rgb(0, 0, 128)));
}

#[test]
fn shd80_automatic_is_no_fill() {
    assert_eq!(one(grpprl(&[(P_SHD80, &shd80(0, 0, 0))])).shading, None);
    assert_eq!(one(grpprl(&[(P_SHD80, &shd80(0x1F, 0x1F, 0))])).shading, None);
}

#[test]
fn shd_exact_colour_wins_over_the_shd80_before_it() {
    // Word 2000+ writes the palette approximation first, then the exact colour.
    let mut papx = grpprl(&[(P_SHD80, &shd80(0, 9, 0))]);
    papx.extend(sprm_cb(P_SHD, &shd(None, Some([0x1C, 0x3A, 0x70]), 0)));
    assert_eq!(one(papx).shading, Some(Rgb(0x1C, 0x3A, 0x70)));
}

#[test]
fn shd_alone_reads_clear_and_solid() {
    assert_eq!(one(sprm_cb(P_SHD, &shd(None, Some([0xD9, 0xE2, 0xF3]), 0))).shading, Some(Rgb(0xD9, 0xE2, 0xF3)));
    assert_eq!(one(sprm_cb(P_SHD, &shd(Some([0x40, 0x80, 0x20]), None, 1))).shading, Some(Rgb(0x40, 0x80, 0x20)));
    assert_eq!(one(sprm_cb(P_SHD, &shd(None, None, 0xFFFF))).shading, None, "ipat 0xFFFF = automatic");
}

#[test]
fn a_shaded_style_shades_its_paragraphs() {
    let d = doc("One\r", grpprl(&[(P_SHD80, &shd80(0, 16, 0))]), vec![(0, 1, Vec::new())]);
    assert_eq!(style_para(&d, "Quiet").shading, Some(Rgb(192, 192, 192)));
    assert_eq!(d.styles.resolve_para(&para(&d, 0)).shading, Some(Rgb(192, 192, 192)));
}

#[test]
fn truncated_operands_change_nothing_and_never_panic() {
    // A variable-length sprm whose cb runs past the grpprl, and an empty Shd.
    let mut papx = P_SHD.to_le_bytes().to_vec();
    papx.push(10);
    papx.extend_from_slice(&[1, 2, 3]);
    let p = one(papx);
    assert_eq!(p.shading, None);
    assert_eq!(one(sprm_cb(P_SHD, &[])).shading, None);
}
