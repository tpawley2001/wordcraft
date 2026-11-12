//! Define New Number Format (Numbering ▾): the form reads the caret's list, OK makes a list with
//! the chosen style, number format text and alignment, and a typed `%` never becomes a placeholder.

use serde_json::json;
use wordcraft_doc::props::Align;
use wordcraft_doc::section::NumFormat;
use wordcraft_engine::Session;

use crate::dialogs::Dialog;
use crate::dialogs::define_number::{NumberForm, join_number_text, number_styles, split_number_text, style_sample};
use crate::{Services, WordApp};

fn app() -> WordApp {
    let mut a = WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default());
    a.run("text.insert", json!({"text": "Item"})).unwrap();
    a
}

fn caret_level0(a: &WordApp) -> wordcraft_doc::numbering::Level {
    let num = wordcraft_engine::cmd::lists::caret_list(&a.session).expect("the caret is in a list");
    wordcraft_engine::cmd::lists::levels_of(&a.session, num).unwrap().into_iter().next().unwrap()
}

#[test]
fn the_number_text_splits_at_the_number_and_joins_back() {
    assert_eq!(split_number_text("Step %1:"), ("Step ".to_string(), ":".to_string()));
    assert_eq!(join_number_text("Step ", ":"), "Step %1:");
    assert_eq!(split_number_text("%1."), (String::new(), ".".to_string()));
    assert_eq!(join_number_text("", "."), "%1.");
    assert_eq!(split_number_text("Note:"), ("Note:".to_string(), String::new()));
}

#[test]
fn a_typed_percent_is_text_not_a_placeholder() {
    assert_eq!(join_number_text("50%", "%2"), "50%12");
    let f = NumberForm { style: "decimal".into(), before: "%2 ".into(), after: "%".into(), align: "left".into() };
    assert_eq!(f.params(), json!({"format": "decimal", "text": "2 %1", "align": "left"}));
    assert_eq!(f.label(3), "2 3");
}

#[test]
fn every_style_but_bullets_and_none_is_offered_with_a_sample() {
    let styles: Vec<NumFormat> = number_styles().collect();
    assert!(!styles.contains(&NumFormat::Bullet) && !styles.contains(&NumFormat::None));
    assert_eq!(styles.len(), NumFormat::ALL.len() - 2);
    assert_eq!(style_sample(NumFormat::Decimal), "1, 2, 3");
    assert_eq!(style_sample(NumFormat::UpperRoman), "I, II, III");
    assert_eq!(style_sample(NumFormat::LowerLetter), "a, b, c");
    assert!(styles.iter().all(|f| !style_sample(*f).is_empty()));
}

#[test]
fn the_ribbon_entry_opens_the_dialog_and_scripts_with_a_format_run_directly() {
    let mut a = app();
    assert_eq!(a.run("para.defineNumber", json!({})).unwrap(), json!({"pending": "defineNumber"}));
    let Some(Dialog::DefineNumber { form }) = &a.dialog else { panic!("dialog") };
    assert_eq!(**form, NumberForm::default());
    assert_eq!(form.params(), json!({"format": "decimal", "text": "%1.", "align": "left"}));
    a.dialog = None;
    a.run("para.defineNumber", json!({"format": "lowerRoman", "text": "%1)"})).unwrap();
    assert!(a.dialog.is_none());
    assert_eq!(caret_level0(&a).format, NumFormat::LowerRoman);
}

#[test]
fn the_form_opens_with_the_caret_lists_format() {
    let mut a = app();
    a.run("para.defineNumber", json!({"format": "lowerLetter", "text": "(%1)", "align": "center"})).unwrap();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let Some(Dialog::DefineNumber { form }) = &a.dialog else { panic!("dialog") };
    let want = NumberForm { style: "lowerLetter".into(), before: "(".into(), after: ")".into(), align: "center".into() };
    assert_eq!(**form, want);
    // A bullet list starts from the plain form.
    let mut b = app();
    b.run("para.bullets", json!({})).unwrap();
    b.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    assert!(matches!(&b.dialog, Some(Dialog::DefineNumber { form }) if **form == NumberForm::default()));
}

#[test]
fn ok_numbers_the_caret_paragraph_with_the_form() {
    let mut a = app();
    let form = NumberForm { style: "upperRoman".into(), before: "Step ".into(), after: ":".into(), align: "right".into() };
    a.run("para.defineNumber", form.params()).unwrap();
    let level = caret_level0(&a);
    assert_eq!((level.format, level.text.as_str(), level.align), (NumFormat::UpperRoman, "Step %1:", Align::Right));
}

/// Drawing the dialog in a frame doesn't panic, and Escape closes it without a list.
#[test]
fn the_dialog_draws_and_escape_changes_nothing() {
    let mut a = app();
    a.run("ui.dialog", json!({"name": "defineNumber"})).unwrap();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    h.run_steps(4);
    assert!(matches!(h.state().dialog, Some(Dialog::DefineNumber { .. })));
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.state().dialog.is_none());
    assert!(wordcraft_engine::cmd::lists::caret_list(&h.state().session).is_none());
}
