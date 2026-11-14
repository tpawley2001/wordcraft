//! View › Immersive Reader is its own view with its own tab, as in Word
//! (support.microsoft.com, "Use Immersive Reader in Word"): Column Width (Very Narrow, Narrow,
//! Moderate, Wide), Page Color, Line Focus (one, three or five lines), Text Spacing and Syllables,
//! and Close Immersive Reader back to the view that was active. Screen only: none of these is an
//! edit, and none is saved in the document.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "text.insert", json!({"text": "Reading is easier with fewer distractions"}));
    s.dirty = false;
    s
}

fn state(s: &mut Session) -> Value {
    run(s, "view.state", json!({}))
}

#[test]
fn immersive_reader_is_its_own_view_and_close_goes_back() {
    let mut s = session();
    run(&mut s, "view.draft", json!({}));
    let mode_before = state(&mut s)["mode"].clone();
    let undo = s.undo_labels();

    assert_eq!(run(&mut s, "view.immersive", json!({}))["value"], true);
    let v = state(&mut s);
    assert_eq!(v["immersive"], true);
    assert_eq!(v["readMode"], false, "Immersive Reader is not Read Mode");
    assert_eq!(v["focusMode"], false);
    assert_eq!(v["mode"], mode_before, "the view under Immersive Reader is kept for Close");

    assert_eq!(run(&mut s, "view.closeImmersive", json!({}))["value"], false);
    let v = state(&mut s);
    assert_eq!(v["immersive"], false);
    assert_eq!(v["mode"], mode_before);

    // No value toggles; an explicit value sets.
    assert_eq!(run(&mut s, "view.immersive", json!({"value": true}))["value"], true);
    assert_eq!(run(&mut s, "view.immersive", json!({"value": true}))["value"], true);
    assert_eq!(run(&mut s, "view.immersive", json!({}))["value"], false);

    assert_eq!(s.undo_labels(), undo, "opening and closing is not an edit");
    assert!(!s.dirty);
}

#[test]
fn opening_immersive_reader_leaves_read_mode_focus_and_full_screen_view() {
    let mut s = session();
    run(&mut s, "view.readMode", json!({}));
    assert_eq!(state(&mut s)["readMode"], true);
    run(&mut s, "view.immersive", json!({"value": true}));
    let v = state(&mut s);
    assert_eq!((v["immersive"].clone(), v["readMode"].clone()), (json!(true), json!(false)));

    run(&mut s, "view.immersive", json!({"value": false}));
    run(&mut s, "view.fullScreenView", json!({"value": true}));
    run(&mut s, "view.immersive", json!({"value": true}));
    assert_eq!(state(&mut s)["fullScreenView"], false);

    // Read Mode closes Immersive Reader.
    run(&mut s, "view.readMode", json!({}));
    let v = state(&mut s);
    assert_eq!((v["immersive"].clone(), v["readMode"].clone()), (json!(false), json!(true)));
}

#[test]
fn column_width_takes_words_four_widths() {
    let mut s = session();
    for w in ["veryNarrow", "narrow", "moderate", "wide"] {
        assert_eq!(run(&mut s, "view.immersiveColumnWidth", json!({"value": w}))["columnWidth"], w);
        assert_eq!(state(&mut s)["immersiveColumnWidth"], w);
    }
    run(&mut s, "view.immersiveColumnWidth", json!({"value": "narrow"}));
    for bad in [json!("medium"), json!("page"), json!(3), json!(null)] {
        assert!(s.run("view.immersiveColumnWidth", &json!({"value": bad})).is_err(), "{bad}");
        assert_eq!(state(&mut s)["immersiveColumnWidth"], "narrow", "a rejected value keeps the width");
    }
}

