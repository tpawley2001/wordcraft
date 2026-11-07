//! Word's accent-composition shortcuts: a dead shortcut (Ctrl+' acute, Ctrl+` grave, Ctrl+Shift+^
//! circumflex, …) inserts nothing and the next typed letter comes out accented; a letter with no
//! accented form is typed unchanged, and any other command in between cancels the accent.

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"));
}

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn typ(s: &mut Session, text: &str) {
    for c in text.chars() {
        run(s, "text.insert", json!({"text": c.to_string()}));
    }
}

fn accent(s: &mut Session, a: &str) {
    run(s, "text.composeAccent", json!({"accent": a}));
}

fn body(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body).trim_end_matches(['\n', '\r']).to_string()
}

/// Each accent with every letter Word composes for it, and the result.
const TABLE: &[(&str, &str, &str)] = &[
    ("acute", "aeiouyAEIOUYdD", "áéíóúýÁÉÍÓÚÝðÐ"),
    ("grave", "aeiouAEIOU", "àèìòùÀÈÌÒÙ"),
    ("circumflex", "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
    ("tilde", "anoANO", "ãñõÃÑÕ"),
    ("diaeresis", "aeiouyAEIOUY", "äëïöüÿÄËÏÖÜŸ"),
    ("ring", "aA", "åÅ"),
    ("ligature", "aAoOs", "æÆœŒß"),
    ("cedilla", "cC", "çÇ"),
    ("stroke", "oO", "øØ"),
];

#[test]
fn every_accent_composes_the_letters_word_composes() {
    for &(name, letters, want) in TABLE {
        let mut s = s();
        for c in letters.chars() {
            accent(&mut s, name);
            typ(&mut s, &c.to_string());
        }
        assert_eq!(body(&s), want, "accent {name}");
    }
}

#[test]
fn the_accent_applies_to_the_next_letter_only() {
    let mut s = s();
    typ(&mut s, "caf");
    accent(&mut s, "acute");
    typ(&mut s, "e e");
    assert_eq!(body(&s), "café e");
}

#[test]
fn a_letter_with_no_accented_form_is_typed_unchanged() {
    let mut s = s();
    accent(&mut s, "cedilla");
    typ(&mut s, "x");
    accent(&mut s, "ring");
    typ(&mut s, "e");
    typ(&mut s, "a");
    assert_eq!(body(&s), "xea");
}

#[test]
fn another_command_cancels_the_pending_accent() {
    let mut s = s();
    typ(&mut s, "ab");
    accent(&mut s, "grave");
    run(&mut s, "caret.left", json!({}));
    typ(&mut s, "e");
    assert_eq!(body(&s), "aeb");
}

#[test]
fn the_last_accent_pressed_wins() {
    let mut s = s();
    accent(&mut s, "acute");
    accent(&mut s, "diaeresis");
    typ(&mut s, "u");
    assert_eq!(body(&s), "ü");
}

#[test]
fn the_dead_key_alone_changes_nothing() {
    let mut s = s();
    typ(&mut s, "a");
    let before = body(&s);
    accent(&mut s, "acute");
    assert_eq!(body(&s), before);
}

#[test]
fn an_unknown_accent_is_refused() {
    let mut s = s();
    assert!(s.run("text.composeAccent", &json!({"accent": "macron"})).is_err());
    assert!(s.run("text.composeAccent", &json!({})).is_err());
}

#[test]
fn inverted_question_and_exclamation_marks() {
    let mut s = s();
    run(&mut s, "text.invertedQuestion", json!({}));
    typ(&mut s, "Qué? ");
    run(&mut s, "text.invertedExclamation", json!({}));
    typ(&mut s, "Hola!");
    assert_eq!(body(&s), "¿Qué? ¡Hola!");
}
