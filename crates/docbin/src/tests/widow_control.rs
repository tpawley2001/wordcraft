//! Widow and orphan control in .doc files ([MS-DOC] §2.6.2 sprmPFWidowControl 0x2431, Bool8): the .doc
//! form of `w:widowControl` (ECMA-376 Part 1 §17.3.1.44). A paragraph without the sprm has it on; files
//! turn it off in a style or on single paragraphs, and Word then leaves one line alone at a page edge.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::ParaProps;

const P_DYA_AFTER: u16 = 0xA414;
const P_F_WIDOW_CONTROL: u16 = 0x2431;

/// Normal (istd 0); No Widow (istd 1, on Normal: widow control off); Tight Child (istd 2, on No
/// Widow: nothing of its own); Back On (istd 3, on No Widow: widow control on). One paragraph per
/// entry of `paras`.
fn doc(text: &'static str, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let off_papx = grpprl(&[(P_F_WIDOW_CONTROL, &[0])]);
    let on_papx = grpprl(&[(P_F_WIDOW_CONTROL, &[1])]);
    let spec = RichSpec {
        text,
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "No Widow", off_papx, Vec::new()),
            (1, 1, "Tight Child", Vec::new(), Vec::new()),
            (1, 1, "Back On", on_papx, Vec::new()),
        ],
        fonts: vec!["Times New Roman"],
        chpx_runs: vec![(0, Vec::new())],
        papx_entries: paras,
    };
    read(&rich_doc(&spec)).expect("opens")
}

fn style_para(doc: &Document, name: &str) -> ParaProps {
    doc.styles.styles.iter().find(|s| s.name == name).expect("style").para.clone()
}

fn para(doc: &Document, i: usize) -> ParaProps {
    doc.body.iter().filter_map(|b| b.as_para()).nth(i).expect("paragraph").props.clone()
}

fn resolved(doc: &Document, i: usize) -> bool {
    doc.styles.resolve_para(&para(doc, i)).widow_control
}

#[test]
fn a_style_can_turn_widow_control_off() {
    let d = doc("One\r", vec![(0, 1, Vec::new())]);
    assert_eq!(style_para(&d, "No Widow").widow_control, Some(false));
    assert_eq!(para(&d, 0).widow_control, None, "the paragraph has no direct setting");
    assert!(!resolved(&d, 0));
}

#[test]
fn a_child_style_inherits_off_and_owns_nothing() {
    let d = doc("Child\r", vec![(0, 2, Vec::new())]);
    assert_eq!(style_para(&d, "Tight Child").widow_control, None);
    assert!(!resolved(&d, 0));
}

#[test]
fn a_child_style_can_turn_it_back_on() {
    let d = doc("Back\r", vec![(0, 3, Vec::new())]);
    assert_eq!(style_para(&d, "Back On").widow_control, Some(true));
    assert!(resolved(&d, 0));
}

#[test]
fn a_direct_zero_turns_it_off_for_that_paragraph_only() {
    let papx = grpprl(&[(P_F_WIDOW_CONTROL, &[0])]);
    let d = doc("One\rTwo\r", vec![(0, 0, papx), (4, 0, Vec::new())]);
    assert_eq!(para(&d, 0).widow_control, Some(false));
    assert!(!resolved(&d, 0), "the paragraph with the direct 0");
    assert_eq!(para(&d, 1).widow_control, None);
    assert!(resolved(&d, 1), "its neighbour keeps the default");
}

#[test]
fn a_direct_one_turns_the_styles_off_back_on() {
    let papx = grpprl(&[(P_F_WIDOW_CONTROL, &[1])]);
    let d = doc("One\rTwo\r", vec![(0, 1, papx), (4, 1, Vec::new())]);
    assert_eq!(para(&d, 0).widow_control, Some(true));
    assert!(resolved(&d, 0), "the paragraph with the direct 1");
    assert!(!resolved(&d, 1), "its neighbour keeps the style's setting");
}

#[test]
fn any_non_zero_operand_is_on() {
    let papx = grpprl(&[(P_F_WIDOW_CONTROL, &[0xFF])]);
    let d = doc("Plain\r", vec![(0, 1, papx)]);
    assert_eq!(para(&d, 0).widow_control, Some(true));
    assert!(resolved(&d, 0));
}

#[test]
fn without_the_sprm_nothing_is_set_and_the_default_stays_on() {
    let papx = grpprl(&[(P_DYA_AFTER, &160i16.to_le_bytes())]);
    let d = doc("Plain\r", vec![(0, 0, papx)]);
    assert_eq!(style_para(&d, "Normal").widow_control, None);
    assert_eq!(para(&d, 0).widow_control, None);
    assert!(resolved(&d, 0));
}

#[test]
fn the_single_sprm_piece_modifier_maps_to_the_widow_control_sprm() {
    assert_eq!(crate::sprm::from_prm0(0x33), Some(P_F_WIDOW_CONTROL));
}
