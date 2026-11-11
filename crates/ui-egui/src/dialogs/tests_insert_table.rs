//! Insert Table dialog: Word's AutoFit behavior choices and Remember dimensions for new tables.

use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
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

fn opened_size(app: &mut WordApp) -> (u32, u32) {
    match Dialog::open("insertTable", app) {
        Some(Dialog::InsertTable { rows, cols, .. }) => (rows, cols),
        _ => panic!("no Insert Table dialog"),
    }
}

/// Types `n` into the number box named `label`.
fn type_number(h: &mut egui_kittest::Harness<'static, WordApp>, label: &str, n: &str) {
    h.get_by_role_and_label(Role::SpinButton, label).click();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Paste(n.into()));
    h.run_steps(2);
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
}

#[test]
fn insert_table_dialog_offers_words_autofit_choices() {
    let mut app = app_with("Example");
    app.dialog = Dialog::open("insertTable", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    for label in ["Fixed column width:", "AutoFit to contents", "AutoFit to window"] {
        h.get_by_role_and_label(Role::RadioButton, label);
    }
    h.get_by_role_and_label(Role::CheckBox, "Remember dimensions for new tables");
}

/// AutoFit to window gives the new table a 100% preferred width.
#[test]
fn insert_table_dialog_autofit_to_window() {
    let mut app = app_with("Example");
    app.dialog = Dialog::open("insertTable", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::RadioButton, "AutoFit to window").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
    let s = &h.state().session;
    // The caret is in the first cell's paragraph: table path + [row, cell, paragraph].
    let p = &s.sel.focus.path.0;
    let t = s.doc.table(s.sel.focus.story, &wordcraft_doc::Path(p[..p.len() - 3].to_vec())).expect("a table at the caret");
    assert_eq!(t.props.width_pct, Some(100.0));
    assert_eq!(t.grid.len(), 5, "Word's default 5 columns");
}

/// Remember dimensions: the next Insert Table opens with the columns and rows just used.
#[test]
fn insert_table_dialog_remembers_dimensions_when_asked() {
    let mut app = app_with("Example");
    assert_eq!(opened_size(&mut app), (2, 5), "Word's default 2 rows x 5 columns");
    app.dialog = Dialog::open("insertTable", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    type_number(&mut h, "Number of columns:", "3");
    type_number(&mut h, "Number of rows:", "4");
    h.get_by_role_and_label(Role::CheckBox, "Remember dimensions for new tables").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none());
    assert_eq!(opened_size(h.state_mut()), (4, 3));
}

/// Guard: without Remember the next dialog opens at 2 x 5 again.
#[test]
fn insert_table_dialog_forgets_dimensions_by_default() {
    let mut app = app_with("Example");
    app.dialog = Dialog::open("insertTable", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    type_number(&mut h, "Number of columns:", "3");
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert_eq!(opened_size(h.state_mut()), (2, 5));
}
