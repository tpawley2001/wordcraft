//! A linked picture's ScreenTip: Word keeps it on the drawing's own link, `wp:docPr/a:hlinkClick
//! tooltip="…"` (ECMA-376 Part 1 §21.1.2.3.5, `CT_Hyperlink` §20.1.2.2.24), next to the address
//! (#5267). It is the run's `CharProps::link_tip`, as for a text hyperlink. Original synthetic
//! packages; the 2×2 picture is generated in code.
use std::io::{Read, Write};
use std::sync::Arc;
use wordcraft_doc::{CharProps, Document, InlineObject, Pos};

const TIP: &str = "Go & see \"it\"";
const TIP_XML: &str = "Go &amp; see &quot;it&quot;";

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 90, 200, 255]))
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

const GRAPHIC: &str = r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="square.png"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdPic"/></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="254000" cy="254000"/></a:xfrm><a:prstGeom prst="rect"/></pic:spPr></pic:pic></a:graphicData></a:graphic>"#;

/// An inline picture's run; `link` goes into its `wp:docPr`.
fn picture_run(id: u32, link: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="254000" cy="254000"/><wp:docPr id="{id}" name="Picture {id}">{link}</wp:docPr>{GRAPHIC}</wp:inline></w:drawing></w:r>"#
    )
}

/// A floating (square-wrapped) picture's run; `link` goes into its `wp:docPr`.
fn floating_picture_run(id: u32, link: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251658240" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="254000" cy="254000"/><wp:wrapSquare wrapText="bothSides"/><wp:docPr id="{id}" name="Picture {id}">{link}</wp:docPr>{GRAPHIC}</wp:anchor></w:drawing></w:r>"#
    )
}

fn package(body: &str) -> Vec<u8> {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>{body}<w:sectPr/></w:body></w:document>"#
    );
    let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdPic" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/square.png"/><Relationship Id="rIdWeb" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/picture" TargetMode="External"/><Relationship Id="rIdRun" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/run" TargetMode="External"/></Relationships>"#;
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let png = png();
    zip(&[
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("word/document.xml", xml.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        ("word/media/square.png", png.as_slice()),
    ])
}

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            (f.name().to_string(), data)
        })
        .collect()
}

