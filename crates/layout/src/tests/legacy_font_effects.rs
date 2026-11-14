//! The legacy font effects (Font dialog Shadow, Outline, Emboss, Engrave) drawn as Microsoft 365
//! Word draws them. Measured 2026-11-14 in Word's PDF export (16.0, Times New Roman and Arial, 8,
//! 11, 24, 36 and 72 pt; identical in both fonts), offsets (dx, dy) in points, y down:
//!   shadow: a RGB(192,192,192) copy at (size / 24, size / 24): 0.36, 0.48, 0.96, 1.56, 3.00
//!   emboss: RGB(153,153,153) at (+0.96, +0.96), then the text colour at (-0.96, -0.96), then the text
//!   engrave: the text colour at (+0.96, +0.96), then RGB(153,153,153) at (-0.96, -0.96), then the text
//!   outline: hollow letters stroked 0.14 pt wide at every size
use crate::wordart::run_effects;
use wordcraft_doc::CharProps;
use wordcraft_doc::effects::ShadowCopy;
use wordcraft_doc::props::Rgb;
use wordcraft_doc::wordart::{TextEffects, TextFill};

const TOL: f32 = 0.01;
const RED: Rgb = Rgb(200, 0, 0);

fn fx(size: f32, c: CharProps) -> TextEffects {
    crate::test_fonts();
    let rc = wordcraft_doc::styles::StyleSheet::default().resolve_char(None, &CharProps { size: Some(size), ..c });
    run_effects(&rc, RED).expect("a legacy effect gives text effects")
}

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < TOL && (a.1 - b.1).abs() < TOL
}

#[test]
fn a_shadow_is_a_light_grey_copy_moved_a_24th_of_the_size_down_and_right() {
    for size in [8.0f32, 11.0, 24.0, 36.0, 72.0] {
        let f = fx(size, CharProps { shadow: Some(true), ..Default::default() });
        let s = f.shadow.expect("a shadow copy");
        assert_eq!(s.color, Rgb(192, 192, 192), "{size} pt");
        assert!(s.blur.abs() < TOL && s.transparency.abs() < TOL, "{size} pt: crisp and opaque: {s:?}");
        let d = size / 24.0;
        assert!(near(s.offset(), (d, d)), "{size} pt: offset {:?}, want ({d}, {d})", s.offset());
        assert!(f.shadow_extra.and_then(|e| e.second).is_none(), "{size} pt: one copy only");
        assert!(f.fill.is_none() && f.outline.is_none(), "{size} pt: the letters stay as they are");
    }
}

#[test]
fn emboss_adds_a_text_colour_copy_up_left_over_a_grey_copy_down_right() {
    for size in [8.0f32, 11.0, 36.0, 72.0] {
        let f = fx(size, CharProps { emboss: Some(true), ..Default::default() });
        let first = f.shadow.expect("the copy right under the text");
        assert_eq!(first.color, RED, "{size} pt: in the text colour");
        assert!(first.blur.abs() < TOL && first.transparency.abs() < TOL, "{size} pt: {first:?}");
        assert!(near(first.offset(), (-0.96, -0.96)), "{size} pt: {:?}", first.offset());
        let second = f.shadow_extra.and_then(|e| e.second).expect("a second copy under the first");
        assert_eq!(second, ShadowCopy { dx: 0.96, dy: 0.96, color: Rgb(153, 153, 153), transparency: 0.0 }, "{size} pt");
    }
}

#[test]
fn engrave_adds_a_grey_copy_up_left_over_a_text_colour_copy_down_right() {
    for size in [8.0f32, 11.0, 36.0, 72.0] {
        let f = fx(size, CharProps { engrave: Some(true), ..Default::default() });
        let first = f.shadow.expect("the copy right under the text");
        assert_eq!(first.color, Rgb(153, 153, 153), "{size} pt");
        assert!(first.blur.abs() < TOL && first.transparency.abs() < TOL, "{size} pt: {first:?}");
        assert!(near(first.offset(), (-0.96, -0.96)), "{size} pt: {:?}", first.offset());
        let second = f.shadow_extra.and_then(|e| e.second).expect("a second copy under the first");
        assert_eq!(second, ShadowCopy { dx: 0.96, dy: 0.96, color: RED, transparency: 0.0 }, "{size} pt");
    }
}

#[test]
fn an_outline_is_a_hairline_around_hollow_letters() {
    for size in [8.0f32, 11.0, 36.0, 72.0] {
        let f = fx(size, CharProps { outline: Some(true), ..Default::default() });
        assert_eq!(f.fill, Some(TextFill::None), "{size} pt: hollow");
        let o = f.outline.expect("an outline");
        assert_eq!(o.color, Some(RED), "{size} pt");
        assert!((o.width - 0.14).abs() < TOL, "{size} pt: {} wide, want 0.14", o.width);
        assert!(f.shadow.is_none(), "{size} pt: no copy");
    }
}
