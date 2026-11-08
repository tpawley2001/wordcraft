//! A package thumbnail kept from the opened file (`docProps/thumbnail.*`, related from
//! `_rels/.rels` with the OPC thumbnail relationship) pictures the first page as the program that
//! made the file last saved it. WordCraft can't draw a new one, so a save leaves the old one out
//! rather than show content the user has since changed or removed (#3627). Hand-written fixtures.

use std::collections::BTreeMap;
use std::io::{Read, Write};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const THUMB_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail";
const PEOPLE_REL: &str = "http://schemas.microsoft.com/office/2011/relationships/people";

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zw.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        assert!(out.insert(f.name().to_string(), b).is_none(), "duplicate part {}", f.name());
    }
    out
}

fn text(files: &BTreeMap<String, Vec<u8>>, name: &str) -> String {
    String::from_utf8_lossy(files.get(name).unwrap_or_else(|| panic!("no {name} in {:?}", files.keys()))).into_owned()
}

/// A package with body text "Kept text", a thumbnail part `thumb` (e.g. "docProps/thumbnail.jpeg")
/// of content type `thumb_ct`, and optionally a people part.
fn package(thumb: &str, thumb_ct: &str, people: bool) -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t>Kept text</w:t></w:r></w:p></w:body></w:document>"#
    );
    let ext = thumb.rsplit('.').next().unwrap();
    let ct = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="{ext}" ContentType="{thumb_ct}"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/people.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.people+xml"/></Types>"#
    );
    let root = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="{THUMB_REL}" Target="{thumb}"/></Relationships>"#
    );
    let people_rel = if people { format!(r#"<Relationship Id="rId1" Type="{PEOPLE_REL}" Target="people.xml"/>"#) } else { String::new() };
    let doc_rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{people_rel}</Relationships>"#
    );
    let people_xml = r#"<w15:people xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"><w15:person w15:author="Ann"><w15:presenceInfo w15:providerId="None" w15:userId="Ann"/></w15:person></w15:people>"#;
    let picture: &[u8] = b"\xff\xd8\xff old first page";
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", ct.as_bytes()),
        ("_rels/.rels", root.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
        ("word/_rels/document.xml.rels", doc_rels.as_bytes()),
        (thumb, picture),
    ];
    if people {
        entries.push(("word/people.xml", people_xml.as_bytes()));
    }
    zip(&entries)
}

/// No thumbnail part, no relationship to it, no content-type override for it; the body is intact.
fn assert_no_thumbnail(files: &BTreeMap<String, Vec<u8>>) {
    assert!(!files.keys().any(|k| k.to_ascii_lowercase().starts_with("docprops/thumbnail")), "{:?}", files.keys());
    let root = text(files, "_rels/.rels");
    assert!(!root.contains("metadata/thumbnail"), "{root}");
    let ct = text(files, "[Content_Types].xml");
    assert!(!ct.to_ascii_lowercase().contains("/docprops/thumbnail"), "{ct}");
    assert!(text(files, "word/document.xml").contains("Kept text"));
}

#[test]
fn a_kept_jpeg_thumbnail_is_left_out_on_save() {
    let doc = wordcraft_docx::read(&package("docProps/thumbnail.jpeg", "image/jpeg", false)).unwrap();
    let files = unzip(&wordcraft_docx::write(&doc).unwrap());
    assert_no_thumbnail(&files);
}

#[test]
fn a_kept_emf_thumbnail_is_left_out_with_remove_personal_information() {
    let mut doc = wordcraft_docx::read(&package("docProps/thumbnail.emf", "image/x-emf", false)).unwrap();
    doc.settings.remove_personal_info = true;
    let files = unzip(&wordcraft_docx::write(&doc).unwrap());
    assert_no_thumbnail(&files);
}

#[test]
fn the_thumbnail_stays_out_over_saves_and_other_kept_parts_stay() {
    let doc = wordcraft_docx::read(&package("docProps/thumbnail.jpeg", "image/jpeg", true)).unwrap();
    let first = wordcraft_docx::write(&doc).unwrap();
    let files = unzip(&first);
    assert_no_thumbnail(&files);
    assert!(files.contains_key("word/people.xml"), "the people part is still carried: {:?}", files.keys());
    assert!(text(&files, "word/_rels/document.xml.rels").contains("/people\""));
    let again = unzip(&wordcraft_docx::write(&wordcraft_docx::read(&first).unwrap()).unwrap());
    assert_no_thumbnail(&again);
    assert!(again.contains_key("word/people.xml"));
}
