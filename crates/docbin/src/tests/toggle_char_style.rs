//! Toggle operands ([MS-DOC] ToggleOperand: 0x80 = the style's value, 0x81 = the opposite of the
//! style's value) on a run that also names a character style (`sprmCIstd`). Word resolves them
//! against the value the paragraph style and the character style give together, and the two
//! styles' toggles cancel each other (bold paragraph style + bold character style = not bold).
//!
//! Measured in Microsoft 365 Word on copies of one document that differ only in a run's CHPX;
//! the paragraph style is bold (Heading 3, Heading 6), the character style is Strong (bold):
//!
//! | run                         | Word    |
//! |-----------------------------|---------|
//! | Strong + sprmCFBold 0x81    | bold    |
//! | Strong + sprmCFBold 0x80    | regular |
//! | Strong + sprmCFBold 0x00    | regular |
//! | Strong + sprmCFBold 0x01    | bold    |
//! | no character style + 0x81   | regular |
//! | no character style + 0x80   | bold    |

use wordcraft_doc::props::CharProps;

use crate::stsh::{RawStyle, STI_USER};
use crate::{char_props, fmt};

/// No parent style.
const NO_BASE: u16 = 0x0FFF;
/// `sprmCFBold 0x01`, a character style's own formatting.
const BOLD_ON: [u8; 3] = [0x35, 0x08, 0x01];

/// Character styles: 0 "Strong" (bold), 1 "Strong Child" (based on Strong, nothing of its own),
/// 2 "Plain" (no formatting).
fn styles() -> Vec<RawStyle> {
    vec![
        RawStyle { sti: STI_USER, stk: 2, istd_base: NO_BASE, name: "Strong".into(), chpx: BOLD_ON.to_vec(), ..Default::default() },
        RawStyle { sti: STI_USER, stk: 2, istd_base: 0, name: "Strong Child".into(), ..Default::default() },
        RawStyle { sti: STI_USER, stk: 2, istd_base: NO_BASE, name: "Plain".into(), ..Default::default() },
    ]
}

/// The direct bold read for a run in a bold paragraph style whose CHPX holds `sprmCIstd istd`
/// (when given) and `sprmCFBold operand`, checked in both sprm orders.
fn bold_in(styles: &[RawStyle], istd: Option<u8>, operand: u8) -> Option<bool> {
    let ids = fmt::style_ids(styles);
    let base = CharProps { bold: Some(true), ..Default::default() };
    let cistd: Vec<u8> = istd.map(|i| vec![0x30, 0x4A, i, 0]).unwrap_or_default();
    let toggle = [0x35, 0x08, operand];
    let style_first = char_props(&[cistd.as_slice(), &toggle[..]].concat(), &[], &base, styles, &ids);
    let style_last = char_props(&[&toggle[..], cistd.as_slice()].concat(), &[], &base, styles, &ids);
    assert_eq!(style_first.bold, style_last.bold, "the order of sprmCIstd and sprmCFBold must not matter");
    assert_eq!(style_first.style.is_some(), istd.is_some(), "the character style reference is kept");
    assert_eq!(style_first.style, style_last.style);
    style_first.bold
}

fn bold(istd: Option<u8>, operand: u8) -> Option<bool> {
    bold_in(&styles(), istd, operand)
}

#[test]
fn opposite_of_the_style_under_a_bold_character_style_is_bold() {
    // Word: bold. The styles together give "not bold" (their toggles cancel), 0x81 inverts that.
    assert_eq!(bold(Some(0), 0x81), Some(true));
}

#[test]
fn same_as_the_style_under_a_bold_character_style_is_regular() {
    // Word: regular. 0x80 keeps what the two styles give together.
    assert_eq!(bold(Some(0), 0x80), Some(false));
}

#[test]
fn absolute_operands_stay_absolute_under_a_character_style() {
    assert_eq!(bold(Some(0), 0x00), Some(false));
    assert_eq!(bold(Some(0), 0x01), Some(true));
}

#[test]
fn without_a_character_style_the_paragraph_style_decides() {
    assert_eq!(bold(None, 0x81), Some(false));
    assert_eq!(bold(None, 0x80), Some(true));
    assert_eq!(bold(None, 0x00), Some(false));
    assert_eq!(bold(None, 0x01), Some(true));
}

#[test]
fn a_character_style_without_bold_changes_nothing() {
    assert_eq!(bold(Some(2), 0x81), Some(false));
    assert_eq!(bold(Some(2), 0x80), Some(true));
}

#[test]
fn bold_inherited_from_the_parent_character_style_counts() {
    // "Strong Child" has no formatting of its own; it is bold through Strong.
    assert_eq!(bold(Some(1), 0x81), Some(true));
    assert_eq!(bold(Some(1), 0x80), Some(false));
}

#[test]
fn a_character_style_based_on_itself_does_not_loop() {
    // A hostile stylesheet: the style names itself as its parent. Its own bold still counts.
    let looped = vec![RawStyle { sti: STI_USER, stk: 2, istd_base: 0, name: "Strong".into(), chpx: BOLD_ON.to_vec(), ..Default::default() }];
    assert_eq!(bold_in(&looped, Some(0), 0x81), Some(true));
    assert_eq!(bold_in(&looped, Some(0), 0x80), Some(false));
}

#[test]
fn the_other_toggles_follow_the_same_rule() {
    // Italic uses the same operand type: an italic character style in an italic paragraph style.
    let styles = vec![RawStyle { sti: STI_USER, stk: 2, istd_base: NO_BASE, name: "Emphasis".into(), chpx: vec![0x36, 0x08, 0x01], ..Default::default() }];
    let ids = fmt::style_ids(&styles);
    let base = CharProps { italic: Some(true), bold: Some(true), ..Default::default() };
    let props = char_props(&[0x30, 0x4A, 0, 0, 0x36, 0x08, 0x81, 0x35, 0x08, 0x81], &[], &base, &styles, &ids);
    assert_eq!(props.italic, Some(true), "italic paragraph + italic character style + 0x81");
    // The character style says nothing about bold: 0x81 inverts the paragraph style's bold.
    assert_eq!(props.bold, Some(false));
}