#[test]
fn page_color_takes_named_colors_and_hex() {
    let mut s = session();
    assert_eq!(state(&mut s)["immersivePageColor"], "none");
    for (given, kept) in [("sepia", "sepia"), ("inverse", "inverse"), ("#e6f0d2", "#E6F0D2"), ("#000000", "#000000"), ("none", "none")] {
        assert_eq!(run(&mut s, "view.immersivePageColor", json!({"value": given}))["pageColor"], kept, "{given}");
        assert_eq!(state(&mut s)["immersivePageColor"], kept);
    }
    run(&mut s, "view.immersivePageColor", json!({"value": "sepia"}));
    for bad in [json!("#12345"), json!("#1234567"), json!("#GGGGGG"), json!("green"), json!(""), json!(5)] {
        assert!(s.run("view.immersivePageColor", &json!({"value": bad})).is_err(), "{bad}");
        assert_eq!(state(&mut s)["immersivePageColor"], "sepia");
    }
}

#[test]
fn line_focus_is_none_one_three_or_five_lines() {
    let mut s = session();
    assert_eq!(state(&mut s)["lineFocus"], 0);
    for n in [1, 3, 5, 0, 3] {
        assert_eq!(run(&mut s, "view.lineFocus", json!({"value": n}))["lineFocus"], n);
        assert_eq!(state(&mut s)["lineFocus"], n);
    }
    for bad in [json!(2), json!(4), json!(6), json!(-1), json!("three"), json!(null)] {
        assert!(s.run("view.lineFocus", &json!({"value": bad})).is_err(), "{bad}");
        assert_eq!(state(&mut s)["lineFocus"], 3);
    }
}

#[test]
fn text_spacing_and_syllables_toggle() {
    let mut s = session();
    for cmd in ["view.textSpacing", "view.syllables"] {
        assert_eq!(run(&mut s, cmd, json!({}))["value"], true, "{cmd}");
        assert_eq!(run(&mut s, cmd, json!({}))["value"], false, "{cmd}");
        assert_eq!(run(&mut s, cmd, json!({"value": true}))["value"], true, "{cmd}");
        assert_eq!(run(&mut s, cmd, json!({"value": true}))["value"], true, "{cmd}");
    }
    let v = state(&mut s);
    assert_eq!((v["textSpacing"].clone(), v["syllables"].clone()), (json!(true), json!(true)));
}

#[test]
fn settings_survive_close_and_reopen_and_are_not_edits() {
    let mut s = session();
    let undo = s.undo_labels();
    let text = s.doc.plain_text(wordcraft_doc::StoryRef::Body);
    run(&mut s, "view.immersive", json!({"value": true}));
    run(&mut s, "view.immersiveColumnWidth", json!({"value": "veryNarrow"}));
    run(&mut s, "view.immersivePageColor", json!({"value": "inverse"}));
    run(&mut s, "view.lineFocus", json!({"value": 5}));
    run(&mut s, "view.textSpacing", json!({"value": true}));
    run(&mut s, "view.syllables", json!({"value": true}));
    run(&mut s, "view.closeImmersive", json!({}));
    run(&mut s, "view.immersive", json!({"value": true}));
    let v = state(&mut s);
    assert_eq!(v["immersiveColumnWidth"], "veryNarrow");
    assert_eq!(v["immersivePageColor"], "inverse");
    assert_eq!(v["lineFocus"], 5);
    assert_eq!((v["textSpacing"].clone(), v["syllables"].clone()), (json!(true), json!(true)));
    assert_eq!(s.undo_labels(), undo);
    assert!(!s.dirty, "screen settings don't make the document unsaved");
    assert_eq!(s.doc.plain_text(wordcraft_doc::StoryRef::Body), text);
}

#[test]
fn the_commands_are_in_the_catalog_under_immersive_reader() {
    let cat = crate::catalog::entries();
    for id in [
        "view.immersive",
        "view.closeImmersive",
        "view.immersiveColumnWidth",
        "view.immersivePageColor",
        "view.lineFocus",
        "view.textSpacing",
        "view.syllables",
    ] {
        assert!(cat.iter().any(|e| e.3 == id), "{id} missing from the command catalog");
    }
}
