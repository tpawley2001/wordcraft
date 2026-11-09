//! Automatic (HTML-style) space before and after ([MS-DOC] §2.6.2 sprmPFDyaBeforeAuto 0x245B,
//! sprmPFDyaAfterAuto 0x245C, Bool8): the .doc form of `w:beforeAutospacing` /
//! `w:afterAutospacing`. Word's built-in "Normal (Web)" style carries them with a stored
//! dyaBefore/dyaAfter of 100 twips; Word spaces such paragraphs 14 pt apart, not 5 pt.

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::ParaProps;
use wordcraft_doc::resolve::AUTO_SPACING;

const P_DYA_BEFORE: u16 = 0xA413;
const P_DYA_AFTER: u16 = 0xA414;
const P_F_DYA_BEFORE_AUTO: u16 = 0x245B;
const P_F_DYA_AFTER_AUTO: u16 = 0x245C;

fn web_style_papx() -> Vec<u8> {
    grpprl(&[(P_DYA_BEFORE, &100i16.to_le_bytes()), (P_DYA_AFTER, &100i16.to_le_bytes()), (P_F_DYA_BEFORE_AUTO, &[1]), (P_F_DYA_AFTER_AUTO, &[1])])
}

/// Normal (istd 0) and Web (istd 1, based on Normal, Auto spacing); one paragraph per entry.
fn doc(text: &'static str, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![(1, 0x0FFF, "Normal", Vec::new(), Vec::new()), (1, 0, "Web", web_style_papx(), Vec::new())],
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

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn a_style_with_auto_spacing_keeps_the_flags_and_the_stored_values() {
    let d = doc("Web\r", vec![(0, 1, Vec::new())]);
    let s = style_para(&d, "Web");
    assert_eq!((s.space_before_auto, s.space_after_auto), (Some(true), Some(true)));
    assert!(s.space_before.is_some_and(|v| close(v, 5.0)), "before {:?}", s.space_before);
    assert!(s.space_after.is_some_and(|v| close(v, 5.0)), "after {:?}", s.space_after);
}

#[test]
fn a_paragraph_in_an_auto_spaced_style_resolves_to_word_auto_spacing() {
    let d = doc("Web\r", vec![(0, 1, Vec::new())]);
    let r = d.styles.resolve_para(&para(&d, 0));
    assert!(r.before_auto && r.after_auto, "auto flags {} {}", r.before_auto, r.after_auto);
    assert!(close(r.space_before, AUTO_SPACING), "before {}", r.space_before);
    assert!(close(r.space_after, AUTO_SPACING), "after {}", r.space_after);
}

#[test]
fn direct_auto_space_before_alone() {
    let papx = grpprl(&[(P_F_DYA_BEFORE_AUTO, &[1])]);
    let d = doc("Plain\r", vec![(0, 0, papx)]);
    let p = para(&d, 0);
    assert_eq!((p.space_before_auto, p.space_after_auto), (Some(true), None));
}

#[test]
fn a_direct_zero_turns_the_styles_auto_off() {
    let papx = grpprl(&[(P_F_DYA_BEFORE_AUTO, &[0])]);
    let d = doc("Web\r", vec![(0, 1, papx)]);
    let p = para(&d, 0);
    assert_eq!(p.space_before_auto, Some(false));
    let r = d.styles.resolve_para(&p);
    assert!(!r.before_auto, "space before is no longer automatic");
    assert!(close(r.space_before, 5.0), "the stored dyaBefore: {}", r.space_before);
    assert!(r.after_auto && close(r.space_after, AUTO_SPACING), "after stays automatic: {}", r.space_after);
}

#[test]
fn no_auto_sprms_leave_the_flags_unset() {
    let papx = grpprl(&[(P_DYA_BEFORE, &240i16.to_le_bytes())]);
    let d = doc("Plain\r", vec![(0, 0, papx)]);
    let p = para(&d, 0);
    assert_eq!((p.space_before_auto, p.space_after_auto), (None, None));
    assert!(p.space_before.is_some_and(|v| close(v, 12.0)));
}
