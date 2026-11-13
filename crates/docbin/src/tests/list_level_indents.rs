//! A list level's paragraph properties ([MS-DOC] §2.9.150 LVL `grpprlPapx`): a positive
//! first-line indent (the number indented like a first line) is kept as a negative hanging
//! indent, and the stop marked as the list tab ([MS-DOC] §2.9.304 TBD, `jc` 6) becomes the
//! level's tab, so the number and the text after it sit where Word puts them.

use crate::list::{RawLvl, level_of};
use wordcraft_doc::numbering::Level;

/// Bytes from a hex string; spaces are only for reading.
fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// Level 0 with the given grpprlPapx and number text "%1.".
fn level(papx: &str) -> Level {
    let raw = RawLvl { start: 1, rgbxch_nums: [1, 0, 0, 0, 0, 0, 0, 0, 0], papx: hex(papx), xst: vec![0, 0x2E], ..Default::default() };
    level_of(&raw, 0, &[])
}

fn check(papx: &str, indent: f32, hanging: f32, tab: Option<f32>) {
    let l = level(papx);
    assert_eq!((l.indent, l.hanging, l.tab), (indent, hanging, tab), "papx {papx}");
}

#[test]
fn legal_numbering_first_line_and_list_tab() {
    check("0f840000 1184d002 15c6 05 00 01 3804 06 5e840000 6084d002", 0.0, -36.0, Some(54.0));
}

#[test]
fn hanging_level_with_list_tab_at_its_indent() {
    check("0f846801 118498fe 15c6 05 00 01 6801 06 5e846801 608498fe", 18.0, 18.0, Some(18.0));
}

#[test]
fn deeper_hanging_level_with_list_tab() {
    check("0f843804 118498fe 15c6 05 00 01 3804 06 5e843804 608498fe", 54.0, 18.0, Some(54.0));
}

#[test]
fn empty_papx_keeps_the_defaults() {
    check("", 18.0, 18.0, None);
}

#[test]
fn indents_without_tabs_have_no_list_tab() {
    check("0f846801 118498fe", 18.0, 18.0, None);
}

#[test]
fn a_plain_left_tab_is_not_the_list_tab() {
    check("0f840000 1184d002 15c6 05 00 01 3804 00", 0.0, -36.0, None);
}

#[test]
fn the_list_tab_is_picked_among_several_stops() {
    check("15c6 08 00 02 d002 3804 00 06", 18.0, 18.0, Some(54.0));
}

#[test]
fn a_cut_short_tab_operand_does_not_panic() {
    check("15c6 05 00 01 3804", 18.0, 18.0, None);
}

#[test]
fn a_huge_first_line_indent_is_clamped() {
    check("1184 ff7f", 18.0, -1000.0, None);
}

#[test]
fn the_papx_variant_of_the_tab_sprm_is_read() {
    // sprmPChgTabsPapx (0xC60D): cb, no removals, one added stop at 1080 twips marked as list tab.
    check("0dc6 05 00 01 3804 06", 18.0, 18.0, Some(54.0));
}

#[test]
fn truncated_tab_operands_never_panic() {
    let full = hex("15c6 08 00 02 d002 3804 00 06");
    for n in 0..=full.len() {
        let raw = RawLvl { papx: full[..n].to_vec(), xst: vec![0x2022], ..Default::default() };
        let _ = level_of(&raw, 0, &[]);
    }
    // Counts that claim more stops than the operand holds.
    let _ = level_of(&RawLvl { papx: hex("15c6 03 ff ff ff"), ..Default::default() }, 0, &[]);
    let _ = level_of(&RawLvl { papx: hex("0dc6 03 00 ff 06"), ..Default::default() }, 0, &[]);
}
