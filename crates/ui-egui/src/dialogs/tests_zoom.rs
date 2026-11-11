//! Zoom dialog: Word's Zoom to choices (200%, 100%, 75%, Page width, Text width, Whole page, Many
//! pages), applied on OK only.

use super::*;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use wordcraft_engine::Session;

fn app_with(text: &str) -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("document.setText", json!({"text": text}));
    app
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

/// Opens Zoom (after `fit` when given), clicks the radio `choice`, then the button `button` if any.
fn pick(fit: Option<&str>, choice: &str, button: Option<&str>) -> egui_kittest::Harness<'static, WordApp> {
    let mut app = app_with("Example");
    if let Some(f) = fit {
        let _ = app.run("view.zoom", json!({"value": f}));
    }
    app.dialog = Dialog::open("zoom", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::RadioButton, choice).click();
    h.run_steps(2);
    if let Some(b) = button {
        h.get_by_role_and_label(Role::Button, b).click();
        h.run_steps(4);
    }
    h
}

#[test]
fn zoom_dialog_text_width_applies_on_ok() {
    let h = pick(None, "Text width", Some("OK"));
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
    assert_eq!(h.state().session.view.fit, "textWidth");
}

#[test]
fn zoom_dialog_many_pages_applies_on_ok() {
    let h = pick(None, "Many pages", Some("OK"));
    assert_eq!(h.state().session.view.fit, "multiplePages");
}

#[test]
fn zoom_dialog_75_percent_applies_on_ok() {
    let h = pick(None, "75%", Some("OK"));
    assert!((h.state().session.view.zoom - 0.75).abs() < 1e-4);
    assert_eq!(h.state().session.view.fit, "");
}

/// A fit choice does nothing until OK: the dialog stays open and the zoom is unchanged.
#[test]
fn zoom_dialog_whole_page_waits_for_ok() {
    let h = pick(None, "Whole page", None);
    assert!(h.state().dialog.is_some(), "picking a choice doesn't close the dialog");
    assert_eq!(h.state().session.view.fit, "");
}

#[test]
fn zoom_dialog_cancel_keeps_the_zoom() {
    let h = pick(None, "Page width", Some("Cancel"));
    assert!(h.state().dialog.is_none());
    assert_eq!(h.state().session.view.fit, "");
    assert!((h.state().session.view.zoom - 1.0).abs() < 1e-4);
}

/// The dialog opens with the fit mode in force selected.
#[test]
fn zoom_dialog_opens_on_the_current_fit() {
    for (fit, label) in [("textWidth", "Text width"), ("onePage", "Whole page"), ("pageWidth", "Page width")] {
        let mut app = app_with("Example");
        let _ = app.run("view.zoom", json!({"value": fit}));
        app.dialog = Dialog::open("zoom", &mut app);
        let mut h = harness(app);
        h.run_steps(4);
        let node = h.get_by_role_and_label(Role::RadioButton, label);
        assert_eq!(node.accesskit_node().toggled(), Some(Toggled::True), "{fit}: {label} selected");
    }
}
