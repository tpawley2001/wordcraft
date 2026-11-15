//! File › Options › Advanced › Cut, copy, and paste › Use smart cut and paste › Settings ›
//! "Adjust sentence and word spacing automatically": deleting, cutting or pasting a whole word
//! fixes the spaces around it, as in Word. Expected texts measured in Microsoft 365 Word
//! (Options.SmartCutPaste and Options.PasteAdjustWordSpacing on, their defaults).

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn doc(text: &str) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "text.insert", json!({"text": text}));
    s
}

fn sel(s: &mut Session, a: usize, b: usize) {
    run(s, "select.range", json!({"anchor": {"block": 0, "off": a}, "focus": {"block": 0, "off": b}}));
}

fn body(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).trim_end_matches(['\n', '\r']).to_string()
}

/// (text, selection start, selection end, command, Word's result)
const DELETES: &[(&str, usize, usize, &str, &str)] = &[
    ("The quick brown fox.", 10, 15, "text.delete", "The quick fox."),
    ("The quick brown fox.", 10, 15, "edit.cut", "The quick fox."),
    ("The quick brown fox.", 10, 15, "text.backspace", "The quick fox."),
    ("The quick brown fox.", 16, 19, "text.delete", "The quick brown."),
    ("The quick brown fox.", 0, 3, "text.delete", "quick brown fox."),
    ("The quick brown fox.", 10, 16, "text.delete", "The quick fox."),
    ("The quick brown fox.", 9, 15, "text.delete", "The quick fox."),
    ("The quick brown fox.", 10, 13, "text.delete", "The quick wn fox."),
    ("The quick brown fox.", 4, 15, "text.delete", "The fox."),
    ("One, two, three.", 5, 8, "text.delete", "One, , three."),
];

#[test]
fn deleting_a_word_leaves_one_space() {
    for &(text, a, b, cmd, want) in DELETES {
        let mut s = doc(text);
        sel(&mut s, a, b);
        run(&mut s, cmd, json!({}));
        assert_eq!(body(&s), want, "{cmd} {a}..{b} in {text:?}");
    }
}

/// (text, caret, pasted text, Word's result)
const PASTES: &[(&str, usize, &str, &str)] = &[
    ("The quick fox.", 9, "brown", "The quick brown fox."),
    ("The quick fox.", 10, "brown", "The quick brown fox."),
    ("The quick fox.", 6, "brown", "The qu brown ick fox."),
    ("The quick fox.", 10, " brown ", "The quick brown fox."),
    ("The quick fox.", 13, "brown", "The quick fox brown."),
    ("The quick fox.", 0, "brown", "brown The quick fox."),
    ("The quick fox.", 9, "brown.", "The quick brown. fox."),
    ("The quick (fox).", 10, "brown", "The quick brown (fox)."),
];

#[test]
fn pasting_a_word_gets_a_space_on_each_side() {
    for &(text, at, paste, want) in PASTES {
        for cmd in ["edit.paste", "edit.pasteText"] {
            let mut s = doc(text);
            sel(&mut s, at, at);
            run(&mut s, cmd, json!({"text": paste}));
            assert_eq!(body(&s), want, "{cmd} {paste:?} at {at} in {text:?}");
        }
    }
}

#[test]
fn either_setting_off_leaves_the_spaces_alone() {
    for flags in [json!({"smartCutPaste": false}), json!({"pasteAdjustWordSpacing": false})] {
        let mut s = doc("The quick brown fox.");
        run(&mut s, "file.options", flags.clone());
        sel(&mut s, 10, 15);
        run(&mut s, "text.delete", json!({}));
        assert_eq!(body(&s), "The quick  fox.", "{flags}");

        let mut s = doc("The quick fox.");
        run(&mut s, "file.options", flags.clone());
        sel(&mut s, 9, 9);
        run(&mut s, "edit.paste", json!({"text": "brown"}));
        assert_eq!(body(&s), "The quickbrown fox.", "{flags}");
    }
}

#[test]
fn both_settings_default_on_and_round_trip_through_file_options() {
    let mut s = Session::new(wordcraft_doc::Document::new());
    let o = run(&mut s, "file.options", json!({}));
    assert_eq!(o["smartCutPaste"], json!(true));
    assert_eq!(o["pasteAdjustWordSpacing"], json!(true));
    let o = run(&mut s, "file.options", json!({"smartCutPaste": false}));
    assert_eq!(o["smartCutPaste"], json!(false));
    assert_eq!(o["pasteAdjustWordSpacing"], json!(true));
}

#[test]
fn the_spacing_fix_is_one_undo_step_with_the_delete() {
    let mut s = doc("The quick brown fox.");
    sel(&mut s, 10, 15);
    run(&mut s, "text.delete", json!({}));
    assert_eq!(body(&s), "The quick fox.");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(body(&s), "The quick brown fox.");
}
