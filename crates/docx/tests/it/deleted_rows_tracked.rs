//! A table row deleted with tracking (`w:trPr/w:del`, ECMA-376 Part 1 §17.13.5.12): read into
//! `RowProps::del` with its author and date, written back inside `w:trPr` after the row
//! properties and before `w:trPrChange` (CT_TrPr). Fixtures are hand-written from the spec.

use std::io::{Read, Write};

use wordcraft_doc::RevisionKind;

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

fn row(trpr: &str, text: &str) -> String {
    format!(r#"<w:tr>{trpr}<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc></w:tr>"#)
}

fn table(rows: &[String]) -> String {
    format!(r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>{}</w:tbl><w:p/>"#, rows.concat())
}

/// Each row's deletion: (kind, author, date) of its revision.
fn deletions(d: &wordcraft_doc::Document) -> Vec<Option<(RevisionKind, String, String)>> {
    let t = d.body.iter().find_map(|b| b.as_table()).unwrap();
    t.rows.iter().map(|r| r.props.del.and_then(|i| d.revisions.get(i as usize)).map(|r| (r.kind, r.author.clone(), r.date.clone()))).collect()
}

const DEL: &str = r#"<w:del w:id="7" w:author="Ann" w:date="2026-01-02T03:04:00Z"/>"#;

#[test]
fn a_deleted_row_is_read_and_written_back() {
    let body = table(&[row("", "A"), row(&format!("<w:trPr>{DEL}</w:trPr>"), "B"), row("<w:trPr><w:cantSplit/></w:trPr>", "C")]);
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let ann = Some((RevisionKind::Delete, "Ann".to_string(), "2026-01-02T03:04:00Z".to_string()));
    assert_eq!(deletions(&d), vec![None, ann.clone(), None]);
    let out = wordcraft_docx::write(&d).unwrap();
    let xml = document_xml(&out);
    // A row with no other row property still gets a `w:trPr`.
    assert!(xml.contains(r#"<w:tr><w:trPr><w:del w:id="#), "{xml}");
    assert!(xml.contains(r#"w:author="Ann" w:date="2026-01-02T03:04:00Z"/></w:trPr>"#), "{xml}");
    assert_eq!(xml.matches("<w:del ").count(), 1, "{xml}");
    let back = wordcraft_docx::read(&out).unwrap();
    assert_eq!(deletions(&back), vec![None, ann, None]);
}

#[test]
fn the_deletion_follows_the_row_properties_and_precedes_a_formatting_change() {
    let change = r#"<w:trPrChange w:id="8" w:author="Bob" w:date="2026-02-03T04:05:00Z"><w:trPr/></w:trPrChange>"#;
    let body = table(&[row(&format!("<w:trPr><w:cantSplit/>{DEL}{change}</w:trPr>"), "A")]);
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    let t = d.body.iter().find_map(|b| b.as_table()).unwrap();
    let (del, fmt) = (t.rows[0].props.del.unwrap(), t.rows[0].props.fmt_change.as_ref().unwrap().rev);
    assert!(del < fmt, "revisions in the file's order");
    let xml = document_xml(&wordcraft_docx::write(&d).unwrap());
    let (c, x, f) = (xml.find("<w:cantSplit/>").unwrap(), xml.find("<w:del ").unwrap(), xml.find("<w:trPrChange ").unwrap());
    assert!(c < x && x < f, "{xml}");
    // Ids follow the order written.
    let id = |at: usize| xml[at..].split("w:id=\"").nth(1).and_then(|s| s.split('"').next()).and_then(|s| s.parse::<u32>().ok()).unwrap();
    assert!(id(x) < id(f), "{xml}");
}

#[test]
fn rows_without_a_deletion_are_written_as_before() {
    let body = table(&[row("", "A"), row("<w:trPr><w:cantSplit/></w:trPr>", "B")]);
    let d = wordcraft_docx::read(&docx(&body)).unwrap();
    assert_eq!(deletions(&d), vec![None, None]);
    assert!(d.revisions.is_empty());
    let out = wordcraft_docx::write(&d).unwrap();
    let xml = document_xml(&out);
    assert!(!xml.contains("<w:del"), "{xml}");
    assert_eq!(xml.matches("<w:trPr>").count(), 1, "only the row with a property has one: {xml}");
    // Saved again, the same bytes.
    let again = document_xml(&wordcraft_docx::write(&wordcraft_docx::read(&out).unwrap()).unwrap());
    assert_eq!(xml, again);
    // The model serializes as before: no field for a row that isn't deleted.
    let t = d.body.iter().find_map(|b| b.as_table()).unwrap();
    assert!(!serde_json::to_string(&t.rows[0].props).unwrap().contains("del"));
}
