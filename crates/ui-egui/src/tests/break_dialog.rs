//! Insert › Break dialog (Alt, I, B), as in Word for Windows.

use crate::WordApp;

fn app() -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default())
}

#[test]
fn the_break_dialog_opens_by_name_on_page_break() {
    let mut a = app();
    let d = crate::dialogs::Dialog::open("break", &mut a).expect("a Break dialog");
    assert_eq!(d.name(), "break");
}

#[test]
fn ok_with_a_page_break_inserts_one_page_break() {
    let mut a = app();
    a.run("text.insert", serde_json::json!({"text": "ab"})).unwrap();
    let before = a.session.doc.body.len();
    crate::dialogs::dialogs_insert::break_dialog_ok(&mut a, "page");
    assert!(a.dialog.is_none(), "OK closes the dialog");
    assert!(a.session.doc.body.len() > before, "a page break splits the paragraph");
}

#[test]
fn ok_with_next_page_starts_a_new_section() {
    let mut a = app();
    a.run("text.insert", serde_json::json!({"text": "ab"})).unwrap();
    let sections = |a: &WordApp| a.session.doc.body.iter().filter(|b| b.as_para().is_some_and(|p| p.section.is_some())).count();
    let before = sections(&a);
    crate::dialogs::dialogs_insert::break_dialog_ok(&mut a, "nextPage");
    assert_eq!(sections(&a), before + 1, "one more section break");
}
