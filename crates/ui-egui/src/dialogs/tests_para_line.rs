//! Paragraph dialog: Word's Line spacing box (Single, 1.5 lines, Double, At least, Exactly,
//! Multiple) and its At box.

use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use wordcraft_doc::props::LineSpacing;
use wordcraft_engine::Session;

fn app_with(text: &str) -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("document.setText", json!({"text": text}));
    app
}

fn line_spacing(app: &WordApp) -> Option<LineSpacing> {
    app.session.doc.para_at(&app.session.sel.focus).and_then(|p| p.props.line_spacing)
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

/// The `lineSpacing` field the dialog stands for when opened on a paragraph with `set`.
fn opened_field(set: Value) -> Value {
    let mut app = app_with("Example");
    let _ = app.run("para.lineSpacing", set);
    let Some(d) = Dialog::open("paragraph", &mut app) else { panic!("no paragraph dialog") };
    para_fields(&d).get("lineSpacing").cloned().unwrap_or(Value::Null)
}

#[test]
fn paragraph_dialog_opens_with_the_paragraphs_line_spacing_rule() {
    assert_eq!(opened_field(json!({"exactly": 18})), json!({"rule": "exactly", "value": 18.0}));
    assert_eq!(opened_field(json!({"atLeast": 12})), json!({"rule": "atLeast", "value": 12.0}));
    assert_eq!(opened_field(json!({"value": 1.15})), json!({"rule": "multiple", "value": 1.15f32 as f64}));
    assert_eq!(opened_field(json!({"value": 2})), json!({"rule": "multiple", "value": 2.0}));
}

/// The combo box shows Word's names for the rule; the At box its value.
#[test]
fn paragraph_dialog_shows_words_line_spacing_names() {
    for (set, shown) in [
        (json!({"value": 1}), "Single"),
        (json!({"value": 1.5}), "1.5 lines"),
        (json!({"value": 2}), "Double"),
        (json!({"value": 3}), "Multiple"),
        (json!({"atLeast": 12}), "At least"),
        (json!({"exactly": 18}), "Exactly"),
    ] {
        let mut app = app_with("Example");
        let _ = app.run("para.lineSpacing", set.clone());
        app.dialog = Dialog::open("paragraph", &mut app);
        let mut h = harness(app);
        h.run_steps(4);
        let value = h.get_by_role_and_label(Role::ComboBox, "Line spacing:").value();
        assert_eq!(value.as_deref(), Some(shown), "{set}");
        h.get_by_role_and_label(Role::SpinButton, "At:");
    }
}

/// Picking Exactly and typing 18 in At gives the paragraph exactly 18 pt, in one undo step.
#[test]
fn paragraph_dialog_sets_exact_line_spacing() {
    let mut app = app_with("Example");
    app.dialog = Dialog::open("paragraph", &mut app);
    let depth = app.session.undo_depth();
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::ComboBox, "Line spacing:").click();
    h.run_steps(2);
    h.get_by_label("Exactly").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::SpinButton, "At:").click();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Paste("18".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert!(h.state().dialog.is_none(), "OK closes the dialog");
    assert_eq!(line_spacing(h.state()), Some(LineSpacing::Exactly(18.0)));
    assert_eq!(h.state().session.undo_depth(), depth + 1, "one undo step");
}

/// Picking At least from Single fills At with Word's 12 pt; OK applies it.
#[test]
fn paragraph_dialog_at_least_starts_at_12_pt() {
    let mut app = app_with("Example");
    app.dialog = Dialog::open("paragraph", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::ComboBox, "Line spacing:").click();
    h.run_steps(2);
    h.get_by_label("At least").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert_eq!(line_spacing(h.state()), Some(LineSpacing::AtLeast(12.0)));
}

/// From Exactly back to Double: a multiple of 2.
#[test]
fn paragraph_dialog_double_replaces_a_fixed_spacing() {
    let mut app = app_with("Example");
    let _ = app.run("para.lineSpacing", json!({"exactly": 30}));
    app.dialog = Dialog::open("paragraph", &mut app);
    let mut h = harness(app);
    h.run_steps(4);
    h.get_by_role_and_label(Role::ComboBox, "Line spacing:").click();
    h.run_steps(2);
    h.get_by_label("Double").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "OK").click();
    h.run_steps(4);
    assert_eq!(line_spacing(h.state()), Some(LineSpacing::Multiple(2.0)));
}

/// Guard: OK without changes keeps an exact spacing and adds no undo step.
#[test]
fn paragraph_dialog_unchanged_ok_keeps_exact_spacing() {
    let mut app = app_with("Example");
    let _ = app.run("para.lineSpacing", json!({"exactly": 18}));
    let depth = app.session.undo_depth();
    let Some(d) = Dialog::open("paragraph", &mut app) else { panic!("no paragraph dialog") };
    let mut d = d;
    if let Dialog::Paragraph { start, .. } = &mut d {
        assert!(!start.is_empty());
    }
    apply_paragraph(&mut app, &d);
    assert_eq!(line_spacing(&app), Some(LineSpacing::Exactly(18.0)));
    assert_eq!(app.session.undo_depth(), depth);
}
