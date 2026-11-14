//! Fit Text (`w:fitText`, ISO/IEC 29500-1 §17.3.2.14: `w:val` = width in twips, `w:id` = the region
//! id shared by runs next to each other) is read and saved again, once, in its place in `w:rPr`
//! (after `w:shd`, before `w:vertAlign`). Fixtures are hand-written from the spec.

use std::io::{Read, Write};

use wordcraft_doc::props::FitText;

fn docx(body: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    );
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, xml) in [("_rels/.rels", root.to_string()), ("word/document.xml", doc)] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(xml.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn fits(d: &wordcraft_doc::Document) -> Vec<Option<FitText>> {
    d.body[0].as_para().unwrap().runs.iter().map(|r| r.props.fit_text).collect()
}

fn document_xml(saved: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(saved)).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml
}

#[test]
fn fit_text_is_read_and_saved_in_its_place() {
    let run = |t: &str, fit: &str| format!(r#"<w:r><w:rPr><w:b/>{fit}<w:vertAlign w:val="baseline"/></w:rPr><w:t>{t}</w:t></w:r>"#);
    let body =
        format!("<w:p>{}{}{}</w:p>", run("ab", r#"<w:fitText w:val="1440" w:id="7"/>"#), run("cd", r#"<w:fitText w:val="720"/>"#), run("ef", ""),);
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let want = vec![Some(FitText { width: 72.0, id: Some(7) }), Some(FitText { width: 36.0, id: None }), None];
    assert_eq!(fits(&d), want);
    let saved = wordcraft_docx::write(&d).unwrap();
    let xml = document_xml(&saved);
    assert!(xml.contains(r#"<w:fitText w:val="1440" w:id="7"/><w:vertAlign"#), "{xml}");
    assert!(xml.contains(r#"<w:fitText w:val="720"/><w:vertAlign"#), "{xml}");
    assert_eq!(xml.matches("<w:fitText").count(), 2, "written once per run: {xml}");
    assert_eq!(fits(&wordcraft_docx::read(&saved).unwrap()), want);
}

#[test]
fn fit_text_without_a_usable_width_is_ignored() {
    let body =
        r#"<w:p><w:r><w:rPr><w:fitText w:id="3"/></w:rPr><w:t>a</w:t></w:r><w:r><w:rPr><w:b/><w:fitText w:val="x"/></w:rPr><w:t>b</w:t></w:r></w:p>"#;
    let d = wordcraft_docx::read(&docx(body)).unwrap();
    assert!(fits(&d).iter().all(Option::is_none), "{:?}", fits(&d));
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(!xml.contains("w:fitText"), "{xml}");
}
