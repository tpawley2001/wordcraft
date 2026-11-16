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

/// Show highlighter marks, Show drawings and text boxes on screen and Show background colors and
/// images in Print Layout view are kept between runs; a settings file from before them has them on.
#[test]
fn the_show_options_survive_a_restart() {
    let mut a = app();
    for id in ["ui.showHighlight", "ui.showDrawings", "ui.showBackgrounds"] {
        run(&mut a, id, json!({"value": false}));
    }
    let saved = serde_json::to_string(&a.prefs()).unwrap();
    let mut b = app();
    b.apply_prefs(serde_json::from_str(&saved).unwrap());
    assert_eq!(run(&mut b, "ui.showHighlight", json!({})), json!({"showHighlight": false}));
    assert_eq!(run(&mut b, "ui.showDrawings", json!({})), json!({"showDrawings": false}));
    assert_eq!(run(&mut b, "ui.showBackgrounds", json!({})), json!({"showBackgrounds": false}));
    let old: UiState = serde_json::from_str(r#"{"tab":"Insert"}"#).unwrap();
    assert!(old.show_highlight && old.show_drawings && old.show_backgrounds);
}

/// Show highlighter marks off: a print job leaves the highlight out (Word: it "displays and
/// prints highlighted text"), as the job param `highlight: false` does; the document keeps it.
#[test]
fn show_highlighter_marks_off_prints_no_highlight() {
    let mut a = app();
    run(&mut a, "select.text", json!({"text": "Body"}));
    run(&mut a, "format.highlight", json!({"color": "yellow"}));
    let got = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = got.clone();
    a.services.print = Some(Box::new(move |bytes| {
        seen.borrow_mut().push(bytes.to_vec());
        Ok(())
    }));
    run(&mut a, "ui.print", json!({}));
    run(&mut a, "ui.print", json!({}));
    run(&mut a, "ui.print", json!({"highlight": false}));
    run(&mut a, "ui.showHighlight", json!({"value": false}));
    let doc = a.session.doc.clone();
    run(&mut a, "ui.print", json!({}));
    assert_eq!(a.session.doc, doc, "printing keeps the highlight in the document");
    let jobs = got.borrow();
    let [on, again, param_off, option_off] = jobs.as_slice() else { panic!("four jobs") };
    assert_eq!(on, again, "a job prints the same bytes each time");
    assert_ne!(on, option_off, "off: the highlight is not printed");
    assert_eq!(param_off, option_off, "the option prints as `highlight: false` does");
    drop(jobs);
    assert!(a.run("ui.print", json!({"highlight": "no"})).is_err(), "a non-bool `highlight` is an error");
}
