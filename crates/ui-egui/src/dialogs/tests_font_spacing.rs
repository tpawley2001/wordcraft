//! Font dialog: Word's Scale, Spacing and Position (Character Spacing), sent on OK only when changed.

use super::*;
use wordcraft_doc::props::CharProps;
use wordcraft_engine::Session;

/// `text` with every character selected.
fn selected(text: &str) -> WordApp {
    let mut app = WordApp::new(Session::new(wordcraft_doc::Document::new()), crate::Services::default());
    let _ = app.run("document.setText", json!({"text": text}));
    let _ = app.run("select.all", json!({}));
    app
}

fn char_at(app: &WordApp, i: usize) -> CharProps {
    app.session.doc.para_at(&wordcraft_doc::Pos::body(0, 0)).map(|p| p.props_of_char(i).clone()).unwrap_or_default()
}

fn opened(app: &mut WordApp) -> Dialog {
    let Some(d) = Dialog::open("font", app) else { panic!("no font dialog") };
    d
}

/// A number the dialog's fields stand for.
fn number(d: &Dialog, key: &str) -> Option<f64> {
    font_props(d).get(key).and_then(Value::as_f64)
}

/// Plain text opens at Scale 100%, Spacing Normal, Position Normal, and OK changes nothing.
#[test]
fn plain_text_opens_normal_and_ok_sends_nothing() {
    let mut app = selected("Example");
    let d = opened(&mut app);
    assert_eq!((number(&d, "scale"), number(&d, "spacing"), number(&d, "position")), (Some(100.0), Some(0.0), Some(0.0)));
    apply_font(&mut app, &d);
    let c = char_at(&app, 0);
    assert_eq!((c.scale, c.spacing, c.position), (None, None, None), "an untouched dialog sets nothing");
}

/// `format.state` names the selection's scale and position, and the dialog opens with them.
#[test]
fn the_dialog_opens_with_the_text_scale_and_position() {
    let mut app = selected("Example");
    app.run("format.scale", json!({"percent": 150})).unwrap();
    app.run("format.position", json!({"points": 3})).unwrap();
    let st = app.session.run("format.state", &json!({})).unwrap_or_default();
    assert_eq!(st.get("scale").and_then(Value::as_f64), Some(150.0));
    assert_eq!(st.get("position").and_then(Value::as_f64), Some(3.0));
    let d = opened(&mut app);
    assert_eq!((number(&d, "scale"), number(&d, "position")), (Some(150.0), Some(3.0)));
    let Dialog::Font { scale, position, .. } = &d else { panic!("not the Font dialog") };
    assert_eq!((*scale, *position), (150.0, 3.0));

    // Lowered text is a negative position, condensed text a negative spacing.
    app.run("format.position", json!({"points": -2.5})).unwrap();
    app.run("format.spacing", json!({"points": -0.5})).unwrap();
    let d = opened(&mut app);
    assert_eq!((number(&d, "position"), number(&d, "spacing")), (Some(-2.5), Some(-0.5)));
}

/// OK sets the scale and position the user chose, as one undo step, and leaves the rest alone.
#[test]
fn ok_sets_scale_and_position_in_one_undo_step() {
    let mut app = selected("Example");
    app.run("format.spacing", json!({"points": 1.5})).unwrap();
    let mut d = opened(&mut app);
    if let Dialog::Font { scale, position, .. } = &mut d {
        *scale = 80.0;
        *position = -3.0;
    }
    apply_font(&mut app, &d);
    let c = char_at(&app, 0);
    assert_eq!((c.scale, c.position, c.spacing), (Some(80.0), Some(-3.0), Some(1.5)));

    app.run("edit.undo", json!({})).unwrap();
    let c = char_at(&app, 0);
    assert_eq!((c.scale, c.position, c.spacing), (None, None, Some(1.5)), "one Undo takes both back");
}

/// Only what changed is sent: a new scale leaves the text's raised position as it is, and Position
/// back to Normal stores zero.
#[test]
fn only_the_changed_setting_is_applied() {
    let mut app = selected("Example");
    app.run("format.position", json!({"points": 4})).unwrap();
    let mut d = opened(&mut app);
    if let Dialog::Font { scale, .. } = &mut d {
        *scale = 200.0;
    }
    apply_font(&mut app, &d);
    let c = char_at(&app, 0);
    assert_eq!((c.scale, c.position), (Some(200.0), Some(4.0)));

    let mut d = opened(&mut app);
    if let Dialog::Font { position, .. } = &mut d {
        *position = 0.0;
    }
    apply_font(&mut app, &d);
    let c = char_at(&app, 0);
    assert_eq!((c.scale, c.position), (Some(200.0), Some(0.0)));
}
