//! Lock aspect ratio (`wp:cNvGraphicFramePr/a:graphicFrameLocks/@noChangeAspect`, ECMA-376 Part 1
//! §20.4.2.4 and §20.1.2.2.19): read into `Float::lock_aspect` and written back, as Word writes it on
//! every picture it inserts.
use std::io::Read;

use wordcraft_doc::{Document, InlineObject, Pos};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape""#;
const LOCKED: &str = r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="1"/></wp:cNvGraphicFramePr>"#;
const UNLOCKED: &str = "<wp:cNvGraphicFramePr/>";

fn png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([10, 20, 30, 255]));
    let mut b = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
    b
}

fn package(drawing: &str) -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NS}><w:body><w:p><w:r><w:drawing>{drawing}</w:drawing></w:r></w:p></w:body></w:document>"#
    );
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdP" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/p.png"/></Relationships>"#;
    let types = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let png = png();
    super::fixtures::zip(&[
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", root.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/_rels/document.xml.rels", rels.as_bytes()),
        ("word/media/p.png", &png),
    ])
}

fn inline_picture(locks: &str) -> String {
    format!(
        r#"<wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1270000" cy="635000"/><wp:docPr id="1" name="Picture 1"/>{locks}<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="p.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdP"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline>"#
    )
}

fn anchored_shape(locks: &str) -> String {
    format!(
        r#"<wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="1" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:wrapSquare wrapText="bothSides"/><wp:docPr id="2" name="Rectangle 2"/>{locks}<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"><wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp></a:graphicData></a:graphic></wp:anchor>"#
    )
}

/// `lock_aspect` of every drawing in the body, in order.
fn locks(d: &Document) -> Vec<Option<bool>> {
    d.body.iter().filter_map(|b| b.as_para()).flat_map(|p| p.objects.iter().filter_map(|o| o.frame()).map(|(_, _, f)| f.lock_aspect)).collect()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The `wp:cNvGraphicFramePr` elements of a saved document.xml (each with its content).
fn frame_prs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<wp:cNvGraphicFramePr") {
        let tail = &rest[i..];
        let end = if let Some(open_end) = tail.find('>') {
            if tail[..open_end].ends_with('/') {
                open_end + 1
            } else {
                tail.find("</wp:cNvGraphicFramePr>").map_or(open_end + 1, |e| e + "</wp:cNvGraphicFramePr>".len())
            }
        } else {
            tail.len()
        };
        out.push(tail[..end].to_string());
        rest = &tail[end..];
    }
    out
}

fn image(lock: Option<bool>) -> InlineObject {
    let mut float = wordcraft_doc::para::Float::default();
    float.lock_aspect = lock;
    InlineObject::Image { media: "img1".into(), w: 100.0, h: 50.0, alt: String::new(), float, crop: [0.0; 4], fx: Default::default(), ole: None }
}

fn shape(lock: Option<bool>) -> InlineObject {
    let mut float = wordcraft_doc::para::Float::default();
    float.lock_aspect = lock;
    InlineObject::Shape {
        kind: Default::default(),
        w: 100.0,
        h: 50.0,
        fill: None,
        stroke: Some(wordcraft_doc::props::Rgb(0, 0, 0)),
        stroke_width: 1.0,
        float,
        story: None,
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

fn saved(obj: InlineObject) -> (String, Document) {
    let mut doc = Document::new();
    doc.media.insert("img1".into(), std::sync::Arc::new(png()));
    doc.insert_object(&Pos::default(), obj, &Default::default()).unwrap();
    let bytes = wordcraft_docx::write(&doc).unwrap();
    (document_xml(&bytes), wordcraft_docx::read(&bytes).unwrap())
}

#[test]
fn a_locked_picture_is_read_locked() {
    let d = wordcraft_docx::read(&package(&inline_picture(LOCKED))).unwrap();
    assert_eq!(locks(&d), vec![Some(true)]);
}

#[test]
fn a_picture_without_the_lock_is_read_unlocked() {
    let d = wordcraft_docx::read(&package(&inline_picture(UNLOCKED))).unwrap();
    assert_eq!(locks(&d), vec![Some(false)]);
    let d =
        wordcraft_docx::read(&package(&inline_picture(r#"<wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect="0"/></wp:cNvGraphicFramePr>"#)))
            .unwrap();
    assert_eq!(locks(&d), vec![Some(false)]);
}

#[test]
fn an_anchored_shape_keeps_its_lock() {
    let d = wordcraft_docx::read(&package(&anchored_shape(LOCKED))).unwrap();
    assert_eq!(locks(&d), vec![Some(true)]);
    let d = wordcraft_docx::read(&package(&anchored_shape(UNLOCKED))).unwrap();
    assert_eq!(locks(&d), vec![Some(false)]);
}

#[test]
fn locked_and_unsaid_pictures_are_saved_locked() {
    for lock in [Some(true), None] {
        let (xml, back) = saved(image(lock));
        let prs = frame_prs(&xml);
        assert_eq!(prs.len(), 1, "{xml}");
        assert!(prs[0].contains("graphicFrameLocks") && prs[0].contains(r#"noChangeAspect="1""#), "{lock:?}: {}", prs[0]);
        assert_eq!(locks(&back), vec![Some(true)], "{lock:?}");
    }
}

#[test]
fn unlocked_pictures_and_unsaid_shapes_are_saved_unlocked() {
    for (obj, what) in [(image(Some(false)), "unlocked picture"), (shape(None), "shape"), (shape(Some(false)), "unlocked shape")] {
        let (xml, back) = saved(obj);
        let prs = frame_prs(&xml);
        assert_eq!(prs.len(), 1, "{what}: {xml}");
        assert!(!prs[0].contains("noChangeAspect=\"1\""), "{what}: {}", prs[0]);
        assert_eq!(locks(&back), vec![Some(false)], "{what}");
    }
}

#[test]
fn a_locked_shape_round_trips() {
    let (xml, back) = saved(shape(Some(true)));
    assert!(frame_prs(&xml).iter().any(|p| p.contains(r#"noChangeAspect="1""#)), "{xml}");
    assert_eq!(locks(&back), vec![Some(true)]);
}
