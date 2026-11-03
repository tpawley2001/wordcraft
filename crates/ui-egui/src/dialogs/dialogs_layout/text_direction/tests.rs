use serde_json::json;
use wordcraft_doc::props::TextDirection;
use wordcraft_engine::Session;

use super::*;
use crate::Services;
use crate::dialogs::Dialog;

fn app() -> WordApp {
    WordApp::new(Session::new(wordcraft_doc::Document::from_text("Dear Jo,\nThanks.")), Services::default())
}

fn current(a: &WordApp) -> TextDirection {
    wordcraft_engine::cmd::page::text_direction_at(&a.session)
}

#[test]
fn text_direction_dialog_opens_by_name() {
    let mut a = app();
    let d = Dialog::open("textDirection", &mut a).expect("textDirection opens a dialog");
    assert_eq!(d.name(), "textDirection");
}

#[test]
fn main_document_form_prefills_and_titles() {
    let a = app();
    let f = TextDirectionForm::new(&a);
    assert_eq!(f.target, TextDirectionTarget::MainDocument);
    assert_eq!(f.title(), "Text Direction - Main Document");
    assert_eq!(f.value, TextDirection::Horizontal);
    assert_eq!(f.choices(), [TextDirection::Horizontal, TextDirection::Down, TextDirection::Up]);
}

#[test]
fn table_cell_form_reads_the_cell_direction() {
    let mut a = app();
    a.run("insert.table", json!({"rows": 1, "cols": 2})).unwrap();
    a.run("table.textDirection", json!({"value": "up"})).unwrap();
    let f = TextDirectionForm::new(&a);
    assert_eq!(f.target, TextDirectionTarget::TableCell);
    assert_eq!(f.title(), "Text Direction - Table Cell");
    assert_eq!(f.value, TextDirection::Up);
    assert_eq!(f.choices(), [TextDirection::Horizontal, TextDirection::Down, TextDirection::Up]);
}

#[test]
fn ok_turns_the_section_in_one_undo_step() {
    let mut a = app();
    let before = serde_json::to_value(&a.session.doc).unwrap();
    let undo = a.session.undo_depth();
    let mut f = TextDirectionForm::new(&a);
    f.value = TextDirection::Down;
    f.ok(&mut a).unwrap();
    assert_eq!(current(&a), TextDirection::Down);
    assert_eq!(a.session.undo_depth(), undo + 1, "one undo step");
    a.run("edit.undo", json!({})).unwrap();
    assert_eq!(serde_json::to_value(&a.session.doc).unwrap(), before, "undo restores the document");
}

#[test]
fn ok_turns_the_caret_cell_only() {
    let mut a = app();
    a.run("insert.table", json!({"rows": 1, "cols": 2})).unwrap();
    let mut f = TextDirectionForm::new(&a);
    f.value = TextDirection::Up;
    f.ok(&mut a).unwrap();
    let t = a.session.doc.body.iter().find_map(|b| b.as_table()).unwrap();
    let dirs: Vec<TextDirection> = t.rows[0].cells.iter().map(|c| c.props.text_direction).collect();
    assert_eq!(dirs, [TextDirection::Up, TextDirection::Horizontal]);
    assert_eq!(a.session.doc.sections().last().map(|(_, s)| s.text_direction), Some(TextDirection::Horizontal), "section untouched");
}

#[test]
fn ok_with_the_current_direction_adds_no_undo_step() {
    let mut a = app();
    let before = serde_json::to_value(&a.session.doc).unwrap();
    let undo = a.session.undo_depth();
    TextDirectionForm::new(&a).ok(&mut a).unwrap();
    assert_eq!(a.session.undo_depth(), undo);
    assert_eq!(serde_json::to_value(&a.session.doc).unwrap(), before);
}
