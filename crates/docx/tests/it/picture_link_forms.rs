//! A picture's hyperlink in the forms Word reads besides an external `wp:docPr/a:hlinkClick`
//! (#5267): a link to a place in the document (an internal hyperlink relationship whose target is
//! `#Name`), a link only on the picture's own `pic:cNvPr`, and a VML picture's `href`. Each
//! becomes the run's `CharProps::link` (and `link_tip`), as for a text hyperlink. Original
//! synthetic packages; the 2×2 picture is generated in code.

use std::io::{Read, Write};
use wordcraft_doc::{Document, InlineObject};

const HYPERLINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 90, 200, 255]))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// An inline picture's run; `frame` goes into its `wp:docPr`, `own` into its `pic:cNvPr`.
fn picture_run(id: u32, frame: &str, own: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="254000" cy="254000"/><wp:docPr id="{id}" name="Picture {id}">{frame}</wp:docPr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="square.png">{own}</pic:cNvPr><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdPic"/></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="254000" cy="254000"/></a:xfrm><a:prstGeom prst="rect"/></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
    )
}

/// A VML picture's run; `attrs` go onto its `v:shape`.
fn vml_run(attrs: &str) -> String {
    format!(r#"<w:r><w:pict><v:shape style="width:20pt;height:20pt"{attrs}><v:imagedata r:id="rIdPic"/></v:shape></w:pict></w:r>"#)
}

fn package(body: &str) -> Vec<u8> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body>{body}<w:sectPr/></w:body></w:document>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = format!(
        r##"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdPic" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/square.png"/><Relationship Id="rIdWeb" Type="{HYPERLINK}" Target="https://example.com/picture" TargetMode="External"/><Relationship Id="rIdOther" Type="{HYPERLINK}" Target="https://example.com/other" TargetMode="External"/><Relationship Id="rIdRun" Type="{HYPERLINK}" Target="https://example.com/run" TargetMode="External"/><Relationship Id="rIdSpot" Type="{HYPERLINK}" Target="#Spot"/><Relationship Id="rIdBare" Type="{HYPERLINK}" Target="#"/><Relationship Id="rIdPart" Type="{HYPERLINK}" Target="media/square.png"/><Relationship Id="rIdFile" Type="{HYPERLINK}" Target="other.docx"/></Relationships>"##
    );
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let png = png();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("word/document.xml", xml.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("word/media/square.png", png.as_slice()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The link and ScreenTip on each picture of the first paragraph, in order.
fn picture_links(doc: &Document) -> Vec<(Option<String>, Option<String>)> {
    let p = doc.body[0].as_para().unwrap();
    p.object_offsets()
        .into_iter()
        .filter(|off| matches!(p.object_at(*off), Some(InlineObject::Image { .. })))
        .map(|off| {
            let props = p.props_of_char(off);
            (props.link.clone(), props.link_tip.as_deref().map(str::to_string))
        })
        .collect()
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

/// Word writes a picture linked to a bookmark or heading as an internal hyperlink relationship
/// whose target is `#Name`.
#[test]
fn a_pictures_link_to_a_place_in_the_document_is_read() {
    let body = format!(
        "<w:p>{}{}{}{}{}</w:p>",
        picture_run(1, r#"<a:hlinkClick r:id="rIdSpot" tooltip="TOP OF PAGE"/>"#, ""),
        picture_run(2, r#"<a:hlinkClick r:id="rIdSpot"/>"#, ""),
        // Internal targets that name no place: no link, and no ScreenTip without one.
        picture_run(3, r#"<a:hlinkClick r:id="rIdBare" tooltip="Lost"/>"#, ""),
        picture_run(4, r#"<a:hlinkClick r:id="rIdPart" tooltip="Lost"/>"#, ""),
        picture_run(5, r#"<a:hlinkClick r:id="rIdFile" tooltip="Lost"/>"#, ""),
    );
    let doc = wordcraft_docx::read(&package(&body)).unwrap();
    assert_eq!(picture_links(&doc), [(some("#Spot"), some("TOP OF PAGE")), (some("#Spot"), None), (None, None), (None, None), (None, None)]);
}

#[test]
fn a_link_only_on_the_pictures_own_properties_is_read() {
    let body = format!(
        "<w:p>{}{}{}{}{}{}<w:hyperlink r:id=\"rIdRun\">{}</w:hyperlink></w:p>",
        picture_run(1, "", r#"<a:hlinkClick r:id="rIdWeb" tooltip="Own tip"/>"#),
        picture_run(2, "", r#"<a:hlinkClick r:id="rIdSpot"/>"#),
        // Both: the frame's link wins whole, its lack of a ScreenTip too.
        picture_run(3, r#"<a:hlinkClick r:id="rIdWeb"/>"#, r#"<a:hlinkClick r:id="rIdOther" tooltip="Own tip"/>"#),
        // The frame's names no usable relationship: the picture's own is the link.
        picture_run(4, r#"<a:hlinkClick r:id="rIdMissing" tooltip="Lost"/>"#, r#"<a:hlinkClick r:id="rIdOther" tooltip="Own tip"/>"#),
        // A hover link is no link; neither is a click that names nothing usable.
        picture_run(5, r#"<a:hlinkHover r:id="rIdWeb"/>"#, r#"<a:hlinkHover r:id="rIdWeb"/>"#),
        picture_run(6, "", r#"<a:hlinkClick r:id="rIdPic" tooltip="Lost"/>"#),
        // A link around the run wins over both.
        picture_run(7, "", r#"<a:hlinkClick r:id="rIdWeb" tooltip="Own tip"/>"#),
    );
    let doc = wordcraft_docx::read(&package(&body)).unwrap();
    assert_eq!(
        picture_links(&doc),
        [
            (some("https://example.com/picture"), some("Own tip")),
            (some("#Spot"), None),
            (some("https://example.com/picture"), None),
            (some("https://example.com/other"), some("Own tip")),
            (None, None),
            (None, None),
            (some("https://example.com/run"), None),
        ]
    );
}

#[test]
fn a_vml_pictures_href_is_its_link() {
    let body = format!(
        "<w:p>{}{}{}{}{}<w:hyperlink r:id=\"rIdRun\">{}</w:hyperlink></w:p>",
        vml_run(r#" href="https://example.com/vml?x=1&amp;y=2" o:button="t""#),
        vml_run(r#" href="  https://example.com/padded  " alt="Not a tip" title="Not a tip""#),
        vml_run(r#" href="""#),
        vml_run(r#" href="   ""#),
        vml_run(""),
        vml_run(r#" href="https://example.com/vml""#),
    );
    let doc = wordcraft_docx::read(&package(&body)).unwrap();
    assert_eq!(
        picture_links(&doc),
        [
            (some("https://example.com/vml?x=1&y=2"), None),
            (some("https://example.com/padded"), None),
            (None, None),
            (None, None),
            (None, None),
            (some("https://example.com/run"), None),
        ]
    );
}

/// Opening and saving twice keeps a link to a place in the document (and its ScreenTip); it is
/// saved as the `w:hyperlink w:anchor` around the run, with no relationship of its own.
#[test]
fn a_pictures_link_to_a_place_survives_open_and_save() {
    let body = format!(
        "<w:p>{}{}</w:p>",
        picture_run(1, r#"<a:hlinkClick r:id="rIdSpot" tooltip="TOP OF PAGE"/>"#, ""),
        picture_run(2, "", r#"<a:hlinkClick r:id="rIdWeb" tooltip="Own tip"/>"#),
    );
    let expected = [(some("#Spot"), some("TOP OF PAGE")), (some("https://example.com/picture"), some("Own tip"))];
    let once = wordcraft_docx::write(&wordcraft_docx::read(&package(&body)).unwrap()).unwrap();
    let xml = part(&once, "word/document.xml");
    assert!(xml.contains(r#"w:anchor="Spot""#), "{xml}");
    let rels = part(&once, "word/_rels/document.xml.rels");
    assert!(!rels.contains("#Spot"), "{rels}");
    assert_eq!(rels.matches("https://example.com/picture").count(), 1, "{rels}");
    let twice = wordcraft_docx::write(&wordcraft_docx::read(&once).unwrap()).unwrap();
    assert_eq!(picture_links(&wordcraft_docx::read(&once).unwrap()), expected);
    assert_eq!(picture_links(&wordcraft_docx::read(&twice).unwrap()), expected);
}

#[test]
fn a_vml_pictures_link_survives_open_and_save() {
    let body = format!("<w:p>{}</w:p>", vml_run(r#" href="https://example.com/vml?x=1&amp;y=2""#));
    let expected = [(some("https://example.com/vml?x=1&y=2"), None)];
    let once = wordcraft_docx::write(&wordcraft_docx::read(&package(&body)).unwrap()).unwrap();
    let rels = part(&once, "word/_rels/document.xml.rels");
    assert_eq!(rels.matches("https://example.com/vml?x=1&amp;y=2").count(), 1, "{rels}");
    let twice = wordcraft_docx::write(&wordcraft_docx::read(&once).unwrap()).unwrap();
    assert_eq!(picture_links(&wordcraft_docx::read(&once).unwrap()), expected);
    assert_eq!(picture_links(&wordcraft_docx::read(&twice).unwrap()), expected);
}
