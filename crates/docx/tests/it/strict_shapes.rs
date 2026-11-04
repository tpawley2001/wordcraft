//! Word shapes in a Strict Open XML file (ISO/IEC 29500 Strict): Word writes `wsp` and its children
//! (`cNvSpPr`, `spPr`, `style`, `txbx`, `bodyPr`) in the Strict `wordprocessingDrawing` namespace
//! (`http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing`) under a `graphicData` whose `uri`
//! is the Word 2010 shape namespace. They are read like the Transitional `wps:` shapes (#3548).
//! Hand-written fixtures.

use std::io::Write;

use wordcraft_doc::para::ShapeKind;
use wordcraft_doc::{Document, InlineObject};

const T_ROOT_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const S_ROOT_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument";
const WPS_URI: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";

fn zip(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, s) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(s.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// A one-paragraph document whose body is `body`. `strict` picks the Strict namespaces (shape
/// elements under `wp:`) or the Transitional ones (shape elements under `wps:`).
fn read(body: &str, strict: bool) -> Document {
    let (w, wp, a, root_rel, shape_ns) = if strict {
        (
            "http://purl.oclc.org/ooxml/wordprocessingml/main",
            "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing",
            "http://purl.oclc.org/ooxml/drawingml/main",
            S_ROOT_REL,
            "",
        )
    } else {
        (
            "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
            "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
            "http://schemas.openxmlformats.org/drawingml/2006/main",
            T_ROOT_REL,
            r#" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape""#,
        )
    };
    let p = if strict { "wp" } else { "wps" };
    let body = body.replace("{p}", p);
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{w}" xmlns:wp="{wp}" xmlns:a="{a}"{shape_ns}{conf}><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        conf = if strict { r#" w:conformance="strict""# } else { "" },
    );
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{root_rel}" Target="word/document.xml"/></Relationships>"#
    );
    let types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    wordcraft_docx::read(&zip(&[("[Content_Types].xml", types), ("_rels/.rels", &rels), ("word/document.xml", &doc)])).unwrap()
}

/// (kind, width pt, height pt, is a text box) of every shape, in reading order.
fn shapes(d: &Document) -> Vec<(ShapeKind, f32, f32, bool)> {
    let mut out = Vec::new();
    d.objects_in_reading_order(&mut |o| {
        if let InlineObject::Shape { kind, w, h, .. } = o {
            out.push((*kind, *w, *h, o.text_box().is_some()));
        }
    });
    out
}

/// An inline rectangle 5104263 x 2122227 EMU = 401.910 x 167.105 pt, no fill, no line.
fn inline_rect() -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="5104263" cy="2122227"/><wp:docPr id="3" name="Rectangle 3"/><a:graphic><a:graphicData uri="{WPS_URI}"><{{p}}:wsp><{{p}}:cNvSpPr/><{{p}}:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="5104263" cy="2122227"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></{{p}}:spPr><{{p}}:bodyPr anchor="ctr"/></{{p}}:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

/// A floating right arrow 1160060 x 320723 EMU = 91.343 x 25.254 pt.
fn anchored_arrow() -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="2" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>1978925</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>382137</wp:posOffset></wp:positionV><wp:extent cx="1160060" cy="320723"/><wp:wrapNone/><wp:docPr id="4" name="Right Arrow 2"/><a:graphic><a:graphicData uri="{WPS_URI}"><{{p}}:wsp><{{p}}:cNvSpPr/><{{p}}:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1160060" cy="320723"/></a:xfrm><a:prstGeom prst="rightArrow"><a:avLst/></a:prstGeom></{{p}}:spPr><{{p}}:bodyPr/></{{p}}:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>"#
    )
}

/// An inline text box 1828800 x 914400 EMU = 144 x 72 pt holding one paragraph.
fn text_box() -> String {
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="1828800" cy="914400"/><wp:docPr id="5" name="Text Box 5"/><a:graphic><a:graphicData uri="{WPS_URI}"><{{p}}:wsp><{{p}}:cNvSpPr txBox="1"/><{{p}}:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></{{p}}:spPr><{{p}}:txbx><w:txbxContent><w:p><w:r><w:t>Inside</w:t></w:r></w:p></w:txbxContent></{{p}}:txbx><{{p}}:bodyPr/></{{p}}:wsp></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

fn close(got: &[(ShapeKind, f32, f32, bool)], want: &[(ShapeKind, f32, f32, bool)]) {
    assert_eq!(got.len(), want.len(), "{got:?} != {want:?}");
    for (g, e) in got.iter().zip(want) {
        assert!(g.0 == e.0 && (g.1 - e.1).abs() < 0.05 && (g.2 - e.2).abs() < 0.05 && g.3 == e.3, "{got:?} != {want:?}");
    }
}

#[test]
fn transitional_shapes_are_read() {
    let d = read(&format!("{}{}{}", inline_rect(), anchored_arrow(), text_box()), false);
    close(
        &shapes(&d),
        &[(ShapeKind::Rectangle, 401.91, 167.105, false), (ShapeKind::Arrow, 91.343, 25.254, false), (ShapeKind::TextBox, 144.0, 72.0, true)],
    );
}

#[test]
fn a_strict_inline_rectangle_is_read() {
    close(&shapes(&read(&inline_rect(), true)), &[(ShapeKind::Rectangle, 401.91, 167.105, false)]);
}

#[test]
fn a_strict_floating_arrow_is_read() {
    close(&shapes(&read(&anchored_arrow(), true)), &[(ShapeKind::Arrow, 91.343, 25.254, false)]);
}

#[test]
fn a_strict_text_box_keeps_its_text() {
    let d = read(&text_box(), true);
    close(&shapes(&d), &[(ShapeKind::TextBox, 144.0, 72.0, true)]);
    let mut story = None;
    d.objects_in_reading_order(&mut |o| story = story.or(o.text_box()));
    let part = d.parts.get(&story.expect("a text box story")).expect("its part");
    assert!(format!("{:?}", part.blocks).contains("Inside"), "{:?}", part.blocks);
}
