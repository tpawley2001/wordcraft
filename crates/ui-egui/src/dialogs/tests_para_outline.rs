//! Paragraph dialog: Word's Outline level box on Indents and Spacing (Body Text, Level 1 ..
//! Level 9; ECMA-376 §17.3.1.20 `w:outlineLvl`, 0..8 = Level 1..9, 9 = body text), greyed for
//! the built-in heading styles, whose level comes from the style.

use super::*;
use wordcraft_engine::Session;

fn app_with(text: &str) -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("document.setText", json!({"text": text}));
    app
}

fn open(app: &mut WordApp) -> Dialog {
    let Some(d) = Dialog::open("paragraph", app) else { panic!("no paragraph dialog") };
    d
}

/// The `i`th body paragraph's own (direct) outline level.
fn own_level(app: &WordApp, i: usize) -> Option<u8> {
    app.session.doc.para(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(i)).and_then(|p| p.props.outline_level)
}

/// The `i`th body paragraph's outline level after its style (None = body text).
fn resolved_level(app: &WordApp, i: usize) -> Option<u8> {
    let p = app.session.doc.para(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(i)).expect("paragraph");
    app.session.doc.styles.resolve_para(&p.props).outline_level
}

fn set_outline(d: &mut Dialog, level: u8) {
    let Dialog::Paragraph { outline, .. } = d else { panic!("not the paragraph dialog") };
    *outline = level;
}

#[test]
fn a_plain_paragraph_shows_body_text() {
    let mut app = app_with("Example");
    let d = open(&mut app);
    let Dialog::Paragraph { outline, outline_fixed, .. } = &d else { panic!() };
    assert_eq!((*outline, *outline_fixed), (9, false));
    assert_eq!(para_fields(&d).get("outlineLevel"), Some(&json!(9)));
}

#[test]
fn a_paragraphs_own_level_shows() {
    let mut app = app_with("Example");
    let _ = app.run("para.outlineLevel", json!({"level": 2}));
    let d = open(&mut app);
    assert_eq!(para_fields(&d).get("outlineLevel"), Some(&json!(2)), "Level 3");
}

#[test]
fn ok_applies_a_new_level() {
    let mut app = app_with("Example");
    let mut d = open(&mut app);
    set_outline(&mut d, 0);
    apply_paragraph(&mut app, &d);
    assert_eq!(own_level(&app, 0), Some(0), "Level 1");
}

#[test]
fn body_text_over_a_level_makes_it_body_text() {
    let mut app = app_with("Example");
    let _ = app.run("para.outlineLevel", json!({"level": 2}));
    let mut d = open(&mut app);
    set_outline(&mut d, 9);
    apply_paragraph(&mut app, &d);
    assert_eq!(resolved_level(&app, 0), None);
}

#[test]
fn an_unchanged_ok_writes_no_level() {
    let mut app = app_with("Example");
    let d = open(&mut app);
    apply_paragraph(&mut app, &d);
    assert_eq!(own_level(&app, 0), None);
}

#[test]
fn a_heading_style_fixes_the_box_at_its_level() {
    let mut app = app_with("Example");
    let _ = app.run("styles.apply", json!({"style": "Heading 2"}));
    assert_eq!(resolved_level(&app, 0), Some(1), "Heading 2 is Level 2");
    let mut d = open(&mut app);
    let Dialog::Paragraph { outline, outline_fixed, .. } = &d else { panic!() };
    assert_eq!((*outline, *outline_fixed), (1, true));
    assert_eq!(para_fields(&d).get("outlineLevel"), None, "a greyed box is never sent");
    set_outline(&mut d, 5);
    apply_paragraph(&mut app, &d);
    assert_eq!(resolved_level(&app, 0), Some(1));
}

#[test]
fn paragraphs_that_differ_show_blank_and_keep_their_levels() {
    let mut app = app_with("One\nTwo");
    let _ = app.run("caret.docStart", json!({}));
    let _ = app.run("para.outlineLevel", json!({"level": 0}));
    let _ = app.run("select.all", json!({}));
    let d = open(&mut app);
    let Dialog::Paragraph { mixed, .. } = &d else { panic!() };
    assert!(mixed.contains("outlineLevel"), "{mixed:?}");
    apply_paragraph(&mut app, &d);
    assert_eq!((resolved_level(&app, 0), resolved_level(&app, 1)), (Some(0), None));
}
