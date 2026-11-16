//! File › Options › Advanced › Show document content › Field shading: Never, Always or When
//! selected (Word's default). Screen only: printing and PDF export never shade fields.

use super::pages::page_render_options;
use super::*;
use wordcraft_layout::display::{Draw, page_layers};
use wordcraft_layout::fields::FIELD_SHADING;

/// "Page " then a PAGE field then " end", caret at the end of the document.
fn app_with_field() -> WordApp {
    let mut a = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Page ")), crate::Services::default());
    a.run("caret.docEnd", json!({})).unwrap();
    a.run("insert.field", json!({"instr": "PAGE"})).unwrap();
    a.run("text.insert", json!({"text": " end"})).unwrap();
    a.run("caret.docEnd", json!({})).unwrap();
    a
}

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

/// The grey fills drawn on the first page on screen.
fn grey_fills(a: &mut WordApp) -> Vec<wordcraft_geom::Rect> {
    let layout = a.session.layout();
    let opts = page_render_options(a, false);
    let page = layout.pages.first().unwrap();
    let mut out = Vec::new();
    for (_, layer) in page_layers(&a.session.doc, page, &opts.display) {
        flat(&layer, &mut out);
    }
    out.into_iter()
        .filter_map(|d| match d {
            Draw::Fill { rect, color, .. } if color == FIELD_SHADING => Some(rect),
            _ => None,
        })
        .collect()
}

#[test]
fn field_shading_is_when_selected_by_default() {
    let mut a = app_with_field();
    assert_eq!(a.run("ui.fieldShading", json!({})).unwrap(), json!({"fieldShading": "selected"}));
    assert!(grey_fills(&mut a).is_empty(), "caret outside the field: no grey");
}

#[test]
fn always_shades_the_field_wherever_the_caret_is() {
    let mut a = app_with_field();
    assert_eq!(a.run("ui.fieldShading", json!({"value": "always"})).unwrap(), json!({"fieldShading": "always"}));
    let fills = grey_fills(&mut a);
    assert_eq!(fills.len(), 1, "one grey fill, over the PAGE field only: {fills:?}");
    let r = fills[0];
    assert!(r.w > 0.0 && r.h > 0.0, "{r:?}");
    // The field sits after "Page ": its fill starts right of the left margin and is narrower
    // than the whole line "Page 1 end".
    let layout = a.session.layout();
    let body = layout.pages.first().unwrap().body;
    assert!(r.x > body.x + 1.0, "{r:?} vs {body:?}");
    assert!(r.w < body.w / 4.0, "{r:?}");
}

#[test]
fn never_shades_no_field_even_when_selected() {
    let mut a = app_with_field();
    a.run("ui.fieldShading", json!({"value": "never"})).unwrap();
    a.run("select.all", json!({})).unwrap();
    assert!(grey_fills(&mut a).is_empty());
}

#[test]
fn when_selected_shades_only_while_the_selection_covers_the_field() {
    let mut a = app_with_field();
    a.run("ui.fieldShading", json!({"value": "selected"})).unwrap();
    assert!(grey_fills(&mut a).is_empty(), "caret at the end");
    a.run("caret.docStart", json!({})).unwrap();
    assert!(grey_fills(&mut a).is_empty(), "caret at the start, before \"Page \"");
    a.run("select.all", json!({})).unwrap();
    assert_eq!(grey_fills(&mut a).len(), 1, "the selection covers the field");
}

#[test]
fn field_shading_rejects_other_values() {
    let mut a = app_with_field();
    assert!(a.run("ui.fieldShading", json!({"value": "sometimes"})).is_err());
    assert!(a.run("ui.fieldShading", json!({"value": true})).is_err());
    assert_eq!(a.run("ui.fieldShading", json!({})).unwrap(), json!({"fieldShading": "selected"}));
}

#[test]
fn pdf_export_never_shades_fields() {
    let mut a = app_with_field();
    a.run("select.all", json!({})).unwrap();
    let mut pdfs = Vec::new();
    for v in ["never", "always", "selected"] {
        a.run("ui.fieldShading", json!({"value": v})).unwrap();
        pdfs.push(wordcraft_engine::io::save_bytes("out.pdf", &a.session.doc).unwrap());
    }
    assert_eq!(pdfs[0], pdfs[1]);
    assert_eq!(pdfs[1], pdfs[2]);
}
