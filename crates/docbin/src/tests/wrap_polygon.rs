//! Wrap polygons of floating shapes: `pWrapPolygonVertices` (0x0383, an `IMsoArray` of
//! points in the 21600-unit wrap space) and `fEditedWrap` in the Group Shape Boolean
//! Properties (0x03BF), [MS-ODRAW] §2.3.4. Records are hand-built from the specification.

use wordcraft_doc::para::{InlineObject, Wrap};

use super::drawing::{art_content, fopt, fsp, rec, sp, spa, spa_flags};
use super::{StructSpec, plc, read, struct_doc, structured_spec};

/// `IMsoArray` of 8-byte points (cbElem 8): nElems, nElemsAlloc, cbElem, then (x, y) i32 pairs.
fn points8(pts: &[(i32, i32)]) -> Vec<u8> {
    let n = pts.len() as u16;
    let mut v = [n.to_le_bytes(), n.to_le_bytes(), 8u16.to_le_bytes()].concat();
    for (x, y) in pts {
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
    }
    v
}

/// `IMsoArray` of 4-byte points (cbElem 0xFFF0): (x, y) i16 pairs.
fn points4(pts: &[(i16, i16)]) -> Vec<u8> {
    let n = pts.len() as u16;
    let mut v = [n.to_le_bytes(), n.to_le_bytes(), 0xFFF0u16.to_le_bytes()].concat();
    for (x, y) in pts {
        v.extend_from_slice(&x.to_le_bytes());
        v.extend_from_slice(&y.to_le_bytes());
    }
    v
}

/// A document whose one paragraph anchors a floating shape (spid 1026) built from `shape`,
/// 1" × 1", with Spa wrap type `wr` (2 square, 4 tight, 5 through).
fn doc_with(shape: Vec<u8>, wr: u16) -> wordcraft_doc::Document {
    let art = art_content(&[], &[shape]);
    let spas = plc(&[0, 2], &spa(1026, [0, 0, 1440, 1440], spa_flags(2, 2, wr, false)));
    let text = "\u{8}\r";
    let ccp = text.chars().count() as u32;
    let spec = StructSpec { text, extra_pairs: vec![(40, spas), (50, art)], ..structured_spec(text, ccp) };
    read(&struct_doc(&spec)).expect("opens")
}

/// An ellipse with the given simple and complex properties.
fn ellipse(simple: &[(u16, u32)], complex: &[(u16, Vec<u8>)]) -> Vec<u8> {
    sp(&[fsp(3, 1026, 0xA00), fopt(simple, complex)])
}

fn first_float(doc: &wordcraft_doc::Document) -> wordcraft_doc::para::Float {
    let p = doc.body.first().and_then(|b| b.as_para()).expect("para");
    let obj = p.objects.first().unwrap_or_else(|| panic!("no object: {:?}", p.objects));
    obj.frame().map(|(_, _, f)| f.clone()).expect("a drawing")
}

fn polygon(doc: &wordcraft_doc::Document) -> Option<(bool, Vec<[i32; 2]>)> {
    first_float(doc).wrap_polygon.map(|p| (p.edited, p.points.clone()))
}

const DIAMOND: [(i32, i32); 5] = [(10_800, 0), (21_600, 10_800), (10_800, 21_600), (0, 10_800), (10_800, 0)];

fn diamond_points() -> Vec<[i32; 2]> {
    DIAMOND.iter().map(|&(x, y)| [x, y]).collect()
}

#[test]
fn a_tight_shape_keeps_its_wrap_polygon() {
    let doc = doc_with(ellipse(&[], &[(0x0383, points8(&DIAMOND))]), 4);
    assert_eq!(first_float(&doc).wrap, Wrap::Tight);
    assert_eq!(polygon(&doc), Some((false, diamond_points())));
}

#[test]
fn a_through_shape_keeps_its_wrap_polygon() {
    let doc = doc_with(ellipse(&[], &[(0x0383, points8(&DIAMOND))]), 5);
    assert_eq!(first_float(&doc).wrap, Wrap::Through);
    assert_eq!(polygon(&doc), Some((false, diamond_points())));
}

