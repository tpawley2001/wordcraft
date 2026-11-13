//! Drawings placed in line with text in a Word 2002+ `.doc`: a `SHAPE` field whose result holds a
//! drawn-object anchor (0x08, an FSPA in `PlcfSpaMom`) and, for older Word versions, a picture
//! (0x01) of the same size. Word shows one inline object, the drawn one, with room on its line
//! for its outline and shadow. The OfficeArt records are built here from [MS-ODRAW].

use wordcraft_doc::para::{InlineObject, Wrap};

use super::drawing::{art_content, fbse_png, fopt, fsp, rec, sp, spa, spa_flags, tiny_png};
use super::{StructSpec, grpprl, plc, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_SPEC: u16 = 0x0855;
const MM_SHAPE: u16 = 0x0064;
/// Line Style Boolean Properties: `fLine` on / off, with its use bit.
const LINE_ON: (u16, u32) = (0x01FF, 0x0008_0008);
const LINE_OFF: (u16, u32) = (0x01FF, 0x0008_0000);
/// Shadow Style Boolean Properties: `fShadow` on, with its use bit.
const SHADOW_ON: (u16, u32) = (0x023F, 0x0002_0002);

/// A PICF with `mm` and goal size (twips) at 100 %, then `rest`.
fn picf(mm: u16, goal: (i16, i16), rest: &[u8]) -> Vec<u8> {
    let mut picf = vec![0u8; 0x44];
    picf[0..4].copy_from_slice(&((0x44 + rest.len()) as u32).to_le_bytes());
    picf[4..6].copy_from_slice(&0x44u16.to_le_bytes());
    picf[6..8].copy_from_slice(&mm.to_le_bytes());
    picf[28..30].copy_from_slice(&goal.0.to_le_bytes());
    picf[30..32].copy_from_slice(&goal.1.to_le_bytes());
    picf[32..34].copy_from_slice(&1000u16.to_le_bytes());
    picf[34..36].copy_from_slice(&1000u16.to_le_bytes());
    [picf.as_slice(), rest].concat()
}

/// The fallback picture: an inline picture frame with no BLIP (an empty shape).
fn empty_fallback(goal: (i16, i16)) -> Vec<u8> {
    let frame = sp(&[fsp(75, 1, 0xA00), fopt(&[LINE_OFF], &[]), rec(0, 0, 0xF010, &[0, 0, 0, 0x80])]);
    picf(MM_SHAPE, goal, &frame)
}

/// A picture shape `spid` (BLIP 1) with `props`, placed relative to character and line as Word
/// stores an inline drawing.
fn picture(spid: u32, props: &[(u16, u32)]) -> Vec<u8> {
    let mut all = vec![(0x0104, 1)];
    all.extend_from_slice(props);
    let tertiary = {
        let mut b = Vec::new();
        for (pid, op) in [(0x0390u16, 3u32), (0x0392, 3)] {
            b.extend_from_slice(&pid.to_le_bytes());
            b.extend_from_slice(&op.to_le_bytes());
        }
        rec(3, 2, 0xF122, &b)
    };
    sp(&[fsp(75, spid, 0xA00), fopt(&all, &[]), tertiary])
}

/// A document of `text` whose 0x08 characters at `anchors` (CP, spid, size in twips) name the
/// `shapes`, and whose 0x01 characters show `data` (one fallback picture at Data offset 0).
fn doc(text: &'static str, anchors: &[(u32, u32, (i32, i32))], shapes: &[Vec<u8>], data: Vec<u8>) -> wordcraft_doc::Document {
    let ccp = text.chars().count() as u32;
    let mut cps: Vec<u32> = anchors.iter().map(|a| a.0).collect();
    cps.push(ccp - 1);
    let spas: Vec<u8> = anchors.iter().flat_map(|&(_, spid, (w, h))| spa(spid, [0, 0, w, h], spa_flags(2, 2, 3, false))).collect();
    let mut extra = vec![(50, art_content(&[fbse_png(&tiny_png())], shapes))];
    if !anchors.is_empty() {
        extra.push((40, plc(&cps, &spas)));
    }
    let spec = StructSpec {
        text,
        data_stream: data,
        chpx_runs: vec![(0, grpprl(&[(PIC_LOCATION, &0u32.to_le_bytes()), (F_SPEC, &[1])]))],
        extra_pairs: extra,
        ..structured_spec(text, ccp)
    };
    read(&struct_doc(&spec)).expect("opens")
}

/// "a", { SHAPE | 0x08 0x01 }, "b": the anchor is CP 10, the fallback CP 11.
const ONE: &str = "a\u{13} SHAPE \u{14}\u{8}\u{1}\u{15}b\r";
/// 209.2 x 156.85 pt.
const SIZE: (i32, i32) = (4184, 3137);

fn one(props: &[(u16, u32)]) -> wordcraft_doc::Document {
    doc(ONE, &[(10, 1025, SIZE)], &[picture(1025, props)], empty_fallback((4184, 3137)))
}

fn objects(doc: &wordcraft_doc::Document) -> &[InlineObject] {
    &doc.body.first().and_then(|b| b.as_para()).expect("para").objects
}

/// The drawings of the first paragraph (fields and markers left out).
fn drawings(doc: &wordcraft_doc::Document) -> Vec<&InlineObject> {
    objects(doc).iter().filter(|o| o.frame().is_some()).collect()
}

/// The effect extent of the only drawing, which must be an inline picture.
fn only_inline_effect(doc: &wordcraft_doc::Document) -> [f32; 4] {
    let d = drawings(doc);
    assert_eq!(d.len(), 1, "{:?}", objects(doc));
    let Some(InlineObject::Image { float, .. }) = d.first() else { panic!("a picture: {:?}", objects(doc)) };
    assert_eq!(float.wrap, Wrap::Inline);
    float.effect
}

fn near(got: [f32; 4], want: [f32; 4]) -> bool {
    got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 0.01)
}

