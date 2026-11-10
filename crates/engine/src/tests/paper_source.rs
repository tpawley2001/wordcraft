//! Layout › Page Setup › Paper › Paper source: `layout.paperSource` sets the printer tray for the
//! section's first page and its other pages (DOCX `w:paperSrc`, ECMA-376 Part 1 §17.6.9).

use serde_json::{Value, json};

use crate::Session;

fn s() -> Session {
    Session::new(wordcraft_doc::Document::from_text("Letter"))
}

fn trays(s: &Session) -> (Option<u16>, Option<u16>) {
    let sect = crate::cmd::page::sect(s);
    (sect.paper_src_first, sect.paper_src_other)
}

#[test]
fn paper_source_sets_each_tray_on_its_own() {
    let mut s = s();
    s.run("layout.paperSource", &json!({"first": 4})).unwrap();
    assert_eq!(trays(&s), (Some(4), None));
    s.run("layout.paperSource", &json!({"other": 15})).unwrap();
    assert_eq!(trays(&s), (Some(4), Some(15)));
    s.run("layout.paperSource", &json!({"first": 7, "other": 1})).unwrap();
    assert_eq!(trays(&s), (Some(7), Some(1)));
}

#[test]
fn null_or_zero_is_the_default_tray() {
    let mut s = s();
    s.run("layout.paperSource", &json!({"first": 4, "other": 15})).unwrap();
    s.run("layout.paperSource", &json!({"first": null})).unwrap();
    assert_eq!(trays(&s), (None, Some(15)));
    s.run("layout.paperSource", &json!({"other": 0})).unwrap();
    assert_eq!(trays(&s), (None, None));
}

#[test]
fn bad_tray_codes_are_refused_and_change_nothing() {
    let mut s = s();
    s.run("layout.paperSource", &json!({"first": 4})).unwrap();
    for bad in [json!({"first": -1}), json!({"first": 70000}), json!({"other": "x"})] {
        assert!(s.run("layout.paperSource", &bad).is_err(), "{bad}");
        assert_eq!(trays(&s), (Some(4), None), "{bad}");
    }
}

#[test]
fn no_params_reads_the_trays_and_every_call_returns_them() {
    let mut s = s();
    let r: Value = s.run("layout.paperSource", &json!({})).unwrap();
    assert_eq!(r, json!({"first": null, "other": null}));
    let r: Value = s.run("layout.paperSource", &json!({"first": 260})).unwrap();
    assert_eq!(r, json!({"first": 260, "other": null}));
    assert_eq!(trays(&s), (Some(260), None));
}

#[test]
fn paper_source_is_one_undo_step() {
    let mut s = s();
    s.run("layout.paperSource", &json!({"first": 4, "other": 15})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(trays(&s), (None, None));
}
