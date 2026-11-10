//! A row's table property exceptions (`w:tblPrEx`, ECMA-376 Part 1 §17.4.61) and the table's own
//! `w:tblCellMar` (§17.4.42) keep only the margin sides the file set, and an exception's
//! `w:tblLook` (§17.4.56) survives a save. Fixtures are hand-written from the spec.

use std::io::{Read, Write};

fn docx(body: &str) -> Vec<u8> {
    let root = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document>"#
    );
    let mut zw = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in [("_rels/.rels", root.as_bytes()), ("word/document.xml", doc.as_bytes())] {
        zw.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zw.write_all(bytes).unwrap();
    }
    zw.finish().unwrap().into_inner()
}

fn document_xml(bytes: &[u8]) -> String {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("word/document.xml").unwrap().read_to_string(&mut s).unwrap();
    s
}

/// One table: `tbl_pr` inside `w:tblPr`, a first row with `ex` inside `w:tblPrEx`, a plain second row.
fn table(tbl_pr: &str, ex: &str) -> String {
    let cell = |t: &str| format!(r#"<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>"#);
    format!(
        r#"<w:tbl><w:tblPr>{tbl_pr}</w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tblPrEx>{ex}</w:tblPrEx>{}</w:tr><w:tr>{}</w:tr></w:tbl><w:p/>"#,
        cell("A"),
        cell("B")
    )
}

/// Read, write, and return the saved document.xml.
fn round_trip(body: &str) -> (String, Vec<u8>) {
    let d = wordcraft_docx::read(&docx(body)).unwrap();
    let out = wordcraft_docx::write(&d).unwrap();
    (document_xml(&out), out)
}

/// The text of the single saved `w:tblPrEx` element.
fn saved_exception(xml: &str) -> &str {
    assert_eq!(xml.matches("<w:tblPrEx>").count(), 1, "{xml}");
    let start = xml.find("<w:tblPrEx>").unwrap();
    let end = xml[start..].find("</w:tblPrEx>").unwrap() + start;
    &xml[start..end]
}

/// The text of the table's own saved `w:tblPr` element.
fn saved_table_props(xml: &str) -> &str {
    let start = xml.find("<w:tblPr>").expect("tblPr written");
    let end = xml[start..].find("</w:tblPr>").unwrap() + start;
    &xml[start..end]
}

#[test]
fn an_exception_with_top_and_bottom_margins_only_keeps_left_and_right_unset() {
    let ex = r#"<w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/></w:tblCellMar>"#;
    let (xml, _) = round_trip(&table("", ex));
    let saved = saved_exception(&xml);
    assert!(saved.contains("<w:tblCellMar>"), "{saved}");
    assert!(saved.contains(r#"<w:top w:w="0" w:type="dxa"/>"#), "{saved}");
    assert!(saved.contains(r#"<w:bottom w:w="0" w:type="dxa"/>"#), "{saved}");
    assert!(!saved.contains("<w:left ") && !saved.contains("<w:start "), "an unset left side must not be written as 0: {saved}");
    assert!(!saved.contains("<w:right ") && !saved.contains("<w:end "), "an unset right side must not be written as 0: {saved}");
}

#[test]
fn an_exception_with_a_left_margin_only_writes_just_that_side() {
    let ex = r#"<w:tblCellMar><w:left w:w="28" w:type="dxa"/></w:tblCellMar>"#;
    let (xml, _) = round_trip(&table("", ex));
    let saved = saved_exception(&xml);
    assert!(saved.contains(r#"<w:tblCellMar><w:left w:w="28" w:type="dxa"/></w:tblCellMar>"#), "{saved}");
}

#[test]
fn an_exception_with_all_four_margins_keeps_all_four() {
    let ex = r#"<w:tblCellMar><w:top w:w="10" w:type="dxa"/><w:left w:w="28" w:type="dxa"/><w:bottom w:w="20" w:type="dxa"/><w:right w:w="30" w:type="dxa"/></w:tblCellMar>"#;
    let (xml, _) = round_trip(&table("", ex));
    let saved = saved_exception(&xml);
    assert!(saved.contains(ex), "{saved}");
}

#[test]
fn the_tables_own_left_and_right_margins_do_not_gain_top_and_bottom() {
    let pr = r#"<w:tblCellMar><w:left w:w="0" w:type="dxa"/><w:right w:w="0" w:type="dxa"/></w:tblCellMar>"#;
    let (xml, _) = round_trip(&table(pr, ""));
    let saved = saved_table_props(&xml);
    assert!(saved.contains(r#"<w:left w:w="0" w:type="dxa"/>"#), "{saved}");
    assert!(saved.contains(r#"<w:right w:w="0" w:type="dxa"/>"#), "{saved}");
    assert!(!saved.contains("<w:top ") && !saved.contains("<w:bottom "), "unset sides must not be written as 0: {saved}");
}

#[test]
fn an_exception_look_survives_a_save() {
    let ex = r#"<w:tblLook w:val="0000"/>"#;
    let (xml, out) = round_trip(&table("", ex));
    let saved = saved_exception(&xml);
    assert!(saved.contains("<w:tblLook"), "{saved}");
    let back = wordcraft_docx::read(&out).unwrap();
    let t = back.body.iter().find_map(|b| b.as_table()).unwrap();
    let look = t.rows[0].props.exceptions.as_ref().expect("first row keeps its exceptions").look;
    // 0000: no header row, total row, first or last column; 0x0200/0x0400 clear = banded rows and columns on.
    let expected = wordcraft_doc::props::TableLook {
        header_row: false,
        total_row: false,
        banded_rows: true,
        first_column: false,
        last_column: false,
        banded_columns: true,
    };
    assert_eq!(look, expected);
}

#[test]
fn an_exception_without_a_look_writes_none() {
    let ex = r#"<w:tblW w:w="3000" w:type="dxa"/>"#;
    let (xml, _) = round_trip(&table("", ex));
    let saved = saved_exception(&xml);
    assert!(!saved.contains("<w:tblLook"), "{saved}");
}
