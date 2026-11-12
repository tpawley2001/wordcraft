//! `format.state` reports the OpenType properties of the selection (Font dialog › Advanced ›
//! OpenType Features): ligatures, number spacing, number forms, stylistic set and contextual
//! alternates, each `null` when the selected runs differ.

use serde_json::{Value, json};

use super::{run, s};
use crate::Session;

/// "Plain fancy" with everything selected.
fn two_words() -> Session {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "Plain fancy"}));
    run(&mut s, "select.all", json!({}));
    s
}

/// `format.set` with `props` on the current selection, then `format.state`'s `key`.
fn set_then(s: &mut Session, props: Value, key: &str) -> Value {
    run(s, "format.set", json!({ "props": props }));
    run(s, "format.state", json!({}))[key].clone()
}

fn ligatures(standard: bool, contextual: bool, historical: bool, discretional: bool) -> Value {
    json!({"ligatures": {"standard": standard, "contextual": contextual, "historical": historical, "discretional": discretional}})
}

#[test]
fn plain_text_reports_the_defaults() {
    let mut s = two_words();
    let st = run(&mut s, "format.state", json!({}));
    assert_eq!(st["ligatures"], json!("standardContextual"), "unset ligatures draw as standard and contextual");
    assert_eq!(st["numSpacing"], json!("default"));
    assert_eq!(st["numForm"], json!("default"));
    assert_eq!(st["stylisticSet"], json!(0));
    assert_eq!(st["contextualAlternates"], json!(true), "unset contextual alternates are on");
}

#[test]
fn ligatures_report_their_w14_name() {
    let mut s = two_words();
    for (props, name) in [
        (ligatures(false, false, false, false), "none"),
        (ligatures(true, false, false, false), "standard"),
        (ligatures(true, true, false, false), "standardContextual"),
        (ligatures(false, false, true, true), "historicalDiscretional"),
        (ligatures(true, true, true, true), "all"),
        (ligatures(true, false, true, false), "standardHistorical"),
    ] {
        assert_eq!(set_then(&mut s, props, "ligatures"), json!(name));
    }
}

#[test]
fn number_spacing_and_forms_report_their_w14_name() {
    let mut s = two_words();
    for name in ["proportional", "tabular", "default"] {
        assert_eq!(set_then(&mut s, json!({ "numSpacing": name }), "numSpacing"), json!(name));
    }
    for name in ["lining", "oldStyle", "default"] {
        assert_eq!(set_then(&mut s, json!({ "numForm": name }), "numForm"), json!(name));
    }
    assert_eq!(run(&mut s, "format.state", json!({}))["numSpacing"], json!("default"), "number forms leave number spacing alone");
}

#[test]
fn stylistic_set_is_the_one_set_switched_on() {
    let mut s = two_words();
    assert_eq!(set_then(&mut s, json!({"stylisticSets": [{"id": 5, "on": true}]}), "stylisticSet"), json!(5));
    assert_eq!(set_then(&mut s, json!({"stylisticSets": [{"id": 20, "on": true}]}), "stylisticSet"), json!(20));
    assert_eq!(set_then(&mut s, json!({"stylisticSets": [{"id": 2, "on": false}, {"id": 7, "on": true}]}), "stylisticSet"), json!(7));
    assert_eq!(set_then(&mut s, json!({"stylisticSets": []}), "stylisticSet"), json!(0));
    assert_eq!(set_then(&mut s, json!({"stylisticSets": [{"id": 3, "on": false}]}), "stylisticSet"), json!(0), "a set switched off is no set");
}

#[test]
fn several_stylistic_sets_report_null() {
    let mut s = two_words();
    let sets = json!({"stylisticSets": [{"id": 1, "on": true}, {"id": 4, "on": true}]});
    assert_eq!(set_then(&mut s, sets, "stylisticSet"), Value::Null);
}

#[test]
fn contextual_alternates_report_on_or_off() {
    let mut s = two_words();
    assert_eq!(set_then(&mut s, json!({"contextualAlternates": false}), "contextualAlternates"), json!(false));
    assert_eq!(set_then(&mut s, json!({"contextualAlternates": true}), "contextualAlternates"), json!(true));
}

#[test]
fn runs_that_differ_report_null_for_that_property_only() {
    let mut s = two_words();
    run(&mut s, "select.text", json!({"text": "fancy"}));
    run(&mut s, "format.set", json!({"props": ligatures(true, true, true, true)}));
    run(&mut s, "format.set", json!({"props": {"numForm": "oldStyle", "stylisticSets": [{"id": 2, "on": true}], "contextualAlternates": false}}));
    let own = run(&mut s, "format.state", json!({}));
    assert_eq!(
        (&own["ligatures"], &own["numForm"], &own["stylisticSet"], &own["contextualAlternates"]),
        (&json!("all"), &json!("oldStyle"), &json!(2), &json!(false))
    );
    run(&mut s, "select.all", json!({}));
    let st = run(&mut s, "format.state", json!({}));
    assert_eq!(st["ligatures"], Value::Null);
    assert_eq!(st["numForm"], Value::Null);
    assert_eq!(st["stylisticSet"], Value::Null);
    assert_eq!(st["contextualAlternates"], Value::Null);
    assert_eq!(st["numSpacing"], json!("default"), "number spacing is the same on both runs");
}
