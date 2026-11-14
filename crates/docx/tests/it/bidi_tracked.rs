//! PR #6300 review: a direction character (U+202A..U+202E) that carries a tracked insertion or
//! deletion stays revised text on save instead of becoming a bare `w:dir` / `w:bdo` element
//! (ECMA-376 Part 1 §17.3.2.8, §17.3.2.3; `w:ins` §17.13.5.18, `w:del` §17.13.5.14).
//! Fixtures are hand-written from the spec.

use std::io::{Read, Write};

use wordcraft_doc::{Block, Document, Paragraph};

const RLE: char = '\u{202B}';
const PDF: char = '\u{202C}';
const INS: &str = r#"w:id="1" w:author="Quill Ramsey" w:date="2026-03-04T05:06:07Z""#;

fn docx(body: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    );
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

fn first_para(d: &Document) -> &Paragraph {
    d.body.iter().find_map(|b| if let Block::Para(p) = &**b { Some(p) } else { None }).unwrap()
}

/// The `CharProps` of the run holding byte offset `at`.
fn props_at(p: &Paragraph, at: usize) -> wordcraft_doc::props::CharProps {
    p.run_ranges().find(|(r, _)| r.contains(&at)).map(|(_, c)| c.clone()).unwrap()
}

/// The text between each `<tag` opening and its `</tag>`, for every occurrence.
fn inside(xml: &str, tag: &str) -> Vec<String> {
    xml.split(&format!("<{tag} ")).skip(1).map(|s| s.split(&format!("</{tag}>")).next().unwrap().to_string()).collect()
}

fn r(t: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{t}</w:t></w:r>"#)
}

#[test]
fn an_inserted_embedding_pair_stays_inserted_text() {
    let body = format!(r#"<w:p>{}<w:ins {INS}>{}</w:ins>{}</w:p>"#, r("x"), r(&format!("{RLE}abc{PDF}")), r("y"));
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let p = first_para(&d);
    assert_eq!(p.text, format!("x{RLE}abc{PDF}y"));
    let rle_at = p.text.find(RLE).unwrap();
    assert!(props_at(p, rle_at).ins.is_some(), "fixture: the RLE is read as inserted");

    let saved = wordcraft_docx::write(&d).unwrap();
    let xml = document_xml(&saved);
    assert!(!xml.contains("<w:dir"), "a tracked RLE/PDF pair must not become a bare w:dir: {xml}");
    let ins = inside(&xml, "w:ins").concat();
    assert!(ins.contains(RLE) && ins.contains(PDF), "the RLE and PDF are written inside w:ins: {xml}");

    let again = wordcraft_docx::read(&saved).unwrap();
    let q = first_para(&again);
    assert_eq!(q.text, p.text);
    for c in [RLE, PDF] {
        let at = q.text.find(c).unwrap();
        assert!(props_at(q, at).ins.is_some(), "{c:?} keeps its insertion after a round trip");
    }
}

#[test]
fn a_deleted_embedding_pair_stays_deleted_text() {
    let body = format!(r#"<w:p>{}<w:del {INS}><w:r><w:delText xml:space="preserve">{RLE}abc{PDF}</w:delText></w:r></w:del>{}</w:p>"#, r("x"), r("y"));
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let p = first_para(&d);
    let rle_at = p.text.find(RLE).unwrap();
    assert!(props_at(p, rle_at).del.is_some(), "fixture: the RLE is read as deleted");

    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    assert!(!xml.contains("<w:dir"), "a deleted RLE/PDF pair must not become a bare w:dir: {xml}");
    let del = inside(&xml, "w:del").concat();
    assert!(del.contains("<w:delText") && del.contains(RLE) && del.contains(PDF), "RLE and PDF stay deleted text: {xml}");
}

#[test]
fn revised_content_between_unrevised_ends_still_writes_the_element() {
    let body = format!(r#"<w:p>{}<w:dir w:val="rtl"><w:ins {INS}>{}</w:ins></w:dir>{}</w:p>"#, r("x"), r("abc"), r("y"));
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let p = first_para(&d);
    assert_eq!(p.text, format!("x{RLE}abc{PDF}y"));
    assert!(props_at(p, p.text.find(RLE).unwrap()).ins.is_none(), "fixture: the RLE itself is not revised");

    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    let dir = inside(&xml, "w:dir");
    assert_eq!(dir.len(), 1, "one w:dir: {xml}");
    assert!(dir[0].contains("<w:ins") && dir[0].contains("abc"), "the inserted run sits inside the w:dir: {xml}");
}
