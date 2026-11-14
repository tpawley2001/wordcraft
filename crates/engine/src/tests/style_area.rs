//! File › Options › Advanced › Display › "Style area pane width in Draft and Outline views", as in
//! Word (wordribbon.tips.net T008024): a user preference in points, 0 (no style area) by default.
//! Setting it is not an edit and never touches the document.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "text.insert", json!({"text": "A heading"}));
    s.dirty = false;
    s
}

fn pref(s: &Session) -> Value {
    serde_json::to_value(&s.prefs).unwrap()["styleAreaWidth"].clone()
}

#[test]
fn the_style_area_is_off_by_default() {
    let mut s = session();
    assert_eq!(pref(&s), json!(0.0));
    assert_eq!(run(&mut s, "view.styleAreaWidth", json!({}))["styleAreaWidth"], json!(0.0));
}

#[test]
fn width_is_set_in_points_and_reported() {
    let mut s = session();
    for w in [36.0, 72.0, 0.0, 1584.0, 50.5] {
        assert_eq!(run(&mut s, "view.styleAreaWidth", json!({"value": w}))["styleAreaWidth"], json!(w));
        assert_eq!(pref(&s), json!(w));
        assert_eq!(run(&mut s, "view.styleAreaWidth", json!({}))["styleAreaWidth"], json!(w), "no value only reports");
    }
}

#[test]
fn out_of_range_widths_are_refused_and_keep_the_width() {
    let mut s = session();
    run(&mut s, "view.styleAreaWidth", json!({"value": 72}));
    for bad in [json!(-1), json!(-0.5), json!(1584.5), json!(100000), json!("72"), json!(true)] {
        assert!(s.run("view.styleAreaWidth", &json!({"value": bad})).is_err(), "{bad}");
        assert_eq!(pref(&s), json!(72.0), "{bad} keeps the width");
    }
}

#[test]
fn setting_the_width_is_not_an_edit() {
    let mut s = session();
    let undo = s.undo_labels();
    let text = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
    run(&mut s, "view.draft", json!({}));
    run(&mut s, "view.styleAreaWidth", json!({"value": 90}));
    assert_eq!(s.undo_labels(), undo);
    assert!(!s.dirty, "a preference doesn't make the document unsaved");
    assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), text);
}

#[test]
fn a_bad_saved_width_loads_as_no_style_area() {
    for bad in [json!(-5.0), json!(5000.0), json!("wide"), json!(null)] {
        let p: crate::prefs::Prefs = serde_json::from_value(json!({"styleAreaWidth": bad})).unwrap_or_else(|e| panic!("{bad}: {e}"));
        assert_eq!(serde_json::to_value(&p).unwrap()["styleAreaWidth"], json!(0.0), "{bad}");
    }
    let p: crate::prefs::Prefs = serde_json::from_value(json!({"styleAreaWidth": 54.0})).unwrap();
    assert_eq!(serde_json::to_value(&p).unwrap()["styleAreaWidth"], json!(54.0));
}

#[test]
fn the_setting_is_in_the_command_catalog() {
    assert!(crate::catalog::entries().iter().any(|e| e.3 == "view.styleAreaWidth"));
}
