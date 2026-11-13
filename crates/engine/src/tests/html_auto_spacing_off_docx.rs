//! A DOCX whose settings turn automatic (HTML-style) paragraph spacing off
//! (`w:doNotUseHTMLParagraphAutoSpacing`, ECMA-376 Part 1 §17.15.1): paragraphs marked
//! `w:beforeAutospacing` / `w:afterAutospacing` keep their stored space before / after, added up.
//! Synthetic fixture.

use std::io::Write;

use wordcraft_doc::StoryRef;

use crate::Session;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn docx(compat: &str) -> Vec<u8> {
    let para = r#"<w:p><w:pPr><w:spacing w:before="100" w:beforeAutospacing="1" w:after="100" w:afterAutospacing="1" w:line="240" w:lineRule="auto"/></w:pPr><w:r><w:t>Automatic spacing</w:t></w:r></w:p>"#;
    let doc = format!(r#"<w:document {W_NS}><w:body>{para}{para}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#);
    let settings = format!(r#"<w:settings {W_NS}><w:compat>{compat}</w:compat></w:settings>"#);
    let files = [
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#.to_string(),
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string(),
        ),
        (
            "word/_rels/document.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#.to_string(),
        ),
        ("word/document.xml", doc),
        ("word/settings.xml", settings),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// The space between the bottom of the first paragraph and the top of the second.
fn gap(compat: &str) -> f32 {
    let mut s = Session::new(wordcraft_docx::read(&docx(compat)).expect("read"));
    let page = &s.layout().pages[0];
    let ys: Vec<(f32, f32)> = page
        .items
        .iter()
        .filter_map(|it| match it {
            wordcraft_layout::Placed::Lines { story: StoryRef::Body, y, para, .. } => Some((*y, *y + para.height)),
            _ => None,
        })
        .collect();
    assert_eq!(ys.len(), 2, "two body paragraphs: {ys:?}");
    ys[1].0 - ys[0].1
}

#[test]
fn automatic_spacing_flags_are_ignored_when_html_auto_spacing_is_off() {
    let g = gap("<w:doNotUseHTMLParagraphAutoSpacing/>");
    assert!((g - 10.0).abs() <= 0.01, "gap {g} pt, want the stored 5 + 5");
}

#[test]
fn automatic_spacing_applies_without_the_setting() {
    let g = gap("");
    assert!((g - 14.0).abs() <= 0.01, "gap {g} pt, want the automatic 14");
}
