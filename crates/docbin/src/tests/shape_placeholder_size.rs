//! The size of an inline shape ([MS-DOC] §2.9.190 PICF, `mfpf.mm` = MM_SHAPE) whose FOPT
//! carries crop fractions ([MS-ODRAW] §2.3.23.2-5, 16.16 fixed, may be negative). A drawing
//! placeholder (the result of a `SHAPE` field) has no picture, and its writer often stores
//! cropFromTop = -0.99976 with cropFromBottom = +0.99976: the pair moves the absent picture
//! and must not shrink the box, which stays goal x scale. A picture's box is goal x scale too
//! ([MS-DOC] §2.9.191 PICMID: `mx` / `my` are the ratio of the final display size to the goal
//! size); its crop only chooses the part of the bitmap that is drawn.

use wordcraft_doc::para::{InlineObject, Wrap};

use super::drawing::{fbse_png, fopt, fsp, rec, sp, tiny_png};
use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_SPEC: u16 = 0x0855;
const MM_SHAPE: u16 = 0x0064;
const CROP_TOP: u16 = 0x0100;
const CROP_BOTTOM: u16 = 0x0101;
const CROP_LEFT: u16 = 0x0102;
const CROP_RIGHT: u16 = 0x0103;
const PIB: u16 = 0x0104;
const F_LINE_OFF: (u16, u32) = (0x01FF, 0x0008_0000);
/// -0.99976 and +0.99976 as 16.16 fixed: what the placeholders store.
const UP: u32 = 0xFFFF_0010;
const DOWN: u32 = 0x0000_FFF0;
/// A quarter, as 16.16 fixed.
const QUARTER: u32 = 0x0000_4000;

/// A PICF with `MM_SHAPE`, goal size (twips) and scale (1000 = 100 %), then `rest`.
fn picf(goal: (i16, i16), scale: (u16, u16), rest: &[u8]) -> Vec<u8> {
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + rest.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[6..8].copy_from_slice(&MM_SHAPE.to_le_bytes());
    picf[28..30].copy_from_slice(&goal.0.to_le_bytes());
    picf[30..32].copy_from_slice(&goal.1.to_le_bytes());
    picf[32..34].copy_from_slice(&scale.0.to_le_bytes());
    picf[34..36].copy_from_slice(&scale.1.to_le_bytes());
    [picf.as_slice(), rest].concat()
}

/// A picture frame (shape type 75) with no picture: only the crops (when given) and `fLine` off.
fn placeholder(crop: Option<(u32, u32)>) -> Vec<u8> {
    let mut props = Vec::new();
    if let Some((top, bottom)) = crop {
        props.push((CROP_TOP, top));
        props.push((CROP_BOTTOM, bottom));
    }
    props.push(F_LINE_OFF);
    sp(&[fsp(75, 1025, 0xA00), fopt(&props, &[]), rec(0, 0, 0xF010, &[0, 0, 0, 0x80])])
}

/// A picture frame whose `pib` names the PNG in the FBSE that follows it, with the crops.
fn picture(top: u32, bottom: u32) -> Vec<u8> {
    let shape = sp(&[fsp(75, 1025, 0xA00), fopt(&[(CROP_TOP, top), (CROP_BOTTOM, bottom), (PIB, 1)], &[])]);
    [shape, fbse_png(&tiny_png())].concat()
}

/// A picture frame with its PNG and the four crops `[left, top, right, bottom]` as fractions.
fn cropped_picture(crop: [f64; 4]) -> Vec<u8> {
    let fixed = |f: f64| (f * 65536.0).round() as i32 as u32;
    let [left, top, right, bottom] = crop.map(fixed);
    let props = [(CROP_TOP, top), (CROP_BOTTOM, bottom), (CROP_LEFT, left), (CROP_RIGHT, right), (PIB, 1)];
    let shape = sp(&[fsp(75, 1025, 0xA00), fopt(&props, &[])]);
    [shape, fbse_png(&tiny_png())].concat()
}

/// The inline box and the drawn crop of a cropped picture with this goal (twips) and scale.
fn cropped_size(goal: (i16, i16), scale: (u16, u16), crop: [f64; 4]) -> (Vec<(f32, f32)>, [f32; 4]) {
    let doc = doc_with(picf(goal, scale, &cropped_picture(crop)));
    let Some(InlineObject::Image { crop: drawn, .. }) = first_para(&doc).objects.first() else {
        panic!("no image: {:?}", first_para(&doc).objects)
    };
    assert_eq!(doc.media.len(), 1);
    (inline_sizes(&doc), *drawn)
}

