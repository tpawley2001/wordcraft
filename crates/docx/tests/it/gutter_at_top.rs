//! Gutter position Top: `w:settings/w:gutterAtTop` (ECMA-376 Part 1 §17.15.1.49, on/off) is read
//! into `Settings::gutter_at_top` and written back once, after `w:mirrorMargins`, both when the
//! settings part is generated and when it is merged into a file's original one.

use wordcraft_doc::Document;

fn settings_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

/// A saved document whose settings part has `extra` added before `</w:settings>`, read back.
fn with_settings(extra: &str) -> Document {
    let bytes = wordcraft_docx::write(&Document::new()).unwrap();
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut data).unwrap();
        if name == "word/settings.xml" {
            data = String::from_utf8(data).unwrap().replacen("</w:settings>", &format!("{extra}</w:settings>"), 1).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    wordcraft_docx::read(&out.finish().unwrap().into_inner()).unwrap()
}

#[test]
fn read_on_off_and_absent() {
    assert!(!with_settings("").settings.gutter_at_top);
    assert!(with_settings("<w:gutterAtTop/>").settings.gutter_at_top);
    assert!(with_settings(r#"<w:gutterAtTop w:val="true"/>"#).settings.gutter_at_top);
    assert!(!with_settings(r#"<w:gutterAtTop w:val="0"/>"#).settings.gutter_at_top);
}

#[test]
fn generated_settings_write_it_once_after_mirror_margins() {
    let mut d = Document::new();
    d.settings.gutter_at_top = true;
    d.settings.mirror_margins = true;
    let xml = settings_xml(&wordcraft_docx::write(&d).unwrap());
    assert_eq!(xml.matches("<w:gutterAtTop").count(), 1, "{xml}");
    assert!(xml.find("<w:mirrorMargins").unwrap() < xml.find("<w:gutterAtTop").unwrap(), "{xml}");
    assert!(!settings_xml(&wordcraft_docx::write(&Document::new()).unwrap()).contains("gutterAtTop"));
}

#[test]
fn a_merged_settings_part_keeps_it_once() {
    // `w:rsids` is a setting the model doesn't hold, so the original part is merged on save.
    let d = with_settings(r#"<w:gutterAtTop/><w:rsids><w:rsidRoot w:val="00A1B2C3"/></w:rsids>"#);
    assert!(d.settings.gutter_at_top);
    let bytes = wordcraft_docx::write(&d).unwrap();
    let xml = settings_xml(&bytes);
    assert_eq!(xml.matches("<w:gutterAtTop").count(), 1, "{xml}");
    assert!(xml.contains("w:rsidRoot"), "merged: {xml}");
    assert!(wordcraft_docx::read(&bytes).unwrap().settings.gutter_at_top);
    // Turned off, it goes.
    let mut off = d.clone();
    off.settings.gutter_at_top = false;
    assert!(!settings_xml(&wordcraft_docx::write(&off).unwrap()).contains("gutterAtTop"));
}
