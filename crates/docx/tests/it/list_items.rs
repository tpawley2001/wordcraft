//! Drop-down and combo box items without `w:displayText` (ECMA-376 §17.5.2.21: optional; the item
//! then shows its `w:value`) are saved without an empty `w:displayText`, which Word refuses to open.
//! All markup here is synthetic.

use std::io::Write;

use wordcraft_doc::Document;
use wordcraft_doc::control::{ControlKind, ListItem};
use wordcraft_docx::{read, write};

const NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
const ROOT_RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

fn package(body: &str) -> Vec<u8> {
    let doc = format!(r#"<w:document {NS}><w:body>{body}</w:body></w:document>"#);
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [("_rels/.rels", ROOT_RELS.as_bytes()), ("word/document.xml", doc.as_bytes())] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn document_xml(docx: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(docx)).unwrap();
    let mut s = String::new();
    std::io::Read::read_to_string(&mut z.by_name("word/document.xml").unwrap(), &mut s).unwrap();
    s
}

fn control(id: i64, list: &str) -> String {
    format!(
        r#"<w:sdt><w:sdtPr><w:id w:val="{id}"/>{list}</w:sdtPr><w:sdtContent><w:p><w:r><w:t>Choose an item.</w:t></w:r></w:p></w:sdtContent></w:sdt>"#
    )
}

fn form() -> Vec<u8> {
    let dd = control(1, r#"<w:dropDownList><w:listItem w:value="Choose an item."/><w:listItem w:displayText="Yes" w:value="Y"/></w:dropDownList>"#);
    let cb = control(2, r#"<w:comboBox><w:listItem w:value="Choose a name."/><w:listItem w:displayText="Name A" w:value="Name A"/></w:comboBox>"#);
    package(&format!("{dd}{cb}"))
}

fn items(d: &Document, id: i64) -> Vec<ListItem> {
    let mut out = None;
    wordcraft_doc::control::walk_controls(&d.body, &mut |c| {
        if c.id == Some(id) {
            out = Some(c.kind.clone());
        }
    });
    match out {
        Some(ControlKind::DropDown { items, .. } | ControlKind::ComboBox { items, .. }) => items,
        other => panic!("control {id}: {other:?}"),
    }
}

#[test]
fn value_only_items_save_without_an_empty_display_text() {
    let xml = document_xml(&write(&read(&form()).unwrap()).unwrap());
    assert!(!xml.contains(r#"w:displayText="""#), "empty w:displayText written: {xml}");
    assert!(xml.contains(r#"<w:listItem w:value="Choose an item."/>"#), "{xml}");
    assert!(xml.contains(r#"<w:listItem w:value="Choose a name."/>"#), "{xml}");
}

#[test]
fn items_with_display_text_keep_it() {
    let xml = document_xml(&write(&read(&form()).unwrap()).unwrap());
    assert!(xml.contains(r#"<w:listItem w:displayText="Yes" w:value="Y"/>"#), "{xml}");
    assert!(xml.contains(r#"<w:listItem w:displayText="Name A" w:value="Name A"/>"#), "{xml}");
}

#[test]
fn items_survive_a_second_round_trip() {
    let first = read(&form()).unwrap();
    let second = read(&write(&first).unwrap()).unwrap();
    for id in [1, 2] {
        assert_eq!(items(&second, id), items(&first, id), "control {id}");
    }
}

#[test]
fn an_item_with_neither_text_never_writes_display_text() {
    let d = read(&package(&control(3, r#"<w:dropDownList><w:listItem w:value=""/></w:dropDownList>"#))).unwrap();
    let xml = document_xml(&write(&d).unwrap());
    assert!(!xml.contains("w:displayText"), "{xml}");
    assert!(xml.contains(r#"<w:listItem w:value=""/>"#), "{xml}");
}
