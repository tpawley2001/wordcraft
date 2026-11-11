//! Lock aspect ratio is a property of each drawing (`Float::lock_aspect`), as in Word: an inserted
//! picture is locked, an inserted shape isn't; `picture.size` with one side keeps the proportions
//! only when the object is locked, and `aspectLocked` stores the setting (the Size dialog's box).
use super::*;

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 30, 255]));
    let mut b = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
    b
}

fn lock(s: &Session) -> Option<bool> {
    let (_, o) = selected(s).unwrap();
    o.frame().unwrap().2.lock_aspect
}

fn size(s: &Session) -> (f32, f32) {
    let (_, o) = selected(s).unwrap();
    obj_size(&o)
}

fn picture() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    let data = super::super::insert::base64_encode(&png(20, 10));
    s.run("insert.picture", &json!({"data": data})).unwrap();
    s
}

fn rectangle() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("insert.shape", &json!({"kind": "rectangle"})).unwrap();
    s.run("picture.size", &json!({"width": 100, "height": 50})).unwrap();
    s
}

#[test]
fn an_inserted_picture_is_locked() {
    let s = picture();
    assert_eq!(lock(&s), Some(true));
}

#[test]
fn an_unlocked_shape_changes_one_side() {
    let mut s = rectangle();
    assert!(!lock(&s).unwrap_or(false), "a shape starts unlocked");
    assert_eq!(size(&s), (100.0, 50.0));
    s.run("picture.size", &json!({"width": 200})).unwrap();
    assert_eq!(size(&s), (200.0, 50.0), "unlocked: only the width");
}

#[test]
fn a_locked_shape_keeps_its_proportions() {
    let mut s = rectangle();
    s.run("picture.size", &json!({"aspectLocked": true})).unwrap();
    assert_eq!(lock(&s), Some(true));
    assert_eq!(size(&s), (100.0, 50.0), "storing the lock alone doesn't resize");
    s.run("picture.size", &json!({"width": 200})).unwrap();
    assert_eq!(size(&s), (200.0, 100.0), "locked: the height follows");
}

#[test]
fn unlocking_a_picture_is_one_undo_step_with_its_size() {
    let mut s = picture();
    let start = size(&s);
    s.run("picture.size", &json!({"width": start.0 * 2.0, "height": start.1, "lockAspect": false, "aspectLocked": false})).unwrap();
    assert_eq!(lock(&s), Some(false));
    assert_eq!(size(&s), (start.0 * 2.0, start.1));
    s.run("picture.size", &json!({"height": start.1 * 3.0})).unwrap();
    assert_eq!(size(&s), (start.0 * 2.0, start.1 * 3.0), "unlocked picture: only the height");
    s.run("edit.undo", &json!({})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!((size(&s), lock(&s)), (start, Some(true)), "size and lock come back together");
}
