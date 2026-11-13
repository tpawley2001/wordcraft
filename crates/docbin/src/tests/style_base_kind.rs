//! A .doc paragraph style whose stored `istdBase` ([MS-DOC] §2.9.260) names a style of another
//! kind, an empty slot or a slot past the end of the style sheet is based on Normal, as Word
//! shows it: its paragraphs take Normal's size, not the 10 pt default, and none of the other
//! style's character formatting. No base (0x0FFF) stays none; other kinds keep their base.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::props::VertAlign;
use wordcraft_doc::{CharProps, Document, Style};

const HPS: u16 = 0x4A43; // sprmCHps, half-points
const ISS: u16 = 0x2A48; // sprmCIss

type RawSpec = (u8, u16, &'static str, Vec<u8>, Vec<u8>);

fn size(hp: u16) -> Vec<u8> {
    grpprl(&[(HPS, &hp.to_le_bytes())])
}

/// Slot 0 Normal (12 pt, base `normal_base`), slot 1 empty, slot 2 character style "Mark"
/// (superscript), then `extra` from slot 3 on; one paragraph in style `para_istd`.
fn doc(normal_base: u16, extra: Vec<RawSpec>, para_istd: u16) -> Document {
    let mut styles: Vec<RawSpec> = vec![
        (1, normal_base, "Normal", Vec::new(), size(24)),
        (1, 0x0FFF, "", Vec::new(), Vec::new()),
        (2, 0x0FFF, "Mark", Vec::new(), grpprl(&[(ISS, &[1])])),
    ];
    styles.extend(extra);
    let spec = RichSpec {
        text: "x\r",
        styles,
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: vec![(1, para_istd, Vec::new())],
    };
    read(&rich_doc(&spec)).expect("opens")
}

/// A document whose slot 3 is paragraph style "S" based on `base`, with `chpx`.
fn with_s(base: u16, chpx: Vec<u8>) -> Document {
    doc(0x0FFF, vec![(1, base, "S", Vec::new(), chpx)], 3)
}

fn named<'a>(d: &'a Document, name: &str) -> &'a Style {
    d.styles.styles.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no style named {name:?}"))
}

fn id_of(d: &Document, name: &str) -> String {
    named(d, name).id.clone()
}

/// The resolved size of a paragraph in style `name`.
fn resolved_size(d: &Document, name: &str) -> f32 {
    d.styles.resolve_char(Some(&id_of(d, name)), &CharProps::default()).size
}

fn resolved_vert(d: &Document, name: &str) -> VertAlign {
    d.styles.resolve_char(Some(&id_of(d, name)), &CharProps::default()).vert_align
}

fn close(got: f32, want: f32) -> bool {
    (got - want).abs() < 0.01
}

#[test]
fn base_character_style_is_normal() {
    let d = with_s(2, Vec::new());
    let s = named(&d, "S");
    assert_eq!(s.based_on, Some(id_of(&d, "Normal")));
    assert!(close(resolved_size(&d, "S"), 12.0), "size {}", resolved_size(&d, "S"));
    assert_eq!(resolved_vert(&d, "S"), VertAlign::Baseline);
    assert_eq!(s.chr.vert_align, None, "superscript is not set");
}

#[test]
fn base_empty_slot_is_normal() {
    let d = with_s(1, Vec::new());
    assert_eq!(named(&d, "S").based_on, Some(id_of(&d, "Normal")));
    assert!(close(resolved_size(&d, "S"), 12.0));
}

#[test]
fn base_past_the_end_is_normal() {
    let d = with_s(40, Vec::new());
    assert_eq!(named(&d, "S").based_on, Some(id_of(&d, "Normal")));
    assert!(close(resolved_size(&d, "S"), 12.0));
}

#[test]
fn base_none_stays_none() {
    let d = with_s(0x0FFF, Vec::new());
    assert_eq!(named(&d, "S").based_on, None);
    assert!(close(resolved_size(&d, "S"), 10.0));
}

#[test]
fn base_paragraph_style_unchanged() {
    let d = doc(0x0FFF, vec![(1, 0, "P", Vec::new(), size(32)), (1, 3, "S", Vec::new(), Vec::new())], 4);
    assert_eq!(named(&d, "S").based_on, Some(id_of(&d, "P")));
    assert_eq!(named(&d, "P").based_on, Some(id_of(&d, "Normal")));
    assert!(close(resolved_size(&d, "S"), 16.0));
}

#[test]
fn own_size_wins() {
    let d = with_s(2, size(16));
    let s = named(&d, "S");
    assert_eq!(s.based_on, Some(id_of(&d, "Normal")));
    assert!(close(resolved_size(&d, "S"), 8.0));
    assert_eq!(resolved_vert(&d, "S"), VertAlign::Baseline);
    assert_eq!(s.chr.vert_align, None);
}

#[test]
fn normal_itself_with_bad_base() {
    let d = doc(2, Vec::new(), 0);
    let n = named(&d, "Normal");
    assert_eq!(n.based_on, None);
    assert_eq!(n.chr.vert_align, None);
    assert_eq!(resolved_vert(&d, "Normal"), VertAlign::Baseline);
    assert!(close(resolved_size(&d, "Normal"), 12.0));
}

#[test]
fn character_style_with_paragraph_base_unchanged() {
    let d = doc(0x0FFF, vec![(2, 0, "C", Vec::new(), Vec::new())], 0);
    let c = named(&d, "C");
    assert_eq!(c.based_on, Some(id_of(&d, "Normal")));
}

#[test]
fn a_paragraph_in_the_style_opens_with_normals_size() {
    let d = with_s(2, Vec::new());
    let p = d.body.iter().filter_map(|b| b.as_para()).next().expect("a paragraph");
    assert_eq!(p.props.style, Some(id_of(&d, "S")));
    let run = p.runs.first().expect("a run");
    let r = d.styles.resolve_char(p.props.style.as_deref(), &run.props);
    assert!(close(r.size, 12.0), "size {}", r.size);
    assert_eq!(r.vert_align, VertAlign::Baseline);
}
