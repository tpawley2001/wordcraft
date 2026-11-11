//! Paragraph dialog: Word's "Automatically adjust right indent when document grid is defined"
//! (ECMA-376 §17.3.1.1 `w:adjustRightInd`) and "Snap to grid when document grid is defined"
//! (§17.3.1.32 `w:snapToGrid`) check boxes on Indents and Spacing; both on unless a paragraph
//! turns them off.

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

/// The first body paragraph's own (adjust_right_indent, snap_to_grid).
fn own(app: &WordApp) -> (Option<bool>, Option<bool>) {
    let p = app.session.doc.para(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(0)).expect("paragraph");
    (p.props.adjust_right_indent, p.props.snap_to_grid)
}

/// Sets check box `i` of the dialog's paragraph options (4 = adjust right indent, 5 = snap to grid).
fn set_opt(d: &mut Dialog, i: usize, on: bool) {
    let Dialog::Paragraph { opts, .. } = d else { panic!("not the paragraph dialog") };
    let Some(o) = opts.iter_mut().nth(i) else { panic!("no option {i}") };
    *o = on;
}

#[test]
fn a_plain_paragraph_has_both_ticked() {
    let mut app = app_with("Example");
    let f = para_fields(&open(&mut app));
    assert_eq!((f.get("adjustRightIndent"), f.get("snapToGrid")), (Some(&json!(true)), Some(&json!(true))));
}

#[test]
fn a_paragraph_off_the_grid_shows_it() {
    let mut app = app_with("Example");
    let Ok(p) = app.session.doc.para_mut(wordcraft_doc::StoryRef::Body, &wordcraft_doc::Path::top(0)) else { panic!("paragraph") };
    p.props.snap_to_grid = Some(false);
    let f = para_fields(&open(&mut app));
    assert_eq!((f.get("adjustRightIndent"), f.get("snapToGrid")), (Some(&json!(true)), Some(&json!(false))));
}

#[test]
fn unticking_snap_to_grid_writes_it() {
    let mut app = app_with("Example");
    let mut d = open(&mut app);
    set_opt(&mut d, 5, false);
    apply_paragraph(&mut app, &d);
    assert_eq!(own(&app), (None, Some(false)));
}

#[test]
fn unticking_adjust_right_indent_writes_it() {
    let mut app = app_with("Example");
    let mut d = open(&mut app);
    set_opt(&mut d, 4, false);
    apply_paragraph(&mut app, &d);
    assert_eq!(own(&app), (Some(false), None));
}

#[test]
fn an_unchanged_ok_writes_neither() {
    let mut app = app_with("Example");
    let d = open(&mut app);
    apply_paragraph(&mut app, &d);
    assert_eq!(own(&app), (None, None));
}
