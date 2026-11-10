//! An embedded object whose storage holds only `\x03ObjInfo`, `\x03PIC` and a QuickDraw `\x03PICT`
//! (Mac Word files: no `\x03EPRINT`, no `\x03META`), referenced from its `EMBED` field's RESULT by a
//! `\x01` with sprmCFOle2 and sprmCPicLocation ([MS-DOC] §2.6.1). Word on Windows doesn't draw the
//! PICT: it draws a white box with a black frame 0.72 pt thick inside its edges, at the
//! METAFILEPICT extents in `\x03PIC` (mm at byte 10, xExt/yExt HIMETRIC at 12/16), not the goal
//! size (dxaGoal/dyaGoal twips at 20/24). Measured in Word's PDF: extents 15134 x 3739 with goal
//! 5380 x 1300 twips draw a 429.07 x 105.98 pt box; 16898 x 16121 (goal 9300 x 8480) 479.11 x 457.03.

use std::io::Write;

use wordcraft_metafile::{Item, Seg};

use super::{StructSpec, grpprl, read, struct_doc, structured_spec};

const PIC_LOCATION: u16 = 0x6A03;
const F_OLE2: u16 = 0x080A;
const F_OBJ: u16 = 0x0856;
const F_SPEC: u16 = 0x0855;

const OBJ_ID: u32 = 996_496_695;

/// `EMBED` field with the object char in its result, as in the real files.
const TEXT: &str = "\u{13} EMBED Word.Picture.8 \u{14}\u{1}\u{15}\r";

/// A tiny QuickDraw PICT v1: picSize, picFrame (top, left, bottom, right, big-endian), version
/// opcode 0x11 0x01, then EndOfPicture 0xFF.
const PICT: [u8; 13] = [0x00, 0x0D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x41, 0x01, 0x0D, 0x11, 0x01, 0xFF];

const OBJ_INFO: [u8; 4] = [0x02, 0x00, 0x03, 0x00];

/// `\x03PIC` as Mac Word writes it (84 bytes): mm, extents (HIMETRIC), goal (twips), mx/my 1000.
fn pic(mm: u16, x_ext: i32, y_ext: i32, dxa: u32, dya: u32) -> Vec<u8> {
    let mut v = vec![0u8; 84];
    v[0..4].copy_from_slice(&0x0002_0054u32.to_le_bytes());
    v[10..12].copy_from_slice(&mm.to_le_bytes());
    v[12..16].copy_from_slice(&x_ext.to_le_bytes());
    v[16..20].copy_from_slice(&y_ext.to_le_bytes());
    v[20..24].copy_from_slice(&dxa.to_le_bytes());
    v[24..28].copy_from_slice(&dya.to_le_bytes());
    v[44..48].copy_from_slice(&1000u32.to_le_bytes());
    v[48..52].copy_from_slice(&1000u32.to_le_bytes());
    v
}

/// A minimal WMF (META_HEADER then META_EOF).
fn wmf() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&9u16.to_le_bytes());
    v.extend_from_slice(&0x0300u16.to_le_bytes());
    v.extend_from_slice(&12u32.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&3u32.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&3u32.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v
}

fn doc() -> Vec<u8> {
    let at = |c: char| TEXT.chars().position(|x| x == c).expect("char in text") as u32;
    let id = OBJ_ID.to_le_bytes();
    let obj = grpprl(&[(F_SPEC, &[1]), (F_OBJ, &[1]), (F_OLE2, &[1]), (PIC_LOCATION, &id)]);
    let ccp = TEXT.chars().count() as u32;
    let spec =
        StructSpec { text: TEXT, chpx_runs: vec![(0, Vec::new()), (at('\u{1}'), obj), (at('\u{15}'), Vec::new())], ..structured_spec(TEXT, ccp) };
    struct_doc(&spec)
}

