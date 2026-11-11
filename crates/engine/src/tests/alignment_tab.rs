//! Header & Footer › Insert Alignment Tab (`insert.alignmentTab`, a positional tab `w:ptab`,
//! ECMA-376 §17.3.3.23) and Show Document Text (`view.showDocumentText`), as in Word.

use serde_json::{Value, json};
use wordcraft_doc::para::{InlineObject, PTabAlign, PTabBase};
use wordcraft_doc::props::TabLeader;
use wordcraft_doc::{Document, Pos};

use crate::Session;

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e:?}"))
}

/// "AB" with the caret between A and B.
fn session() -> Session {
    let mut s = Session::new(Document::from_text("AB"));
    run(&mut s, "caret.set", json!({"pos": {"block": 0, "off": 1}}));
    s
}

/// The positional tab at offset 1 of the first body paragraph.
fn ptab(s: &Session) -> (PTabAlign, PTabBase, TabLeader) {
    let pos = Pos::body(0, 1);
    match s.doc.para_at(&pos).and_then(|p| p.object_at(pos.off)) {
        Some(InlineObject::PositionalTab { align, relative_to, leader }) => (*align, *relative_to, *leader),
        other => panic!("no positional tab at 1: {other:?}"),
    }
}

fn text(s: &Session) -> String {
    s.doc.para_at(&Pos::body(0, 0)).expect("paragraph 0").plain_text()
}

#[test]
fn defaults_are_left_margin_no_leader() {
    let mut s = session();
    run(&mut s, "insert.alignmentTab", json!({}));
    assert_eq!(ptab(&s), (PTabAlign::Left, PTabBase::Margin, TabLeader::None));
    assert_eq!(text(&s), "A\tB");
}

#[test]
fn each_option_is_kept() {
    let cases = [
        (json!({"align": "center"}), (PTabAlign::Center, PTabBase::Margin, TabLeader::None)),
        (json!({"align": "right", "relativeTo": "indent", "leader": "dot"}), (PTabAlign::Right, PTabBase::Indent, TabLeader::Dot)),
        (json!({"leader": "hyphen"}), (PTabAlign::Left, PTabBase::Margin, TabLeader::Hyphen)),
        (json!({"leader": "underscore"}), (PTabAlign::Left, PTabBase::Margin, TabLeader::Underscore)),
        (json!({"leader": "middleDot"}), (PTabAlign::Left, PTabBase::Margin, TabLeader::MiddleDot)),
    ];
    for (params, want) in cases {
        let mut s = session();
        run(&mut s, "insert.alignmentTab", params.clone());
        assert_eq!(ptab(&s), want, "{params}");
    }
}

#[test]
fn bad_values_are_refused_and_change_nothing() {
    for params in [json!({"align": "middle"}), json!({"relativeTo": "page"}), json!({"leader": "dots"})] {
        let mut s = session();
        assert!(s.run("insert.alignmentTab", &params).is_err(), "{params}");
        assert_eq!(text(&s), "AB", "{params}");
    }
}

#[test]
fn one_undo_removes_it() {
    let mut s = session();
    run(&mut s, "insert.alignmentTab", json!({"align": "right"}));
    assert_eq!(text(&s), "A\tB");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(text(&s), "AB");
}

#[test]
fn replaces_the_selection() {
    let mut s = Session::new(Document::from_text("AxxB"));
    run(&mut s, "select.range", json!({"anchor": {"block": 0, "off": 1}, "focus": {"block": 0, "off": 3}}));
    run(&mut s, "insert.alignmentTab", json!({"align": "center"}));
    assert_eq!(text(&s), "A\tB");
    assert_eq!(ptab(&s), (PTabAlign::Center, PTabBase::Margin, TabLeader::None));
}

/// `ViewState::show_document_text`, read through its serialized form.
fn shown(s: &Session) -> Value {
    serde_json::to_value(&s.view).unwrap()["showDocumentText"].clone()
}

#[test]
fn show_document_text_is_on_by_default_and_toggles() {
    let mut s = session();
    assert_eq!(shown(&s), true);
    assert_eq!(run(&mut s, "view.showDocumentText", json!({}))["value"], false);
    assert_eq!(shown(&s), false);
    assert_eq!(run(&mut s, "view.showDocumentText", json!({"value": true}))["value"], true);
    assert_eq!(run(&mut s, "view.showDocumentText", json!({"value": true}))["value"], true);
    assert_eq!(shown(&s), true);
}

#[test]
fn show_document_text_is_not_a_document_change() {
    let mut s = session();
    let before = s.doc.clone();
    run(&mut s, "view.showDocumentText", json!({"value": false}));
    assert_eq!(s.doc, before);
}
