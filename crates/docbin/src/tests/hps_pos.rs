//! `sprmCHpsPos` (0x4845, [MS-DOC] §2.6.1) raises (+) or lowers (−) text by signed half-points at full
//! size: OOXML `w:position` (ECMA-376 Part 1 §17.3.2.24), `CharProps::position` in points. Superscript
//! and subscript are `sprmCIss` (0x2A48: 0 baseline, 1 superscript, 2 subscript), independent of it.
//! An inline picture's anchor character keeps its run's formatting, so a lowered equation picture
//! (Equation Editor 3.0 objects are nearly always lowered) sits lower on its line, as in Word.

use wordcraft_doc::props::VertAlign;

use super::drawing::{fopt, fsp, rec, sp};
use super::{RichSpec, StructSpec, grpprl, para0, read, rich_doc, struct_doc, structured_spec};

const HPS_POS: u16 = 0x4845; // sprmCHpsPos, signed half-points
const ISS: u16 = 0x2A48; // sprmCIss
const HPS: u16 = 0x4A43; // sprmCHps, half-points
const PIC_LOCATION: u16 = 0x6A03; // sprmCPicLocation
const F_SPEC: u16 = 0x0855; // sprmCFSpec
const MM_SHAPE: u16 = 0x0064;

// CPs: a=0 b=1 c=2 d=3 e=4 f=5 g=6 \r=7: one character per run.
const TEXT: &str = "abcdefg\r";

