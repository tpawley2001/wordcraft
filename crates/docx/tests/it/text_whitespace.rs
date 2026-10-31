//! White space at the ends of `w:t` and `w:delText` (XML 1.0 §2.10, ECMA-376 Part 1 §17.3.3.31).
//! Fixtures are hand-written from the spec.

use std::io::Write;

use wordcraft_doc::StoryRef;

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

/// Without `xml:space="preserve"` the spaces at the ends of a text element are not part of the
/// text (Word and LibreOffice drop them); with it they are kept. Deleted text follows the same rule.
#[test]
fn unpreserved_end_spaces_are_dropped() {
    let body = r#"<w:p><w:r><w:t>Kept </w:t></w:r><w:r><w:t xml:space="preserve"> kept </w:t></w:r><w:r><w:t>
    indented</w:t></w:r><w:del w:id="1" w:author="A"><w:r><w:delText> gone</w:delText></w:r></w:del><w:r><w:t xml:space="preserve"> end</w:t></w:r></w:p>"#;
    let doc = wordcraft_docx::read(&docx(body)).unwrap();
    assert_eq!(doc.plain_text(StoryRef::Body), "Kept kept indentedgone end");
}

/// A document WordCraft wrote keeps its spaces through a save and reopen: the writer marks every
/// text element `xml:space="preserve"`.
#[test]
fn saved_spaces_survive_a_round_trip() {
    let body = r#"<w:p><w:r><w:t xml:space="preserve">  two leading, two trailing  </w:t></w:r></w:p>"#;
    let doc = wordcraft_docx::read(&docx(body)).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    assert_eq!(again.plain_text(StoryRef::Body), "  two leading, two trailing  ");
}
