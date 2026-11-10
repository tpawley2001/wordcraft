//! File › Options › Display › Print document properties (Word's default: off): a print of the
//! document is followed by the Document Properties report on its own page. A Document Info job
//! (`what`) prints only its report, as before; the job param `properties` overrides the option.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Three pages: two page breaks.
fn three_pages() -> Session {
    let body = r#"<w:p><w:r><w:t>One</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>Two</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>Three</w:t></w:r></w:p>"#;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    );
    let mut s = Session::new(wordcraft_docx::read_xml(xml.as_bytes()).expect("flat OPC reads"));
    s.doc.core.title = "Synthetic title".into();
    s
}

/// Pages in a PDF: page dictionaries are written uncompressed, each with one `/MediaBox[`.
fn pdf_pages(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes).matches("/MediaBox[").count()
}

fn job_pages(s: &Session, v: Value) -> usize {
    let (bytes, _) = s.print_job_pdf(&v).unwrap_or_else(|e| panic!("{v}: {e}"));
    pdf_pages(&bytes)
}

#[test]
fn off_by_default_and_the_document_prints_alone() {
    let s = three_pages();
    assert!(!s.prefs.print_properties, "Word's default is off");
    assert_eq!(s.export_layout().pages.len(), 3, "fixture has three pages");
    assert_eq!(job_pages(&s, json!({})), 3);
}

#[test]
fn the_option_appends_the_properties_page_after_the_document() {
    let mut s = three_pages();
    let rev = s.rev();
    let r = run(&mut s, "file.options", json!({"printProperties": true}));
    assert_eq!(r["printProperties"], json!(true), "file.options returns the setting");
    assert!(s.prefs.print_properties);
    assert_eq!(job_pages(&s, json!({})), 4, "document pages, then the report's page");
    let (saved, _) = s.print_pdf(&json!({})).expect("prints");
    assert_eq!(pdf_pages(&saved), 4, "Save these pages as PDF prints it too");
    assert_eq!(job_pages(&s, json!({"range": "custom", "pages": "2"})), 2, "the chosen page, then the report");
    assert_eq!(s.export_layout().pages.len(), 3, "the document's own layout is unchanged");
    assert_eq!(s.rev(), rev, "printing doesn't touch the document");
}

#[test]
fn the_job_param_overrides_the_option() {
    let mut s = three_pages();
    assert_eq!(job_pages(&s, json!({"properties": true})), 4);
    run(&mut s, "file.options", json!({"printProperties": true}));
    assert_eq!(job_pages(&s, json!({"properties": false})), 3);
}

#[test]
fn document_info_and_imposed_jobs_are_unchanged() {
    let mut s = three_pages();
    let jobs = [json!({"what": "properties"}), json!({"what": "styles"}), json!({"pagesPerSheet": 2})];
    let before: Vec<usize> = jobs.iter().map(|v| job_pages(&s, v.clone())).collect();
    run(&mut s, "file.options", json!({"printProperties": true}));
    let after: Vec<usize> = jobs.iter().map(|v| job_pages(&s, v.clone())).collect();
    assert_eq!(after, before, "{jobs:?}");
}

#[test]
fn the_option_persists_with_the_prefs() {
    let mut s = three_pages();
    run(&mut s, "file.options", json!({"printProperties": true}));
    let saved = serde_json::to_value(&s.prefs).unwrap();
    assert_eq!(saved["printProperties"], json!(true), "{saved}");
    let old: crate::prefs::Prefs = serde_json::from_value(json!({"printReverse": true})).unwrap();
    assert!(!old.print_properties, "old prefs load with Word's default");
    let shown = run(&mut s, "file.options", json!({}));
    assert_eq!(shown["printProperties"], json!(true));
}
