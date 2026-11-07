//! Building Blocks Organizer (`insert.buildingBlocks`), its insert behaviours (ECMA-376 §17.12.1
//! `w:behavior`: content, own paragraph, own page) and F3 inserting AutoText by name
//! (`insert.autoTextByName`), as in Word.

use serde_json::{Value, json};
use wordcraft_doc::{BuildingBlock, Document};

use crate::Session;

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

fn texts(s: &Session) -> Vec<String> {
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()
}

fn caret(s: &mut Session, para: u32, off: usize) {
    let at = json!({"story": "body", "path": [para], "off": off});
    run(s, "select.range", json!({"anchor": at, "focus": at}));
}

/// A document building block holding one paragraph of `text`.
fn entry(name: &str, gallery: &str, category: &str, behavior: &str, text: &str) -> BuildingBlock {
    let src = Document::from_text(text);
    BuildingBlock {
        name: name.into(),
        gallery: gallery.into(),
        category: category.into(),
        behaviors: if behavior.is_empty() { vec![] } else { vec![behavior.into()] },
        blocks: src.body.iter().cloned().collect(),
        ..Default::default()
    }
}

fn names(v: &Value) -> Vec<String> {
    v["entries"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect()
}

fn index_of(v: &Value, name: &str) -> u64 {
    v["entries"].as_array().unwrap().iter().find(|e| e["name"] == name).unwrap()["index"].as_u64().unwrap()
}

#[test]
fn create_saves_the_selection_with_word_defaults() {
    let mut s = Session::new(Document::from_text("Alpha beta\nGamma"));
    run(&mut s, "select.text", json!({"text": "beta"}));
    let v = run(&mut s, "insert.buildingBlocks", json!({"create": {"name": "B"}}));
    let e = &v["entries"][0];
    assert_eq!(e["name"], "B");
    assert_eq!(e["gallery"], "docParts");
    assert_eq!(e["category"], "General");
    assert_eq!(e["description"], "");
    assert_eq!(e["behavior"], "content");
    assert_eq!(s.doc.glossary.len(), 1);
    assert!(s.dirty, "the glossary is saved with the document");

    // Every property given.
    run(&mut s, "select.text", json!({"text": "Gamma"}));
    let v = run(
        &mut s,
        "insert.buildingBlocks",
        json!({"create": {"name": "G", "gallery": "autoTxt", "category": "Mine", "description": "third letter", "behavior": "p"}}),
    );
    let g = v["entries"].as_array().unwrap().iter().find(|e| e["name"] == "G").unwrap().clone();
    assert_eq!((g["gallery"].clone(), g["category"].clone(), g["description"].clone(), g["behavior"].clone()), (json!("autoTxt"), json!("Mine"), json!("third letter"), json!("p")));
    assert_eq!(s.doc.glossary.iter().find(|e| e.name == "G").unwrap().behaviors, vec!["p".to_string()]);

    // Same name and gallery: redefined, not duplicated.
    run(&mut s, "select.text", json!({"text": "Alpha"}));
    run(&mut s, "insert.buildingBlocks", json!({"create": {"name": "B"}}));
    assert_eq!(s.doc.glossary.iter().filter(|e| e.name == "B").count(), 1);
}

#[test]
fn create_refuses_bad_input_and_changes_nothing() {
    let mut s = Session::new(Document::from_text("Alpha beta"));
    caret(&mut s, 0, 2);
    assert!(s.run("insert.buildingBlocks", &json!({"create": {"name": "X"}})).is_err(), "nothing selected");
    run(&mut s, "select.text", json!({"text": "beta"}));
    for bad in [json!({"name": " "}), json!({"name": "X", "gallery": "nope"}), json!({"name": "X", "behavior": "nope"})] {
        assert!(s.run("insert.buildingBlocks", &json!({"create": bad})).is_err(), "{bad}");
    }
    assert!(s.doc.glossary.is_empty());
}

#[test]
fn list_is_sorted_by_gallery_category_name_and_hides_placeholders() {
    let mut s = Session::new(Document::from_text("x"));
    let mut ph = entry("Placeholder", "docParts", "General", "", "Click here");
    ph.types = vec!["bbPlcHdr".into()];
    s.doc.glossary = vec![
        entry("zeta", "docParts", "General", "", "z"),
        entry("Alpha", "docParts", "General", "", "a"),
        entry("Sig", "autoTxt", "General", "", "s"),
        entry("beta", "docParts", "Archive", "", "b"),
        ph,
    ];
    let v = run(&mut s, "insert.buildingBlocks", json!({}));
    assert_eq!(names(&v), ["Sig", "beta", "Alpha", "zeta"]);
    assert_eq!(index_of(&v, "zeta"), 0, "index is the glossary position");
    assert_eq!(index_of(&v, "beta"), 3);
}

#[test]
fn modify_and_delete_by_index() {
    let mut s = Session::new(Document::from_text("x"));
    s.doc.glossary = vec![entry("One", "docParts", "General", "", "1"), entry("Two", "docParts", "General", "", "2")];
    run(&mut s, "insert.buildingBlocks", json!({"modify": 1, "name": "Deux", "category": "French", "description": "d", "behavior": "pg"}));
    let e = &s.doc.glossary[1];
    assert_eq!((e.name.as_str(), e.category.as_str(), e.description.as_str()), ("Deux", "French", "d"));
    assert_eq!(e.behaviors, vec!["pg".to_string()]);
    assert_eq!(texts_of(e), ["2"], "content unchanged");
    assert!(s.run("insert.buildingBlocks", &json!({"modify": 7, "name": "x"})).is_err());

    let v = run(&mut s, "insert.buildingBlocks", json!({"delete": 0}));
    assert_eq!(names(&v), ["Deux"]);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc.glossary.len(), 2, "one undo step");
}

