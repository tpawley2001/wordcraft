//! An embedded object's `{\result …}` (RTF 1.9.1, "Objects") is what Word shows for it: its picture imports in place.

use super::*;

fn png_hex() -> (Vec<u8>, String) {
    let pixels = image::RgbaImage::from_pixel(1, 1, image::Rgba([40, 80, 160, 255]));
    let mut png = Vec::new();
    pixels.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let hex = png.iter().map(|b| format!("{b:02x}")).collect();
    (png, hex)
}

fn images(f: &Flow) -> Vec<&model::Img> {
    f.blocks
        .iter()
        .filter_map(|b| match b {
            FBlock::Para(p) => Some(&p.inlines),
            FBlock::Table(_) => None,
        })
        .flatten()
        .filter_map(|i| match i {
            Inline::Image(img) => Some(img),
            _ => None,
        })
        .collect()
}

const OBJ: &str = "\\object\\objemb\\objw1531\\objh1002{\\*\\objclass Package}{\\*\\objdata 01050000020000000800000050616b6b616765}";

#[test]
fn the_result_picture_of_an_object_is_shown() {
    let (png, hex) = png_hex();
    let rtf = format!("{{\\rtf1\\ansi Before{{{OBJ}{{\\result{{\\pict\\pngblip\\picwgoal1440\\pichgoal720 {hex}}}}}}}After\\par}}");
    let f = parse(rtf.as_bytes()).unwrap();
    let imgs = images(&f);
    assert_eq!(imgs.len(), 1);
    assert_eq!(imgs[0].data.as_ref(), &png);
    assert_eq!(imgs[0].ext, "png");
    assert_eq!((imgs[0].w, imgs[0].h), (72.0, 36.0));
    assert_eq!(texts(&f), ["Normal:BeforeAfter"]);
}

#[test]
fn a_shppict_result_keeps_one_picture() {
    let (png, hex) = png_hex();
    let pict = format!("{{\\pict\\pngblip\\picwgoal1440\\pichgoal720 {hex}}}");
    let rtf = format!("{{\\rtf1\\ansi Before{{{OBJ}{{\\result{{\\*\\shppict{pict}}}{{\\nonshppict{{\\pict\\wmetafile8 {hex}}}}}}}}}After\\par}}");
    let f = parse(rtf.as_bytes()).unwrap();
    let imgs = images(&f);
    assert_eq!(imgs.len(), 1);
    assert_eq!(imgs[0].data.as_ref(), &png);
    assert_eq!(texts(&f), ["Normal:BeforeAfter"]);
}

#[test]
fn an_object_without_a_result_adds_nothing() {
    let rtf = format!("{{\\rtf1\\ansi Before{{{OBJ}}}After\\par}}");
    let f = parse(rtf.as_bytes()).unwrap();
    assert!(images(&f).is_empty());
    assert_eq!(texts(&f), ["Normal:BeforeAfter"]);
}