/// The drawn crop is the stored one (16.16 fixed, so within a ten-thousandth).
fn same_crop(drawn: [f32; 4], stored: [f64; 4]) -> bool {
    drawn.iter().zip(stored).all(|(d, s)| (f64::from(*d) - s).abs() <= 1e-4)
}

/// `text` with every picture character pointing at the start of `data`.
fn doc_of(text: &'static str, data: Vec<u8>) -> wordcraft_doc::Document {
    let ccp = text.chars().count() as u32;
    let spec = StructSpec {
        text,
        data_stream: data,
        chpx_runs: vec![(0, grpprl(&[(PIC_LOCATION, &0u32.to_le_bytes()), (F_SPEC, &[1])]))],
        ..structured_spec(text, ccp)
    };
    read(&struct_doc(&spec)).expect("opens")
}

/// "a", the picture, a line break, "After".
fn doc_with(data: Vec<u8>) -> wordcraft_doc::Document {
    doc_of("a\u{1}\u{b}After\r", data)
}

fn first_para(doc: &wordcraft_doc::Document) -> &wordcraft_doc::para::Paragraph {
    doc.body.first().and_then(|b| b.as_para()).expect("para")
}

/// The sizes of the first paragraph's inline (not floating) objects.
fn inline_sizes(doc: &wordcraft_doc::Document) -> Vec<(f32, f32)> {
    first_para(doc).objects.iter().filter_map(|o| o.frame()).filter(|(_, _, f)| f.wrap == Wrap::Inline).map(|(w, h, _)| (w, h)).collect()
}

fn close(got: &[(f32, f32)], want: (f32, f32)) -> bool {
    got.len() == 1 && (got[0].0 - want.0).abs() <= 0.05 && (got[0].1 - want.1).abs() <= 0.05
}

/// The box of a placeholder (no picture) with this goal, scale and crop pair.
fn placeholder_size(goal: (i16, i16), scale: (u16, u16), crop: Option<(u32, u32)>) -> Vec<(f32, f32)> {
    let doc = doc_with(picf(goal, scale, &placeholder(crop)));
    assert!(doc.media.is_empty(), "no image bytes are invented");
    inline_sizes(&doc)
}

#[test]
fn a_placeholder_with_the_opposite_crop_pair_keeps_its_full_height() {
    // 13200 x 345 / 1000 / 20 = 227.7; 4080 x 844 / 1000 / 20 = 172.176.
    let got = placeholder_size((13200, 4080), (345, 844), Some((UP, DOWN)));
    assert!(close(&got, (227.7, 172.176)), "{got:?}");
}

#[test]
fn a_second_placeholder_with_the_opposite_crop_pair_keeps_its_full_height() {
    // 13200 x 341 / 1000 / 20 = 225.06; 4440 x 892 / 1000 / 20 = 198.024.
    let got = placeholder_size((13200, 4440), (341, 892), Some((UP, DOWN)));
    assert!(close(&got, (225.06, 198.024)), "{got:?}");
}

#[test]
fn an_enlarged_placeholder_with_the_opposite_crop_pair_keeps_its_full_height() {
    // 6120 x 1333 / 1000 / 20 = 407.898; 2880 x 1529 / 1000 / 20 = 220.176.
    let got = placeholder_size((6120, 2880), (1333, 1529), Some((UP, DOWN)));
    assert!(close(&got, (407.898, 220.176)), "{got:?}");
}

#[test]
fn a_placeholder_without_crop_keeps_its_goal_size() {
    let got = placeholder_size((4680, 2880), (1000, 1000), None);
    assert!(close(&got, (234.0, 144.0)), "{got:?}");
}

#[test]
fn a_placeholder_ignores_a_positive_crop_because_it_has_no_picture() {
    let got = placeholder_size((4000, 2000), (1000, 1000), Some((QUARTER, QUARTER)));
    assert!(close(&got, (200.0, 100.0)), "{got:?}");
}

