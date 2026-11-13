//! `w:noTabHangInd` (a legacy `w:compat` option, ECMA-376 Part 1 §17.15.1) is read into the
//! document settings (serialized as `noTabHangInd`) and written back, in CT_Compat's order:
//! after `w:wpJustification`, before `w:noLeading`.

use wordcraft_doc::Document;

fn settings_xml(d: &Document) -> String {
    let bytes = wordcraft_docx::write(d).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

fn rt(d: &Document) -> Document {
    wordcraft_docx::read(&wordcraft_docx::write(d).unwrap()).unwrap()
}

/// Whether the document's settings carry the flag.
fn flag(d: &Document) -> bool {
    serde_json::to_value(&d.settings).unwrap()["noTabHangInd"].as_bool().unwrap_or(false)
}

/// A new document with the flag set through the settings' serialized form.
fn flagged() -> Document {
    let mut d = Document::new();
    let mut v = serde_json::to_value(&d.settings).unwrap();
    v["noTabHangInd"] = serde_json::Value::Bool(true);
    d.settings = serde_json::from_value(v).unwrap();
    d
}

/// A document saved with the flag, its `w:noTabHangInd` element replaced by `element`.
fn with_element(element: &str) -> Vec<u8> {
    let bytes = wordcraft_docx::write(&flagged()).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut data).unwrap();
        if name == "word/settings.xml" {
            let xml = String::from_utf8(data).unwrap();
            assert!(xml.contains("<w:noTabHangInd/>"), "{xml}");
            data = xml.replace("<w:noTabHangInd/>", element).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    out.finish().unwrap().into_inner()
}

#[test]
fn no_tab_hang_ind_reads_on_off_and_absent() {
    assert!(flag(&wordcraft_docx::read(&with_element("<w:noTabHangInd/>")).unwrap()));
    assert!(flag(&wordcraft_docx::read(&with_element(r#"<w:noTabHangInd w:val="1"/>"#)).unwrap()));
    assert!(flag(&wordcraft_docx::read(&with_element(r#"<w:noTabHangInd w:val="true"/>"#)).unwrap()));
    assert!(!flag(&wordcraft_docx::read(&with_element(r#"<w:noTabHangInd w:val="0"/>"#)).unwrap()));
    assert!(!flag(&wordcraft_docx::read(&with_element(r#"<w:noTabHangInd w:val="false"/>"#)).unwrap()));
    assert!(!flag(&wordcraft_docx::read(&with_element("")).unwrap()));
}

#[test]
fn no_tab_hang_ind_round_trips() {
    let d = Document::new();
    assert!(!flag(&rt(&d)));
    assert!(!settings_xml(&d).contains("noTabHangInd"));
    let d = flagged();
    assert!(flag(&rt(&d)));
    assert_eq!(settings_xml(&d).matches("<w:noTabHangInd").count(), 1);
    // Saved twice, still once.
    assert_eq!(settings_xml(&rt(&d)).matches("<w:noTabHangInd").count(), 1);
}

#[test]
fn no_tab_hang_ind_keeps_the_compatibility_mode() {
    let mut d = flagged();
    d.settings.compat_mode = 11;
    let back = rt(&d);
    assert!(flag(&back));
    assert_eq!(back.settings.compat_mode, 11);
}

#[test]
fn no_tab_hang_ind_is_written_in_schema_order() {
    let mut d = flagged();
    d.settings.wp_justification = true;
    d.settings.no_leading = true;
    d.settings.do_not_expand_shift_return = true;
    let xml = settings_xml(&d);
    let at = |e: &str| xml.find(e).unwrap_or_else(|| panic!("{e} missing in {xml}"));
    assert!(at("<w:compat>") < at("<w:noTabHangInd"));
    assert!(at("<w:wpJustification") < at("<w:noTabHangInd"));
    assert!(at("<w:noTabHangInd") < at("<w:noLeading"));
    assert!(at("<w:noTabHangInd") < at("<w:doNotExpandShiftReturn"));
    assert!(at("<w:noTabHangInd") < at("<w:compatSetting"));
}
