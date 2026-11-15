//! Reveal Formatting › Distinguish style source: `view.revealStyleSource` makes `format.reveal` tag
//! each font and paragraph entry with where it comes from: `direct` formatting, a `style` (named in
//! `styleName`), or the document `default`.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e:?}"))
}

const SOURCED: [&str; 4] = ["FONT", "LANGUAGE", "ALIGNMENT AND INDENTATION", "SPACING"];

/// Every section of a listing, as (heading, entries).
fn sections(r: &Value) -> Vec<(String, Vec<Value>)> {
    let mut out = Vec::new();
    for group in r["groups"].as_array().expect("groups") {
        for section in group["sections"].as_array().expect("sections") {
            let heading = section["heading"].as_str().unwrap_or_default().to_string();
            out.push((heading, section["entries"].as_array().cloned().unwrap_or_default()));
        }
    }
    out
}

/// The first entry of section `heading` that `pick` accepts.
fn entry(r: &Value, heading: &str, pick: impl Fn(&Value) -> bool) -> Value {
    sections(r)
        .into_iter()
        .find(|(h, _)| h == heading)
        .and_then(|(_, entries)| entries.into_iter().find(|e| pick(e)))
        .unwrap_or_else(|| panic!("no matching entry in {heading}: {r}"))
}

fn text_is(t: &'static str) -> impl Fn(&Value) -> bool {
    move |e| e["text"] == t
}

fn label_is(l: &'static str) -> impl Fn(&Value) -> bool {
    move |e| e["label"] == l
}

fn has(k: &'static str) -> impl Fn(&Value) -> bool {
    move |e| e.get(k).is_some()
}

fn heading1_name(s: &Session) -> String {
    s.doc.styles.get("Heading1").map(|st| st.name.clone()).expect("Heading1 style")
}

/// "Title" in Heading1, whose style sets bold, 20 pt and 12 pt before; "Body" in Normal.
fn heading_doc() -> Session {
    let mut s = Session::new(Document::from_text("Title\nBody"));
    run(&mut s, "styles.modify", json!({"style": "Heading1", "chr": {"bold": true, "size": 20}, "para": {"spaceBefore": 12}}));
    run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 1}}));
    run(&mut s, "para.style", json!({"style": "Heading1"}));
    s
}

fn on(s: &mut Session) -> Value {
    run(s, "view.revealStyleSource", json!({"value": true}))
}

#[test]
fn off_by_default_and_entries_carry_no_source() {
    let mut s = heading_doc();
    let r = run(&mut s, "format.reveal", json!({}));
    assert_eq!(r["styleSource"], json!(false), "{r}");
    for (heading, entries) in sections(&r) {
        for e in entries {
            assert!(e.get("source").is_none() && e.get("styleName").is_none(), "{heading}: {e}");
        }
    }
}

#[test]
fn the_command_sets_and_toggles_the_check_box() {
    let mut s = Session::new(Document::from_text("One"));
    assert_eq!(run(&mut s, "view.revealStyleSource", json!({}))["styleSource"], json!(true));
    assert!(s.view.reveal_style_source);
    assert_eq!(run(&mut s, "view.revealStyleSource", json!({}))["styleSource"], json!(false));
    assert_eq!(run(&mut s, "view.revealStyleSource", json!({"value": true}))["styleSource"], json!(true));
    assert_eq!(run(&mut s, "view.revealStyleSource", json!({"value": true}))["styleSource"], json!(true));
    assert_eq!(run(&mut s, "format.reveal", json!({}))["styleSource"], json!(true));
}

#[test]
fn formatting_from_the_paragraph_style_names_the_style() {
    let mut s = heading_doc();
    let name = heading1_name(&s);
    let r = on(&mut s);
    for e in [entry(&r, "FONT", text_is("Bold")), entry(&r, "FONT", has("pt")), entry(&r, "SPACING", label_is("Before"))] {
        assert_eq!(e["source"], json!("style"), "{e}");
        assert_eq!(e["styleName"], json!(name), "{e}");
    }
}

#[test]
fn direct_formatting_over_the_style_is_direct() {
    let mut s = heading_doc();
    run(&mut s, "select.range", json!({"anchor": {"block": 0, "off": 0}, "focus": {"block": 0, "off": 5}}));
    run(&mut s, "format.size", json!({"size": 12}));
    run(&mut s, "format.italic", json!({"value": true}));
    run(&mut s, "para.alignCenter", json!({}));
    let r = on(&mut s);
    let size = entry(&r, "FONT", has("pt"));
    assert_eq!((size["pt"].clone(), size["source"].clone()), (json!(12.0), json!("direct")), "{size}");
    assert!(size.get("styleName").is_none(), "{size}");
    assert_eq!(entry(&r, "FONT", text_is("Italic"))["source"], json!("direct"));
    assert_eq!(entry(&r, "ALIGNMENT AND INDENTATION", text_is("Centered"))["source"], json!("direct"));
    // Bold still comes from Heading 1.
    let bold = entry(&r, "FONT", text_is("Bold"));
    assert_eq!((bold["source"].clone(), bold["styleName"].clone()), (json!("style"), json!(heading1_name(&s))), "{bold}");
}

#[test]
fn only_font_and_paragraph_sections_carry_a_source() {
    let mut s = heading_doc();
    run(&mut s, "format.color", json!({"color": "1b1611"}));
    let r = on(&mut s);
    for (heading, entries) in sections(&r) {
        for e in entries {
            if SOURCED.contains(&heading.as_str()) {
                let source = e["source"].as_str().unwrap_or_else(|| panic!("{heading}: no source: {e}"));
                assert!(["direct", "style", "default"].contains(&source), "{heading}: {e}");
                assert_eq!(e.get("styleName").is_some(), source == "style", "{heading}: {e}");
            } else {
                assert!(e.get("source").is_none() && e.get("styleName").is_none(), "{heading}: {e}");
            }
        }
    }
}

#[test]
fn compare_differences_ignore_the_source() {
    let mut s = heading_doc();
    run(&mut s, "view.revealCompare", json!({"value": true}));
    run(&mut s, "caret.set", json!({"pos": {"block": 1, "off": 1}}));
    let before = run(&mut s, "format.reveal", json!({}))["differences"].clone();
    assert!(before.as_array().is_some_and(|d| !d.is_empty()), "{before}");
    let after = on(&mut s)["differences"].clone();
    assert_eq!(before, after);
}
