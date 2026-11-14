//! Gutter position Top (`w:gutterAtTop`, ECMA-376 Part 1 §17.15.1.49): the gutter widens the top
//! margin of every page instead of the left one, and wins over mirror margins and `w:rtlGutter`.
//! Expected values measured in Microsoft 365 Word (Letter, 1" margins, 1" gutter): body at x=72,
//! y=144; header stays at x=72, 0.5" from the top. Synthetic DOCX fixtures.

use std::io::Write;

use crate::Session;

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

/// Letter, 1" margins, a 1" gutter (1440 twips). `settings` goes into w:settings, `sect` into w:sectPr.
fn docx(settings: &str, sect: &str, landscape: bool) -> Vec<u8> {
    let pg = if landscape { r#"<w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>"# } else { r#"<w:pgSz w:w="12240" w:h="15840"/>"# };
    let paras: String = (1..=60).map(|i| format!("<w:p><w:r><w:t>Line {i:02} body text</w:t></w:r></w:p>")).collect();
    let doc = format!(
        r#"<w:document {W_NS}><w:body>{paras}<w:sectPr>{pg}<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="1440"/>{sect}</w:sectPr></w:body></w:document>"#
    );
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
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#.to_string(),
        ),
        ("word/document.xml", doc),
        ("word/settings.xml", format!("<w:settings {W_NS}>{settings}</w:settings>")),
    ];
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, body) in files {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(body.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

/// (left, top, width, height) of the body text area on each page, points.
fn bodies(settings: &str, sect: &str, landscape: bool) -> Vec<(f32, f32, f32, f32)> {
    let mut s = Session::new(wordcraft_docx::read(&docx(settings, sect, landscape)).expect("read"));
    s.layout().pages.iter().map(|p| (p.body.x, p.body.y, p.body.w, p.body.h)).collect()
}

fn close(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01 && (a.2 - b.2).abs() < 0.01 && (a.3 - b.3).abs() < 0.01
}

#[test]
fn a_left_gutter_stays_on_the_left() {
    for (i, b) in bodies("", "", false).into_iter().enumerate() {
        assert!(close(b, (144.0, 72.0, 396.0, 648.0)), "page {}: {b:?}", i + 1);
    }
}

#[test]
fn gutter_at_top_widens_the_top_margin_on_every_page() {
    let pages = bodies("<w:gutterAtTop/>", "", false);
    assert!(pages.len() >= 2);
    for (i, b) in pages.into_iter().enumerate() {
        assert!(close(b, (72.0, 144.0, 468.0, 576.0)), "page {}: {b:?}", i + 1);
    }
}

#[test]
fn gutter_at_top_wins_over_mirror_margins() {
    for (i, b) in bodies("<w:mirrorMargins/><w:gutterAtTop/>", "", false).into_iter().enumerate() {
        assert!(close(b, (72.0, 144.0, 468.0, 576.0)), "page {}: {b:?}", i + 1);
    }
}

#[test]
fn gutter_at_top_wins_over_an_rtl_gutter() {
    for (i, b) in bodies("<w:gutterAtTop/>", "<w:rtlGutter/>", false).into_iter().enumerate() {
        assert!(close(b, (72.0, 144.0, 468.0, 576.0)), "page {}: {b:?}", i + 1);
    }
}

#[test]
fn gutter_at_top_off_keeps_the_gutter_on_the_left() {
    for (i, b) in bodies(r#"<w:gutterAtTop w:val="0"/>"#, "", false).into_iter().enumerate() {
        assert!(close(b, (144.0, 72.0, 396.0, 648.0)), "page {}: {b:?}", i + 1);
    }
}

#[test]
fn gutter_at_top_on_a_landscape_page() {
    for (i, b) in bodies("<w:gutterAtTop/>", "", true).into_iter().enumerate() {
        assert!(close(b, (72.0, 144.0, 648.0, 396.0)), "page {}: {b:?}", i + 1);
    }
}
