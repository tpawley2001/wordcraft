//! GOTOBUTTON and MACROBUTTON fields: F9 stores their display text, and Alt+Shift+F9 (Word's
//! DoFieldClick, `fields.click`) follows a GOTOBUTTON to its bookmark or page. Macros never run.
//
// Note for the implementer (remove when done): this file was written before it could be compiled.
// If a helper name or type here doesn't match the engine, fix only that; never change what an
// assertion checks.

use serde_json::json;
use wordcraft_doc::{InlineObject, Pos, StoryRef};

use super::{id, run, s, text};
use crate::Session;

/// Every field of the body, as (code, result).
fn fields(s: &Session) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in s.doc.para_paths(StoryRef::Body) {
        let Some(p) = s.doc.para(StoryRef::Body, &path) else { continue };
        for o in &p.objects {
            if let InlineObject::Field { instr, result, .. } = o {
                out.push((instr.trim().to_string(), result.clone()));
            }
        }
    }
    out
}

/// "See below. " then a field, and a second paragraph "Target text" whose "Target" is bookmarked
/// as `Chapter1`. The caret ends just after the field.
fn with_button(instr: &str) -> Session {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "See below. "}));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "Target text"}));
    s.sel = crate::Selection { anchor: Pos::body(1, 0), focus: Pos::body(1, 6) };
    run(&mut s, "insert.bookmark", json!({"name": "Chapter1"}));
    s.sel = crate::Selection::caret(Pos::body(0, "See below. ".len()));
    run(&mut s, "insert.field", json!({"instr": instr}));
    s
}

#[test]
fn alt_shift_f9_is_do_field_click() {
    assert_eq!(id("Alt+Shift+F9"), Some("fields.click"));
}

#[test]
fn do_field_click_on_a_gotobutton_selects_its_bookmark() {
    let mut s = with_button("GOTOBUTTON Chapter1 Go to chapter");
    run(&mut s, "fields.previous", json!({}));
    run(&mut s, "fields.click", json!({}));
    assert_eq!(s.selected_text(), "Target");
    assert_eq!(s.sel.focus.path, Pos::body(1, 0).path, "the selection is in the bookmarked paragraph");
}

#[test]
fn do_field_click_with_the_caret_just_after_the_button_follows_it() {
    let mut s = with_button("GOTOBUTTON Chapter1 Go to chapter");
    run(&mut s, "fields.click", json!({}));
    assert_eq!(s.selected_text(), "Target");
}

#[test]
fn do_field_click_does_nothing_for_unknown_targets_macros_and_other_fields() {
    for instr in ["GOTOBUTTON NoSuchMark Somewhere", "MACROBUTTON NoMacro [Click here]", "PAGE"] {
        let mut s = with_button(instr);
        run(&mut s, "fields.previous", json!({}));
        let (before_text, before_sel) = (text(&s), s.sel.clone());
        s.dirty = false;
        run(&mut s, "fields.click", json!({}));
        assert_eq!(text(&s), before_text, "{instr}");
        assert_eq!(s.sel, before_sel, "{instr}: the selection stays");
        assert!(!s.dirty, "{instr}: nothing changed");
    }
}

#[test]
fn f9_stores_the_display_text_as_the_result() {
    let mut s = with_button("MACROBUTTON NoMacro [Click  here and type]");
    run(&mut s, "references.updateFields", json!({"all": true}));
    assert_eq!(fields(&s), vec![("MACROBUTTON NoMacro [Click  here and type]".to_string(), "[Click  here and type]".to_string())]);

    let mut s = with_button("GOTOBUTTON Chapter1 \"Go to chapter\"");
    run(&mut s, "references.updateFields", json!({"all": true}));
    assert_eq!(fields(&s)[0].1, "Go to chapter");
}