#[test]
fn a_cropped_picture_keeps_its_scaled_goal_size() {
    // A quarter off the top and the bottom: the box stays 200 x 100 pt, the picture is drawn cropped.
    let doc = doc_with(picf((4000, 2000), (1000, 1000), &picture(QUARTER, QUARTER)));
    let got = inline_sizes(&doc);
    assert!(close(&got, (200.0, 100.0)), "{got:?}");
    let Some(InlineObject::Image { crop, .. }) = first_para(&doc).objects.first() else {
        panic!("no image: {:?}", first_para(&doc).objects)
    };
    assert_eq!(*crop, [0.0, 0.25, 0.0, 0.25]);
    assert_eq!(doc.media.len(), 1);
}

#[test]
fn a_picture_with_the_opposite_crop_pair_keeps_its_goal_height() {
    // The crop pair moves the picture inside its box; the box is the goal size, 100 pt tall.
    let doc = doc_with(picf((4000, 2000), (1000, 1000), &picture(UP, DOWN)));
    let got = inline_sizes(&doc);
    assert!(close(&got, (200.0, 100.0)), "{got:?}");
    assert!(matches!(first_para(&doc).objects.first(), Some(InlineObject::Image { .. })), "{:?}", first_para(&doc).objects);
}

#[test]
fn a_placeholder_with_extreme_crops_keeps_its_goal_size() {
    let got = placeholder_size((4000, 2000), (1000, 1000), Some((0x7FFF_FFFF, 0x8000_0000)));
    assert!(close(&got, (200.0, 100.0)), "{got:?}");
    let got = placeholder_size((4000, 2000), (1000, 1000), Some((0x8000_0000, 0x7FFF_FFFF)));
    assert!(close(&got, (200.0, 100.0)), "{got:?}");
}

#[test]
fn a_picture_with_extreme_crops_stays_within_the_size_bounds() {
    for (top, bottom) in [(0x7FFF_FFFFu32, 0x8000_0000u32), (0x8000_0000, 0x7FFF_FFFF), (0x7FFF_FFFF, 0x7FFF_FFFF), (0x8000_0000, 0x8000_0000)] {
        let doc = doc_with(picf((4000, 2000), (1000, 1000), &picture(top, bottom)));
        let got = inline_sizes(&doc);
        assert_eq!(got.len(), 1, "{top:#x} {bottom:#x}: {:?}", first_para(&doc).objects);
        let (w, h) = got[0];
        assert!((1.0..=1584.0).contains(&w) && (1.0..=1584.0).contains(&h), "{top:#x} {bottom:#x}: {w} x {h}");
        let Some(InlineObject::Image { crop, .. }) = first_para(&doc).objects.first() else {
            panic!("no image: {:?}", first_para(&doc).objects)
        };
        assert!(crop.iter().all(|c| (0.0..=0.99).contains(c)), "{top:#x} {bottom:#x}: {crop:?}");
    }
}

#[test]
fn a_placeholder_in_a_shape_field_result_keeps_its_full_height_and_the_field() {
    // "a", { SHAPE \* MERGEFORMAT | picture }, line break, "After".
    let text = "a\u{13} SHAPE  \\* MERGEFORMAT \u{14}\u{1}\u{15}\u{b}After\r";
    let doc = doc_of(text, picf((13200, 4080), (345, 844), &placeholder(Some((UP, DOWN)))));
    let got = inline_sizes(&doc);
    assert!(close(&got, (227.7, 172.176)), "{got:?} {:?}", first_para(&doc).objects);
    let fields: Vec<&str> =
        first_para(&doc).objects.iter().filter_map(|o| if let InlineObject::Field { instr, .. } = o { Some(instr.as_str()) } else { None }).collect();
    assert_eq!(fields.len(), 1, "{:?}", first_para(&doc).objects);
    assert!(fields[0].trim_start().starts_with("SHAPE"), "{fields:?}");
}

// The boxes below are goal x scale, as measured in Word's output for pictures cropped on one,
// two, three and four sides: the crop never changes the box.

#[test]
fn a_picture_cropped_on_three_sides_is_its_goal_size_times_its_scale() {
    let crop = [0.0, 0.0628, 0.3656, 0.1228];
    let (got, drawn) = cropped_size((20480, 15360), (190, 243), crop);
    assert!(close(&got, (194.56, 186.62)), "{got:?}");
    assert!(same_crop(drawn, crop), "{drawn:?}");
}

