//! Character spacing, scale and kerning from Font › Advanced ([MS-DOC] §2.6.1 sprmCDxaSpace
//! 0x8840, twips; sprmCCharScale 0x4852, percent 1..=600; sprmCHpsKern 0x484B, half-points).
//! Word lays condensed .doc text narrower (govdocs1 003631.doc: -2 twips, 0.1 pt per character).

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

const C_DXA_SPACE: u16 = 0x8840;
const C_CHAR_SCALE: u16 = 0x4852;
const C_HPS_KERN: u16 = 0x484B;

/// One paragraph "Hello" whose run carries `run`; the paragraph style "Body" (istd 1) carries
/// `style_chpx`.
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

fn with(op: u16, operand: &[u8]) -> CharProps {
    run_props(&doc(grpprl(&[(op, operand)]), Vec::new()))
}

fn close(got: Option<f32>, want: f32) -> bool {
    got.is_some_and(|v| (v - want).abs() < 0.001)
}

#[test]
fn condensed_and_expanded_spacing_is_read_in_points() {
    for (twips, pt) in [(-2i16, -0.1f32), (60, 3.0), (31680, 1584.0), (-31680, -1584.0)] {
        let c = with(C_DXA_SPACE, &twips.to_le_bytes());
        assert!(close(c.spacing, pt), "{twips} twips: {:?}", c.spacing);
    }
}

#[test]
fn character_scale_is_read_in_percent_and_out_of_range_values_are_ignored() {
    assert!(close(with(C_CHAR_SCALE, &150u16.to_le_bytes()).scale, 150.0));
    assert_eq!(with(C_CHAR_SCALE, &0u16.to_le_bytes()).scale, None);
    assert_eq!(with(C_CHAR_SCALE, &601u16.to_le_bytes()).scale, None);
}

#[test]
fn kerning_threshold_is_read_from_half_points() {
    assert!(close(with(C_HPS_KERN, &24u16.to_le_bytes()).kern, 12.0));
    assert!(close(with(C_HPS_KERN, &0u16.to_le_bytes()).kern, 0.0));
}

#[test]
fn a_run_without_the_sprms_has_none() {
    let c = run_props(&doc(Vec::new(), Vec::new()));
    assert_eq!((c.spacing, c.scale, c.kern), (None, None, None));
}

#[test]
fn a_style_carries_its_spacing() {
    let d = doc(Vec::new(), grpprl(&[(C_DXA_SPACE, &20i16.to_le_bytes())]));
    let style = d.styles.styles.iter().find(|s| s.name == "Body").expect("style");
    assert!(close(style.chr.spacing, 1.0), "{:?}", style.chr.spacing);
}

#[test]
fn a_truncated_operand_is_ignored_without_panic() {
    // The grpprl ends one byte into the two-byte operand.
    let mut g = Vec::from(C_DXA_SPACE.to_le_bytes());
    g.push(0xFE);
    assert_eq!(run_props(&doc(g, Vec::new())).spacing, None);
}
