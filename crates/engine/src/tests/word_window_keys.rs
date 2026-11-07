//! Word's default keys that had no command: Ctrl+F6 / Alt+F6 go to the next document window,
//! Ctrl+Shift+F6 / Alt+Shift+F6 to the previous one, and Ctrl+Shift+Q applies the Symbol font.

use serde_json::{Value, json};
use wordcraft_doc::StoryRef;

use crate::{Session, cmd};

fn s() -> Session {
    Session::new(wordcraft_doc::Document::new())
}

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn text(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body)
}

fn id(k: &str) -> Option<&'static str> {
    cmd::registry().by_shortcut(k).map(|c| c.id)
}

#[test]
fn window_and_symbol_font_keys_resolve() {
    if cfg!(target_os = "macos") {
        assert_eq!(id("Mod+F6"), None);
        assert_eq!(id("Mod+Shift+Q"), None);
    } else {
        assert_eq!(id("Mod+F6"), Some("view.nextWindow"));
        assert_eq!(id("Alt+F6"), Some("view.nextWindow"));
        assert_eq!(id("Mod+Shift+F6"), Some("view.previousWindow"));
        assert_eq!(id("Alt+Shift+F6"), Some("view.previousWindow"));
        assert_eq!(id("Mod+Shift+Q"), Some("format.symbolFont"));
    }
}

#[test]
fn next_and_previous_window_cycle_in_opening_order_and_wrap() {
    let mut s = s();
    let a = s.document_id();
    run(&mut s, "text.insert", json!({"text": "Alpha"}));
    run(&mut s, "file.new", json!({}));
    let b = s.document_id();
    run(&mut s, "text.insert", json!({"text": "Beta"}));
    run(&mut s, "file.new", json!({}));
    let c = s.document_id();
    run(&mut s, "text.insert", json!({"text": "Gamma"}));
    let depth = |s: &mut Session, d: u64| {
        let back = s.document_id();
        s.activate(d).unwrap();
        let n = s.undo_depth();
        s.activate(back).unwrap();
        n
    };
    let before: Vec<usize> = [a, b, c].iter().map(|&d| depth(&mut s, d)).collect();

    let list = run(&mut s, "view.nextWindow", json!({}));
    assert_eq!(s.document_id(), a, "after the last window comes the first");
    assert_eq!(text(&s), "Alpha");
    assert_eq!(list["documents"].as_array().map(Vec::len), Some(3), "returns the document list");
    run(&mut s, "view.nextWindow", json!({}));
    assert_eq!((s.document_id(), text(&s).as_str()), (b, "Beta"));
    run(&mut s, "view.previousWindow", json!({}));
    assert_eq!((s.document_id(), text(&s).as_str()), (a, "Alpha"));
    run(&mut s, "view.previousWindow", json!({}));
    assert_eq!((s.document_id(), text(&s).as_str()), (c, "Gamma"), "before the first window comes the last");

    let after: Vec<usize> = [a, b, c].iter().map(|&d| depth(&mut s, d)).collect();
    assert_eq!(before, after, "switching windows adds no undo step to any document");
}

#[test]
fn one_window_stays_active() {
    let mut s = s();
    let a = s.document_id();
    run(&mut s, "text.insert", json!({"text": "Only"}));
    run(&mut s, "view.nextWindow", json!({}));
    assert_eq!(s.document_id(), a);
    run(&mut s, "view.previousWindow", json!({}));
    assert_eq!(s.document_id(), a);
    assert_eq!(text(&s), "Only");
}

#[test]
fn symbol_font_applies_symbol_to_the_selection_in_one_undo_step() {
    let mut s = s();
    run(&mut s, "text.insert", json!({"text": "abc"}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "format.font", json!({"name": "Georgia", "eastAsia": "SimSun"}));
    run(&mut s, "format.symbolFont", json!({}));
    let st = run(&mut s, "format.state", json!({}));
    assert_eq!(st["font"], "Symbol");
    assert_eq!(st["fontEastAsia"], "SimSun", "the East Asian font is left alone");
    assert_eq!(text(&s), "abc", "the characters are unchanged");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(run(&mut s, "format.state", json!({}))["font"], "Georgia");
}
