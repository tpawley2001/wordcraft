//! A Word 2007 (VML) WordArt shape inside a VML text box. Word's own file keeps the WordArt as a
//! `v:shape` with a `v:textpath` inside the box's `w:txbxContent`, so there is only one level of
//! `w:txbxContent`. A save must not write the WordArt as a DrawingML text box (`wps:txbx` with its
//! own `w:txbxContent`) inside the outer box's `w:txbxContent`: Word crashes opening such a file.
//! Fixtures are hand-written from ECMA-376 Part 4 (VML `textbox`, `textpath`) and Part 1 §17.17.1.1
//! (`txbxContent`).

use std::io::{Read, Write};

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn read_body(body: &str) -> wordcraft_doc::Document {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:w10="urn:schemas-microsoft-com:office:word"><w:body>{body}</w:body></w:document>"#
    );
    wordcraft_docx::read(&zip(&[("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())])).unwrap()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// The deepest nesting of `w:txbxContent` elements in `xml`.
fn txbx_depth(xml: &str) -> usize {
    let (mut depth, mut max) = (0usize, 0usize);
    let mut rest = xml;
    while let Some(i) = rest.find("w:txbxContent") {
        let closing = rest[..i].ends_with("</");
        let after = &rest[i + "w:txbxContent".len()..];
        let self_closing = !closing && after.split('>').next().is_some_and(|t| t.ends_with('/'));
        if closing {
            depth = depth.saturating_sub(1);
        } else if !self_closing && rest[..i].ends_with('<') {
            depth += 1;
            max = max.max(depth);
        }
        rest = after;
    }
    max
}

const WORDART_SHAPETYPE: &str = r#"<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:path textpathok="t" o:connecttype="custom"/><v:textpath on="t" fitshape="t"/></v:shapetype>"#;

fn wordart(text: &str) -> String {
    format!(
        r##"<w:p><w:r><w:pict>{WORDART_SHAPETYPE}<v:shape id="art1" type="#_x0000_t136" style="width:120pt;height:30pt" fillcolor="black"><v:textpath style="font-family:&quot;Arial&quot;;font-size:20pt" string="{text}"/></v:shape></w:pict></w:r></w:p>"##
    )
}

/// A VML text box holding a paragraph `Outer` and then `inner` (block content).
fn vml_text_box(inner: &str) -> String {
    format!(
        r##"<w:p><w:r><w:pict><v:shape id="box1" type="#_x0000_t202" style="position:absolute;width:200pt;height:120pt"><v:textbox><w:txbxContent><w:p><w:r><w:t>Outer</w:t></w:r></w:p>{inner}</w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>"##
    )
}

#[test]
fn wordart_in_a_vml_text_box_saves_without_a_nested_txbx_content() {
    let d = read_body(&vml_text_box(&wordart("SAMPLE")));
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(xml.contains("Outer"), "the box's own text is saved");
    assert!(xml.contains("SAMPLE"), "the WordArt's text is saved");
    assert_eq!(txbx_depth(&xml), 1, "no w:txbxContent inside another w:txbxContent: {xml}");
}

#[test]
fn wordart_in_a_text_box_keeps_its_text_through_a_second_round_trip() {
    let d = read_body(&vml_text_box(&wordart("SAMPLE")));
    let once = wordcraft_docx::write(&d).unwrap();
    let twice = wordcraft_docx::write(&wordcraft_docx::read(&once).unwrap()).unwrap();
    let xml = document_xml(&twice);
    assert!(xml.contains("Outer") && xml.contains("SAMPLE"), "{xml}");
    assert_eq!(txbx_depth(&xml), 1, "{xml}");
}

#[test]
fn two_wordart_shapes_in_one_text_box_both_survive_unnested() {
    let d = read_body(&vml_text_box(&format!("{}{}", wordart("ONE"), wordart("TWO"))));
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(xml.contains("ONE") && xml.contains("TWO"), "{xml}");
    assert_eq!(txbx_depth(&xml), 1, "{xml}");
}

#[test]
fn wordart_outside_a_text_box_still_saves_with_its_text() {
    // Baseline: a top-level WordArt keeps saving as it does today (one txbxContent level at most).
    let d = read_body(&wordart("SAMPLE"));
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(xml.contains("SAMPLE"), "{xml}");
    assert!(txbx_depth(&xml) <= 1, "{xml}");
}
