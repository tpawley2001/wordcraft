//! AutoCaption (References › Insert Caption › AutoCaption) and custom caption labels kept in the
//! document's settings (ECMA-376 Part 1 §17.15.1: `w:captions`, `w:caption`, `w:autoCaptions`).

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Session;

const TABLE: &str = "Microsoft Word Table";

fn run(s: &mut Session, cmd: &str, v: Value) -> Value {
    s.run(cmd, &v).unwrap_or_else(|e| panic!("{cmd} {v}: {e:?}"))
}

/// The body as "para text" lines, with "<table>" for a table.
fn outline(s: &Session) -> Vec<String> {
    s.doc.body.iter().map(|b| b.as_para().map_or("<table>".to_string(), |p| p.plain_text())).collect()
}

fn captions(s: &Session) -> Vec<String> {
    s.doc.body.iter().filter_map(|b| b.as_para()).filter(|p| p.props.style.as_deref() == Some("Caption")).map(|p| p.plain_text()).collect()
}

fn reopen(s: &Session) -> Session {
    Session::new(wordcraft_docx::read(&wordcraft_docx::write(&s.doc).unwrap()).unwrap())
}

fn labels(v: &Value) -> Vec<String> {
    v["labels"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect()
}

fn table_entry(v: &Value) -> Value {
    v["objects"].as_array().unwrap().iter().find(|o| o["name"] == TABLE).expect("table object listed").clone()
}

#[test]
fn custom_labels_live_in_the_document() {
    let mut s = Session::new(Document::from_text("Body"));
    assert_eq!(labels(&run(&mut s, "references.captionLabel", json!({}))), ["Figure", "Table", "Equation"]);
    let v = run(&mut s, "references.captionLabel", json!({"add": "Exhibit"}));
    assert_eq!(labels(&v), ["Figure", "Table", "Equation", "Exhibit"]);
    assert!(s.dirty);
    run(&mut s, "references.captionLabel", json!({"add": "Exhibit"}));
    assert!(s.run("references.captionLabel", &json!({"add": "  "})).is_err());
    assert!(s.run("references.captionLabel", &json!({"delete": "Table"})).is_err(), "built-in");

    let mut back = reopen(&s);
    assert_eq!(labels(&run(&mut back, "references.captionLabel", json!({}))), ["Figure", "Table", "Equation", "Exhibit"]);
    assert!(crate::cmd::citations::caption_labels(&back).contains(&"Exhibit".to_string()));

    run(&mut back, "references.captionLabel", json!({"delete": "Exhibit"}));
    assert_eq!(labels(&run(&mut back, "references.captionLabel", json!({}))), ["Figure", "Table", "Equation"]);
}

#[test]
fn a_table_gets_its_caption_when_inserted() {
    let mut s = Session::new(Document::from_text("Intro"));
    let v = run(&mut s, "references.autoCaption", json!({}));
    assert_eq!(table_entry(&v)["label"], Value::Null, "off by default");

    run(&mut s, "references.autoCaption", json!({"set": {"object": TABLE, "label": "Table", "position": "above"}}));
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "insert.table", json!({"rows": 2, "cols": 2}));
    let o = outline(&s);
    let t = o.iter().position(|x| x == "<table>").unwrap();
    assert_eq!(o[t - 1], "Table 1", "{o:?}");
    assert_eq!(captions(&s), ["Table 1"]);
    assert!(s.sel.focus.path.cell().is_some(), "caret in the new table");

    // One undo step removes table and caption together.
    run(&mut s, "edit.undo", json!({}));
    assert!(!outline(&s).contains(&"<table>".to_string()));
    assert!(captions(&s).is_empty());
}

#[test]
fn below_position_numbering_and_turning_it_off() {
    let mut s = Session::new(Document::from_text("Intro"));
    run(&mut s, "references.autoCaption", json!({"set": {"object": TABLE, "label": "Table"}}));
    assert_eq!(table_entry(&run(&mut s, "references.autoCaption", json!({})))["position"], "below", "Word's default");
    for _ in 0..2 {
        run(&mut s, "caret.docEnd", json!({}));
        run(&mut s, "insert.table", json!({"rows": 1, "cols": 1}));
        run(&mut s, "caret.docEnd", json!({}));
    }
    assert_eq!(captions(&s), ["Table 1", "Table 2"]);
    let o = outline(&s);
    let t = o.iter().position(|x| x == "<table>").unwrap();
    assert_eq!(o[t + 1], "Table 1", "below the table: {o:?}");

    run(&mut s, "references.autoCaption", json!({"set": {"object": TABLE, "label": null}}));
    run(&mut s, "insert.table", json!({"rows": 1, "cols": 1}));
    assert_eq!(captions(&s).len(), 2, "off: no caption");
}

#[test]
fn custom_label_autocaption_survives_save() {
    let mut s = Session::new(Document::from_text("Intro"));
    assert!(s.run("references.autoCaption", &json!({"set": {"object": TABLE, "label": "Exhibit"}})).is_err(), "unknown label");
    run(&mut s, "references.captionLabel", json!({"add": "Exhibit"}));
    run(&mut s, "references.autoCaption", json!({"set": {"object": TABLE, "label": "Exhibit", "position": "above"}}));

    let mut back = reopen(&s);
    let e = table_entry(&run(&mut back, "references.autoCaption", json!({})));
    assert_eq!((e["label"].clone(), e["position"].clone()), (json!("Exhibit"), json!("above")));
    run(&mut back, "caret.docEnd", json!({}));
    run(&mut back, "insert.table", json!({"rows": 1, "cols": 2}));
    assert_eq!(captions(&back), ["Exhibit 1"]);

    // Insert Caption without a position uses the label's stored one.
    run(&mut back, "caret.docStart", json!({}));
    run(&mut back, "references.caption", json!({"label": "Exhibit", "text": ""}));
    let o = outline(&back);
    assert_eq!(o.iter().filter(|x| x.starts_with("Exhibit")).count(), 2, "{o:?}");
    assert!(o[0].starts_with("Exhibit 1"), "above the first paragraph: {o:?}");
}
