//! AutoCorrect Options › Math AutoCorrect: Add and Delete go through `tools.autocorrect`, and
//! Recognized Functions… opens its own dialog over the AutoCorrect one.

use egui_kittest::kittest::Queryable;
use serde_json::json;
use wordcraft_engine::Session;

use crate::WordApp;
use crate::dialogs::Dialog;
use crate::dialogs_review::{AutoCorrectForm, FunctionsForm, ReviewDialog, math_add, math_delete};

fn app() -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("ui.language", json!({"value": "en"}));
    app
}

fn count(app: &mut WordApp) -> u64 {
    app.session.run("tools.autocorrect", &json!({})).unwrap()["mathEntryCount"].as_u64().unwrap()
}

#[test]
fn add_and_delete_round_trip_through_the_command() {
    let mut a = app();
    let mut f = AutoCorrectForm::new(&a);
    f.tab = 2;
    f.math_from = "\\RR".into();
    f.math_to = "\u{211D}".into();
    assert!(math_add(&mut a, &mut f));
    assert_eq!(count(&mut a), 781);
    assert!(a.session.prefs.autocorrect.math_entries.contains(&("\\RR".to_string(), "\u{211D}".to_string())));
    assert!(f.options.math().list().iter().any(|(n, v, _)| n == "\\RR" && v == "\u{211D}"), "the list shows the new entry");

    f.math_from = "\\RR".into();
    assert!(math_delete(&mut a, &mut f));
    assert_eq!(count(&mut a), 780);
    f.math_from = "\\alpha".into();
    assert!(math_delete(&mut a, &mut f));
    assert_eq!(count(&mut a), 779);
    assert!(a.session.prefs.autocorrect.math_removed.contains(&"\\alpha".to_string()));
    assert!(!f.options.math().list().iter().any(|(n, _, _)| n == "\\alpha"));
}

#[test]
fn recognized_functions_apply_on_ok() {
    let mut a = app();
    let mut f = FunctionsForm::new(&a);
    assert_eq!(f.functions.len(), 55);
    f.name = "foo".into();
    f.add();
    f.name = "sin".into();
    f.delete();
    assert!(f.functions.contains(&"foo".to_string()) && !f.functions.contains(&"sin".to_string()));
    assert_eq!(a.session.prefs.autocorrect.functions_added, Vec::<String>::new(), "nothing applies before OK");
    f.apply(&mut a).unwrap();
    let r = a.session.run("tools.autocorrect", &json!({})).unwrap();
    let list: Vec<&str> = r["functions"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
    assert!(list.contains(&"foo") && !list.contains(&"sin"));
}

#[test]
fn recognized_functions_opens_from_the_math_tab_and_returns() {
    let mut app = app();
    app.dialog = Dialog::open("autoCorrect", &mut app);
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).with_accessibility_check(false).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.ctx.set_fonts(crate::theme::font_definitions(false, false));
    h.ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
    let settle = |h: &mut egui_kittest::Harness<'static, WordApp>| {
        for _ in 0..4 {
            h.step();
        }
    };
    settle(&mut h);
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, "Math AutoCorrect").last().unwrap().click();
    settle(&mut h);
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, "Recognized Functions…").last().unwrap().click();
    settle(&mut h);
    match &h.state().dialog {
        Some(Dialog::RecognizedFunctions { form, resume }) => {
            assert_eq!(form.functions.len(), 55);
            assert!(matches!(resume.as_deref(), Some(Dialog::Review(d)) if matches!(**d, ReviewDialog::AutoCorrect(ref f) if f.tab == 2)));
        }
        d => panic!("no Recognized Functions dialog: {d:?}"),
    }
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, "Cancel").last().unwrap().click();
    settle(&mut h);
    assert!(matches!(&h.state().dialog, Some(Dialog::Review(d)) if matches!(**d, ReviewDialog::AutoCorrect(_))), "Cancel goes back to AutoCorrect");
}
