//! Font dialog › Advanced: the OpenType Features group (Ligatures, Number spacing, Number forms,
//! Stylistic sets, Use Contextual Alternates), sent on OK only when changed, and the Scale box's
//! typed percent.

use super::font::parse_scale;
use super::*;
use wordcraft_doc::props::{CharProps, Ligatures, NumForm, NumSpacing, StyleSet};
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

type Shown = (Option<String>, Option<String>, Option<String>, Option<u8>, Option<bool>);

/// The five OpenType fields of an open dialog.
fn shown(d: &Dialog) -> Shown {
    let Dialog::Font { ligatures, num_spacing, num_form, stylistic_set, contextual_alternates, .. } = d else { panic!("not the font dialog") };
    (ligatures.clone(), num_spacing.clone(), num_form.clone(), *stylistic_set, *contextual_alternates)
}

fn plain() -> Shown {
    (Some("standardContextual".into()), Some("default".into()), Some("default".into()), Some(0), Some(true))
}

/// "Plain fancy" as two runs (the second bold), everything selected.
fn two_runs() -> WordApp {
    let mut app = selected("Plain fancy");
    let _ = app.run("select.text", json!({"text": "fancy"}));
    let _ = app.run("format.set", json!({"props": {"bold": true}}));
    let _ = app.run("select.all", json!({}));
    app
}

/// Sets one field through `set`, presses OK, and returns the app.
fn ok_with(app: &mut WordApp, set: impl Fn(&mut Dialog)) {
    let mut d = opened(app);
    set(&mut d);
    apply_font(app, &d);
}

macro_rules! field {
    ($d:expr, $name:ident) => {{
        let Dialog::Font { $name, .. } = $d else { panic!("not the font dialog") };
        $name
    }};
}

#[test]
fn plain_text_opens_with_the_defaults() {
    let mut app = selected("Example");
    assert_eq!(shown(&opened(&mut app)), plain());
}

#[test]
fn ok_untouched_adds_no_opentype_property() {
    let mut app = selected("Example");
    let d = opened(&mut app);
    apply_font(&mut app, &d);
    let c = char_at(&app, 0);
    assert_eq!(c.ligatures, None);
    assert_eq!(c.num_form, None);
    assert_eq!(c.num_spacing, None);
    assert_eq!(c.stylistic_sets, None);
    assert_eq!(c.contextual_alternates, None);
}

#[test]
fn each_feature_lands_on_both_runs_in_one_undo_step() {
    type Check = fn(&CharProps) -> bool;
    let cases: [(fn(&mut Dialog), Check, Shown); 5] = [
        (
            |d| *field!(d, ligatures) = Some("all".into()),
            |c| c.ligatures == Some(Ligatures { standard: true, contextual: true, historical: true, discretional: true }),
            (Some("all".into()), plain().1, plain().2, plain().3, plain().4),
        ),
        (
            |d| *field!(d, num_spacing) = Some("tabular".into()),
            |c| c.num_spacing == Some(NumSpacing::Tabular),
            (plain().0, Some("tabular".into()), plain().2, plain().3, plain().4),
        ),
        (
            |d| *field!(d, num_form) = Some("oldStyle".into()),
            |c| c.num_form == Some(NumForm::OldStyle),
            (plain().0, plain().1, Some("oldStyle".into()), plain().3, plain().4),
        ),
        (
            |d| *field!(d, stylistic_set) = Some(3),
            |c| c.stylistic_sets == Some(vec![StyleSet { id: 3, on: true }]),
            (plain().0, plain().1, plain().2, Some(3), plain().4),
        ),
        (
            |d| *field!(d, contextual_alternates) = Some(false),
            |c| c.contextual_alternates == Some(false),
            (plain().0, plain().1, plain().2, plain().3, Some(false)),
        ),
    ];
    for (set, check, reopened) in cases {
        let mut app = two_runs();
        ok_with(&mut app, set);
        assert!(check(&char_at(&app, 0)) && check(&char_at(&app, 8)), "{reopened:?} lands on both runs");
        assert_eq!(char_at(&app, 8).bold, Some(true), "the runs keep their own formatting");
        assert_eq!(shown(&opened(&mut app)), reopened);
        let _ = app.run("edit.undo", json!({}));
        assert!(!check(&char_at(&app, 0)) && !check(&char_at(&app, 8)), "{reopened:?} is one undo step");
        assert_eq!(shown(&opened(&mut app)), plain());
    }
}

