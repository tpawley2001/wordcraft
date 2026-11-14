//! `w:characterSpacingControl` (ECMA-376 Part 1 §17.15.1.18) is read into `Settings::char_spacing`
//! and saved back; absent or unknown values read as `doNotCompress`, and a new document saves
//! `doNotCompress`.

use wordcraft_doc::{CharSpacing, Document};

fn settings_xml(d: &Document) -> String {
    let bytes = wordcraft_docx::write(d).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/settings.xml").unwrap(), &mut xml).unwrap();
    xml
}

/// A saved new document whose `w:characterSpacingControl` is replaced by `element` (may be empty).
fn with_element(element: &str) -> Document {
    let bytes = wordcraft_docx::write(&Document::new()).unwrap();
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..zin.len() {
        let mut f = zin.by_index(i).unwrap();
        let name = f.name().to_string();
        let mut data = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut data).unwrap();
        if name == "word/settings.xml" {
            let xml = String::from_utf8(data).unwrap();
            let old = r#"<w:characterSpacingControl w:val="doNotCompress"/>"#;
            assert!(xml.contains(old), "{xml}");
            data = xml.replacen(old, element, 1).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        std::io::Write::write_all(&mut out, &data).unwrap();
    }
    wordcraft_docx::read(&out.finish().unwrap().into_inner()).unwrap()
}

const ALL: [CharSpacing; 3] = [CharSpacing::DoNotCompress, CharSpacing::CompressPunctuation, CharSpacing::CompressPunctuationAndJapaneseKana];

#[test]
fn reads_each_value() {
    for cs in ALL {
        let d = with_element(&format!(r#"<w:characterSpacingControl w:val="{}"/>"#, cs.to_ooxml()));
        assert_eq!(d.settings.char_spacing, cs);
    }
}

#[test]
fn absent_or_unknown_reads_as_do_not_compress() {
    assert_eq!(with_element("").settings.char_spacing, CharSpacing::DoNotCompress);
    assert_eq!(with_element(r#"<w:characterSpacingControl w:val="squeezeEverything"/>"#).settings.char_spacing, CharSpacing::DoNotCompress);
    assert_eq!(with_element("<w:characterSpacingControl/>").settings.char_spacing, CharSpacing::DoNotCompress);
}

#[test]
fn every_value_round_trips_once() {
    for cs in ALL {
        let mut d = Document::new();
        d.settings.char_spacing = cs;
        let xml = settings_xml(&d);
        assert_eq!(xml.matches("w:characterSpacingControl").count(), 1, "{xml}");
        assert!(xml.contains(&format!(r#"<w:characterSpacingControl w:val="{}"/>"#, cs.to_ooxml())), "{xml}");
        let back = wordcraft_docx::read(&wordcraft_docx::write(&d).unwrap()).unwrap();
        assert_eq!(back.settings.char_spacing, cs);
        // Saved again (the settings part is merged with the one read): still one element.
        let xml = settings_xml(&back);
        assert_eq!(xml.matches("w:characterSpacingControl").count(), 1, "{xml}");
        assert!(xml.contains(cs.to_ooxml()), "{xml}");
    }
}

#[test]
fn a_new_document_writes_do_not_compress() {
    assert!(settings_xml(&Document::new()).contains(r#"<w:characterSpacingControl w:val="doNotCompress"/>"#));
}
