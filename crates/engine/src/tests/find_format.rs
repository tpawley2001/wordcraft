//! Find and Replace by formatting (Word's Find › Format and Replace › Format, #4893).
//
// Note for the implementer (remove when done): if a helper name or type here doesn't match the
// engine, fix only that; never change what an assertion checks.

use serde_json::json;
use wordcraft_doc::Pos;

use super::{run, s, text};
use crate::{Selection, Session};

/// "alpha beta gamma delta beta" with "beta gamma" (6..16) bold and the last "beta" (23..27) italic.
fn doc() -> Session {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "alpha beta gamma delta beta"}));
    s.sel = Selection { anchor: Pos::body(0, 6), focus: Pos::body(0, 16) };
    run(&mut s, "format.bold", json!({}));
    s.sel = Selection { anchor: Pos::body(0, 23), focus: Pos::body(0, 27) };
    run(&mut s, "format.italic", json!({}));
    s.sel = Selection::caret(Pos::body(0, 0));
    s
}

fn offs(v: &serde_json::Value) -> Vec<(u64, u64)> {
    v["matches"]
        .as_array()
        .expect("matches")
        .iter()
        .map(|m| (m["start"]["offset"].as_u64().or(m["start"]["off"].as_u64()).unwrap(), m["end"]["offset"].as_u64().or(m["end"]["off"].as_u64()).unwrap()))
        .collect()
}

fn bold_at(s: &mut Session, a: usize, b: usize) -> serde_json::Value {
    s.sel = Selection { anchor: Pos::body(0, a), focus: Pos::body(0, b) };
    run(s, "format.state", json!({}))["bold"].clone()
}

#[test]
fn text_with_a_format_matches_only_where_every_character_has_it() {
    let mut s = doc();
    let r = run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": {"bold": true}}));
    assert_eq!(r["count"], 1);
    assert_eq!(offs(&r), vec![(6, 10)]);
    let r = run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": {"italic": true}}));
    assert_eq!(offs(&r), vec![(23, 27)]);
    let r = run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": {"bold": false}}));
    assert_eq!(offs(&r), vec![(23, 27)], "false is a condition: not bold");
}

#[test]
fn empty_text_with_a_format_finds_each_maximal_formatted_stretch() {
    let mut s = doc();
    let r = run(&mut s, "edit.advancedFind", json!({"text": "", "format": {"bold": true}}));
    assert_eq!(offs(&r), vec![(6, 16)], "one match for the whole bold stretch");
}

#[test]
fn no_formatting_clears_the_conditions() {
    let mut s = doc();
    run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": {"bold": true}}));
    let r = run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": null}));
    assert_eq!(r["count"], 2);
}

#[test]
fn find_next_keeps_the_format() {
    let mut s = doc();
    run(&mut s, "edit.find", json!({"text": "beta", "format": {"italic": true}}));
    assert_eq!(s.sel.ordered(), (Pos::body(0, 23), Pos::body(0, 27)));
    s.sel = Selection::caret(Pos::body(0, 0));
    run(&mut s, "edit.findNext", json!({}));
    assert_eq!(s.sel.ordered(), (Pos::body(0, 23), Pos::body(0, 27)), "the plain bold beta is skipped");
}

#[test]
fn replace_all_with_empty_text_and_a_replace_format_keeps_the_text() {
    let mut s = doc();
    let r = run(&mut s, "edit.replaceAll", json!({"text": "", "with": "", "format": {"italic": true}, "replaceFormat": {"bold": true, "italic": false}}));
    assert_eq!(r["replaced"], 1);
    assert_eq!(text(&s), "alpha beta gamma delta beta");
    assert_eq!(bold_at(&mut s, 23, 27), json!(true));
    s.sel = Selection { anchor: Pos::body(0, 23), focus: Pos::body(0, 27) };
    assert_eq!(run(&mut s, "format.state", json!({}))["italic"], json!(false));
    assert_eq!(bold_at(&mut s, 0, 5), json!(false), "unmatched text is untouched");
}

#[test]
fn replace_all_text_and_format_is_one_undo_step() {
    let mut s = doc();
    let r = run(&mut s, "edit.replaceAll", json!({"text": "beta", "with": "BETA", "format": {"bold": true}, "replaceFormat": {"bold": false, "italic": true}}));
    assert_eq!(r["replaced"], 1);
    assert_eq!(text(&s), "alpha BETA gamma delta beta");
    assert_eq!(bold_at(&mut s, 6, 10), json!(false));
    assert_eq!(bold_at(&mut s, 11, 16), json!(true), "gamma keeps its bold");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(text(&s), "alpha beta gamma delta beta");
    assert_eq!(bold_at(&mut s, 6, 10), json!(true));
}

#[test]
fn style_condition_uses_the_paragraph_style() {
    let mut s = doc();
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "beta heading"}));
    run(&mut s, "styles.apply", json!({"style": "Heading 1"}));
    let r = run(&mut s, "edit.advancedFind", json!({"text": "beta", "format": {"style": "Heading 1"}}));
    assert_eq!(r["count"], 1);
    assert_eq!(r["matches"][0]["start"]["path"], json!([1]));
}
