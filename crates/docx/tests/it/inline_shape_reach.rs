//! A picture placed in line with text with room for its outline and shadow, as the Word 97-2003
//! reader makes it from a `SHAPE` field (an inline picture whose effect extent is the outline's
//! width all round plus a 6 pt shadow up and right): saved as `wp:inline` with that
//! `wp:effectExtent`, and read back with the same room. Built from the model.

use wordcraft_doc::para::{Float, Wrap};
use wordcraft_doc::props::CharProps;
use wordcraft_doc::{Document, InlineObject, Paragraph, para_block};

fn tiny_png() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(2, 2, |x, y| image::Rgba([x as u8 * 90, y as u8 * 90, 40, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn an_inline_picture_keeps_its_outline_and_shadow_room() {
    let mut d = Document::new();
    let media = d.add_media(tiny_png(), "png");
    let pic = InlineObject::Image {
        media,
        w: 209.2,
        h: 156.85,
        alt: String::new(),
        float: Float { wrap: Wrap::Inline, effect: [0.75, 7.5, 7.5, 0.75], ..Default::default() },
        crop: [0.0; 4],
        fx: Default::default(),
        ole: None,
    };
    let mut p = Paragraph::with_text("", CharProps::default());
    p.insert_object(0, pic, &CharProps::default()).unwrap();
    d.body = vec![para_block(p)];
    let bytes = wordcraft_docx::write(&d).expect("write");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
    assert!(xml.contains("<wp:inline"), "{xml}");
    assert!(!xml.contains("<wp:anchor"), "{xml}");
    assert!(xml.contains(r#"<wp:effectExtent l="9525" t="95250" r="95250" b="9525"/>"#), "{xml}");
    let r = wordcraft_docx::read(&bytes).expect("read");
    let p = r.body.iter().find_map(|b| b.as_para()).expect("para");
    let (_, _, float) = p.objects.iter().find_map(|o| o.frame()).expect("the picture");
    assert_eq!(float.wrap, Wrap::Inline);
    let want = [0.75, 7.5, 7.5, 0.75];
    assert!(float.effect.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01), "{:?}", float.effect);
}
