//! The size of an inline shape ([MS-DOC] §2.9.190 PICF, `mfpf.mm` = MM_SHAPE) whose FOPT
//! carries crop fractions ([MS-ODRAW] §2.3.23.2-5, 16.16 fixed, may be negative). A drawing
//! placeholder (the result of a `SHAPE` field) has no picture, and its writer often stores
//! cropFromTop = -0.99976 with cropFromBottom = +0.99976: the pair moves the absent picture
//! and must not shrink the box, which stays goal x scale. With a picture, the size takes the
//! signed crop sum off the goal size.

use wordcraft_doc::para::{InlineObject, Wrap};

use super::drawing::{fbse_png, fopt, fsp, rec, sp, tiny_png};
use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_SPEC: u16 = 0x0855;
const MM_SHAPE: u16 = 0x0064;
const CROP_TOP: u16 = 0x0100;
const CROP_BOTTOM: u16 = 0x0101;
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
fn a_cropped_picture_still_loses_its_cropped_share() {
    // A quarter off the top and the bottom: 100 pt x (1 - 0.25 - 0.25) = 50 pt, drawn cropped.
    let doc = doc_with(picf((4000, 2000), (1000, 1000), &picture(QUARTER, QUARTER)));
    let got = inline_sizes(&doc);
    assert!(close(&got, (200.0, 50.0)), "{got:?}");
    let Some(InlineObject::Image { crop, .. }) = first_para(&doc).objects.first() else {
        panic!("no image: {:?}", first_para(&doc).objects)
    };
    assert_eq!(*crop, [0.0, 0.25, 0.0, 0.25]);
    assert_eq!(doc.media.len(), 1);
}

#[test]
fn a_picture_with_the_opposite_crop_pair_keeps_its_goal_height() {
    // The crops are signed: 1 - (-0.99976) - 0.99976 = 1, so the picture is 100 pt tall.
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
