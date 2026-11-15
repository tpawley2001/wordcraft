//! Find reads fields the way Word does. With field codes hidden it searches the text the field
//! shows (its result), and a match may run from the paragraph's text into the result and out
//! again; the code text is not searched. With field codes shown it searches the code text
//! instead, a match never crosses the field's edge, and `^d` finds the whole field. Measured in
//! Microsoft 365 Word 16.0 (Find.Execute, wildcards off, TextRetrievalMode/ShowFieldCodes off and
//! on) on `alpha { QUOTE "red fox" } omega`, 2026-11-14.

use serde_json::json;
use wordcraft_doc::{CharProps, Document, InlineObject, StoryRef};

use crate::Session;

/// `alpha ` + QUOTE field showing `red fox` + ` omega`.
fn session() -> Session {
    let mut s = Session::new(Document::new());
    s.run("document.setText", &json!({ "text": "alpha  omega" })).unwrap();
    let path = s.doc.para_paths(StoryRef::Body).remove(0);
    let p = s.doc.para_mut(StoryRef::Body, &path).unwrap();
    let field = InlineObject::Field {
        instr: " QUOTE \"red fox\" ".into(),
        result: "red fox".into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: None,
    };
    p.insert_object(6, field, &CharProps::default()).unwrap();
    s
}

fn count(s: &mut Session, pattern: &str) -> usize {
    s.run("edit.find", &json!({ "text": pattern })).unwrap_or_else(|e| panic!("{pattern}: {e}"));
    s.find.results.len()
}

fn check(s: &mut Session, rows: &[(&str, usize)]) {
    let wrong: Vec<String> = rows
        .iter()
        .filter_map(|(pattern, word)| {
            let ours = count(s, pattern);
            (ours != *word).then(|| format!("{pattern:?}: Word {word} match(es), ours {ours}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn codes_hidden_find_searches_the_result_text() {
    let mut s = session();
    check(
        &mut s,
        &[
            ("red", 1),
            ("fox", 1),
            ("QUOTE", 0),
            // A match runs from the paragraph's text into the result and out of it.
            ("a red", 1),
            ("fox ome", 1),
            ("^d", 0),
            ("alpha", 1),
            ("omega", 1),
        ],
    );
}

#[test]
fn codes_shown_find_searches_the_code_text_and_stops_at_the_field_edge() {
    let mut s = session();
    s.run("view.fieldCodes", &json!({ "value": true })).unwrap();
    check(
        &mut s,
        &[
            ("red", 1),
            ("fox", 1),
            ("QUOTE", 1),
            ("a red", 0),
            ("fox ome", 0),
            ("^d", 1),
            // The braces drawn around a shown code are not characters of the document.
            ("{ QUOTE", 0),
            ("alpha", 1),
            ("omega", 1),
        ],
    );
}

#[test]
fn turning_codes_off_again_searches_the_result_again() {
    let mut s = session();
    s.run("view.fieldCodes", &json!({ "value": true })).unwrap();
    s.run("view.fieldCodes", &json!({ "value": false })).unwrap();
    check(&mut s, &[("QUOTE", 0), ("red", 1), ("a red", 1)]);
}
