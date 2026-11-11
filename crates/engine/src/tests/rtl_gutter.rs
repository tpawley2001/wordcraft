//! `w:rtlGutter` (ECMA-376 Part 1 §17.6.16) puts the section's gutter on the right of the page:
//! the text starts at the left margin and keeps its width. Synthetic DOCX fixtures.

use std::io::Write;

use wordcraft_doc::StoryRef;

use crate::Session;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

/// Letter, 1" margins, a 0.25" gutter (360 twips), with or without `w:rtlGutter`.
fn docx(rtl_gutter: bool) -> Vec<u8> {
    let flag = if rtl_gutter { "<w:rtlGutter/>" } else { "" };
    let doc = format!(
        r#"<w:document {W_NS}><w:body><w:p><w:pPr><w:spacing w:before="0" w:after="0"/></w:pPr><w:r><w:t>Body text</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="360"/>{flag}</w:sectPr></w:body></w:document>"#
    );
    let files = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string(),
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string(),
        ),
        ("word/document.xml", doc),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// The first page's text area (left, right) and its first body line's left edge, points.
fn text_area(rtl_gutter: bool) -> (f32, f32, f32) {
    let mut s = Session::new(wordcraft_docx::read(&docx(rtl_gutter)).expect("read"));
    let page = &s.layout().pages[0];
    let line_x = page
        .items
        .iter()
        .find_map(|it| match it {
            wordcraft_layout::Placed::Lines { story: StoryRef::Body, x, .. } => Some(*x),
            _ => None,
        })
        .expect("a body line on page 1");
    (page.body.x, page.body.x + page.body.w, line_x)
}

#[test]
fn the_gutter_widens_the_left_margin_by_default() {
    let (left, right, line) = text_area(false);
    assert!((left - 90.0).abs() < 0.01 && (right - 540.0).abs() < 0.01, "{left}..{right}");
    assert!((line - 90.0).abs() < 0.01, "{line}");
}

#[test]
fn an_rtl_gutter_widens_the_right_margin() {
    let (left, right, line) = text_area(true);
    assert!((left - 72.0).abs() < 0.01 && (right - 522.0).abs() < 0.01, "{left}..{right}");
    assert!((line - 72.0).abs() < 0.01, "{line}");
}

#[test]
fn mirror_margins_keep_the_gutter_inside() {
    let mut doc = wordcraft_docx::read(&docx(true)).expect("read");
    doc.settings.mirror_margins = true;
    let mut s = Session::new(doc);
    let page = &s.layout().pages[0];
    assert!((page.body.x - 90.0).abs() < 0.01, "{}", page.body.x);
}
