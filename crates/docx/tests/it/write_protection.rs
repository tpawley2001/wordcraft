//! Read-only recommended and Password to modify (`w:writeProtection`, ECMA-376 Part 1
//! §17.15.1.93): read and written in both hash forms, and recommended only. Synthetic fixtures
//! written for these tests from the spec; the names are made up.
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;
const RT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";

fn zip(entries: &[(&str, String)]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        z.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(text.as_bytes()).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            parts.insert(f.name().to_string(), s);
        }
    }
    parts
}

/// A package whose settings hold `write_protection` (the whole element).
fn fixture(write_protection: &str) -> Vec<u8> {
    let root_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{RT}officeDocument" Target="word/document.xml"/></Relationships>"#
    );
    let doc_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{RT}settings" Target="settings.xml"/></Relationships>"#
    );
    zip(&[
        ("_rels/.rels", root_rels),
        ("word/document.xml", format!(r#"<w:document {W_NS}><w:body><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:body></w:document>"#)),
        ("word/_rels/document.xml.rels", doc_rels),
        ("word/settings.xml", format!(r#"<w:settings {W_NS}>{write_protection}<w:defaultTabStop w:val="720"/></w:settings>"#)),
    ])
}

fn settings_of(doc: &wordcraft_doc::Document) -> String {
    unzip(&wordcraft_docx::write(doc).unwrap()).remove("word/settings.xml").unwrap_or_default()
}

#[test]
fn a_modify_password_is_read_and_written_in_the_current_form() {
    let attrs = wordcraft_docx::protection_hash("open sesame", 1000).unwrap();
    let mut el = String::from("<w:writeProtection");
    for (k, v) in &attrs {
        el.push_str(&format!(r#" {k}="{v}""#));
    }
    el.push_str("/>");
    let doc = wordcraft_docx::read(&fixture(&el)).unwrap();
    let wp = doc.settings.write_protection.as_ref().expect("read");
    assert!(!wp.recommended);
    assert_eq!(wp.hash, attrs);
    assert_eq!(wordcraft_docx::check_protection_password(&wp.hash, "open sesame"), wordcraft_docx::PasswordCheck::Correct);
    // Written back first in `w:settings`, before `w:view`/`w:zoom` and the rest.
    let s = settings_of(&doc);
    let at = s.find("<w:writeProtection").expect("written");
    assert!(at < s.find("<w:defaultTabStop").unwrap(), "{s}");
    for (k, v) in &attrs {
        assert!(s.contains(&format!(r#"{k}="{v}""#)), "{k} lost: {s}");
    }
    let again = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    assert_eq!(again.settings.write_protection, doc.settings.write_protection);
}

#[test]
fn a_modify_password_is_read_and_written_in_the_legacy_form() {
    // ECMA-376 Part 4 §14.8.1: the older `crypt*`/`hash`/`salt` set.
    let el = r#"<w:writeProtection w:cryptProviderType="rsaFull" w:cryptAlgorithmClass="hash" w:cryptAlgorithmType="typeAny" w:cryptAlgorithmSid="4" w:cryptSpinCount="100000" w:hash="A1B2C3D4" w:salt="E5F6"/>"#;
    let doc = wordcraft_docx::read(&fixture(el)).unwrap();
    let wp = doc.settings.write_protection.as_ref().expect("read");
    assert!(!wp.recommended);
    assert!(wp.hash.iter().any(|(k, v)| k == "w:cryptAlgorithmSid" && v == "4"));
    assert!(wp.hash.iter().any(|(k, v)| k == "w:hash" && v == "A1B2C3D4"));
    let s = settings_of(&doc);
    for kept in [r#"w:cryptAlgorithmSid="4""#, r#"w:hash="A1B2C3D4""#, r#"w:salt="E5F6""#, r#"w:cryptSpinCount="100000""#] {
        assert!(s.contains(kept), "{kept} lost: {s}");
    }
    let again = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    assert_eq!(again.settings.write_protection, doc.settings.write_protection);
}

#[test]
fn read_only_recommended_alone_is_read_and_written() {
    let doc = wordcraft_docx::read(&fixture(r#"<w:writeProtection w:recommended="1"/>"#)).unwrap();
    let wp = doc.settings.write_protection.as_ref().expect("read");
    assert!(wp.recommended && wp.hash.is_empty());
    let s = settings_of(&doc);
    assert!(s.contains(r#"<w:writeProtection w:recommended="1"/>"#), "{s}");
    // Off, or absent, is no write protection at all.
    for settings in ["", r#"<w:writeProtection w:recommended="0"/>"#] {
        let doc = wordcraft_docx::read(&fixture(settings)).unwrap();
        assert!(doc.settings.write_protection.is_none(), "{settings}");
        assert!(!settings_of(&doc).contains("writeProtection"));
    }
}
