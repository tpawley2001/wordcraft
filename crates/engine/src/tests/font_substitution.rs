//! Font Substitution (File › Options › Advanced): `font.substitution`, `font.setSubstitute`,
//! `font.convertPermanently`.

use serde_json::json;

use crate::Session;

const MISSING: &str = "Wordcraft Nowhere Face";
const BUNDLED: &str = "JetBrains Mono";

/// A session with "Missing" in [`MISSING`] and "Bundled" in [`BUNDLED`].
fn session() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({"text": "Bundled\nMissing"})).unwrap();
    s.run("select.all", &json!({})).unwrap();
    s.run("format.font", &json!({"name": BUNDLED})).unwrap();
    s.run("select.text", &json!({"text": "Missing"})).unwrap();
    s.run("format.font", &json!({"name": MISSING})).unwrap();
    s
}

/// `font.substitution`'s rows as (font, shownAs, substitute).
fn rows(s: &mut Session) -> Vec<(String, Vec<String>, Option<String>)> {
    let v = s.run("font.substitution", &json!({})).unwrap();
    v["fonts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let shown = f["shownAs"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
            (f["font"].as_str().unwrap().to_string(), shown, f["substitute"].as_str().map(str::to_string))
        })
        .collect()
}

/// The font name of the text "Missing" as stored in the document.
fn stored_font(s: &mut Session) -> serde_json::Value {
    s.run("select.text", &json!({"text": "Missing"})).unwrap();
    s.run("format.state", &json!({})).unwrap()["font"].clone()
}

#[test]
fn lists_each_missing_font_with_no_choice() {
    let mut s = session();
    let r = rows(&mut s);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].0, MISSING);
    assert!(!r[0].1.is_empty());
    assert_eq!(r[0].2, None, "Default until the user picks one");
}

#[test]
fn a_display_substitute_changes_what_is_drawn_not_the_document() {
    let mut s = session();
    s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
    let r = rows(&mut s);
    assert_eq!(r[0].1, vec![BUNDLED.to_string()]);
    assert_eq!(r[0].2.as_deref(), Some(BUNDLED));
    assert_eq!(stored_font(&mut s), json!(MISSING), "the document still names the missing font");
    assert_eq!(s.run("format.state", &json!({})).unwrap()["fontShownAs"], json!(BUNDLED));
}

#[test]
fn names_match_whatever_their_case() {
    let mut s = session();
    s.run("font.setSubstitute", &json!({"font": MISSING.to_lowercase(), "substitute": BUNDLED})).unwrap();
    assert_eq!(rows(&mut s)[0].2.as_deref(), Some(BUNDLED));
}

#[test]
fn null_goes_back_to_default() {
    let mut s = session();
    let before = rows(&mut s);
    s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
    s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": null})).unwrap();
    assert_eq!(rows(&mut s), before);
}

#[test]
fn a_substitute_that_is_not_installed_is_refused() {
    let mut s = session();
    assert!(s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": "Wordcraft Also Nowhere"})).is_err());
    assert_eq!(rows(&mut s)[0].2, None);
}

#[test]
fn an_installed_font_cannot_be_substituted() {
    let mut s = session();
    s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
    assert!(s.run("font.setSubstitute", &json!({"font": BUNDLED, "substitute": BUNDLED})).is_err());
}

#[test]
fn convert_permanently_renames_the_font_in_one_undo_step() {
    let mut s = session();
    s.run("font.setSubstitute", &json!({"font": MISSING, "substitute": BUNDLED})).unwrap();
    s.run("font.convertPermanently", &json!({"font": MISSING})).unwrap();
    assert_eq!(stored_font(&mut s), json!(BUNDLED));
    assert!(rows(&mut s).is_empty(), "nothing is missing any more");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(stored_font(&mut s), json!(MISSING));
}

#[test]
fn convert_permanently_takes_an_explicit_target() {
    let mut s = session();
    s.run("font.convertPermanently", &json!({"font": MISSING, "to": BUNDLED})).unwrap();
    assert_eq!(stored_font(&mut s), json!(BUNDLED));
}

#[test]
fn convert_permanently_refuses_a_font_that_is_not_missing() {
    let mut s = session();
    assert!(s.run("font.convertPermanently", &json!({"font": BUNDLED, "to": BUNDLED})).is_err());
    assert!(s.run("font.convertPermanently", &json!({"font": MISSING, "to": "Wordcraft Also Nowhere"})).is_err());
    assert_eq!(stored_font(&mut s), json!(MISSING), "a refused conversion changes nothing");
    s.run("font.convertPermanently", &json!({"font": MISSING, "to": BUNDLED})).unwrap();
}
