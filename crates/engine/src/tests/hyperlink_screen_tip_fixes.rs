//! A hyperlink's ScreenTip (`CharProps::link_tip`, DOCX `w:hyperlink/@w:tooltip`, ECMA-376
//! §17.16.22) stays with its link: rejecting a tracked formatting change puts the old formatting
//! back but keeps the link and its ScreenTip, and `insert.link` without a `screenTip` keeps the
//! tip the link has, however long the file made it.

use serde_json::{Value, json};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// (link target, ScreenTip, bold) of every character of the body's first paragraph.
fn chars(s: &Session) -> Vec<(Option<String>, Option<String>, bool)> {
    let para = s.doc.body.first().and_then(|b| b.as_para()).unwrap();
    para.text
        .char_indices()
        .map(|(i, _)| para.props_of_char(i))
        .map(|c| (c.link.clone(), c.link_tip.as_deref().map(str::to_string), c.bold == Some(true)))
        .collect()
}

fn has_format_change(s: &Session) -> bool {
    let para = s.doc.body.first().and_then(|b| b.as_para()).unwrap();
    para.runs.iter().any(|r| r.props.fmt_change.is_some())
}

/// "a link" to example.com, with `tip` when given, selected, then made bold with Track Changes on.
fn tracked_bold_link(tip: Option<&str>) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    let mut params = json!({"url": "https://example.com", "text": "a link"});
    if let Some(tip) = tip {
        params["screenTip"] = json!(tip);
    }
    run(&mut s, "insert.link", params);
    run(&mut s, "select.all", json!({}));
    run(&mut s, "review.trackChanges", json!({}));
    run(&mut s, "format.bold", json!({}));
    assert!(has_format_change(&s), "the bold is a tracked formatting change");
    assert!(chars(&s).iter().all(|c| c.2), "{:?}", chars(&s));
    s
}

/// "a link" to example.com whose ScreenTip is `tip` as a file gave it (not checked by the command), selected.
fn link_with_stored_tip(tip: &str) -> Session {
    let mut s = Session::new(wordcraft_doc::Document::new());
    run(&mut s, "insert.link", json!({"url": "https://example.com", "text": "a link"}));
    run(&mut s, "select.all", json!({}));
    let (a, b) = s.sel.ordered();
    s.doc.format_range(&a, &b, &|c| c.link_tip = Some(tip.into())).unwrap();
    assert!(chars(&s).iter().all(|c| c.1.as_deref() == Some(tip)));
    s
}

#[test]
fn rejecting_a_tracked_format_change_keeps_the_screen_tip() {
    let mut s = tracked_bold_link(Some("More about it"));
    run(&mut s, "review.rejectAll", json!({}));
    assert!(!has_format_change(&s));
    let got = chars(&s);
    assert_eq!(got.len(), "a link".len());
    for c in &got {
        assert_eq!(c, &(Some("https://example.com".to_string()), Some("More about it".to_string()), false), "{got:?}");
    }
}

#[test]
fn accepting_a_tracked_format_change_keeps_the_screen_tip() {
    let mut s = tracked_bold_link(Some("More about it"));
    run(&mut s, "review.acceptAll", json!({}));
    assert!(!has_format_change(&s));
    let got = chars(&s);
    assert_eq!(got.len(), "a link".len());
    for c in &got {
        assert_eq!(c, &(Some("https://example.com".to_string()), Some("More about it".to_string()), true), "{got:?}");
    }
}

#[test]
fn rejecting_a_tracked_format_change_on_a_link_without_a_tip_adds_none() {
    let mut s = tracked_bold_link(None);
    run(&mut s, "review.rejectAll", json!({}));
    let got = chars(&s);
    assert_eq!(got.len(), "a link".len());
    for c in &got {
        assert_eq!(c, &(Some("https://example.com".to_string()), None, false), "{got:?}");
    }
}

#[test]
fn rejecting_then_undoing_brings_the_tracked_change_and_the_tip_back() {
    let mut s = tracked_bold_link(Some("More about it"));
    run(&mut s, "review.rejectAll", json!({}));
    assert!(chars(&s).iter().all(|c| c.1.as_deref() == Some("More about it") && !c.2), "{:?}", chars(&s));
    run(&mut s, "edit.undo", json!({}));
    assert!(has_format_change(&s));
    assert!(chars(&s).iter().all(|c| c.1.as_deref() == Some("More about it") && c.2), "{:?}", chars(&s));
}

#[test]
fn insert_link_without_a_screen_tip_keeps_a_long_stored_tip() {
    let long = "t".repeat(300);
    let mut s = link_with_stored_tip(&long);
    run(&mut s, "insert.link", json!({"url": "https://example.org"}));
    let got = chars(&s);
    assert_eq!(got.len(), "a link".len());
    assert!(got.iter().all(|c| c.0.as_deref() == Some("https://example.org") && c.1.as_deref() == Some(long.as_str())), "{got:?}");
}

#[test]
fn insert_link_sending_a_long_stored_tip_back_is_refused_and_changes_nothing() {
    let long = "t".repeat(300);
    let mut s = link_with_stored_tip(&long);
    assert!(s.run("insert.link", &json!({"url": "https://example.org", "screenTip": long})).is_err());
    let got = chars(&s);
    assert!(got.iter().all(|c| c.0.as_deref() == Some("https://example.com") && c.1.as_deref() == Some(long.as_str())), "{got:?}");
}

#[test]
fn insert_link_with_an_empty_screen_tip_clears_a_long_stored_tip() {
    let mut s = link_with_stored_tip(&"t".repeat(300));
    run(&mut s, "insert.link", json!({"url": "https://example.org", "screenTip": ""}));
    let got = chars(&s);
    assert!(got.iter().all(|c| c.0.as_deref() == Some("https://example.org") && c.1.is_none()), "{got:?}");
}
