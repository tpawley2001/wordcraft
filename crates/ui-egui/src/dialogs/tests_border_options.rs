//! Borders and Shading › Options…: a paragraph border's From text distances (Top, Bottom, Left,
//! Right; Word's defaults 1, 1, 4, 4 pt) show in the dialog's form as `para.space` and survive a
//! change of the border's colour, instead of going back to the defaults.

use super::*;
use crate::dialogs_para::BordersForm;
use wordcraft_engine::Session;

fn app_with(text: &str) -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("document.setText", json!({"text": text}));
    app
}

fn form(app: &WordApp) -> BordersForm {
    BordersForm::read(app, 0).expect("the Borders and Shading form")
}

/// The form's From text distances as the dialog's state reports them: top, bottom, left, right.
fn form_space(f: &BordersForm) -> Vec<f64> {
    let v = serde_json::to_value(f).expect("the form serializes");
    let space = v.pointer("/para/space").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    space.iter().filter_map(|x| x.as_f64()).collect()
}

/// The caret paragraph's border distances from text (top, bottom, left, right) and the top colour.
fn para_space(app: &WordApp) -> ([Option<f32>; 4], Option<wordcraft_doc::props::Rgb>) {
    let b = app.session.doc.para_at(&app.session.sel.focus).and_then(|p| p.props.borders).unwrap_or_default();
    ([b.top, b.bottom, b.left, b.right].map(|x| x.map(|x| x.space)), b.top.and_then(|x| x.color))
}

fn boxed(app: &mut WordApp) {
    let _ = app.run("para.borders", json!({"sides": {"top": {"space": 6}, "bottom": {"space": 8}, "left": {"space": 10}, "right": {"space": 12}}}));
}

#[test]
fn the_form_opens_with_the_paragraphs_distances() {
    let mut app = app_with("Boxed paragraph");
    boxed(&mut app);
    assert_eq!(form_space(&form(&app)), [6.0, 8.0, 10.0, 12.0]);
}

#[test]
fn a_paragraph_without_borders_opens_with_words_defaults() {
    let app = app_with("Plain paragraph");
    assert_eq!(form_space(&form(&app)), [1.0, 1.0, 4.0, 4.0]);
}

#[test]
fn changing_the_colour_keeps_the_distances() {
    let mut app = app_with("Boxed paragraph");
    boxed(&mut app);
    let mut f = form(&app);
    f.para.color = "C00000".into();
    f.apply(&mut app);
    let (space, color) = para_space(&app);
    assert_eq!(color, wordcraft_doc::props::Rgb::parse("C00000"));
    assert_eq!(space, [Some(6.0), Some(8.0), Some(10.0), Some(12.0)]);
}

#[test]
fn a_new_box_gets_words_default_distances() {
    let mut app = app_with("Plain paragraph");
    let mut f = form(&app);
    f.para.setting = crate::dialogs_para::Setting::Box;
    f.para.sides = [true, true, true, true, false];
    f.apply(&mut app);
    assert_eq!(para_space(&app).0, [Some(1.0), Some(1.0), Some(4.0), Some(4.0)]);
}
