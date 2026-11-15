//! Character settings stored as CHP sprms ([MS-DOC] §2.6.1) that the model, the DOCX writer and
//! layout already know: outline, shadow, engrave (sprmCFImprint), emboss, double strikethrough,
//! Do not check spelling (sprmCFNoProof), web-hidden and special-hidden text, emphasis marks
//! (sprmCKcd), and text shading and borders (sprmCShd80/sprmCShd, sprmCBrc80/sprmCBrc).

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::{BorderStyle, CharProps, EmphasisMark, Rgb};

const C_F_OUTLINE: u16 = 0x0838;
const C_F_SHADOW: u16 = 0x0839;
const C_F_IMPRINT: u16 = 0x0854;
const C_F_EMBOSS: u16 = 0x0858;
const C_F_D_STRIKE: u16 = 0x2A53;
const C_F_NO_PROOF: u16 = 0x0875;
const C_F_WEB_HIDDEN: u16 = 0x0811;
const C_F_SPEC_VANISH: u16 = 0x0818;
const C_KCD: u16 = 0x2A34;
const C_SHD80: u16 = 0x4866;
const C_SHD: u16 = 0xCA71;
const C_BRC80: u16 = 0x6865;
const C_BRC: u16 = 0xCA72;

/// "Hello" in one run with `run` as its CHPX, in a paragraph of style Body (istd 1) whose CHPX is `style_chpx`.
fn doc(run: Vec<u8>, style_chpx: Vec<u8>) -> Document {
    let spec = RichSpec {
        text: "Hello\r",
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Body", Vec::new(), style_chpx)],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, run)],
        papx_entries: vec![(5, 1, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn run_props(d: &Document) -> CharProps {
    d.body.iter().find_map(|b| b.as_para()).expect("paragraph").props_of_char(0).clone()
}

fn with(run: Vec<u8>) -> CharProps {
    run_props(&doc(run, Vec::new()))
}

fn one(op: u16, operand: &[u8]) -> CharProps {
    with(grpprl(&[(op, operand)]))
}

/// A variable-length sprm (spra 6): opcode, cb, operand.
fn sprm_cb(op: u16, body: &[u8]) -> Vec<u8> {
    let mut v = op.to_le_bytes().to_vec();
    v.push(body.len() as u8);
    v.extend_from_slice(body);
    v
}

type Get = fn(&CharProps) -> Option<bool>;

const FLAGS: [(u16, Get); 8] = [
    (C_F_OUTLINE, |c| c.outline),
    (C_F_SHADOW, |c| c.shadow),
    (C_F_IMPRINT, |c| c.engrave),
    (C_F_EMBOSS, |c| c.emboss),
    (C_F_D_STRIKE, |c| c.double_strike),
    (C_F_NO_PROOF, |c| c.no_proof),
    (C_F_WEB_HIDDEN, |c| c.web_hidden),
    (C_F_SPEC_VANISH, |c| c.spec_vanish),
];

#[test]
fn each_flag_reads_on_and_explicit_off() {
    for (op, get) in FLAGS {
        assert_eq!(get(&one(op, &[1])), Some(true), "sprm {op:#06X} = 1");
        assert_ne!(get(&one(op, &[0])), Some(true), "sprm {op:#06X} = 0 is not on");
        assert_ne!(get(&with(Vec::new())), Some(true), "no sprm {op:#06X}: not on");
    }
}

#[test]
fn a_character_style_flag_reaches_the_run_and_0x81_flips_it_off() {
    // ToggleOperand 0x81 = the opposite of the style's value ([MS-DOC] §2.9.326), as for bold.
    for (op, get) in [(C_F_OUTLINE, FLAGS[0].1), (C_F_EMBOSS, FLAGS[3].1), (C_F_NO_PROOF, FLAGS[5].1)] {
        let style = grpprl(&[(op, &[1])]);
        let d = doc(Vec::new(), style.clone());
        let st = d.styles.styles.iter().find(|s| s.name == "Body").expect("style").chr.clone();
        assert_eq!(get(&st), Some(true), "style Body carries sprm {op:#06X}");
        let flipped = doc(grpprl(&[(op, &[0x81])]), style);
        assert_ne!(get(&run_props(&flipped)), Some(true), "0x81 over a style that turns {op:#06X} on: off");
    }
}

#[test]
fn emphasis_marks_read_each_value() {
    let table = [
        (0u8, Some(EmphasisMark::None)),
        (1, Some(EmphasisMark::Dot)),
        (2, Some(EmphasisMark::Comma)),
        (3, Some(EmphasisMark::Circle)),
        (4, Some(EmphasisMark::UnderDot)),
        (9, None), // not a value the spec defines: ignored
    ];
    for (v, want) in table {
        assert_eq!(one(C_KCD, &[v]).emphasis, want, "kcd {v}");
    }
}

/// A 2-byte `Shd80`: icoFore (bits 0-4), icoBack (5-9), ipat (10-15).
fn shd80(fore: u16, back: u16, ipat: u16) -> [u8; 2] {
    (fore | (back << 5) | (ipat << 10)).to_le_bytes()
}

#[test]
fn text_shading_reads_shd80_and_the_exact_shd_after_it() {
    assert_eq!(one(C_SHD80, &shd80(0, 7, 0)).shading, Some(Rgb(255, 255, 0)), "clear: background ico 7 yellow");
    assert_eq!(one(C_SHD80, &shd80(9, 0, 1)).shading, Some(Rgb(0, 0, 128)), "solid: foreground ico 9 dark blue");
    assert_eq!(one(C_SHD80, &shd80(0, 0, 0)).shading, None, "automatic");
    // Word 2000+: the palette fallback first, then the exact colour (cvFore auto, cvBack 1C3A70, clear).
    let mut run = grpprl(&[(C_SHD80, &shd80(0, 9, 0))]);
    run.extend(sprm_cb(C_SHD, &[0, 0, 0, 0xFF, 0x1C, 0x3A, 0x70, 0, 0, 0]));
    assert_eq!(with(run).shading, Some(Rgb(0x1C, 0x3A, 0x70)));
    assert_eq!(with(sprm_cb(C_SHD, &[0, 0, 0, 0xFF, 0, 0, 0, 0xFF, 0xFF, 0xFF])).shading, None, "ipat 0xFFFF: automatic");
}

#[test]
fn text_border_reads_brc80_and_brc() {
    // Brc80: dptLineWidth 4 (0.5 pt), brcType 1 (single), ico 6 (red), dptSpace 1.
    let b = one(C_BRC80, &[4, 1, 6, 1]).border.expect("Brc80 border");
    assert_eq!(b.style, BorderStyle::Single);
    assert!((b.width - 0.5).abs() < 0.01, "width {}", b.width);
    assert_eq!(b.color, Some(Rgb(255, 0, 0)));
    assert!((b.space - 1.0).abs() < 0.01, "space {}", b.space);
    // Brc: cv 2E7D32, dptLineWidth 12 (1.5 pt), brcType 3 (double), dptSpace 0.
    let b = with(sprm_cb(C_BRC, &[0x2E, 0x7D, 0x32, 0, 12, 3, 0, 0])).border.expect("Brc border");
    assert_eq!(b.style, BorderStyle::Double);
    assert!((b.width - 1.5).abs() < 0.01, "width {}", b.width);
    assert_eq!(b.color, Some(Rgb(0x2E, 0x7D, 0x32)));
}

#[test]
fn truncated_operands_change_nothing_and_never_panic() {
    assert_eq!(one(C_KCD, &[]).emphasis, None);
    assert_eq!(one(C_BRC80, &[4, 1]).border, None);
    let mut run = C_SHD.to_le_bytes().to_vec();
    run.extend_from_slice(&[10, 1, 2]);
    assert_eq!(with(run).shading, None);
}