fn texts_of(e: &BuildingBlock) -> Vec<String> {
    e.blocks.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect()
}

/// Insert "Inserted" with `behavior` at offset 6 of "Alpha beta" (between "Alpha " and "beta").
fn insert_mid(behavior: &str, via_quick_parts: bool) -> Vec<String> {
    let mut s = Session::new(Document::from_text("Alpha beta\nGamma"));
    s.doc.glossary = vec![entry("Ins", "docParts", "General", behavior, "Inserted")];
    caret(&mut s, 0, 6);
    if via_quick_parts {
        run(&mut s, "insert.quickParts", json!({"insert": "Ins"}));
    } else {
        run(&mut s, "insert.buildingBlocks", json!({"insert": 0}));
    }
    texts(&s)
}

#[test]
fn content_goes_inline() {
    for via in [false, true] {
        assert_eq!(insert_mid("content", via), ["Alpha Insertedbeta", "Gamma"]);
        assert_eq!(insert_mid("", via), ["Alpha Insertedbeta", "Gamma"]);
    }
}

#[test]
fn own_paragraph_splits_the_paragraph_at_the_caret() {
    for via in [false, true] {
        assert_eq!(insert_mid("p", via), ["Alpha ", "Inserted", "beta", "Gamma"], "via quickParts: {via}");
    }
    // At a paragraph start: before it; at its end: after it; no new empty paragraphs.
    let mut s = Session::new(Document::from_text("Alpha\nGamma"));
    s.doc.glossary = vec![entry("Ins", "docParts", "General", "p", "Inserted")];
    caret(&mut s, 1, 0);
    run(&mut s, "insert.buildingBlocks", json!({"insert": 0}));
    assert_eq!(texts(&s), ["Alpha", "Inserted", "Gamma"]);
    caret(&mut s, 0, 5);
    run(&mut s, "insert.buildingBlocks", json!({"insert": 0}));
    assert_eq!(texts(&s), ["Alpha", "Inserted", "Inserted", "Gamma"]);
}

#[test]
fn own_page_puts_page_breaks_around_the_content() {
    let joined = insert_mid("pg", false).join("\n");
    let (before, rest) = joined.split_once("Inserted").expect("inserted");
    assert!(before.starts_with("Alpha ") && before.contains('\u{000C}'), "a page break before: {joined:?}");
    assert!(rest.contains('\u{000C}') && rest.ends_with("beta\nGamma"), "a page break after: {joined:?}");
    assert_eq!(joined.matches('\u{000C}').count(), 2, "{joined:?}");

    // At the document start no break is needed before it.
    let mut s = Session::new(Document::from_text("Alpha"));
    s.doc.glossary = vec![entry("Ins", "docParts", "General", "pg", "Inserted")];
    caret(&mut s, 0, 0);
    run(&mut s, "insert.buildingBlocks", json!({"insert": 0}));
    let joined = texts(&s).join("\n");
    assert!(joined.starts_with("Inserted"), "{joined:?}");
    assert_eq!(joined.matches('\u{000C}').count(), 1, "{joined:?}");
}

#[test]
fn f3_replaces_the_word_before_the_caret_with_its_autotext() {
    let cases: [(&str, Result<&str, ()>); 5] = [
        ("Dear sig", Ok("Dear Kind regards, Tyche")), // exact name, any case
        ("Dear SIG", Ok("Dear Kind regards, Tyche")),
        ("Dear addr", Ok("Dear 1 Main Street")),       // unique prefix of "Address"
        ("Dear a", Err(())),                           // "Address" and "Agenda": ambiguous
        ("Dear zzz", Err(())),                         // no entry
    ];
    for (typed, want) in cases {
        let mut s = Session::new(Document::from_text(typed));
        s.doc.glossary = vec![
            entry("Sig", "autoTxt", "General", "", "Kind regards, Tyche"),
            entry("Address", "autoTxt", "General", "", "1 Main Street"),
            entry("Agenda", "autoTxt", "General", "", "Agenda items"),
            entry("dear", "docParts", "General", "", "not AutoText"),
        ];
        caret(&mut s, 0, typed.chars().count());
        let r = s.run("insert.autoTextByName", &json!({}));
        match want {
            Ok(text) => {
                r.unwrap_or_else(|e| panic!("{typed}: {e:?}"));
                assert_eq!(texts(&s), [text], "{typed}");
                run(&mut s, "edit.undo", json!({}));
                assert_eq!(texts(&s), [typed], "one undo step");
            }
            Err(()) => {
                assert!(r.is_err(), "{typed}");
                assert_eq!(texts(&s), [typed], "unchanged");
            }
        }
    }
}

#[test]
fn f3_is_bound_and_alt_f3_is_unchanged() {
    let id = |k: &str| crate::cmd::registry().by_shortcut(k).map(|c| c.id);
    assert_eq!(id("F3"), Some("insert.autoTextByName"));
    assert_eq!(id("Alt+F3"), Some("insert.autoText"));
}