fn with_storage(streams: &[(&str, &[u8])]) -> Vec<u8> {
    let mut comp = cfb::CompoundFile::open(std::io::Cursor::new(doc())).expect("cfb open");
    if !comp.is_storage("/ObjectPool") {
        comp.create_storage("/ObjectPool").expect("ObjectPool");
    }
    comp.create_storage(format!("/ObjectPool/_{OBJ_ID}")).expect("object storage");
    for (name, bytes) in streams {
        comp.create_stream(format!("/ObjectPool/_{OBJ_ID}/{name}")).expect("stream").write_all(bytes).expect("write");
    }
    comp.flush().expect("flush");
    comp.into_inner().into_inner()
}

/// The Mac Word storage: ObjInfo, PIC and PICT only.
fn pict_only(pic: &[u8]) -> Vec<u8> {
    with_storage(&[("\u{3}ObjInfo", &OBJ_INFO), ("\u{3}PIC", pic), ("\u{3}PICT", &PICT)])
}

/// Every inline image in the body: (media bytes, w, h).
fn images(bytes: &[u8]) -> Vec<(Vec<u8>, f32, f32)> {
    let doc = read(bytes).expect("opens");
    let mut out = Vec::new();
    for block in &doc.body {
        let Some(p) = block.as_para() else { continue };
        for o in &p.objects {
            if let wordcraft_doc::para::InlineObject::Image { media, w, h, .. } = o {
                out.push((doc.media.get(media).expect("media entry").to_vec(), *w, *h));
            }
        }
    }
    out
}

fn himetric_pt(v: i32) -> f32 {
    (f64::from(v) * 72.0 / 2540.0) as f32
}

fn assert_size(got: (f32, f32), want: (f32, f32), what: &str) {
    assert!((got.0 - want.0).abs() < 0.01 && (got.1 - want.1).abs() < 0.01, "{what}: got {got:?}, want {want:?}");
}

fn one_image(bytes: &[u8]) -> (Vec<u8>, f32, f32) {
    let imgs = images(bytes);
    assert_eq!(imgs.len(), 1, "the object is one inline picture");
    imgs.into_iter().next().expect("one")
}

#[test]
fn a_pict_only_object_is_a_framed_box_at_its_pic_extents() {
    let (media, w, h) = one_image(&pict_only(&pic(8, 15134, 3739, 5380, 1300)));
    assert_size((w, h), (himetric_pt(15134), himetric_pt(3739)), "HIMETRIC extents, not the 269 x 65 pt goal");
    assert_size((w, h), (429.0047, 105.9879), "Word's box");
    assert_ne!(media, PICT.to_vec(), "the PICT is not the picture");
    assert!(wordcraft_metafile::is_metafile(&media), "the placeholder is a WMF or EMF");
}

#[test]
fn the_extents_win_over_a_different_goal() {
    let (_, w, h) = one_image(&pict_only(&pic(8, 16898, 16121, 9300, 8480)));
    assert_size((w, h), (himetric_pt(16898), himetric_pt(16121)), "479.0 x 457.0 pt, not 465 x 424");
}

/// Bounding box of a path's points: [x0, y0, x1, y1].
fn bbox(segs: &[Seg]) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    let mut add = |x: f32, y: f32| {
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    };
    for s in segs {
        match *s {
            Seg::Move(x, y) | Seg::Line(x, y) => add(x, y),
            Seg::Cubic(a, b2, c, d, e, f) => {
                add(a, b2);
                add(c, d);
                add(e, f);
            }
            Seg::Close => {}
        }
    }
    b
}

fn rgb(c: [u8; 4]) -> [u8; 3] {
    [c[0], c[1], c[2]]
}

