//! CREATEDATE and PRINTDATE show the document's own dates (ECMA-376 Part 1 §17.16.5.11,
//! §17.16.5.48): `dcterms:created` and `cp:lastPrinted`, in local time, with the field's `\@`
//! picture. A document never printed (or with no valid date) keeps the field's cached result.
//! Printing records Last Printed. SAVEDATE is left to its own change.
//!
//! Times are midday UTC, so the local date is the same in every zone from UTC-11 to UTC+11.

use serde_json::{Value, json};
use wordcraft_doc::{Document, InlineObject};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// A session whose body holds one field `instr` with cached result `kept`.
fn with_field(instr: &str) -> Session {
    let mut s = Session::new(Document::new());
    run(&mut s, "insert.field", json!({"instr": instr, "result": "kept"}));
    s
}

/// The result of the body's first field.
fn result(s: &Session) -> String {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.objects.iter())
        .find_map(|o| match o {
            InlineObject::Field { result, .. } => Some(result.clone()),
            _ => None,
        })
        .expect("a field")
}

fn update(s: &mut Session) {
    run(s, "references.updateFields", json!({}));
}

#[test]
fn createdate_shows_the_created_date_with_its_picture() {
    let mut s = with_field(r#"CREATEDATE \@ "yyyy-MM-dd""#);
    s.doc.core.created = "2024-03-15T12:00:00Z".into();
    update(&mut s);
    assert_eq!(result(&s), "2024-03-15");
}

#[test]
fn createdate_uses_words_default_picture() {
    let mut s = with_field("CREATEDATE");
    s.doc.core.created = "2024-03-15T12:00:00Z".into();
    update(&mut s);
    assert_eq!(result(&s), "3/15/2024");
}

#[test]
fn printdate_shows_the_last_printed_date() {
    let mut s = with_field(r#"PRINTDATE \@ "yyyy-MM-dd""#);
    s.doc.core.last_printed = "2023-07-04T12:00:00Z".into();
    update(&mut s);
    assert_eq!(result(&s), "2023-07-04");
}

#[test]
fn printdate_of_a_document_never_printed_keeps_its_result() {
    let mut s = with_field(r#"PRINTDATE \@ "yyyy-MM-dd""#);
    s.doc.core.last_printed = String::new();
    update(&mut s);
    assert_eq!(result(&s), "kept");
}

#[test]
fn createdate_with_no_valid_date_keeps_its_result() {
    let mut s = with_field("CREATEDATE");
    s.doc.core.created = "not a date".into();
    update(&mut s);
    assert_eq!(result(&s), "kept");
}

#[test]
fn printing_records_last_printed_and_it_is_saved() {
    let mut s = Session::new(Document::from_text("Body"));
    let path = std::env::temp_dir().join(format!("wordcraft-document-dates-{}.pdf", std::process::id()));
    run(&mut s, "file.printPdf", json!({"path": path.to_string_lossy()}));
    let _ = std::fs::remove_file(&path);
    let lp = s.doc.core.last_printed.clone();
    let year = &crate::cmd::now_iso()[..4];
    assert!(lp.starts_with(year) && lp.ends_with('Z'), "last printed {lp:?}");
    let flat = String::from_utf8(wordcraft_docx::write_flat(&s.doc).expect("flat OPC writes")).expect("utf-8");
    assert!(flat.contains(&format!("<cp:lastPrinted>{lp}</cp:lastPrinted>")), "cp:lastPrinted saved");
}
