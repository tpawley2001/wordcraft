//! Paste Special: the Paste and Paste link radio buttons. Paste link is offered for text copied
//! from a saved document and pastes it as the result of a LINK field.

use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use wordcraft_doc::{Pos, StoryRef};
use wordcraft_engine::Session;

fn app_with(text: &str, path: Option<&str>) -> WordApp {
    let mut session = Session::new(wordcraft_doc::Document::from_text(text));
    session.path = path.map(std::path::PathBuf::from);
    WordApp::new(session, crate::Services::default())
}

fn copy_first_word(app: &mut WordApp) {
    app.run("select.range", json!({"anchor": Pos::body(0, 0), "focus": Pos::body(0, 5)})).unwrap();
    app.run("edit.copy", json!({})).unwrap();
    app.run("caret.set", json!({"pos": Pos::body(0, 16)})).unwrap();
}

fn harness(app: WordApp) -> egui_kittest::Harness<'static, WordApp> {
    egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    )
}

fn link_disabled(app: WordApp) -> bool {
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::RadioButton, "Paste link:").accesskit_node().is_disabled()
}

#[test]
fn paste_special_link_radio_needs_a_saved_source() {
    let mut unsaved = app_with("Alpha beta gamma", None);
    copy_first_word(&mut unsaved);
    let d = Dialog::paste_special(&unsaved, &json!({}));
    assert!(matches!(&d, Dialog::PasteSpecial { link_formats, .. } if link_formats.is_empty()));
    unsaved.dialog = Some(d);
    assert!(link_disabled(unsaved), "an unsaved source has nothing to link to");

    let mut saved = app_with("Alpha beta gamma", Some("/home/ann/Report.docx"));
    copy_first_word(&mut saved);
    let d = Dialog::paste_special(&saved, &json!({}));
    assert!(matches!(&d, Dialog::PasteSpecial { link_formats, .. } if link_formats.iter().any(|f| f == "formatted")));
    saved.dialog = Some(d);
    assert!(!link_disabled(saved));
}

#[test]
fn paste_special_ok_with_paste_link_inserts_a_link_field() {
    let mut app = app_with("Alpha beta gamma", Some("/home/ann/Report.docx"));
    copy_first_word(&mut app);
    app.dialog = Some(Dialog::paste_special(&app, &json!({})));
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::RadioButton, "Paste link:").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    let app = h.state();
    let fields = app.session.doc.field_ranges(StoryRef::Body);
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].instr, r#"LINK Word.Document.12 "/home/ann/Report.docx" "OLE_LINK1" \a \r"#);
    assert_eq!(app.session.doc.bookmarks().first().map(|(n, _)| n.as_str()), Some("OLE_LINK1"));
    assert!(app.dialog.is_none());
}
