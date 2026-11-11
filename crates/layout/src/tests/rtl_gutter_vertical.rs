//! A right-to-left gutter (`w:rtlGutter`, ECMA-376 §17.6.16) widens the real page's right margin
//! in a vertical section too: the unturned page's bands come from the real page's left and right
//! bands, the gutter on the right unless mirror margins keep it on the inside.

use crate::vertical::unturned_sect;
use wordcraft_doc::props::TextDirection;
use wordcraft_doc::section::SectionProps;

fn sect(dir: TextDirection, rtl_gutter: bool) -> SectionProps {
    SectionProps {
        page_w: 612.0,
        page_h: 792.0,
        margin_left: 72.0,
        margin_right: 90.0,
        margin_top: 72.0,
        margin_bottom: 72.0,
        gutter: 36.0,
        rtl_gutter,
        text_direction: dir,
        ..Default::default()
    }
}

/// (top, bottom, left, right, gutter) of the unturned page.
fn bands(dir: TextDirection, rtl_gutter: bool, mirror: bool) -> (f32, f32, f32, f32, f32) {
    let v = unturned_sect(&sect(dir, rtl_gutter), 72.0, mirror);
    (v.margin_top, v.margin_bottom, v.margin_left, v.margin_right, v.gutter)
}

#[test]
fn top_to_bottom_section_keeps_a_left_gutter_on_the_left() {
    assert_eq!(bands(TextDirection::Down, false, false), (90.0, 108.0, 72.0, 72.0, 0.0));
}

#[test]
fn top_to_bottom_section_puts_a_right_to_left_gutter_on_the_right() {
    assert_eq!(bands(TextDirection::Down, true, false), (126.0, 72.0, 72.0, 72.0, 0.0));
}

#[test]
fn mirror_margins_keep_the_gutter_inside_in_a_vertical_section() {
    assert_eq!(bands(TextDirection::Down, true, true), (90.0, 108.0, 72.0, 72.0, 0.0));
}

#[test]
fn bottom_to_top_section_keeps_a_left_gutter_on_the_left() {
    assert_eq!(bands(TextDirection::Up, false, false), (108.0, 90.0, 72.0, 72.0, 0.0));
}

#[test]
fn bottom_to_top_section_puts_a_right_to_left_gutter_on_the_right() {
    assert_eq!(bands(TextDirection::Up, true, false), (72.0, 126.0, 72.0, 72.0, 0.0));
}
