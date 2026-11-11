//! "Print only the data from a form" (File › Options › Advanced › When printing this document,
//! `w:printFormsData`): a print of the document puts only what was filled into its legacy form
//! fields on the paper, where the full layout has it.

use std::io::Read;

use serde_json::{Value, json};
use wordcraft_layout::display::{DisplayOptions, Draw, page_display};

use crate::Session;

fn flat_opc(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<?mso-application progid="Word.Document"?>
<pkg:package xmlns:pkg="http://schemas.microsoft.com/office/2006/xmlPackage">
  <pkg:part pkg:name="/_rels/.rels" pkg:contentType="application/vnd.openxmlformats-package.relationships+xml">
    <pkg:xmlData><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships></pkg:xmlData>
  </pkg:part>
  <pkg:part pkg:name="/word/document.xml" pkg:contentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml">
    <pkg:xmlData><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}</w:body></w:document></pkg:xmlData>
  </pkg:part>
</pkg:package>"#
    )
}

/// "Name: " + a FORMTEXT field whose result is "Ada", then "Plain words".
fn form() -> Session {
    let body = r#"<w:p><w:r><w:t xml:space="preserve">Name: </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="Text1"/><w:enabled/><w:calcOnExit w:val="0"/><w:textInput/></w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> FORMTEXT </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Ada</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>
<w:p><w:r><w:t>Plain words</w:t></w:r></w:p>"#;
    Session::new(wordcraft_docx::read_xml(flat_opc(body).as_bytes()).expect("flat OPC reads"))
}

fn draws(s: &Session, forms_data_only: bool) -> Vec<Draw> {
    let lay = s.export_layout();
    let opts = DisplayOptions { forms_data_only, ..DisplayOptions::default() };
    page_display(&s.doc, &lay.pages[0], &opts)
}

/// (joined glyph text, positions of the glyphs that draw the text `of`, wherever a draw holds it).
fn glyph_text(draws: &[Draw], of: &str) -> (String, Vec<(u32, f32, f32)>) {
    let mut text = String::new();
    let mut at = Vec::new();
    for d in draws {
        if let Draw::Glyphs { text: t, glyphs, ranges, .. } = d {
            text.push_str(t);
            if let Some(k) = t.find(of) {
                let span = k..k + of.len();
                at.extend(glyphs.iter().zip(ranges).filter(|(_, r)| span.contains(&r.start)).map(|(g, _)| *g));
            }
        }
    }
    (text, at)
}

#[test]
fn display_keeps_only_the_form_data_in_place() {
    let s = form();
    let all = draws(&s, false);
    let (text, full_at) = glyph_text(&all, "Ada");
    for want in ["Name", "Ada", "Plain"] {
        assert!(text.contains(want), "{want} drawn with the option off: {text:?}");
    }
    let only = draws(&s, true);
    let (text, only_at) = glyph_text(&only, "Ada");
    assert_eq!(text, "Ada", "only the field's result is drawn");
    assert_eq!(only_at, full_at, "the data stays where the full layout has it");
    let other = only.iter().filter(|d| matches!(d, Draw::Fill { .. } | Draw::Line { .. } | Draw::Image { .. })).count();
    assert_eq!(other, 0, "no fills, lines or images");
}

#[test]
fn file_options_sets_the_document_setting() {
    let mut s = form();
    assert!(!s.doc.settings.print_forms_data, "off unless the file says so");
    let r = s.run("file.options", &json!({"printFormsData": true})).expect("sets");
    assert_eq!(r["printFormsData"], json!(true));
    assert!(s.doc.settings.print_forms_data);
    for bad in [json!("yes"), json!(1), json!([true])] {
        assert!(s.run("file.options", &json!({"printFormsData": bad})).is_err(), "{bad}");
        assert!(s.doc.settings.print_forms_data, "{bad} changes nothing");
    }
    let r = s.run("file.options", &json!({"printFormsData": false})).expect("clears");
    assert_eq!(r["printFormsData"], json!(false));
}

fn settings_xml(bytes: &[u8]) -> String {
    let mut xml = String::new();
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap().by_name("word/settings.xml").unwrap().read_to_string(&mut xml).unwrap();
    xml
}

#[test]
fn docx_round_trips_the_setting() {
    let mut s = form();
    let off = wordcraft_docx::write(&s.doc).expect("saves");
    assert!(!settings_xml(&off).contains("printFormsData"), "off: not written");
    s.doc.settings.print_forms_data = true;
    let on = wordcraft_docx::write(&s.doc).expect("saves");
    let xml = settings_xml(&on);
    assert_eq!(xml.matches("<w:printFormsData/>").count(), 1, "{xml}");
    let back = wordcraft_docx::read(&on).expect("reads");
    assert!(back.settings.print_forms_data, "read back");
    let mut cleared = back.clone();
    cleared.settings.print_forms_data = false;
    let again = wordcraft_docx::write(&cleared).expect("saves");
    assert!(!settings_xml(&again).contains("printFormsData"), "turned off: the original element goes");
}

#[test]
fn print_jobs_draw_only_the_data() {
    let mut s = form();
    let job = |s: &Session, v: Value| s.print_job_pdf(&v).expect("prints").0.len();
    let full = job(&s, json!({}));
    s.doc.settings.print_forms_data = true;
    let data = job(&s, json!({}));
    assert!(data < full, "fewer glyphs drawn: {data} vs {full} bytes");
}
