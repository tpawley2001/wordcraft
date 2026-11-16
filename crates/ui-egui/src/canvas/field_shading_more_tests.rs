//! Field shading beyond the acceptance tests: a caret inside a range field's result, one field
//! of two selected, field codes, and the setting kept between runs.

use super::pages::page_render_options;
use super::*;
use wordcraft_doc::{CharProps, Document, InlineObject, Pos};
use wordcraft_layout::display::{Draw, page_layers};
use wordcraft_layout::fields::FIELD_SHADING;

fn flat(draws: &[Draw], out: &mut Vec<Draw>) {
    for d in draws {
        match d {
            Draw::Rotated { items, .. } | Draw::Clip { items, .. } | Draw::Turned { items, .. } | Draw::Figure { draws: items, .. } => {
                flat(items, out)
            }
            other => out.push(other.clone()),
        }
    }
}

fn grey_fills(a: &mut WordApp) -> Vec<wordcraft_geom::Rect> {
    let layout = a.session.layout();
    let opts = page_render_options(a, false);
    let mut out = Vec::new();
    for (_, layer) in page_layers(&a.session.doc, layout.pages.first().unwrap(), &opts.display) {
        flat(&layer, &mut out);
    }
    out.into_iter()
        .filter_map(|d| match d {
            Draw::Fill { rect, color, .. } if color == FIELD_SHADING => Some(rect),
            _ => None,
        })
        .collect()
}

fn caret(a: &mut WordApp, off: usize) {
    a.run("select.range", json!({"anchor": Pos::body(0, off), "focus": Pos::body(0, off)})).unwrap();
}

/// "See " then a range field whose result is "ABC" (start marker at 4, result 7..10, end marker
/// at 10), then " here".
fn app_with_range_field() -> WordApp {
    let mut doc = Document::from_text("See  here");
    let props = CharProps::default();
    let at =
        doc.insert_object(&Pos::body(0, 4), InlineObject::FieldStart { instr: "REF _Ref1 \\h".into(), locked: false, dirty: false }, &props).unwrap();
    let at = doc.insert_text(&at, "ABC", &props).unwrap();
    doc.insert_object(&at, InlineObject::FieldEnd, &props).unwrap();
    WordApp::new(wordcraft_engine::Session::new(doc), crate::Services::default())
}

#[test]
fn a_caret_inside_a_field_result_shades_it_when_selected() {
    let mut a = app_with_range_field();
    a.run("ui.fieldShading", json!({"value": "selected"})).unwrap();
    caret(&mut a, 8);
    let fills = grey_fills(&mut a);
    assert_eq!(fills.len(), 1, "the caret is in the field's result: {fills:?}");
    // Before the field (ahead of its start marker), just after its result or its end marker,
    // and at the end of the paragraph: no grey.
    for off in [2, 4, 10, 13, 18] {
        caret(&mut a, off);
        assert!(grey_fills(&mut a).is_empty(), "caret at {off}");
    }
    // Never: not even with the caret inside.
    a.run("ui.fieldShading", json!({"value": "never"})).unwrap();
    caret(&mut a, 8);
    assert!(grey_fills(&mut a).is_empty());
}

/// "A " PAGE " B " PAGE " C": selecting the first field shades only that one.
#[test]
fn only_the_selected_field_is_shaded() {
    let mut a = WordApp::new(wordcraft_engine::Session::new(Document::from_text("A ")), crate::Services::default());
    a.run("caret.docEnd", json!({})).unwrap();
    a.run("insert.field", json!({"instr": "PAGE"})).unwrap();
    a.run("text.insert", json!({"text": " B "})).unwrap();
    a.run("insert.field", json!({"instr": "PAGE"})).unwrap();
    a.run("text.insert", json!({"text": " C"})).unwrap();
    a.run("ui.fieldShading", json!({"value": "always"})).unwrap();
    let both = grey_fills(&mut a);
    assert_eq!(both.len(), 2, "{both:?}");
    a.run("ui.fieldShading", json!({"value": "selected"})).unwrap();
    // "A " is bytes 0..2; the first field's object follows.
    let obj = wordcraft_doc::para::OBJ.len_utf8();
    a.run("select.range", json!({"anchor": Pos::body(0, 1), "focus": Pos::body(0, 2 + obj)})).unwrap();
    let one = grey_fills(&mut a);
    assert_eq!(one.len(), 1, "{one:?}");
    let first = both.iter().map(|r| r.x).fold(f32::INFINITY, f32::min);
    assert!((one[0].x - first).abs() < 0.01, "the first field: {one:?} vs {both:?}");
}

/// Field codes (Alt+F9) are shaded by the same setting on screen: Never takes their grey away.
#[test]
fn field_codes_follow_the_setting_on_screen() {
    let mut a = WordApp::new(wordcraft_engine::Session::new(Document::from_text("Page ")), crate::Services::default());
    a.run("caret.docEnd", json!({})).unwrap();
    a.run("insert.field", json!({"instr": "PAGE"})).unwrap();
    a.run("text.insert", json!({"text": " end"})).unwrap();
    a.run("view.fieldCodes", json!({"value": true})).unwrap();
    a.run("ui.fieldShading", json!({"value": "always"})).unwrap();
    assert!(!grey_fills(&mut a).is_empty(), "the code is on grey");
    a.run("ui.fieldShading", json!({"value": "never"})).unwrap();
    assert!(grey_fills(&mut a).is_empty(), "Never: no grey behind the code");
}

#[test]
fn field_shading_survives_a_restart() {
    let mut a = app_with_range_field();
    a.run("ui.fieldShading", json!({"value": "always"})).unwrap();
    let saved = serde_json::to_string(&a.prefs()).unwrap();
    let mut b = app_with_range_field();
    b.apply_prefs(serde_json::from_str(&saved).unwrap());
    assert_eq!(b.run("ui.fieldShading", json!({})).unwrap(), json!({"fieldShading": "always"}));
    // An older settings file: Word's default, When selected.
    let mut c = app_with_range_field();
    c.ui = serde_json::from_str(r#"{"tab":"Insert"}"#).unwrap();
    assert_eq!(c.run("ui.fieldShading", json!({})).unwrap(), json!({"fieldShading": "selected"}));
}
