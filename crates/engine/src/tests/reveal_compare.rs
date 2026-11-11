//! Reveal Formatting › Compare to another selection: `view.revealCompare` stores the selection's
//! formatting as the first selection, and `format.reveal` then lists the differences between it and
//! the current selection, section by section, as Word's Formatting differences.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e:?}"))
}

fn caret(s: &mut Session, block: usize) {
    run(s, "caret.set", json!({"pos": {"block": block, "off": 1}}));
}

/// "One" in Arial 14 pt bold, "Two" left as the document makes it.
fn two_paragraphs() -> Session {
    let mut s = Session::new(Document::from_text("One\nTwo"));
    run(&mut s, "select.range", json!({"anchor": {"block": 0, "off": 0}, "focus": {"block": 0, "off": 3}}));
    run(&mut s, "format.font", json!({"name": "Arial"}));
    run(&mut s, "format.size", json!({"size": 14}));
    run(&mut s, "format.bold", json!({"value": true}));
    s
}

/// The current listing's first entry of `kind` in the FONT section.
fn font_entry(s: &mut Session, kind: &str) -> Value {
    let r = run(s, "format.reveal", json!({}));
    r["groups"][0]["sections"][0]["entries"].as_array().unwrap().iter().find(|e| e.get(kind).is_some()).cloned().unwrap()
}

fn diffs(s: &mut Session) -> Vec<Value> {
    let r = run(s, "format.reveal", json!({}));
    assert_eq!(r["compare"], true);
    r["differences"].as_array().expect("differences while compare is on").clone()
}

fn find<'a>(d: &'a [Value], heading: &str, pred: impl Fn(&Value) -> bool) -> Option<&'a Value> {
    d.iter().find(|x| x["heading"] == heading && pred(x))
}

#[test]
fn compare_lists_font_differences_against_the_first_selection() {
    let mut s = two_paragraphs();
    caret(&mut s, 1);
    let default_font = font_entry(&mut s, "font");
    let default_size = font_entry(&mut s, "pt");
    caret(&mut s, 0);
    let r = run(&mut s, "view.revealCompare", json!({"value": true}));
    assert_eq!(r["compare"], true);
    caret(&mut s, 1);
    let d = diffs(&mut s);
    let font = find(&d, "FONT", |x| x["first"].get("font").is_some()).expect("font difference");
    assert_eq!(font["group"], "Font");
    assert_eq!(font["first"]["font"], "Arial");
    assert_eq!(font["second"], default_font);
    let size = find(&d, "FONT", |x| x["first"].get("pt").is_some()).expect("size difference");
    assert_eq!(size["first"]["pt"], 14.0);
    assert_eq!(size["second"], default_size);
    let bold = find(&d, "FONT", |x| x["first"]["text"] == "Bold").expect("bold difference");
    assert!(bold["second"].is_null(), "Bold only on the first side: {bold}");
    assert!(find(&d, "MARGINS", |_| true).is_none(), "same section, no margin difference: {d:?}");
}

#[test]
fn the_same_place_has_no_differences() {
    let mut s = two_paragraphs();
    caret(&mut s, 0);
    run(&mut s, "view.revealCompare", json!({"value": true}));
    assert_eq!(diffs(&mut s), Vec::<Value>::new());
}

#[test]
fn alignment_and_spacing_differences_are_keyed_by_section() {
    let mut s = Session::new(Document::from_text("One\nTwo"));
    caret(&mut s, 1);
    run(&mut s, "para.alignCenter", json!({}));
    run(&mut s, "para.spacing", json!({"after": 12}));
    caret(&mut s, 0);
    run(&mut s, "para.spacing", json!({"after": 0}));
    run(&mut s, "view.revealCompare", json!({}));
    caret(&mut s, 1);
    let d = diffs(&mut s);
    let align = find(&d, "ALIGNMENT AND INDENTATION", |x| x["first"].get("text").is_some()).expect("alignment");
    assert_eq!(align["group"], "Paragraph");
    assert_eq!(align["first"]["text"], "Left");
    assert_eq!(align["second"]["text"], "Centered");
    let after = find(&d, "SPACING", |x| x["first"]["label"] == "After").expect("space after");
    assert_eq!(after["first"]["pt"], 0.0);
    assert_eq!(after["second"]["pt"], 12.0);
    assert!(find(&d, "SPACING", |x| x["first"]["label"] == "Before").is_none(), "equal entries give no difference");
}

#[test]
fn off_by_default_and_toggles_off() {
    let mut s = two_paragraphs();
    let r = run(&mut s, "format.reveal", json!({}));
    assert_eq!(r["compare"], false);
    assert!(r.get("differences").is_none());
    let r = run(&mut s, "view.revealCompare", json!({}));
    assert_eq!(r["compare"], true, "absent value toggles on");
    let r = run(&mut s, "view.revealCompare", json!({}));
    assert_eq!(r["compare"], false, "and off");
    assert!(run(&mut s, "format.reveal", json!({})).get("differences").is_none());
}

#[test]
fn a_first_selection_whose_text_was_deleted_does_not_panic() {
    let mut s = Session::new(Document::from_text("One\nTwo three four"));
    run(&mut s, "caret.set", json!({"pos": {"block": 1, "off": 12}}));
    run(&mut s, "view.revealCompare", json!({"value": true}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "text.backspace", json!({}));
    let r = run(&mut s, "format.reveal", json!({}));
    assert_eq!(r["compare"], true);
    assert!(r["differences"].is_array());
}
