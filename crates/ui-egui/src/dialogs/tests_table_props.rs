//! Table Properties: Measure in Percent for the table's preferred width, and Alt Text (Title and
//! Description), applied with the table settings as one undo step.

use super::*;
use egui_kittest::kittest::Queryable;
use wordcraft_doc::props::{Align, TableProps};
use wordcraft_engine::Session;

fn app_with_table() -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    app.run("insert.table", json!({"rows": 2, "cols": 2})).unwrap();
    app
}

/// The properties of the table the caret is in.
fn table_props(app: &WordApp) -> TableProps {
    let (tp, _, _) = app.session.sel.focus.path.cell().expect("the caret is in a table");
    app.session.doc.table(app.session.sel.focus.story, &tp).expect("a table").props.clone()
}

/// Opens Table Properties and hands back its form and the values it opened with.
fn open(app: &mut WordApp) -> (TableForm, TableForm) {
    app.dialog = None;
    app.run("table.properties", json!({})).unwrap();
    let Some(Dialog::TableProperties { form, basis }) = app.dialog.take() else { panic!("Table Properties did not open") };
    (*form, *basis)
}

fn apply(app: &mut WordApp, commands: Vec<(&'static str, Value)>) {
    for (id, params) in commands {
        app.run(id, params).unwrap();
    }
}

#[test]
fn a_percent_width_table_opens_and_stays_in_percent() {
    let mut a = app_with_table();
    a.run("table.properties", json!({"widthPct": 50})).unwrap();
    let (form, basis) = open(&mut a);
    assert!(form.width_on && form.width_percent, "Preferred width is on and measured in Percent");
    assert!((form.width - 50.0).abs() < 1e-3, "the box shows the percentage, got {}", form.width);
    assert_eq!(form.changes(&basis), json!({}), "untouched, nothing changes");

    let mut f = form.clone();
    f.width = 75.0;
    assert_eq!(f.changes(&basis), json!({"widthPct": 75.0}));
    apply(&mut a, f.commands(&basis));
    assert_eq!(table_props(&a).width_pct, Some(75.0));

    let (again, basis) = open(&mut a);
    assert!(again.width_on && again.width_percent && (again.width - 75.0).abs() < 1e-3, "reopened: {} {}", again.width_percent, again.width);
    assert_eq!(again.changes(&basis), json!({}));
}

#[test]
fn measure_in_switches_between_percent_and_a_fixed_width() {
    let mut a = app_with_table();
    // A width in points first, so the table starts out with a fixed width.
    a.run("table.properties", json!({"width": 144.0})).unwrap();
    let (form, basis) = open(&mut a);
    assert!(form.width_on && !form.width_percent, "a fixed width is not measured in Percent");
    assert!((form.width - 2.0).abs() < 1e-3, "144 pt is 2 in, got {}", form.width);

    // Measure in: Percent, 50.
    let mut f = form.clone();
    f.width_percent = true;
    f.width = 50.0;
    assert_eq!(f.changes(&basis), json!({"widthPct": 50.0}));
    apply(&mut a, f.commands(&basis));
    assert_eq!(table_props(&a).width_pct, Some(50.0));

    // Measure in: Inches, 3.
    let (form, basis) = open(&mut a);
    assert!(form.width_percent);
    let mut f = form.clone();
    f.width_percent = false;
    f.width = 3.0;
    assert_eq!(f.changes(&basis), json!({"width": 216.0}));
    apply(&mut a, f.commands(&basis));
    let p = table_props(&a);
    assert_eq!((p.width, p.width_pct), (Some(216.0), None));

    // Preferred width unchecked on a percent table: automatic, no percentage left behind.
    a.run("table.properties", json!({"widthPct": 40})).unwrap();
    let (form, basis) = open(&mut a);
    let mut f = form.clone();
    f.width_on = false;
    assert_eq!(f.changes(&basis), json!({"width": null}));
    apply(&mut a, f.commands(&basis));
    let p = table_props(&a);
    assert_eq!((p.width, p.width_pct), (None, None));
}

#[test]
fn alt_text_title_and_description_are_shown_and_set() {
    let mut a = app_with_table();
    let (form, basis) = open(&mut a);
    assert_eq!((form.alt_title.as_str(), form.alt_description.as_str()), ("", ""));
    assert!(form.commands(&basis).is_empty(), "untouched, OK runs nothing");

    let mut f = form.clone();
    f.alt_title = "Prices".into();
    f.alt_description = "Prices by year".into();
    assert_eq!(f.changes(&basis), json!({}), "alt text is not a table.properties setting");
    assert_eq!(f.commands(&basis), vec![("table.altText", json!({"title": "Prices", "text": "Prices by year"}))]);
    apply(&mut a, f.commands(&basis));
    let p = table_props(&a);
    assert_eq!((p.caption.as_deref(), p.description.as_deref()), (Some("Prices"), Some("Prices by year")));

    // Reopened, the boxes show them; only a changed box is sent, and an emptied one clears.
    let (form, basis) = open(&mut a);
    assert_eq!((form.alt_title.as_str(), form.alt_description.as_str()), ("Prices", "Prices by year"));
    assert!(form.commands(&basis).is_empty());
    let mut f = form.clone();
    f.alt_title.clear();
    assert_eq!(f.commands(&basis), vec![("table.altText", json!({"title": ""}))]);
    apply(&mut a, f.commands(&basis));
    let p = table_props(&a);
    assert_eq!((p.caption.as_deref(), p.description.as_deref()), (None, Some("Prices by year")));

    // With a table setting changed too, table.properties runs first.
    let (form, basis) = open(&mut a);
    let mut f = form.clone();
    f.align = "center".into();
    f.alt_description = "By year".into();
    assert_eq!(f.commands(&basis), vec![("table.properties", json!({"align": "center"})), ("table.altText", json!({"text": "By year"}))]);
}

#[test]
fn ok_applies_table_settings_and_alt_text_as_one_undo_step() {
    let mut a = app_with_table();
    let story = a.session.sel.focus.story;
    let (tp, _, _) = a.session.sel.focus.path.cell().expect("the caret is in a table");
    a.run("table.properties", json!({})).unwrap();
    let Some(Dialog::TableProperties { form, .. }) = a.dialog.as_mut() else { panic!("Table Properties did not open") };
    form.align = "center".into();
    form.alt_title = "Prices".into();

    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1440.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        a,
    );
    for _ in 0..4 {
        h.step();
    }
    h.get_by_label("OK").click();
    for _ in 0..4 {
        h.step();
    }
    let a = h.state_mut();
    assert!(a.dialog.is_none(), "OK closes the dialog");
    let p = a.session.doc.table(story, &tp).expect("a table").props.clone();
    assert_eq!((p.align, p.caption.as_deref()), (Some(Align::Center), Some("Prices")));

    a.run("edit.undo", json!({})).unwrap();
    let p = a.session.doc.table(story, &tp).expect("a table").props.clone();
    assert_ne!(p.align, Some(Align::Center), "one Undo takes the alignment back");
    assert_eq!(p.caption, None, "and the alt text title with it");
}