#[test]
fn ligature_entries_store_their_flags() {
    let lig = |s, c, h, d| Some(Ligatures { standard: s, contextual: c, historical: h, discretional: d });
    for (name, want) in [
        ("none", lig(false, false, false, false)),
        ("all", lig(true, true, true, true)),
        ("historicalDiscretional", lig(false, false, true, true)),
        ("standard", lig(true, false, false, false)),
    ] {
        let mut app = selected("Example");
        ok_with(&mut app, |d| *field!(d, ligatures) = Some(name.into()));
        assert_eq!(char_at(&app, 0).ligatures, want, "{name}");
    }
}

#[test]
fn stylistic_set_and_back_to_default() {
    let mut app = selected("Example");
    ok_with(&mut app, |d| *field!(d, stylistic_set) = Some(5));
    assert_eq!(char_at(&app, 0).stylistic_sets, Some(vec![StyleSet { id: 5, on: true }]));
    ok_with(&mut app, |d| *field!(d, stylistic_set) = Some(0));
    assert_eq!(char_at(&app, 0).stylistic_sets, Some(vec![]));
    assert_eq!(shown(&opened(&mut app)).3, Some(0));
}

#[test]
fn mixed_selection_shows_blank_and_ok_keeps_each_runs_value() {
    let mut app = selected("Plain fancy");
    let _ = app.run("select.text", json!({"text": "fancy"}));
    let _ = app.run(
        "format.set",
        json!({"props": {
            "ligatures": {"standard": true, "contextual": true, "historical": true, "discretional": true},
            "numForm": "lining",
            "stylisticSets": [{"id": 2, "on": true}],
            "contextualAlternates": false
        }}),
    );
    let _ = app.run("select.all", json!({}));
    let mut d = opened(&mut app);
    assert_eq!(shown(&d), (None, Some("default".into()), None, None, None));
    // Something else changes: the mixed features stay as each run has them.
    *field!(&mut d, size) = "14".into();
    apply_font(&mut app, &d);
    let (a, b) = (char_at(&app, 0), char_at(&app, 8));
    assert_eq!((a.size, b.size), (Some(14.0), Some(14.0)));
    assert_eq!((a.ligatures, a.num_form, a.stylistic_sets, a.contextual_alternates), (None, None, None, None));
    assert_eq!(b.ligatures, Some(Ligatures { standard: true, contextual: true, historical: true, discretional: true }));
    assert_eq!(b.num_form, Some(NumForm::Lining));
    assert_eq!(b.stylistic_sets, Some(vec![StyleSet { id: 2, on: true }]));
    assert_eq!(b.contextual_alternates, Some(false));
    assert_eq!(a.num_spacing, None, "number spacing was the same and untouched");
}

#[test]
fn a_ligature_name_outside_the_list_is_kept() {
    let mut app = selected("Example");
    let _ = app.run("format.set", json!({"props": {"ligatures": {"standard": true, "historical": true}}}));
    let d = opened(&mut app);
    assert_eq!(shown(&d).0.as_deref(), Some("standardHistorical"));
    apply_font(&mut app, &d);
    assert_eq!(char_at(&app, 0).ligatures, Some(Ligatures { standard: true, contextual: false, historical: true, discretional: false }));
}

#[test]
fn parse_scale_takes_whole_percentages() {
    for (typed, want) in [
        ("150", Some(150.0)),
        ("150%", Some(150.0)),
        (" 150 % ", Some(150.0)),
        ("66.6", Some(67.0)),
        ("1", Some(1.0)),
        ("600", Some(600.0)),
        ("0", None),
        ("0.4", None),
        ("601", None),
        ("-5", None),
        ("", None),
        ("abc", None),
        ("%", None),
        ("1e2", None),
        ("NaN", None),
        ("inf", None),
    ] {
        assert_eq!(parse_scale(typed), want, "{typed:?}");
    }
}
