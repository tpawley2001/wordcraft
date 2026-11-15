//! Form Field Shading off (`w:doNotShadeFormData`, ECMA-376 Part 1 §17.15.1.42) is a document
//! setting the model owns: it is read, written at its CT_Settings position, and dropped when
//! turned off, also when the original settings part is merged on save.

use std::io::Read;

fn flat_opc(settings: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/_rels/document.xml.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>a</w:t></w:r></w:p></w:body></w:document></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/settings.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml">
    <pkg:xmlData><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{settings}</w:settings></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    )
}

fn read(settings: &str) -> wordcraft_doc::Document {
    wordcraft_docx::read_xml(flat_opc(settings).as_bytes()).expect("flat OPC reads")
}

fn settings_xml(bytes: &[u8]) -> String {
    let mut xml = String::new();
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap().by_name("word/settings.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml
}

#[test]
fn reads_the_setting() {
    let cases = [
        ("", false),
        ("<w:doNotShadeFormData/>", true),
        (r#"<w:doNotShadeFormData w:val="1"/>"#, true),
        (r#"<w:doNotShadeFormData w:val="true"/>"#, true),
        (r#"<w:doNotShadeFormData w:val="0"/>"#, false),
        (r#"<w:doNotShadeFormData w:val="false"/>"#, false),
    ];
    for (xml, want) in cases {
        assert_eq!(read(xml).settings.do_not_shade_form_data, want, "{xml:?}");
    }
}

#[test]
fn writes_and_reads_back() {
    let mut d = read("");
    let plain = wordcraft_docx::write(&d).expect("saves");
    assert!(!settings_xml(&plain).contains("doNotShadeFormData"), "off: not written");
    d.settings.do_not_shade_form_data = true;
    let on = wordcraft_docx::write(&d).expect("saves");
    let xml = settings_xml(&on);
    assert_eq!(xml.matches("<w:doNotShadeFormData/>").count(), 1, "{xml}");
    assert!(wordcraft_docx::read(&on).expect("reads").settings.do_not_shade_form_data, "read back");
}

#[test]
fn written_at_its_ct_settings_place() {
    // §17.15.1 order: ... w:drawingGridVerticalOrigin, w:doNotShadeFormData, w:noPunctuationKerning ...
    let mut d = read(r#"<w:zoom w:percent="100"/><w:noPunctuationKerning/>"#);
    d.settings.do_not_shade_form_data = true;
    let xml = settings_xml(&wordcraft_docx::write(&d).expect("saves"));
    let at = xml.find("<w:doNotShadeFormData/>").unwrap_or_else(|| panic!("written: {xml}"));
    if let Some(z) = xml.find("<w:zoom") {
        assert!(z < at, "after w:zoom: {xml}");
    }
    if let Some(k) = xml.find("<w:noPunctuationKerning") {
        assert!(at < k, "before w:noPunctuationKerning: {xml}");
    }
}

#[test]
fn turned_off_the_original_element_goes() {
    // The rsids make the settings part one the model doesn't fully own, so the save merges it.
    let mut d = read(r#"<w:doNotShadeFormData/><w:rsids><w:rsidRoot w:val="00A1B2C3"/></w:rsids>"#);
    assert!(d.settings.do_not_shade_form_data);
    d.settings.do_not_shade_form_data = false;
    let xml = settings_xml(&wordcraft_docx::write(&d).expect("saves"));
    assert!(!xml.contains("doNotShadeFormData"), "{xml}");
    assert!(xml.contains("00A1B2C3"), "the rest of the original part is kept: {xml}");
}
