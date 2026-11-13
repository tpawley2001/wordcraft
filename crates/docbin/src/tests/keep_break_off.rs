//! A stored 0 for the three on/off paragraph sprms of [MS-DOC] §2.6.2 (each operand a Bool8:
//! 0 = off, any other byte = on): sprmPFKeep 0x2405 (`w:keepLines`, ECMA-376 Part 1 §17.3.1.14),
//! sprmPFKeepFollow 0x2406 (`w:keepNext`, §17.3.1.15) and sprmPFPageBreakBefore 0x2407
//! (`w:pageBreakBefore`, §17.3.1.23). A style based on one that has the property on, or a
//! paragraph's direct formatting, turns it off with the 0: a `heading 2` based on a `heading 1`
//! that starts a new page does not start one itself (Word).

use super::{RichSpec, grpprl, read, rich_doc};
use wordcraft_doc::Document;
use wordcraft_doc::props::ParaProps;

const P_KEEP: u16 = 0x2405;
const P_KEEP_FOLLOW: u16 = 0x2406;
const P_PAGE_BREAK_BEFORE: u16 = 0x2407;

/// The three sprms with the model field each one sets and its resolved value.
type Own = fn(&ParaProps) -> Option<bool>;
type Resolved = fn(&Document, &ParaProps) -> bool;
const SPRMS: [(u16, &str, Own, Resolved); 3] = [
    (P_KEEP, "keep lines", |p| p.keep_lines, |d, p| d.styles.resolve_para(p).keep_lines),
    (P_KEEP_FOLLOW, "keep next", |p| p.keep_next, |d, p| d.styles.resolve_para(p).keep_next),
    (P_PAGE_BREAK_BEFORE, "page break before", |p| p.page_break_before, |d, p| d.styles.resolve_para(p).page_break_before),
];

/// Normal (istd 0); On (istd 1, on Normal: the sprm = 1); Off Child (istd 2, on On: the sprm = 0);
/// Plain Child (istd 3, on On: nothing of its own); Grandchild (istd 4, on Off Child: nothing of
/// its own). One paragraph per entry of `paras`.
fn doc(sprm: u16, text: &'static str, paras: Vec<(u32, u16, Vec<u8>)>) -> Document {
    let spec = RichSpec {
        text,
        styles: vec![
            (1, 0x0FFF, "Normal", Vec::new(), Vec::new()),
            (1, 0, "On", grpprl(&[(sprm, &[1])]), Vec::new()),
            (1, 1, "Off Child", grpprl(&[(sprm, &[0])]), Vec::new()),
            (1, 1, "Plain Child", Vec::new(), Vec::new()),
            (1, 2, "Grandchild", Vec::new(), Vec::new()),
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

#[test]
fn a_style_that_turns_it_on_owns_it() {
    for (sprm, what, own, resolved) in SPRMS {
        let d = doc(sprm, "One\r", vec![(0, 1, Vec::new())]);
        assert_eq!(own(&style_para(&d, "On")), Some(true), "{what}: the style");
        assert_eq!(own(&para(&d, 0)), None, "{what}: the paragraph has no direct setting");
        assert!(resolved(&d, &para(&d, 0)), "{what}: resolved");
    }
}

#[test]
fn a_child_style_turns_it_off_with_a_stored_zero() {
    for (sprm, what, own, resolved) in SPRMS {
        let d = doc(sprm, "One\r", vec![(0, 2, Vec::new())]);
        assert_eq!(own(&style_para(&d, "Off Child")), Some(false), "{what}: the child style's own setting");
        assert_eq!(own(&para(&d, 0)), None, "{what}: the paragraph has no direct setting");
        assert!(!resolved(&d, &para(&d, 0)), "{what}: resolved");
    }
}

#[test]
fn a_child_style_without_the_sprm_inherits_on() {
    for (sprm, what, own, resolved) in SPRMS {
        let d = doc(sprm, "One\r", vec![(0, 3, Vec::new())]);
        assert_eq!(own(&style_para(&d, "Plain Child")), None, "{what}: merely inherited");
        assert!(resolved(&d, &para(&d, 0)), "{what}: resolved");
    }
}

#[test]
fn a_grandchild_inherits_the_off() {
    for (sprm, what, own, resolved) in SPRMS {
        let d = doc(sprm, "One\r", vec![(0, 4, Vec::new())]);
        assert_eq!(own(&style_para(&d, "Grandchild")), None, "{what}: merely inherited");
        assert!(!resolved(&d, &para(&d, 0)), "{what}: resolved");
    }
}

#[test]
fn a_direct_zero_turns_the_styles_setting_off_for_that_paragraph_only() {
    for (sprm, what, own, resolved) in SPRMS {
        let d = doc(sprm, "One\rTwo\r", vec![(0, 1, grpprl(&[(sprm, &[0])])), (4, 1, Vec::new())]);
        assert_eq!(own(&style_para(&d, "On")), Some(true), "{what}: the style");
        assert_eq!(own(&para(&d, 0)), Some(false), "{what}: the direct 0");
        assert!(!resolved(&d, &para(&d, 0)), "{what}: the paragraph with the direct 0");
        assert_eq!(own(&para(&d, 1)), None, "{what}: its neighbour has no direct setting");
        assert!(resolved(&d, &para(&d, 1)), "{what}: its neighbour keeps the style's setting");
    }
}

#[test]
fn direct_formatting_on_a_normal_paragraph() {
    // Operand -> the paragraph's own value. Bool8: 0x80, 0x81 and 0xFF are on, not style toggles.
    let rows: [(u8, bool); 5] = [(0, false), (1, true), (0x80, true), (0x81, true), (0xFF, true)];
    for (sprm, what, own, resolved) in SPRMS {
        for (operand, want) in rows {
            let d = doc(sprm, "Plain\r", vec![(0, 0, grpprl(&[(sprm, &[operand])]))]);
            assert_eq!(own(&style_para(&d, "Normal")), None, "{what}: Normal");
            assert_eq!(own(&para(&d, 0)), Some(want), "{what}: operand {operand:#04x}");
            assert_eq!(resolved(&d, &para(&d, 0)), want, "{what}: operand {operand:#04x} resolved");
        }
    }
}

#[test]
fn without_the_sprm_nothing_is_set() {
    for (sprm, what, own, resolved) in SPRMS {
        let d = doc(sprm, "Plain\r", vec![(0, 0, Vec::new())]);
        assert_eq!(own(&style_para(&d, "Normal")), None, "{what}: Normal");
        assert_eq!(own(&para(&d, 0)), None, "{what}: the paragraph");
        assert!(!resolved(&d, &para(&d, 0)), "{what}: resolved");
    }
}