#[test]
fn a_picture_in_a_shape_field_is_one_inline_object() {
    let doc = one(&[LINE_OFF]);
    let d = drawings(&doc);
    assert_eq!(d.len(), 1, "{:?}", objects(&doc));
    let Some(InlineObject::Image { w, h, float, .. }) = d.first() else { panic!("a picture: {:?}", objects(&doc)) };
    assert_eq!(float.wrap, Wrap::Inline);
    assert!((w - 209.2).abs() <= 0.05 && (h - 156.85).abs() <= 0.05, "{w} x {h}");
    assert_eq!((float.x, float.y), (0.0, 0.0));
    assert_eq!(float.effect, [0.0; 4]);
    assert!(!objects(&doc).iter().any(|o| matches!(o, InlineObject::Shape { .. })), "no empty inline shape: {:?}", objects(&doc));
    let text = &doc.body.first().and_then(|b| b.as_para()).expect("para").text;
    assert!(text.starts_with("a\u{FFFC}") && text.ends_with('b'), "{text:?}");
}

#[test]
fn outline_and_shadow_give_the_reach() {
    // Line on at the default 0.75 pt; shadow 6 pt right and 6 pt up.
    let doc = one(&[LINE_ON, SHADOW_ON, (0x0205, 76_200), (0x0206, (-76_200i32) as u32)]);
    let got = only_inline_effect(&doc);
    assert!(near(got, [0.75, 7.5, 7.5, 0.75]), "{got:?}");
}

#[test]
fn the_line_width_counts() {
    let doc = one(&[LINE_ON, (0x01CB, 38_100)]);
    let got = only_inline_effect(&doc);
    assert!(near(got, [3.0; 4]), "{got:?}");
}

#[test]
fn a_shadow_down_and_left() {
    let doc = one(&[LINE_OFF, SHADOW_ON, (0x0205, (-114_300i32) as u32), (0x0206, 38_100)]);
    let got = only_inline_effect(&doc);
    assert!(near(got, [9.0, 0.0, 0.0, 3.0]), "{got:?}");
}

#[test]
fn a_shadow_switched_off_takes_no_room() {
    let doc = one(&[LINE_OFF, (0x023F, 0x0002_0000), (0x0205, 76_200), (0x0206, 76_200)]);
    assert_eq!(only_inline_effect(&doc), [0.0; 4]);
}