fn part(bytes: &[u8], name: &str) -> String {
    let files = unzip(bytes);
    String::from_utf8(files.into_iter().find(|(n, _)| n == name).unwrap().1).unwrap()
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

/// Each `<a:hlinkClick …>` start tag of `xml`, in order.
fn clicks(xml: &str) -> Vec<&str> {
    xml.match_indices("<a:hlinkClick ").map(|(i, _)| &xml[i..i + xml[i..].find('>').unwrap() + 1]).collect()
}

/// The decoded value of `name="…"` in a start tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let raw = &tag[start..start + tag[start..].find('"')?];
    Some(raw.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

fn some(s: &str) -> Option<String> {
    Some(s.to_string())
}

#[test]
fn a_screen_tip_on_a_pictures_link_is_read() {
    let body = format!(
        "<w:p>{}{}{}{}<w:hyperlink r:id=\"rIdRun\" w:tooltip=\"Run tip\">{}</w:hyperlink><w:hyperlink r:id=\"rIdRun\">{}</w:hyperlink></w:p>",
        // Word's way: the link and its ScreenTip on the drawing.
        picture_run(1, &format!(r#"<a:hlinkClick r:id="rIdWeb" tooltip="{TIP_XML}"/>"#)),
        // An empty ScreenTip is none.
        picture_run(2, r#"<a:hlinkClick r:id="rIdWeb" tooltip=""/>"#),
        // No link (the relationship is missing, or is no hyperlink): no ScreenTip either.
        picture_run(3, r#"<a:hlinkClick r:id="rIdMissing" tooltip="Lost"/>"#),
        picture_run(4, r#"<a:hlinkClick r:id="rIdPic" tooltip="Lost"/>"#),
        // A link around the run wins whole: its address and its ScreenTip, or its lack of one.
        picture_run(5, r#"<a:hlinkClick r:id="rIdWeb" tooltip="Drawing tip"/>"#),
        picture_run(6, r#"<a:hlinkClick r:id="rIdWeb" tooltip="Drawing tip"/>"#),
    );
    let doc = wordcraft_docx::read(&package(&body)).unwrap();
    assert_eq!(
        picture_links(&doc),
        [
            (some("https://example.com/picture"), some(TIP)),
            (some("https://example.com/picture"), None),
            (None, None),
            (None, None),
            (some("https://example.com/run"), some("Run tip")),
            (some("https://example.com/run"), None),
        ]
    );
}

#[test]
fn a_screen_tip_on_a_floating_pictures_link_is_read() {
    let body =
        format!("<w:p>{}<w:r><w:t>Text</w:t></w:r></w:p>", floating_picture_run(1, &format!(r#"<a:hlinkClick r:id="rIdWeb" tooltip="{TIP_XML}"/>"#)));
    let doc = wordcraft_docx::read(&package(&body)).unwrap();
    assert_eq!(picture_links(&doc), [(some("https://example.com/picture"), some(TIP))]);
}

#[test]
fn a_linked_picture_is_saved_with_its_screen_tip_on_the_drawing() {
    let mut doc = Document::new();
    doc.media.insert("square.png".into(), Arc::new(png()));
    let picture = || InlineObject::Image {
        media: "square.png".into(),
        w: 20.0,
        h: 20.0,
        alt: String::new(),
        float: Default::default(),
        crop: [0.0; 4],
        ole: None,
        fx: Default::default(),
    };
    let linked = |l: &str, tip: Option<&str>| CharProps { link: Some(l.to_string()), link_tip: tip.map(Box::from), ..Default::default() };
    // Inserted at the start each time, so the document order is: tipped, plain, empty tip, place.
    // A place in this document stays a `w:hyperlink w:anchor` only: no `a:hlinkClick`, so no tooltip there.
    doc.insert_object(&Pos::body(0, 0), picture(), &linked("#Spot", Some("Place tip"))).unwrap();
    doc.insert_object(&Pos::body(0, 0), picture(), &linked("https://example.com/empty", Some(""))).unwrap();
    doc.insert_object(&Pos::body(0, 0), picture(), &linked("https://example.com/plain", None)).unwrap();
    doc.insert_object(&Pos::body(0, 0), picture(), &linked("https://example.com/tipped", Some(TIP))).unwrap();
    let bytes = wordcraft_docx::write(&doc).unwrap();
    let xml = part(&bytes, "word/document.xml");
    let tags = clicks(&xml);
    assert_eq!(tags.len(), 3, "{xml}");
    assert_eq!(attr(tags[0], "tooltip").as_deref(), Some(TIP), "{xml}");
    assert!(tags[0].contains(TIP_XML), "the ScreenTip is XML-escaped: {}", tags[0]);
    assert!(attr(tags[0], "r:id").is_some(), "{xml}");
    // No ScreenTip, or an empty one: no tooltip attribute.
    assert_eq!(attr(tags[1], "tooltip"), None, "{xml}");
    assert_eq!(attr(tags[2], "tooltip"), None, "{xml}");
    let back = wordcraft_docx::read(&bytes).unwrap();
    let want = [
        (some("https://example.com/tipped"), some(TIP)),
        (some("https://example.com/plain"), None),
        (some("https://example.com/empty"), None),
        (some("#Spot"), some("Place tip")),
    ];
    assert_eq!(picture_links(&back), want);
}

/// A file as Word writes a linked floating picture: the link and ScreenTip only on the drawing.
/// Opening and saving it twice keeps both, and the saved drawing still carries the ScreenTip.
#[test]
fn a_drawings_screen_tip_survives_open_and_save() {
    let body = format!(
        "<w:p>{}{}</w:p>",
        picture_run(1, &format!(r#"<a:hlinkClick r:id="rIdWeb" tooltip="{TIP_XML}"/>"#)),
        picture_run(2, r#"<a:hlinkClick r:id="rIdWeb"/>"#),
    );
    let mut bytes = package(&body);
    for round in 0..2 {
        let doc = wordcraft_docx::read(&bytes).unwrap();
        assert_eq!(
            picture_links(&doc),
            [(some("https://example.com/picture"), some(TIP)), (some("https://example.com/picture"), None)],
            "round {round}"
        );
        bytes = wordcraft_docx::write(&doc).unwrap();
        let xml = part(&bytes, "word/document.xml");
        let tags = clicks(&xml);
        assert_eq!(tags.len(), 2, "round {round}: {xml}");
        assert_eq!(attr(tags[0], "tooltip").as_deref(), Some(TIP), "round {round}: {xml}");
        assert_eq!(attr(tags[1], "tooltip"), None, "round {round}: {xml}");
        // The drawing's link alone (the `w:hyperlink` around the run taken out) reads back the same.
        let mut bare = String::new();
        let mut rest = xml.as_str();
        while let Some(i) = rest.find("<w:hyperlink ") {
            bare.push_str(&rest[..i]);
            rest = &rest[i + rest[i..].find('>').unwrap() + 1..];
        }
        bare.push_str(rest);
        let bare = bare.replace("</w:hyperlink>", "");
        assert_ne!(bare, xml, "round {round}");
        let files = unzip(&bytes);
        let entries: Vec<(&str, &[u8])> =
            files.iter().map(|(n, b)| (n.as_str(), if n == "word/document.xml" { bare.as_bytes() } else { b.as_slice() })).collect();
        let doc = wordcraft_docx::read(&zip(&entries)).unwrap();
        assert_eq!(
            picture_links(&doc),
            [(some("https://example.com/picture"), some(TIP)), (some("https://example.com/picture"), None)],
            "round {round}, drawing only"
        );
    }
}
