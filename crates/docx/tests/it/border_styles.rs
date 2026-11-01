//! ECMA-376 Part 1, 17.18.2 `ST_Border` line styles: each one Word can write survives a read and a save,
//! on paragraph borders and on table cell borders. Hand-written fixtures from the spec.

use std::io::{Read, Write};

const STYLES: &[&str] = &[
    "dotDotDash",
    "doubleWave",
    "dashDotStroked",
    "thinThickSmallGap",
    "thickThinSmallGap",
    "thinThickThinSmallGap",
    "thinThickMediumGap",
    "thickThinMediumGap",
    "thinThickThinMediumGap",
    "thinThickLargeGap",
    "thickThinLargeGap",
    "thinThickThinLargeGap",
    "threeDEmboss",
    "threeDEngrave",
    "outset",
    "inset",
];

fn package(val: &str) -> Vec<u8> {
    let w = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{w}"><w:body><w:p><w:pPr><w:pBdr><w:top w:val="{val}" w:sz="24" w:space="1" w:color="auto"/></w:pBdr></w:pPr><w:r><w:t>Bordered</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/><w:tcBorders><w:left w:val="{val}" w:sz="12" w:space="0" w:color="auto"/></w:tcBorders></w:tcPr><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/></w:body></w:document>"#
    );
    let ct = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [("[Content_Types].xml", ct), ("_rels/.rels", rels), ("word/document.xml", doc.as_str())] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The `w:val` of the first `<w:{side} …>` element inside `<w:{container}>`.
fn border_val(xml: &str, container: &str, side: &str) -> Option<String> {
    let inner = xml.split(&format!("<w:{container}>")).nth(1)?.split(&format!("</w:{container}>")).next()?;
    let el = inner.split(&format!("<w:{side} ")).nth(1)?;
    Some(el.split("w:val=\"").nth(1)?.split('"').next()?.to_string())
}

fn round_trip(val: &str) -> (String, String) {
    let once = wordcraft_docx::write(&wordcraft_docx::read(&package(val)).unwrap()).unwrap();
    let twice = wordcraft_docx::write(&wordcraft_docx::read(&once).unwrap()).unwrap();
    (document_xml(&once), document_xml(&twice))
}

#[test]
fn word_line_styles_survive_open_and_save() {
    let mut lost = Vec::new();
    for val in STYLES {
        let (once, twice) = round_trip(val);
        for (xml, pass) in [(&once, "first"), (&twice, "second")] {
            let p = border_val(xml, "pBdr", "top");
            let c = border_val(xml, "tcBorders", "left");
            if p.as_deref() != Some(*val) {
                lost.push(format!("{val}: paragraph border saved as {p:?} ({pass} save)"));
            }
            if c.as_deref() != Some(*val) {
                lost.push(format!("{val}: cell border saved as {c:?} ({pass} save)"));
            }
        }
    }
    assert!(lost.is_empty(), "{}", lost.join("\n"));
}

#[test]
fn existing_styles_and_unknown_values_are_unchanged() {
    for (input, expected) in [("single", "single"), ("double", "double"), ("triple", "triple"), ("bogusStyle", "single")] {
        let (once, _) = round_trip(input);
        assert_eq!(border_val(&once, "pBdr", "top").as_deref(), Some(expected), "{input}");
        assert_eq!(border_val(&once, "tcBorders", "left").as_deref(), Some(expected), "{input}");
    }
}
