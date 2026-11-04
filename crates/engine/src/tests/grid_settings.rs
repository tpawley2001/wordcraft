//! Drawing grid settings (Layout › Align › Grid Settings…): spacing, origin and how often
//! gridlines are shown, stored in settings.xml as Word does (ECMA-376 Part 1 §17.15.1).

use serde_json::{Value, json};

use crate::Session;

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn grid(s: &mut Session) -> Value {
    s.run("view.gridSettings", &json!({})).unwrap_or_else(|e| panic!("view.gridSettings: {e}"))
}

fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or_else(|| panic!("{k} missing in {v}"))
}

/// A Flat OPC document whose settings part holds `settings` (children of `w:settings`).
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
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Grid</w:t></w:r></w:p></w:body></w:document></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/settings.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml">
    <pkg:xmlData><w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{settings}</w:settings></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    )
}

fn open_flat(settings: &str) -> Session {
    Session::new(wordcraft_docx::read_xml(flat_opc(settings).as_bytes()).expect("flat OPC reads"))
}

#[test]
fn a_new_document_has_words_blank_document_grid() {
    let mut s = s();
    let g = grid(&mut s);
    assert_eq!(f(&g, "horizontalSpacing"), 9.0);
    assert_eq!(f(&g, "verticalSpacing"), 9.0);
    assert_eq!(f(&g, "horizontalEvery"), 0.0, "no vertical gridlines by default");
    assert_eq!(f(&g, "verticalEvery"), 2.0, "a horizontal gridline every 2 units (0.25 in)");
    assert_eq!(g["useMargins"], true);
}

#[test]
fn settings_are_set_returned_and_undone() {
    let mut s = s();
    let g = s
        .run(
            "view.gridSettings",
            &json!({"horizontalSpacing": 18.0, "verticalSpacing": 6.0, "horizontalEvery": 3, "verticalEvery": 1, "useMargins": false, "horizontalOrigin": 72.0, "verticalOrigin": 36.0}),
        )
        .unwrap();
    assert_eq!(f(&g, "horizontalSpacing"), 18.0);
    assert_eq!(f(&g, "verticalSpacing"), 6.0);
    assert_eq!(f(&g, "horizontalEvery"), 3.0);
    assert_eq!(f(&g, "verticalEvery"), 1.0);
    assert_eq!(g["useMargins"], false);
    assert_eq!(f(&g, "horizontalOrigin"), 72.0);
    assert_eq!(f(&g, "verticalOrigin"), 36.0);
    // A parameter left out stays as it is.
    let g = s.run("view.gridSettings", &json!({"verticalEvery": 4})).unwrap();
    assert_eq!(f(&g, "horizontalSpacing"), 18.0);
    assert_eq!(f(&g, "verticalEvery"), 4.0);
    s.run("edit.undo", &json!({})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    let g = grid(&mut s);
    assert_eq!(f(&g, "horizontalSpacing"), 9.0);
    assert_eq!(f(&g, "verticalEvery"), 2.0);
    assert_eq!(g["useMargins"], true);
}

#[test]
fn out_of_range_values_are_refused_and_change_nothing() {
    let mut s = s();
    for bad in [
        json!({"horizontalSpacing": 0.0}),
        json!({"verticalSpacing": -3.0}),
        json!({"verticalSpacing": 5000.0}),
        json!({"horizontalEvery": -1}),
        json!({"verticalEvery": 128}),
        json!({"horizontalOrigin": -1.0}),
    ] {
        assert!(s.run("view.gridSettings", &bad).is_err(), "{bad} should be refused");
    }
    let g = grid(&mut s);
    assert_eq!(f(&g, "horizontalSpacing"), 9.0);
    assert_eq!(f(&g, "verticalSpacing"), 9.0);
    assert_eq!(f(&g, "horizontalEvery"), 0.0);
    assert_eq!(f(&g, "verticalEvery"), 2.0);
}

#[test]
fn reading_explicit_elements() {
    // 360 twips = 18 pt, 120 = 6 pt, 1440 = 72 pt, 720 = 36 pt.
    let mut s = open_flat(
        r#"<w:drawingGridHorizontalSpacing w:val="360"/><w:drawingGridVerticalSpacing w:val="120"/><w:displayHorizontalDrawingGridEvery w:val="2"/><w:displayVerticalDrawingGridEvery w:val="3"/><w:doNotUseMarginsForDrawingGridOrigin/><w:drawingGridHorizontalOrigin w:val="1440"/><w:drawingGridVerticalOrigin w:val="720"/>"#,
    );
    let g = grid(&mut s);
    assert_eq!(f(&g, "horizontalSpacing"), 18.0);
    assert_eq!(f(&g, "verticalSpacing"), 6.0);
    assert_eq!(f(&g, "horizontalEvery"), 2.0);
    assert_eq!(f(&g, "verticalEvery"), 3.0);
    assert_eq!(g["useMargins"], false);
    assert_eq!(f(&g, "horizontalOrigin"), 72.0);
    assert_eq!(f(&g, "verticalOrigin"), 36.0);
}

#[test]
fn absent_elements_read_as_the_ecma_defaults() {
    let mut s = open_flat(r#"<w:defaultTabStop w:val="720"/>"#);
    let g = grid(&mut s);
    assert_eq!(f(&g, "horizontalSpacing"), 9.0);
    assert_eq!(f(&g, "horizontalEvery"), 1.0, "absent displayHorizontalDrawingGridEvery is 1");
    assert_eq!(f(&g, "verticalEvery"), 1.0, "absent displayVerticalDrawingGridEvery is 1");
    assert_eq!(g["useMargins"], true);
}

#[test]
fn word_default_elements_read_as_zero_and_two() {
    let mut s = open_flat(r#"<w:displayHorizontalDrawingGridEvery w:val="0"/><w:displayVerticalDrawingGridEvery w:val="2"/>"#);
    let g = grid(&mut s);
    assert_eq!(f(&g, "horizontalEvery"), 0.0);
    assert_eq!(f(&g, "verticalEvery"), 2.0);
}

#[test]
fn settings_survive_a_docx_round_trip() {
    let mut s = s();
    s.run(
        "view.gridSettings",
        &json!({"horizontalSpacing": 18.0, "verticalSpacing": 6.0, "horizontalEvery": 1, "verticalEvery": 5, "useMargins": false, "horizontalOrigin": 72.0, "verticalOrigin": 36.0}),
    )
    .unwrap();
    let bytes = wordcraft_docx::write(&s.doc).expect("docx writes");
    let mut back = Session::new(wordcraft_docx::read(&bytes).expect("docx reads"));
    let g = grid(&mut back);
    assert_eq!(f(&g, "horizontalSpacing"), 18.0);
    assert_eq!(f(&g, "verticalSpacing"), 6.0);
    assert_eq!(f(&g, "horizontalEvery"), 1.0);
    assert_eq!(f(&g, "verticalEvery"), 5.0);
    assert_eq!(g["useMargins"], false);
    assert_eq!(f(&g, "horizontalOrigin"), 72.0);
    assert_eq!(f(&g, "verticalOrigin"), 36.0);
}

#[test]
fn a_new_document_saves_words_default_display_elements() {
    let mut s = s();
    let bytes = wordcraft_docx::write(&s.doc).expect("docx writes");
    let mut back = Session::new(wordcraft_docx::read(&bytes).expect("docx reads"));
    let g = grid(&mut back);
    assert_eq!(f(&g, "horizontalEvery"), 0.0, "written, not left to the absent default of 1");
    assert_eq!(f(&g, "verticalEvery"), 2.0);
    let _ = grid(&mut s);
}