fn doc(chpx_runs: Vec<(u32, Vec<u8>)>) -> wordcraft_doc::Document {
    let spec = RichSpec {
        text: TEXT,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new())],
        fonts: vec!["Times New Roman"],
        chpx_runs,
        papx_entries: vec![(0, 0, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

/// The properties of the run covering byte `at` of the paragraph text.
fn run_at(p: &wordcraft_doc::Paragraph, at: usize) -> wordcraft_doc::CharProps {
    let mut start = 0;
    for r in &p.runs {
        if at >= start && at < start + r.len {
            return r.props.clone();
        }
        start += r.len;
    }
    panic!("no run covers byte {at}: {:?}", p.runs)
}

fn pos(p: &wordcraft_doc::CharProps) -> f32 {
    p.position.unwrap_or(0.0)
}

fn plain_vert(p: &wordcraft_doc::CharProps) -> bool {
    matches!(p.vert_align, None | Some(VertAlign::Baseline))
}

fn by_char() -> wordcraft_doc::Paragraph {
    para0(&doc(vec![
        (0, grpprl(&[(HPS_POS, &(-10i16).to_le_bytes())])),
        (1, grpprl(&[(HPS_POS, &6i16.to_le_bytes())])),
        (2, grpprl(&[(HPS_POS, &0i16.to_le_bytes())])),
        (3, grpprl(&[(ISS, &[1])])),
        (4, grpprl(&[(ISS, &[2])])),
        (5, grpprl(&[(ISS, &[0])])),
        (6, grpprl(&[(ISS, &[1]), (HPS_POS, &(-4i16).to_le_bytes())])),
    ]))
}

#[test]
fn hps_pos_lowers_text_by_half_points_without_subscript() {
    let p = by_char();
    let a = run_at(&p, 0);
    assert!((pos(&a) - -5.0).abs() < 1e-3, "-10 half-points = 5 pt lowered: {a:?}");
    assert!(plain_vert(&a), "a lowered run is not subscript: {a:?}");
}

#[test]
fn hps_pos_raises_text_by_half_points_without_superscript() {
    let p = by_char();
    let b = run_at(&p, 1);
    assert!((pos(&b) - 3.0).abs() < 1e-3, "+6 half-points = 3 pt raised: {b:?}");
    assert!(plain_vert(&b), "a raised run is not superscript: {b:?}");
    let c = run_at(&p, 2);
    assert!(pos(&c).abs() < 1e-3 && plain_vert(&c), "hpsPos 0 is plain: {c:?}");
}

#[test]
fn iss_sets_superscript_and_subscript() {
    let p = by_char();
    let d = run_at(&p, 3);
    assert_eq!(d.vert_align, Some(VertAlign::Superscript), "{d:?}");
    assert!(pos(&d).abs() < 1e-3, "superscript alone has no position: {d:?}");
    let e = run_at(&p, 4);
    assert_eq!(e.vert_align, Some(VertAlign::Subscript), "{e:?}");
    let f = run_at(&p, 5);
    assert!(plain_vert(&f), "iss 0 is baseline: {f:?}");
}

#[test]
fn iss_and_hps_pos_are_independent() {
    let g = run_at(&by_char(), 6);
    assert_eq!(g.vert_align, Some(VertAlign::Superscript), "{g:?}");
    assert!((pos(&g) - -2.0).abs() < 1e-3, "{g:?}");
}

/// A PICF ([MS-DOC] §2.9.190) with `mm`, goal size (twips) at 100 %, then `rest`.
fn picf(mm: u16, goal: (i16, i16), rest: &[u8]) -> Vec<u8> {
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + rest.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[6..8].copy_from_slice(&mm.to_le_bytes());
    picf[28..30].copy_from_slice(&goal.0.to_le_bytes());
    picf[30..32].copy_from_slice(&goal.1.to_le_bytes());
    picf[32..34].copy_from_slice(&1000u16.to_le_bytes());
    picf[34..36].copy_from_slice(&1000u16.to_le_bytes());
    [picf.as_slice(), rest].concat()
}

/// An inline picture-frame shape with no picture data: Word keeps it as an object of its goal size.
fn frame() -> Vec<u8> {
    sp(&[fsp(75, 1025, 0xA00), fopt(&[(0x01FF, 0x0008_0000)], &[]), rec(0, 0, 0xF010, &[0, 0, 0, 0x80])])
}

/// "a", an inline picture whose CHPX also carries `extra`, a line break, "After". The picture is at CP 1.
fn picture_doc(extra: &[(u16, &[u8])]) -> wordcraft_doc::Paragraph {
    let text = "a\u{1}\u{b}After\r";
    let ccp = text.chars().count() as u32;
    let mut prls: Vec<(u16, &[u8])> = vec![(PIC_LOCATION, &[0, 0, 0, 0]), (F_SPEC, &[1])];
    prls.extend_from_slice(extra);
    let spec = StructSpec {
        text,
        data_stream: picf(MM_SHAPE, (340, 340), &frame()),
        chpx_runs: vec![(0, Vec::new()), (1, grpprl(&prls)), (2, Vec::new())],
        ..structured_spec(text, ccp)
    };
    para0(&read(&struct_doc(&spec)).expect("opens"))
}

fn object_props(p: &wordcraft_doc::Paragraph) -> wordcraft_doc::CharProps {
    assert_eq!(p.objects.len(), 1, "one inline object: {:?}", p.objects);
    run_at(p, p.text.find('\u{FFFC}').expect("object char"))
}

#[test]
fn a_lowered_inline_picture_keeps_its_position() {
    let p = picture_doc(&[(HPS_POS, &(-14i16).to_le_bytes())]);
    let o = object_props(&p);
    assert!((pos(&o) - -7.0).abs() < 1e-3, "-14 half-points = 7 pt lowered: {o:?}");
    assert!(plain_vert(&o), "{o:?}");
}

#[test]
fn an_inline_picture_keeps_its_run_size() {
    let p = picture_doc(&[(HPS, &24u16.to_le_bytes())]);
    let o = object_props(&p);
    assert_eq!(o.size, Some(12.0), "{o:?}");
}

#[test]
fn an_inline_picture_without_position_stays_on_the_baseline() {
    let o = object_props(&picture_doc(&[]));
    assert!(pos(&o).abs() < 1e-3 && plain_vert(&o), "{o:?}");
}
