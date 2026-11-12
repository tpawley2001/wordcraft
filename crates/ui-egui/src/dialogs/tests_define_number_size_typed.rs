//! Define New Number Format › Font…: a size typed in the Size box counts without Enter, as in
//! Word: clicking OK, Tab, or moving to the Underline style box keeps it.

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use wordcraft_engine::Session;

use crate::dialogs::Dialog;
use crate::{Services, WordApp};

fn harness(a: WordApp) -> egui_kittest::Harness<'static, WordApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    )
}

/// A one-paragraph document, `define` run first when given, with the dialog open and Font… shown.
fn open(define: Option<serde_json::Value>) -> egui_kittest::Harness<'static, WordApp> {
    let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
    a.run("text.insert", json!({"text": "Item"})).unwrap();
    if let Some(p) = define {
        a.run("para.defineNumber", p).unwrap();
    }
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = harness(a);
    h.run_steps(4);
    let shown = matches!(&h.state().dialog, Some(Dialog::DefineNumber { form }) if form.show_font);
    if !shown {
        h.get_by_label("Font…").click();
        h.run_steps(4);
    }
    h
}

/// Clicks the Size field and types `text` one character at a time (no Enter).
fn type_size(h: &mut egui_kittest::Harness<'static, WordApp>, text: &str) {
    h.get_by_role_and_label(Role::TextInput, "Size:").click();
    h.run_steps(2);
    for c in text.chars() {
        h.get_by_role_and_label(Role::TextInput, "Size:").type_text(&c.to_string());
        h.run_steps(2);
    }
}

fn ok_and_level0(mut h: egui_kittest::Harness<'static, WordApp>) -> wordcraft_doc::numbering::Level {
    h.get_by_label("OK").click();
    h.run_steps(4);
    let a = h.state();
    assert!(a.dialog.is_none(), "OK closes the dialog");
    let num = wordcraft_engine::cmd::lists::caret_list(&a.session).expect("the caret is in a list");
    wordcraft_engine::cmd::lists::levels_of(&a.session, num).unwrap().into_iter().next().unwrap()
}

#[test]
fn a_size_typed_then_ok_clicked_is_kept() {
    let mut h = open(None);
    type_size(&mut h, "20");
    assert_eq!(ok_and_level0(h).chr.size, Some(20.0));
}

#[test]
fn a_size_typed_then_tab_then_ok_is_kept() {
    let mut h = open(None);
    type_size(&mut h, "10.5");
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
    assert_eq!(ok_and_level0(h).chr.size, Some(10.5));
}

#[test]
fn text_that_is_no_size_sets_no_size() {
    let mut h = open(None);
    type_size(&mut h, "abc");
    assert_eq!(ok_and_level0(h).chr.size, None);
}

#[test]
fn clearing_the_size_box_clears_the_numbers_size() {
    let mut h = open(Some(json!({"format": "decimal", "text": "%1.", "size": 20})));
    h.get_by_role_and_label(Role::TextInput, "Size:").click();
    h.run_steps(2);
    for _ in 0..4 {
        h.key_press(egui::Key::Backspace);
        h.run_steps(1);
    }
    h.run_steps(2);
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "Size:").value().as_deref(), Some(""));
    assert_eq!(ok_and_level0(h).chr.size, None);
}
