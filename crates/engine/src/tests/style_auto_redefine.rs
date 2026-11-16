//! "Automatically update" on a paragraph style (`w:autoRedefine`, ECMA-376 §17.7.4.2): formatting a
//! paragraph in that style changes the style, so every paragraph in it follows, instead of leaving
//! direct formatting, as in Word. Word honours the flag on paragraph styles only.

use serde_json::{Value, json};
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{Align, Document, Pos, StoryRef};

use crate::{Selection, Session};

fn run(s: &mut Session, id: &str, v: Value) -> Value {
    s.run(id, &v).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Paragraphs "Alpha beta gamma" / "Delta epsilon" / "Zeta eta", the first two in paragraph style
/// `AutoBody` (Automatically update = `auto`), the third in Normal. Caret in paragraph 0.
fn doc(auto: bool) -> Session {
    let mut s = Session::new(Document::new());
    std::sync::Arc::make_mut(&mut s.doc.styles).upsert(Style {
        id: "AutoBody".into(),
        name: "AutoBody".into(),
        kind: StyleKind::Paragraph,
        based_on: Some("Normal".into()),
        auto_redefine: auto,
        ..Default::default()
    });
    run(&mut s, "text.insert", json!({"text": "Alpha beta gamma"}));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "Delta epsilon"}));
    run(&mut s, "text.newParagraph", json!({}));
    run(&mut s, "text.insert", json!({"text": "Zeta eta"}));
    for i in 0..2 {
        let p = s.doc.para_mut(StoryRef::Body, &Pos::body(i, 0).path).unwrap();
        p.props.style = Some("AutoBody".into());
    }
    s.sel = Selection::caret(Pos::body(0, 3));
    s
}

fn style(s: &Session, id: &str) -> Style {
    s.doc.styles.get(id).unwrap().clone()
}

fn para(s: &Session, i: usize) -> wordcraft_doc::Paragraph {
    s.doc.para_at(&Pos::body(i, 0)).unwrap().clone()
}

fn select(s: &mut Session, i: usize, a: usize, b: usize) {
    s.sel = Selection { anchor: Pos::body(i, a), focus: Pos::body(i, b) };
}

#[test]
fn paragraph_formatting_redefines_the_style() {
    let mut s = doc(true);
    run(&mut s, "para.alignCenter", json!({}));
    assert_eq!(style(&s, "AutoBody").para.align, Some(Align::Center), "the style takes the alignment");
    for i in 0..2 {
        let p = para(&s, i);
        assert_eq!(p.props.align, None, "paragraph {i} has no direct alignment");
        assert_eq!(p.props.style.as_deref(), Some("AutoBody"), "paragraph {i} stays in AutoBody");
    }
}

#[test]
fn flag_off_keeps_direct_formatting() {
    let mut s = doc(false);
    run(&mut s, "para.alignCenter", json!({}));
    assert_eq!(style(&s, "AutoBody").para.align, None);
    assert_eq!(para(&s, 0).props.align, Some(Align::Center));
    assert_eq!(para(&s, 1).props.align, None);
}

#[test]
fn other_styles_untouched() {
    let mut s = doc(true);
    run(&mut s, "para.alignCenter", json!({}));
    assert_eq!(para(&s, 2).props.align, None);
    assert_eq!(style(&s, "Normal").para.align, None);
}

#[test]
fn bold_on_the_whole_paragraph_redefines_the_style() {
    let mut s = doc(true);
    let len = para(&s, 0).len();
    select(&mut s, 0, 0, len);
    run(&mut s, "format.bold", json!({"value": true}));
    assert_eq!(style(&s, "AutoBody").chr.bold, Some(true), "the style takes bold");
    for i in 0..2 {
        let p = para(&s, i);
        for off in 0..p.len() {
            assert_eq!(p.props_at(off).bold, None, "paragraph {i} offset {off} has no direct bold");
        }
    }
}

#[test]
fn bold_on_one_word_stays_direct() {
    let mut s = doc(true);
    select(&mut s, 0, 6, 10); // "beta"
    run(&mut s, "format.bold", json!({"value": true}));
    assert_eq!(style(&s, "AutoBody").chr.bold, None, "part of a paragraph doesn't redefine the style");
    let p = para(&s, 0);
    assert_eq!(p.props_at(7).bold, Some(true), "the word is bold directly");
    assert_eq!(p.props_at(1).bold, None);
}

#[test]
fn character_styles_ignore_the_flag() {
    let mut s = doc(false);
    std::sync::Arc::make_mut(&mut s.doc.styles).upsert(Style {
        id: "AutoChar".into(),
        name: "AutoChar".into(),
        kind: StyleKind::Character,
        auto_redefine: true,
        ..Default::default()
    });
    {
        let p = s.doc.para_mut(StoryRef::Body, &Pos::body(0, 0).path).unwrap();
        p.format(0, 5, &|c| c.style = Some("AutoChar".into())).unwrap();
    }
    select(&mut s, 0, 0, 5); // "Alpha"
    run(&mut s, "format.bold", json!({"value": true}));
    assert_eq!(style(&s, "AutoChar").chr.bold, None, "character styles never redefine");
    assert_eq!(para(&s, 0).props_at(1).bold, Some(true));
}

#[test]
fn one_undo_restores_style_and_paragraph() {
    let mut s = doc(true);
    run(&mut s, "para.alignCenter", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(style(&s, "AutoBody").para.align, None, "undo puts the style back");
    assert_eq!(para(&s, 0).props.align, None);
    assert_eq!(para(&s, 1).props.align, None);
}
