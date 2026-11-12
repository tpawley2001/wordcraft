//! `<w:wpJustification w:val="0"/>` (ECMA-376 Part 1 §17.15.1, an on/off compat option) reads
//! as off, and `<w:wpJustification/>` as on.

use wordcraft_doc::Document;

/// A document saved with the flag, its `w:wpJustification` element replaced by `element`.
fn with_element(element: &str) -> Vec<u8> {
    let mut d = Document::new();
    d.settings.wp_justification = true;
    let src = wordcraft_docx::write(&d).unwrap();
    let mut zin = zip::ZipArchive::new(std::io::Cursor::new(src)).unwrap();
    let mut bytes = Vec::new();
    {
        let mut zout = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
        for i in 0..zin.len() {
            let mut f = zin.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut data).unwrap();
            if name == "word/settings.xml" {
                let xml = String::from_utf8(data).unwrap();
                assert!(xml.contains("<w:wpJustification/>"), "{xml}");
                data = xml.replace("<w:wpJustification/>", element).into_bytes();
            }
            zout.start_file(name.as_str(), zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zout, &data).unwrap();
        }
        zout.finish().unwrap();
    }
    bytes
}

#[test]
fn val_0_reads_off() {
    assert!(!wordcraft_docx::read(&with_element(r#"<w:wpJustification w:val="0"/>"#)).unwrap().settings.wp_justification);
    assert!(!wordcraft_docx::read(&with_element(r#"<w:wpJustification w:val="false"/>"#)).unwrap().settings.wp_justification);
}

#[test]
fn val_1_and_bare_read_on() {
    assert!(wordcraft_docx::read(&with_element("<w:wpJustification/>")).unwrap().settings.wp_justification);
    assert!(wordcraft_docx::read(&with_element(r#"<w:wpJustification w:val="1"/>"#)).unwrap().settings.wp_justification);
}
