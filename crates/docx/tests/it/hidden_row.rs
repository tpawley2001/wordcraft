//! A table row whose end-of-row mark is hidden text (`w:trPr/w:hidden`, ECMA-376 Part 1 §17.4.20
//! "Hidden Table Row Marker") reads as `RowProps::hidden` and is written back; a row without it
//! writes no `w:hidden`. Fixtures are hand-written.

use std::io::Write;

use wordcraft_doc::Document;

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn docx(body: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}</w:body></w:document>"#);
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, part) in [("_rels/.rels", root), ("word/document.xml", doc.as_str())] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(part.as_bytes()).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut z.by_name("word/document.xml").unwrap(), &mut xml).unwrap();
    xml
}

/// A two-row table: the first row's `w:trPr` holds `first`, the second has none.
fn read(first: &str) -> Document {
    let cell = r#"<w:tc><w:p><w:r><w:rPr><w:vanish/></w:rPr><w:t>a</w:t></w:r></w:p></w:tc>"#;
    let body = format!(
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:trPr>{first}</w:trPr>{cell}</w:tr><w:tr>{cell}</w:tr></w:tbl><w:p/>"#
    );
    wordcraft_docx::read(&docx(&body)).unwrap()
}

fn hidden(d: &Document) -> Vec<bool> {
    d.body.iter().find_map(|b| b.as_table()).unwrap().rows.iter().map(|r| r.props.hidden).collect()
}

#[test]
fn a_hidden_row_mark_reads_and_round_trips() {
    let d = read("<w:hidden/>");
    assert_eq!(hidden(&d), vec![true, false]);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = document_xml(&bytes);
    assert_eq!(xml.matches("<w:hidden/>").count(), 1, "only the hidden row writes w:hidden: {xml}");
    assert_eq!(hidden(&wordcraft_docx::read(&bytes).unwrap()), vec![true, false]);
}

#[test]
fn an_off_hidden_row_mark_is_not_hidden() {
    assert_eq!(hidden(&read(r#"<w:hidden w:val="0"/>"#)), vec![false, false]);
    assert_eq!(hidden(&read(r#"<w:hidden w:val="false"/>"#)), vec![false, false]);
    let d = read("<w:cantSplit/>");
    assert_eq!(hidden(&d), vec![false, false]);
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(!xml.contains("<w:hidden"), "no row writes w:hidden: {xml}");
}
