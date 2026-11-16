//! File › Options › Advanced › Print › Scale content for A4 or 8.5 x 11" paper sizes (on by default):
//! a print job whose printer paper (`printerPaper`) is Letter scales A4 pages onto it, and the other
//! way round. Only that pair; an explicit `paper` (Scale to paper size) wins; the document is unchanged.

use serde_json::{Value, json};

use crate::Session;

const A4: (f32, f32) = (595.0, 842.0);
const LETTER: (f32, f32) = (612.0, 792.0);
const LEGAL: (f32, f32) = (612.0, 1008.0);

fn session(page: (f32, f32)) -> Session {
    let mut s = Session::new(crate::sample::sample_document());
    s.doc.last_section.page_w = page.0;
    s.doc.last_section.page_h = page.1;
    s
}

/// Page dictionaries are written uncompressed: `/MediaBox[0 0 w h]`.
fn media(bytes: &[u8]) -> Vec<(f32, f32)> {
    let raw = String::from_utf8_lossy(bytes).into_owned();
    raw.split("/MediaBox[")
        .skip(1)
        .filter_map(|r| r.split(']').next())
        .map(|r| r.split_whitespace().filter_map(|n| n.parse::<f32>().ok()).collect::<Vec<_>>())
        .map(|n| (n[2].round(), n[3].round()))
        .collect()
}

/// The pref as saved (`printScaleA4Letter`).
fn pref(s: &Session) -> Value {
    serde_json::to_value(&s.prefs).unwrap()["printScaleA4Letter"].clone()
}

fn first_box(s: &Session, v: Value) -> (f32, f32) {
    let (bytes, _) = s.print_job_pdf(&v).unwrap_or_else(|e| panic!("{v}: {e}"));
    *media(&bytes).first().expect("a page")
}

#[test]
fn a4_prints_scaled_onto_letter_paper_by_default() {
    let s = session(A4);
    assert_eq!(pref(&s), json!(true), "on by default, as in Word");
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": "letter"})), LETTER);
    // Save these pages as PDF is a print job too.
    let (bytes, _) = s.print_pdf(&json!({"range": "current", "printerPaper": "letter"})).unwrap();
    assert_eq!(media(&bytes), vec![LETTER]);
}

#[test]
fn letter_prints_scaled_onto_a4_and_landscape_stays_landscape() {
    assert_eq!(first_box(&session(LETTER), json!({"range": "current", "printerPaper": "A4"})), A4);
    assert_eq!(first_box(&session((A4.1, A4.0)), json!({"range": "current", "printerPaper": "letter"})), (LETTER.1, LETTER.0));
}

#[test]
fn off_or_overridden_prints_the_own_size() {
    let mut s = session(A4);
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": "letter", "scaleA4Letter": false})), A4);
    s.run("file.options", &json!({"printScaleA4Letter": false})).unwrap();
    assert_eq!(pref(&s), json!(false));
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": "letter"})), A4);
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": "letter", "scaleA4Letter": true})), LETTER);
}

#[test]
fn only_the_a4_letter_pair_and_only_with_a_known_printer_paper() {
    let s = session(A4);
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": "A4"})), A4);
    assert_eq!(first_box(&s, json!({"range": "current"})), A4);
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": null})), A4);
    assert_eq!(first_box(&session(LEGAL), json!({"range": "current", "printerPaper": "A4"})), LEGAL);
    assert_eq!(first_box(&session(LEGAL), json!({"range": "current", "printerPaper": "letter"})), LEGAL);
}

#[test]
fn explicit_paper_wins() {
    let s = session(A4);
    assert_eq!(first_box(&s, json!({"range": "current", "printerPaper": "letter", "paper": "Legal"})), LEGAL);
}

#[test]
fn bad_printer_paper_is_an_error_and_the_document_is_unchanged() {
    let s = session(A4);
    let rev = s.rev();
    for bad in [json!("Quarto"), json!(5), json!({"w": 1}), json!("x".repeat(10_000))] {
        assert!(s.print_job_pdf(&json!({"printerPaper": bad})).is_err(), "{bad}");
    }
    let _ = first_box(&s, json!({"range": "current", "printerPaper": "letter"}));
    assert_eq!((s.doc.last_section.page_w, s.doc.last_section.page_h, s.rev()), (A4.0, A4.1, rev));
}