#[test]
fn a_picture_cropped_at_the_top_is_its_goal_size_times_its_scale() {
    let crop = [0.0, 0.137, 0.0, 0.0];
    let (got, drawn) = cropped_size((10368, 8640), (550, 476), crop);
    assert!(close(&got, (285.12, 205.63)), "{got:?}");
    assert!(same_crop(drawn, crop), "{drawn:?}");
}

#[test]
fn a_picture_cropped_to_under_half_is_its_goal_size_times_its_scale() {
    let crop = [0.0, 0.4625, 0.5688, 0.1263];
    let (got, drawn) = cropped_size((8640, 8640), (347, 332), crop);
    assert!(close(&got, (149.90, 143.42)), "{got:?}");
    assert!(same_crop(drawn, crop), "{drawn:?}");
}

#[test]
fn a_picture_cropped_at_the_top_and_bottom_is_its_goal_size_times_its_scale() {
    let crop = [0.0, 0.1146, 0.0, 0.2812];
    let (got, drawn) = cropped_size((7680, 5760), (800, 483), crop);
    assert!(close(&got, (307.20, 139.10)), "{got:?}");
    assert!(same_crop(drawn, crop), "{drawn:?}");
}

#[test]
fn a_picture_with_its_right_half_cropped_keeps_its_width() {
    let (got, drawn) = cropped_size((2000, 1000), (500, 500), [0.0, 0.0, 0.5, 0.0]);
    assert!(close(&got, (50.0, 25.0)), "{got:?}");
    assert_eq!(drawn, [0.0, 0.0, 0.5, 0.0]);
}

#[test]
fn a_picture_with_a_negative_crop_keeps_its_goal_size() {
    let (got, drawn) = cropped_size((4000, 2000), (1000, 1000), [0.0, -0.25, 0.0, 0.0]);
    assert!(close(&got, (200.0, 100.0)), "{got:?}");
    assert_eq!(drawn, [0.0; 4]);
}

// PICMID crop fields (PICF offsets 36, 38, 40, 42: left, top, right, bottom in twips of the goal, signed).
// Older Word versions store the crop there; when any is nonzero Word's box is the goal minus those crops,
// times the scale (measured in Microsoft 365 Word's PDF of a Word 97-2003 file).

/// `picf` with the four PICMID crop fields `[left, top, right, bottom]` (twips) set.
fn picf_with_picmid_crop(goal: (i16, i16), scale: (u16, u16), crop: [i16; 4], rest: &[u8]) -> Vec<u8> {
    let mut out = picf(goal, scale, rest);
    for (i, c) in crop.iter().enumerate() {
        out[36 + 2 * i..38 + 2 * i].copy_from_slice(&c.to_le_bytes());
    }
    out
}

#[test]
fn a_picture_with_picmid_crop_fields_is_sized_from_the_cropped_goal() {
    let fopt = [566.0 / 20475.0, 1700.0 / 15360.0, 1133.0 / 20475.0, 2267.0 / 15360.0];
    let data = picf_with_picmid_crop((20475, 15360), (240, 218), [566, 1700, 1133, 2267], &cropped_picture(fopt));
    let doc = doc_with(data);
    let got = inline_sizes(&doc);
    // (20475 - 566 - 1133) x 0.240 / 20 = 225.312; (15360 - 1700 - 2267) x 0.218 / 20 = 124.184.
    assert!(close(&got, (225.312, 124.184)), "{got:?}");
    let Some(InlineObject::Image { crop: drawn, .. }) = first_para(&doc).objects.first() else {
        panic!("no image: {:?}", first_para(&doc).objects)
    };
    assert!(same_crop(*drawn, fopt), "{drawn:?}");
}

#[test]
fn picmid_crop_fields_size_the_box_even_without_fopt_crops() {
    // A negative left crop adds 1000 twips: (20475 + 1000) / 20 = 1073.75; height 15360 / 20 = 768.
    let data = picf_with_picmid_crop((20475, 15360), (1000, 1000), [-1000, 0, 0, 0], &cropped_picture([0.0; 4]));
    let got = inline_sizes(&doc_with(data));
    assert!(close(&got, (1073.75, 768.0)), "{got:?}");
}

#[test]
fn picmid_crops_larger_than_the_goal_clamp_the_box() {
    let data = picf_with_picmid_crop((20475, 15360), (1000, 1000), [15000, 0, 15000, 0], &cropped_picture([0.0; 4]));
    let got = inline_sizes(&doc_with(data));
    assert!(close(&got, (1.0, 768.0)), "{got:?}");
}
