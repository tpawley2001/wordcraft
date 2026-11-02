//! A negative top margin (ECMA-376 Part 1 §17.6.11 `w:pgMar w:top`) is used at its absolute value and the body is not
//! moved down for a header taller than it, as in Word. Synthetic DOCX fixtures.

use std::io::{Read, Write};

use wordcraft_doc::StoryRef;

use crate::Session;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn docx(top: i32) -> Vec<u8> {
    let line = r#"<w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="480" w:lineRule="exact"/></w:pPr><w:r><w:t>Header line</w:t></w:r></w:p>"#;
    let header = format!(r#"<w:hdr {W_NS}>{}</w:hdr>"#, line.repeat(8));
    let doc = format!(
        r#"<w:document {W_NS}><w:body><w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr><w:r><w:t>Body text</w:t></w:r></w:p><w:sectPr><w:headerReference w:type="default" r:id="rId2"/><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="{top}" w:right="1800" w:bottom="1440" w:left="1800" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    let files = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#.to_string(),
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string(),
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#.to_string(),
        ),
        ("word/document.xml", doc),
        ("word/header1.xml", header),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// Top of the first body line on page 1, in points from the top of the page.
fn first_body_top(top: i32) -> f32 {
    let mut s = Session::new(wordcraft_docx::read(&docx(top)).expect("read"));
    let page = &s.layout().pages[0];
    page.items
        .iter()
        .find_map(|it| match it {
            wordcraft_layout::Placed::Lines { story: StoryRef::Body, y, .. } => Some(*y),
            _ => None,
        })
        .expect("a body line on page 1")
}

#[test]
fn a_negative_top_margin_is_not_pushed_down_by_a_tall_header() {
    let top = first_body_top(-1440);
    assert!((top - 72.0).abs() <= 0.5, "body line top {top}pt, Word 72pt");
}

#[test]
fn a_positive_top_margin_is_still_pushed_below_the_tall_header() {
    let top = first_body_top(1440);
    assert!(top > 200.0, "body line top {top}pt: the 8 x 24pt header at 36pt should push it below 200pt");
}

#[test]
fn a_negative_top_margin_is_saved_negative() {
    let doc = wordcraft_docx::read(&docx(-1440)).expect("read");
    let out = wordcraft_docx::write(&doc).expect("write");
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(out)).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    assert!(xml.contains(r#"w:top="-1440""#), "saved pgMar: {}", xml.split("<w:pgMar").nth(1).unwrap_or(&xml).split('>').next().unwrap_or(""));
}
