//! File › Options › Advanced › Show document content: crop marks (on, as in Word), text
//! boundaries, picture placeholders and the draft font (off). Per-user, screen-only settings.

use serde_json::{Value, json};

use crate::{UiState, WordApp};

fn app() -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Body")), crate::Services::default())
}

fn run(a: &mut WordApp, id: &str, v: Value) -> Value {
    a.run(id, v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

#[test]
fn defaults_are_words() {
    let mut a = app();
    assert_eq!(run(&mut a, "ui.cropMarks", json!({})), json!({"cropMarks": true}));
    assert_eq!(run(&mut a, "ui.textBoundaries", json!({})), json!({"textBoundaries": false}));
    assert_eq!(run(&mut a, "ui.picturePlaceholders", json!({})), json!({"picturePlaceholders": false}));
    assert_eq!(run(&mut a, "ui.draftFont", json!({})), json!({"draftFont": false, "name": "Courier New", "size": 10.0}));
}

#[test]
fn each_bool_option_sets_and_refuses_junk() {
    for (id, key) in [("ui.cropMarks", "cropMarks"), ("ui.textBoundaries", "textBoundaries"), ("ui.picturePlaceholders", "picturePlaceholders")] {
        let mut a = app();
        let before = run(&mut a, id, json!({}))[key].clone();
        let flipped = json!(!before.as_bool().unwrap());
        assert_eq!(run(&mut a, id, json!({"value": flipped})), json!({ (key): flipped }), "{id}");
        for junk in [json!({"value": "no"}), json!({"value": 0}), json!({"value": null})] {
            assert!(a.run(id, junk.clone()).is_err(), "{id} {junk}");
        }
        assert_eq!(run(&mut a, id, json!({}))[key], flipped, "{id}: a rejected value keeps the setting");
    }
}

#[test]
fn the_draft_font_takes_a_name_and_size() {
    let mut a = app();
    let r = run(&mut a, "ui.draftFont", json!({"value": true, "name": "Consolas", "size": 12}));
    assert_eq!(r, json!({"draftFont": true, "name": "Consolas", "size": 12.0}));
    for junk in [json!({"size": 0}), json!({"size": 2000}), json!({"name": ""}), json!({"value": "yes"})] {
        assert!(a.run("ui.draftFont", junk.clone()).is_err(), "{junk}");
    }
    assert_eq!(run(&mut a, "ui.draftFont", json!({})), json!({"draftFont": true, "name": "Consolas", "size": 12.0}));
}

#[test]
fn the_options_survive_a_restart_and_old_settings_get_words_defaults() {
    let mut a = app();
    run(&mut a, "ui.cropMarks", json!({"value": false}));
    run(&mut a, "ui.textBoundaries", json!({"value": true}));
    run(&mut a, "ui.picturePlaceholders", json!({"value": true}));
    run(&mut a, "ui.draftFont", json!({"value": true, "name": "Consolas", "size": 11}));
    let saved = serde_json::to_string(&a.prefs()).unwrap();
    let mut b = app();
    b.apply_prefs(serde_json::from_str(&saved).unwrap());
    assert_eq!(run(&mut b, "ui.cropMarks", json!({})), json!({"cropMarks": false}));
    assert_eq!(run(&mut b, "ui.textBoundaries", json!({})), json!({"textBoundaries": true}));
    assert_eq!(run(&mut b, "ui.picturePlaceholders", json!({})), json!({"picturePlaceholders": true}));
    assert_eq!(run(&mut b, "ui.draftFont", json!({})), json!({"draftFont": true, "name": "Consolas", "size": 11.0}));
    // A settings file from before these options: Word's defaults.
    let old: UiState = serde_json::from_str(r#"{"tab":"Insert"}"#).unwrap();
    let mut c = app();
    c.ui = old;
    assert_eq!(run(&mut c, "ui.cropMarks", json!({})), json!({"cropMarks": true}));
    assert_eq!(run(&mut c, "ui.draftFont", json!({})), json!({"draftFont": false, "name": "Courier New", "size": 10.0}));
}

/// The options are screen settings: none of them changes the document.
#[test]
fn the_options_are_not_edits() {
    let mut a = app();
    a.session.dirty = false;
    let text = a.session.doc.plain_text(wordcraft_doc::StoryRef::Body);
    run(&mut a, "ui.cropMarks", json!({"value": false}));
    run(&mut a, "ui.textBoundaries", json!({"value": true}));
    run(&mut a, "ui.picturePlaceholders", json!({"value": true}));
    run(&mut a, "ui.draftFont", json!({"value": true}));
    assert!(!a.session.dirty);
    assert_eq!(a.session.doc.plain_text(wordcraft_doc::StoryRef::Body), text);
}
