//! Reveal Formatting (`format.reveal`): the formatting at the caret, grouped Font, Paragraph and
//! Section as Word's Reveal Formatting pane groups them.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

fn s() -> Session {
    Session::new(Document::from_text("Body text"))
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// The items under `heading` in group `group`.
fn items(r: &Value, group: &str, heading: &str) -> Vec<String> {
    let g = r["groups"].as_array().unwrap().iter().find(|g| g["name"] == group).unwrap_or_else(|| panic!("no group {group}: {r}"));
    let sec = g["sections"].as_array().unwrap().iter().find(|x| x["heading"] == heading).unwrap_or_else(|| panic!("no {heading} in {group}: {r}"));
    sec["items"].as_array().unwrap().iter().map(|i| i.as_str().unwrap().to_string()).collect()
}

fn select_all(s: &mut Session) {
    run(s, "select.all", json!({}));
}

#[test]
fn groups_come_in_word_order() {
    let mut s = s();
    let r = run(&mut s, "format.reveal", json!({}));
    let names: Vec<&str> = r["groups"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Font", "Paragraph", "Section"]);
}

#[test]
fn font_lists_name_size_and_effects_that_are_on() {
    let mut s = s();
    select_all(&mut s);
    run(&mut s, "format.font", json!({"name": "Arial"}));
    run(&mut s, "format.size", json!({"size": 14}));
    run(&mut s, "format.bold", json!({"value": true}));
    let r = run(&mut s, "format.reveal", json!({}));
    assert_eq!(items(&r, "Font", "FONT"), ["Arial", "14 pt", "Bold"]);

    run(&mut s, "format.size", json!({"size": 10.5}));
    run(&mut s, "format.italic", json!({"value": true}));
    let r = run(&mut s, "format.reveal", json!({}));
    assert_eq!(items(&r, "Font", "FONT"), ["Arial", "10.5 pt", "Bold", "Italic"]);
}

#[test]
fn paragraph_lists_style_alignment_indents_and_spacing() {
    let mut s = s();
    run(&mut s, "para.alignCenter", json!({}));
    run(&mut s, "para.indents", json!({"left": 36, "firstLine": -18}));
    run(&mut s, "para.spacing", json!({"before": 6, "after": 0}));
    run(&mut s, "para.double", json!({}));
    let r = run(&mut s, "format.reveal", json!({}));
    assert_eq!(items(&r, "Paragraph", "PARAGRAPH STYLE"), ["Normal"]);
    assert_eq!(items(&r, "Paragraph", "ALIGNMENT AND INDENTATION"), ["Centered", "Left: 0.5\"", "Hanging: 0.25\""]);
    assert_eq!(items(&r, "Paragraph", "SPACING"), ["Before: 6 pt", "After: 0 pt", "Line spacing: double"]);
}

#[test]
fn line_spacing_wording() {
    let cases: &[(&str, Value, &str)] = &[
        ("para.single", json!({}), "Line spacing: single"),
        ("para.oneAndHalf", json!({}), "Line spacing: 1.5 lines"),
        ("para.lineSpacing", json!({"value": 3}), "Line spacing: Multiple 3 li"),
        ("para.lineSpacing", json!({"atLeast": 12}), "Line spacing: At least 12 pt"),
        ("para.lineSpacing", json!({"exactly": 15.5}), "Line spacing: Exactly 15.5 pt"),
    ];
    for (cmd, v, want) in cases {
        let mut s = s();
        run(&mut s, cmd, v.clone());
        let r = run(&mut s, "format.reveal", json!({}));
        assert_eq!(items(&r, "Paragraph", "SPACING").last().map(String::as_str), Some(*want), "{cmd} {v}");
    }
}

#[test]
fn section_lists_margins_and_paper_in_inches() {
    let mut s = s();
    let r = run(&mut s, "format.reveal", json!({}));
    // Word's default page: US Letter (612 x 792 pt), 1" margins.
    assert_eq!(items(&r, "Section", "MARGINS"), ["Left: 1\"", "Right: 1\"", "Top: 1\"", "Bottom: 1\""]);
    assert_eq!(items(&r, "Section", "PAPER"), ["Width: 8.5\"", "Height: 11\""]);
}

#[test]
fn reveal_does_not_change_the_document() {
    let mut s = s();
    let before = s.doc.clone();
    run(&mut s, "format.reveal", json!({}));
    assert_eq!(format!("{:?}", s.doc), format!("{before:?}"));
}
