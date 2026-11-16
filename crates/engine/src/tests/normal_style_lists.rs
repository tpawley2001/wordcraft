//! File › Options › Advanced › Use Normal style for bulleted or numbered lists (off by default, as
//! in Word): on, Bullets and Numbering keep a Normal paragraph's style instead of switching it to
//! List Paragraph.

use super::{run, s};
use crate::Session;
use serde_json::json;

/// The first body paragraph: (in a list, its style).
fn first(s: &Session) -> (bool, Option<String>) {
    let p = s.doc.body.iter().find_map(|b| b.as_para()).expect("a paragraph");
    (p.props.numbering.is_some_and(|n| n.num != 0), p.props.style.clone())
}

fn not_list_paragraph(style: &Option<String>) -> bool {
    style.as_deref().is_none_or(|st| st == "Normal")
}

fn with_text(on: bool) -> Session {
    let mut s = s();
    if on {
        run(&mut s, "file.options", json!({"normalStyleForLists": true}));
    }
    run(&mut s, "text.insert", json!({"text": "item"}));
    s
}

#[test]
fn off_by_default_and_reported_by_file_options() {
    let mut s = s();
    assert!(!s.prefs.normal_style_for_lists);
    let r = run(&mut s, "file.options", json!({"normalStyleForLists": false}));
    assert_eq!(r["normalStyleForLists"], json!(false), "{r}");
    run(&mut s, "file.options", json!({"normalStyleForLists": true}));
    assert!(s.prefs.normal_style_for_lists);
}

#[test]
fn off_bullets_switch_to_list_paragraph() {
    let mut s = with_text(false);
    run(&mut s, "para.bullets", json!({}));
    assert_eq!(first(&s), (true, Some("ListParagraph".into())));
}

#[test]
fn on_bullets_and_numbering_keep_normal() {
    for cmd in ["para.bullets", "para.numbering"] {
        let mut s = with_text(true);
        run(&mut s, cmd, json!({}));
        let (in_list, style) = first(&s);
        assert!(in_list, "{cmd}");
        assert!(not_list_paragraph(&style), "{cmd}: {style:?}");
    }
}

#[test]
fn on_toggle_off_leaves_normal() {
    let mut s = with_text(true);
    run(&mut s, "para.bullets", json!({}));
    run(&mut s, "para.bullets", json!({}));
    let (in_list, style) = first(&s);
    assert!(!in_list);
    assert!(not_list_paragraph(&style), "{style:?}");
}

#[test]
fn on_heading_keeps_its_style() {
    let mut s = with_text(true);
    run(&mut s, "para.style", json!({"style": "Heading1"}));
    run(&mut s, "para.numbering", json!({}));
    assert_eq!(first(&s), (true, Some("Heading1".into())));
}

#[test]
fn on_autoformat_list_keeps_normal() {
    let mut s = s();
    run(&mut s, "file.options", json!({"normalStyleForLists": true}));
    for c in "* item".chars() {
        run(&mut s, "text.insert", json!({"text": c.to_string()}));
    }
    run(&mut s, "text.newParagraph", json!({}));
    let (in_list, style) = first(&s);
    assert!(in_list, "AutoFormat made a list");
    assert!(not_list_paragraph(&style), "{style:?}");
}

#[test]
fn switching_the_option_does_not_restyle_existing_lists() {
    let mut s = with_text(true);
    run(&mut s, "para.bullets", json!({}));
    run(&mut s, "file.options", json!({"normalStyleForLists": false}));
    let (in_list, style) = first(&s);
    assert!(in_list);
    assert!(not_list_paragraph(&style), "{style:?}");
}