#[test]
fn fedited_wrap_marks_the_polygon_edited_only_with_its_fuse_bit() {
    // 0x03BF: fEditedWrap is bit 6, fUsefEditedWrap bit 22.
    let edited = doc_with(ellipse(&[(0x03BF, (1 << 22) | (1 << 6))], &[(0x0383, points8(&DIAMOND))]), 4);
    assert_eq!(polygon(&edited).map(|p| p.0), Some(true));
    let not_used = doc_with(ellipse(&[(0x03BF, 1 << 6)], &[(0x0383, points8(&DIAMOND))]), 4);
    assert_eq!(polygon(&not_used).map(|p| p.0), Some(false));
    let cleared = doc_with(ellipse(&[(0x03BF, 1 << 22)], &[(0x0383, points8(&DIAMOND))]), 4);
    assert_eq!(polygon(&cleared).map(|p| p.0), Some(false));
}

#[test]
fn short_points_are_read_too() {
    let pts: Vec<(i16, i16)> = DIAMOND.iter().map(|&(x, y)| (x as i16, y as i16)).collect();
    let doc = doc_with(ellipse(&[], &[(0x0383, points4(&pts))]), 4);
    assert_eq!(polygon(&doc), Some((false, diamond_points())));
}

#[test]
fn points_outside_the_shape_are_kept() {
    // Word's own polygons reach a little past the frame (here x = -37, as in a group whose
    // arrows end at its left edge).
    let pts = [(6_517, 0), (-37, 8_486), (6_517, 21_523), (21_600, 21_523), (21_600, 0)];
    let doc = doc_with(ellipse(&[], &[(0x0383, points8(&pts))]), 4);
    assert_eq!(polygon(&doc).map(|p| p.1), Some(pts.iter().map(|&(x, y)| [x, y]).collect()));
}

#[test]
fn square_wrapping_ignores_the_polygon() {
    let doc = doc_with(ellipse(&[], &[(0x0383, points8(&DIAMOND))]), 2);
    assert_eq!(first_float(&doc).wrap, Wrap::Square);
    assert_eq!(polygon(&doc), None);
}

#[test]
fn malformed_arrays_give_no_polygon_and_do_not_panic() {
    for data in [
        points8(&DIAMOND[..2]),                                                        // fewer than three points
        [9u16.to_le_bytes(), 9u16.to_le_bytes(), 8u16.to_le_bytes()].concat(),         // nElems with no data
        [3u16.to_le_bytes(), 3u16.to_le_bytes(), 5u16.to_le_bytes(), [0; 2]].concat(), // unknown cbElem
        vec![1, 2],                                                                    // shorter than the header
    ] {
        let doc = doc_with(ellipse(&[], &[(0x0383, data)]), 4);
        assert_eq!(polygon(&doc), None);
    }
    // nElems larger than the data: only the whole points present are read.
    let mut lying = points8(&DIAMOND);
    lying[0..2].copy_from_slice(&40_000u16.to_le_bytes());
    let doc = doc_with(ellipse(&[], &[(0x0383, lying)]), 4);
    assert_eq!(polygon(&doc), Some((false, diamond_points())));
}

#[test]
fn a_tight_group_keeps_its_wrap_polygon() {
    // A group (FSP fGroup) with one rectangle member; the polygon is on the group's own FOPT.
    let coords = |r: [i32; 4]| r.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
    let head = sp(&[fsp(0, 1026, 0x201), rec(1, 0, 0xF009, &coords([0, 0, 1440, 1440])), fopt(&[], &[(0x0383, points8(&DIAMOND))])]);
    let member = sp(&[fsp(1, 1027, 0xA02), rec(0, 0, 0xF00F, &coords([0, 0, 1440, 1440]))]);
    let group = rec(0xF, 0, 0xF003, &[head, member].concat());
    let doc = doc_with(group, 4);
    let p = doc.body.first().and_then(|b| b.as_para()).expect("para");
    assert!(matches!(p.objects.first(), Some(InlineObject::Group { .. })), "{:?}", p.objects);
    assert_eq!(polygon(&doc), Some((false, diamond_points())));
}
