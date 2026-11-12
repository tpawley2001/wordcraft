//! Contextual spacing in .doc files ([MS-DOC] §2.6.2 sprmPFContextualSpacing 0x246D, Bool8,
//! the same-style check box of Paragraph > Spacing): the .doc form of `w:contextualSpacing`
//! (ECMA-376 Part 1 §17.3.1.9). Word's built-in List Paragraph style carries it with 8 pt after, so the
//! items of a list sit together and the 8 pt show only after the last one.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::ParaProps;

const P_DYA_AFTER: u16 = 0xA414;
const P_F_CONTEXTUAL_SPACING: u16 = 0x246D;

/// Normal (istd 0); List Paragraph (istd 1, on Normal: 8 pt after, contextual spacing on);
/// Loose List (istd 2, on List Paragraph: contextual spacing off); Tight Child (istd 3, on List
/// Paragraph: nothing of its own). One paragraph per entry of `paras`.
fn doc(text: &'static str, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let list_papx = grpprl(&[(P_DYA_AFTER, &160i16.to_le_bytes()), (P_F_CONTEXTUAL_SPACING, &[1])]);
    let loose_papx = grpprl(&[(P_F_CONTEXTUAL_SPACING, &[0])]);
    let spec = RichSpec {
        text,
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "List Paragraph", list_papx, Vec::new()),
            (1, 1, "Loose List", loose_papx, Vec::new()),
            (1, 1, "Tight Child", Vec::new(), Vec::new()),
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
    doc.styles.resolve_para(&para(doc, i)).contextual_spacing
}

#[test]
fn the_list_paragraph_style_keeps_contextual_spacing_and_its_space_after() {
    let d = doc("One\r", vec![(0, 1, Vec::new())]);
    let s = style_para(&d, "List Paragraph");
    assert_eq!(s.contextual_spacing, Some(true));
    assert!(s.space_after.is_some_and(|v| (v - 8.0).abs() < 0.01), "after {:?}", s.space_after);
}

#[test]
fn a_paragraph_in_the_style_resolves_to_contextual_spacing_without_owning_it() {
    let d = doc("One\rTwo\r", vec![(0, 1, Vec::new()), (4, 1, Vec::new())]);
    assert_eq!(para(&d, 0).contextual_spacing, None, "the paragraph has no direct setting");
    assert!(resolved(&d, 0), "first item");
    assert!(resolved(&d, 1), "second item");
}

#[test]
fn a_child_style_inherits_it_and_owns_nothing() {
    let d = doc("Child\r", vec![(0, 3, Vec::new())]);
    assert_eq!(style_para(&d, "Tight Child").contextual_spacing, None);
    assert!(resolved(&d, 0));
}

#[test]
fn a_child_style_can_turn_it_off() {
    let d = doc("Loose\r", vec![(0, 2, Vec::new())]);
    assert_eq!(style_para(&d, "Loose List").contextual_spacing, Some(false));
    assert!(!resolved(&d, 0));
}

#[test]
fn direct_formatting_turns_it_on_for_a_normal_paragraph() {
    let papx = grpprl(&[(P_F_CONTEXTUAL_SPACING, &[1])]);
    let d = doc("Plain\r", vec![(0, 0, papx)]);
    assert_eq!(para(&d, 0).contextual_spacing, Some(true));
    assert!(resolved(&d, 0));
}

#[test]
fn a_direct_zero_turns_the_styles_setting_off() {
    let papx = grpprl(&[(P_F_CONTEXTUAL_SPACING, &[0])]);
    let d = doc("One\rTwo\r", vec![(0, 1, papx), (4, 1, Vec::new())]);
    assert_eq!(para(&d, 0).contextual_spacing, Some(false));
    assert!(!resolved(&d, 0), "the paragraph with the direct 0");
    assert!(resolved(&d, 1), "its neighbour keeps the style's setting");
}

#[test]
fn any_non_zero_operand_is_on() {
    let papx = grpprl(&[(P_F_CONTEXTUAL_SPACING, &[0xFF])]);
    let d = doc("Plain\r", vec![(0, 0, papx)]);
    assert_eq!(para(&d, 0).contextual_spacing, Some(true));
}

#[test]
fn without_the_sprm_nothing_is_set() {
    let papx = grpprl(&[(P_DYA_AFTER, &160i16.to_le_bytes())]);
    let d = doc("Plain\r", vec![(0, 0, papx)]);
    assert_eq!(style_para(&d, "Normal").contextual_spacing, None);
    assert_eq!(para(&d, 0).contextual_spacing, None);
    assert!(!resolved(&d, 0));
}