#[test]
fn two_on_a_line_follow_each_other() {
    // { SHAPE | 0x08 0x01 } " " { SHAPE | 0x08 0x01 }: anchors at CPs 9 and 22.
    let text = "\u{13} SHAPE \u{14}\u{8}\u{1}\u{15} \u{13} SHAPE \u{14}\u{8}\u{1}\u{15}\r";
    let doc = doc(
        text,
        &[(9, 1025, (4560, 3444)), (22, 1026, (4495, 3960))],
        &[picture(1025, &[LINE_OFF]), picture(1026, &[LINE_OFF])],
        empty_fallback((4560, 3444)),
    );
    let d = drawings(&doc);
    assert_eq!(d.len(), 2, "{:?}", objects(&doc));
    for o in &d {
        assert!(matches!(o, InlineObject::Image { float, .. } if float.wrap == Wrap::Inline), "{o:?}");
    }
    assert_eq!(d.iter().map(|o| o.frame().map(|f| f.0)).collect::<Vec<_>>(), [Some(228.0), Some(224.75)]);
    // Picture, space, picture: the space sits between the two anchors' U+FFFC.
    let p = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let shown: String = p.text.chars().filter(|c| *c != '\u{FFFC}').collect();
    assert_eq!(shown, " ");
    let first = p.text.find('\u{FFFC}').expect("an object");
    let space = p.text.find(' ').expect("the space");
    let last = p.text.rfind('\u{FFFC}').expect("an object");
    assert!(first < space && space < last, "{:?}", p.text);
}

#[test]
fn an_anchor_outside_the_field_still_floats() {
    // 0x08 at CP 0 (floating), then { SHAPE | 0x08 0x01 } with its anchor at CP 10.
    let text = "\u{8}\u{13} SHAPE \u{14}\u{8}\u{1}\u{15}\r";
    let doc = doc(
        text,
        &[(0, 1025, SIZE), (10, 1026, SIZE)],
        &[picture(1025, &[LINE_OFF]), picture(1026, &[LINE_OFF])],
        empty_fallback((4184, 3137)),
    );
    let d = drawings(&doc);
    assert_eq!(d.len(), 2, "{:?}", objects(&doc));
    let wraps: Vec<Wrap> = d.iter().filter_map(|o| o.frame().map(|f| f.2.wrap)).collect();
    assert_eq!(wraps, [Wrap::InFrontOfText, Wrap::Inline]);
    let Some((_, _, float)) = d.first().and_then(|o| o.frame()) else { panic!() };
    assert_eq!((float.h_rel, float.v_rel), (wordcraft_doc::para::Anchor::Character, wordcraft_doc::para::Anchor::Line));
}

#[test]
fn an_anchor_with_no_shape_keeps_the_fallback() {
    // No FSPA for the anchor: the empty inline shape 100 x 50 pt stays, as today.
    let doc = doc(ONE, &[], &[], empty_fallback((2000, 1000)));
    let d = drawings(&doc);
    assert_eq!(d.len(), 1, "{:?}", objects(&doc));
    let Some(InlineObject::Shape { w, h, float, .. }) = d.first() else { panic!("an empty shape: {:?}", objects(&doc)) };
    assert_eq!((*w, *h, float.wrap), (100.0, 50.0, Wrap::Inline));
}

#[test]
fn an_anchor_naming_an_unknown_shape_keeps_the_fallback() {
    let doc = doc(ONE, &[(10, 4242, SIZE)], &[picture(1025, &[LINE_OFF])], empty_fallback((2000, 1000)));
    let d = drawings(&doc);
    assert_eq!(d.len(), 1, "{:?}", objects(&doc));
    assert!(matches!(d.first(), Some(InlineObject::Shape { w, .. }) if *w == 100.0), "{:?}", objects(&doc));
}

#[test]
fn hostile_values_keep_the_reach_finite_and_bounded() {
    for (dx, dy) in [(0x7FFF_FFFFu32, 0x8000_0000u32), (0x8000_0000, 0x7FFF_FFFF)] {
        let doc = one(&[LINE_ON, (0x01CB, 0xFFFF_FFFF), SHADOW_ON, (0x0205, dx), (0x0206, dy)]);
        let got = only_inline_effect(&doc);
        assert!(got.iter().all(|v| v.is_finite() && (0.0..=1584.0).contains(v)), "{got:?}");
    }
}
