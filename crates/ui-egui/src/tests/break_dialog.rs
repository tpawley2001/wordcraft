//! Insert › Break dialog (Alt, I, B), as in Word for Windows.

use crate::WordApp;

/// The body's text (a page break is U+000C), without internal revision counters.
fn body(a: &WordApp) -> String {
    a.session.doc.plain_text(wordcraft_doc::StoryRef::Body)
}

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
    crate::dialogs::dialogs_insert::break_dialog_ok(&mut a, "page");
    assert!(a.dialog.is_none(), "OK closes the dialog");
    // The dialog's page break is the same inline break Ctrl+Enter inserts.
    let mut b = app();
    b.run("text.insert", serde_json::json!({"text": "ab"})).unwrap();
    let before = body(&b);
    b.run("text.pageBreak", serde_json::json!({})).unwrap();
    assert_ne!(body(&b), before, "Ctrl+Enter changes the text");
    assert_eq!(body(&a), body(&b), "OK inserts one page break, like Ctrl+Enter");
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
