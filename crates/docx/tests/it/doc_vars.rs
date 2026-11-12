//! Document variables (`w:docVars` / `w:docVar`, ECMA-376 Part 1 §17.15.1.32 and §17.15.1.31):
//! read into the model, written from it, and no longer only carried as the original settings
//! bytes. Synthetic fixtures written for these tests from the spec; the names are made up.
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use wordcraft_doc::DocVar;

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

/// A package whose settings hold `before` and `after` (whole elements, in CT_Settings order)
/// around the default tab stop.
fn fixture(before: &str, after: &str) -> Vec<u8> {
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
        ("word/settings.xml", format!(r#"<w:settings {W_NS}>{before}<w:defaultTabStop w:val="720"/>{after}</w:settings>"#)),
    ])
}

fn settings_of(doc: &wordcraft_doc::Document) -> String {
    unzip(&wordcraft_docx::write(doc).unwrap()).remove("word/settings.xml").unwrap_or_default()
}

fn var(name: &str, value: &str) -> DocVar {
    DocVar { name: name.into(), value: value.into() }
}

const VARS: &str = r#"<w:docVars><w:docVar w:name="Client" w:val="Acme &amp; Sons"/><w:docVar w:name="Two Words" w:val="Second value"/><w:docVar w:name="Empty" w:val=""/><w:docVar w:name="NoVal"/></w:docVars>"#;

#[test]
fn variables_are_read_in_file_order() {
    let doc = wordcraft_docx::read(&fixture("", VARS)).unwrap();
    assert_eq!(doc.settings.doc_vars, [var("Client", "Acme & Sons"), var("Two Words", "Second value"), var("Empty", ""), var("NoVal", "")]);
    assert_eq!(doc.doc_var("client"), Some("Acme & Sons"));
    assert_eq!(doc.doc_var("Empty"), None);
}

#[test]
fn a_variable_without_a_name_is_skipped() {
    let doc = wordcraft_docx::read(&fixture(
        "",
        r#"<w:docVars><w:docVar w:val="lost"/><w:docVar w:name="" w:val="lost"/><w:docVar w:name="Kept" w:val="1"/></w:docVars>"#,
    ))
    .unwrap();
    assert_eq!(doc.settings.doc_vars, [var("Kept", "1")]);
}

#[test]
fn over_long_names_and_values_are_skipped() {
    let long_name = "n".repeat(256);
    let long_value = "v".repeat(65_281);
    let ok_name = "n".repeat(255);
    let xml = format!(
        r#"<w:docVars><w:docVar w:name="{long_name}" w:val="x"/><w:docVar w:name="Big" w:val="{long_value}"/><w:docVar w:name="{ok_name}" w:val="fits"/></w:docVars>"#
    );
    let doc = wordcraft_docx::read(&fixture("", &xml)).unwrap();
    assert_eq!(doc.settings.doc_vars, [var(&ok_name, "fits")]);
}

#[test]
fn at_most_4096_variables_are_read() {
    let mut xml = String::from("<w:docVars>");
    for i in 0..4100 {
        xml.push_str(&format!(r#"<w:docVar w:name="v{i}" w:val="{i}"/>"#));
    }
    xml.push_str("</w:docVars>");
    let doc = wordcraft_docx::read(&fixture("", &xml)).unwrap();
    assert_eq!(doc.settings.doc_vars.len(), 4096);
    assert_eq!(doc.settings.doc_vars[4095], var("v4095", "4095"));
}

#[test]
fn variables_survive_a_save_and_reopen() {
    let doc = wordcraft_docx::read(&fixture("", VARS)).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    assert_eq!(again.settings.doc_vars, doc.settings.doc_vars);
    assert_eq!(again.settings.doc_vars.len(), 4);
}

#[test]
fn a_new_document_writes_its_variables_and_none_when_it_has_none() {
    let mut doc = wordcraft_doc::Document::new();
    assert!(!settings_of(&doc).contains("docVar"), "no variables: no element");
    doc.settings.doc_vars = vec![var("Client", "A<B> & \"C\""), var("Ref", "7")];
    let s = settings_of(&doc);
    assert_eq!(s.matches("<w:docVars>").count(), 1, "{s}");
    assert!(s.contains(r#"<w:docVar w:name="Ref" w:val="7"/>"#), "{s}");
    assert!(s.find(r#"w:name="Client""#).unwrap() < s.find(r#"w:name="Ref""#).unwrap(), "file order: {s}");
    let again = wordcraft_docx::read(&wordcraft_docx::write(&doc).unwrap()).unwrap();
    assert_eq!(again.settings.doc_vars, doc.settings.doc_vars);
}

/// The model owns the variables: one changed or removed after opening a file is saved that way,
/// not brought back from the file's original settings bytes.
#[test]
fn a_changed_or_removed_variable_is_not_restored_from_the_original_settings() {
    // `w:proofState` is a setting the model doesn't hold, so the writer merges into the original part.
    let mut doc = wordcraft_docx::read(&fixture(r#"<w:proofState w:spelling="clean"/>"#, VARS)).unwrap();
    assert_eq!(doc.settings.doc_vars.len(), 4);
    doc.settings.doc_vars.retain(|v| v.name != "Two Words");
    doc.settings.doc_vars[0].value = "Zenith".into();
    let s = settings_of(&doc);
    assert!(s.contains("w:proofState"), "the unowned setting is kept: {s}");
    assert!(s.contains(r#"<w:docVar w:name="Client" w:val="Zenith"/>"#), "{s}");
    assert!(!s.contains("Two Words") && !s.contains("Acme"), "{s}");
    assert_eq!(s.matches("<w:docVars>").count(), 1, "{s}");

    doc.settings.doc_vars.clear();
    let s = settings_of(&doc);
    assert!(s.contains("w:proofState") && !s.contains("docVar"), "{s}");
}
