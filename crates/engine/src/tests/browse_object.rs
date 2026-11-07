//! Select Browse Object (Alt+Ctrl+Home) and Ctrl+PageDown / Ctrl+PageUp browsing by it, as in Word
//! (Windows and Linux). Find stays the default browse object.

use serde_json::{Value, json};
use wordcraft_doc::{Path, StoryRef};

use crate::{Session, cmd};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn id(k: &str) -> Option<&'static str> {
    cmd::registry().by_shortcut(k).map(|c| c.id)
}

/// The body paragraph index the caret is in.
fn caret_para(s: &Session) -> usize {
    assert_eq!(s.sel.focus.story, StoryRef::Body);
    s.doc.para_paths(StoryRef::Body).iter().position(|p| *p == s.sel.focus.path).expect("caret in a body paragraph")
}

/// 0 intro, 1 Head A (Heading 1), 2 body one + field, 3 Head B (Heading 2), 4 body two
fn doc() -> Session {
    let mut s = Session::new(wordcraft_doc::Document::from_text("intro\nHead A\nbody one\nHead B\nbody two"));
    for (t, style) in [("Head A", "Heading 1"), ("Head B", "Heading 2")] {
        run(&mut s, "select.text", json!({"text": t}));
        run(&mut s, "styles.apply", json!({"style": style}));
    }
    run(&mut s, "select.text", json!({"text": "body one"}));
    run(&mut s, "caret.end", json!({}));
    run(&mut s, "insert.field", json!({"instr": "PAGE"}));
    run(&mut s, "caret.docStart", json!({}));
    s
}

#[test]
fn browse_keys() {
    if cfg!(target_os = "macos") {
        assert_ne!(id("Mod+Alt+Home"), Some("nav.selectBrowseObject"));
    } else {
        assert_eq!(id("Mod+PageDown"), Some("nav.browseNext"));
        assert_eq!(id("Mod+PageUp"), Some("nav.browsePrevious"));
        assert_eq!(id("Mod+Alt+Home"), Some("nav.selectBrowseObject"));
        assert_eq!(id("Shift+F4"), Some("edit.findNext"), "Find Next keeps Shift+F4");
        assert_eq!(id("F3"), Some("insert.autoTextByName"), "F3 inserts AutoText by name");
    }
}

#[test]
fn the_palette_lists_the_objects_and_find_is_the_default() {
    let mut s = doc();
    let r = run(&mut s, "nav.selectBrowseObject", json!({}));
    assert_eq!(r["current"], "find");
    let names: Vec<&str> = r["objects"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(names, ["goto", "find", "edit", "heading", "graphic", "table", "field", "endnote", "footnote", "comment", "section", "page"]);
    run(&mut s, "nav.browseBy", json!({"object": "heading"}));
    assert_eq!(run(&mut s, "nav.selectBrowseObject", json!({}))["current"], "heading");
    assert!(s.run("nav.browseBy", &json!({"object": "nonsense"})).is_err());
}

#[test]
fn browse_by_heading_goes_to_each_heading_start() {
    let mut s = doc();
    run(&mut s, "nav.browseBy", json!({"object": "heading"}));
    run(&mut s, "nav.browseNext", json!({}));
    assert_eq!((caret_para(&s), s.sel.focus.off), (1, 0));
    run(&mut s, "nav.browseNext", json!({}));
    assert_eq!((caret_para(&s), s.sel.focus.off), (3, 0));
    run(&mut s, "nav.browsePrevious", json!({}));
    assert_eq!((caret_para(&s), s.sel.focus.off), (1, 0));
    assert_eq!(s.doc.plain_text(StoryRef::Body).lines().count(), 5, "browsing changes nothing");
}

#[test]
fn browse_by_field_and_table() {
    let mut s = doc();
    run(&mut s, "nav.browseBy", json!({"object": "field"}));
    run(&mut s, "nav.browseNext", json!({}));
    assert_eq!(caret_para(&s), 2);
    assert_eq!(s.sel.focus.off, "body one".len(), "before the field");

    let mut s = doc();
    run(&mut s, "select.text", json!({"text": "body two"}));
    run(&mut s, "caret.end", json!({}));
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "nav.browseBy", json!({"object": "table"}));
    run(&mut s, "nav.browseNext", json!({}));
    let p: &Path = &s.sel.focus.path;
    assert!(p.cell().is_some(), "the caret is in the table's first cell: {p:?}");
}

#[test]
fn nothing_further_leaves_the_caret() {
    let mut s = doc();
    run(&mut s, "caret.docEnd", json!({}));
    let before = s.sel.focus.clone();
    run(&mut s, "nav.browseBy", json!({"object": "heading"}));
    let r = run(&mut s, "nav.browseNext", json!({}));
    assert_eq!(r["found"], false);
    assert_eq!(s.sel.focus, before);
}

#[test]
fn find_is_browsed_by_default() {
    let mut s = doc();
    run(&mut s, "edit.findNext", json!({"text": "body"}));
    assert_eq!(caret_para(&s), 2);
    run(&mut s, "nav.browseNext", json!({}));
    assert_eq!(caret_para(&s), 4, "Ctrl+PageDown repeats Find Next");
}