#[test]
fn the_placeholder_is_a_white_box_with_a_black_frame_inside_its_edges() {
    let (media, w, h) = one_image(&pict_only(&pic(8, 15134, 3739, 5380, 1300)));
    let pic = wordcraft_metafile::parse(&media).expect("the placeholder parses");
    assert!(pic.width > 0.0 && pic.height > 0.0);
    // Box points per picture unit.
    let (sx, sy) = (w / pic.width, h / pic.height);
    let tol = 0.5;
    let covers_all = |b: [f32; 4]| b[0] * sx <= tol && b[1] * sy <= tol && b[2] * sx >= w - tol && b[3] * sy >= h - tol;

    let mut white_at = None;
    let mut black: Vec<(usize, [f32; 4], f32, bool)> = Vec::new(); // (index, ink bbox, thickness pt, filled)
    for (i, item) in pic.items.iter().enumerate() {
        let Item::Path { segs, fill, stroke, .. } = item else { continue };
        let b = bbox(segs);
        if fill.is_some_and(|c| rgb(c) == [255, 255, 255] && c[3] == 255) && covers_all(b) && white_at.is_none() {
            white_at = Some(i);
        }
        if fill.is_some_and(|c| rgb(c) == [0, 0, 0] && c[3] > 0) {
            let t = ((b[2] - b[0]) * sx).min((b[3] - b[1]) * sy);
            black.push((i, b, t, true));
        }
        if let Some((_, sw)) = stroke.filter(|(c, _)| rgb(*c) == [0, 0, 0] && c[3] > 0) {
            let half = sw / 2.0;
            black.push((i, [b[0] - half, b[1] - half, b[2] + half, b[3] + half], sw * sx, false));
        }
    }
    let white_at = white_at.expect("a white filled path covers the whole box");
    assert!(!black.is_empty(), "a black frame is drawn");
    assert!(black.iter().all(|&(i, ..)| i > white_at), "the frame is drawn over the white fill");
    let union =
        black.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |u, (_, b, ..)| [u[0].min(b[0]), u[1].min(b[1]), u[2].max(b[2]), u[3].max(b[3])]);
    assert!(covers_all(union), "the frame reaches all four edges: {union:?} of {} x {}", pic.width, pic.height);
    assert!(
        union[0] * sx >= -tol && union[1] * sy >= -tol && union[2] * sx <= w + tol && union[3] * sy <= h + tol,
        "the frame stays inside the box: {union:?}"
    );
    let (cx, cy) = (pic.width / 2.0, pic.height / 2.0);
    assert!(black.iter().filter(|b| b.3).all(|(_, b, ..)| !(b[0] < cx && cx < b[2] && b[1] < cy && cy < b[3])), "the inside stays white");
    for (_, _, t, _) in &black {
        assert!((t - 0.72).abs() <= 0.15, "the frame is 0.72 pt thick, got {t}");
    }
}

#[test]
fn without_usable_extents_the_goal_size_is_used() {
    for (mm, x) in [(8u16, 0i32), (1, 15134)] {
        let (_, w, h) = one_image(&pict_only(&pic(mm, x, 3739, 5380, 1300)));
        assert_size((w, h), (269.0, 65.0), &format!("mm {mm}, xExt {x}: the goal size"));
    }
}

#[test]
fn hostile_extents_fall_back_to_the_goal() {
    for x in [i32::MAX, -15134] {
        let (_, w, h) = one_image(&pict_only(&pic(8, x, 3739, 5380, 1300)));
        assert_size((w, h), (269.0, 65.0), &format!("xExt {x}"));
    }
}

#[test]
fn a_meta_object_with_a_pict_too_still_draws_its_wmf_at_the_goal_size() {
    let w = wmf();
    let mut meta = Vec::new();
    for x in [8i16, 1746, 370, 0] {
        meta.extend_from_slice(&x.to_le_bytes());
    }
    meta.extend_from_slice(&w);
    let pic = pic(8, 1746, 370, 1320, 280);
    let bytes = with_storage(&[("\u{3}ObjInfo", &OBJ_INFO), ("\u{3}PIC", &pic), ("\u{3}META", &meta), ("\u{3}PICT", &PICT)]);
    let (media, width, height) = one_image(&bytes);
    assert_eq!(media, w, "the META metafile, as before");
    assert_size((width, height), (66.0, 14.0), "the goal size, as before");
}

#[test]
fn without_a_pict_or_without_a_pic_nothing_is_drawn() {
    let pic = pic(8, 15134, 3739, 5380, 1300);
    let no_pict = with_storage(&[("\u{3}ObjInfo", &OBJ_INFO), ("\u{3}PIC", &pic)]);
    assert!(images(&no_pict).is_empty(), "no presentation at all: nothing invented");
    let no_pic = with_storage(&[("\u{3}ObjInfo", &OBJ_INFO), ("\u{3}PICT", &PICT)]);
    assert!(images(&no_pic).is_empty(), "no size: nothing invented");
}
