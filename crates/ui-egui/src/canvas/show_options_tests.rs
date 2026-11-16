//! File › Options › Display › Show highlighter marks, and Advanced › Show document content ›
//! Show background colors and images in Print Layout view / Show drawings and text boxes on
//! screen. All on by default; off changes only what the screen draws, never the document or a PDF.

use super::pages::page_render_options;
use super::*;
use wordcraft_doc::Rgb;
use wordcraft_doc::props::Highlight;
use wordcraft_layout::display::{Draw, page_layers};

fn app(text: &str) -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text(text)), crate::Services::default())
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

/// The colours of every fill drawn on the first page on screen.
fn fill_colors(a: &mut WordApp) -> Vec<Rgb> {
    let layout = a.session.layout();
    let opts = page_render_options(a, false);
    let page = layout.pages.first().unwrap();
    let mut out = Vec::new();
    for (_, layer) in page_layers(&a.session.doc, page, &opts.display) {
        flat(&layer, &mut out);
    }
    out.into_iter()
        .filter_map(|d| match d {
            Draw::Fill { color, .. } => Some(color),
            _ => None,
        })
        .collect()
}

#[test]
fn the_three_options_are_on_by_default() {
    let mut a = app("Text");
    assert_eq!(a.run("ui.showHighlight", json!({})).unwrap(), json!({"showHighlight": true}));
    assert_eq!(a.run("ui.showDrawings", json!({})).unwrap(), json!({"showDrawings": true}));
    assert_eq!(a.run("ui.showBackgrounds", json!({})).unwrap(), json!({"showBackgrounds": true}));
    assert!(a.run("ui.showHighlight", json!({"value": "no"})).is_err());
    assert!(a.run("ui.showDrawings", json!({"value": 0})).is_err());
    assert!(a.run("ui.showBackgrounds", json!({"value": "off"})).is_err());
}

#[test]
fn show_highlighter_marks_off_hides_highlight_but_keeps_the_document() {
    let mut a = app("Marked and shaded text");
    a.run("select.text", json!({"text": "Marked"})).unwrap();
    a.run("format.highlight", json!({"color": "yellow"})).unwrap();
    let yellow = Highlight::Yellow.rgb().unwrap();
    assert!(fill_colors(&mut a).contains(&yellow), "on: the yellow highlight is drawn");
    let doc = a.session.doc.clone();
    assert_eq!(a.run("ui.showHighlight", json!({"value": false})).unwrap(), json!({"showHighlight": false}));
    assert!(!fill_colors(&mut a).contains(&yellow), "off: no highlight on screen");
    assert!(page_render_options(&a, false).display.hide_highlight);
    assert_eq!(a.session.doc, doc, "the document keeps its highlight");
    a.run("ui.showHighlight", json!({"value": true})).unwrap();
    assert!(fill_colors(&mut a).contains(&yellow), "back on");
}

#[test]
fn show_drawings_off_hides_shapes_on_screen() {
    let mut a = app("Text");
    a.run("insert.shape", json!({"kind": "rectangle"})).unwrap();
    assert!(!page_render_options(&a, false).display.hide_drawings, "on by default");
    assert_eq!(a.run("ui.showDrawings", json!({"value": false})).unwrap(), json!({"showDrawings": false}));
    assert!(page_render_options(&a, false).display.hide_drawings);
    a.run("ui.showDrawings", json!({"value": true})).unwrap();
    assert!(!page_render_options(&a, false).display.hide_drawings);
}

#[test]
fn show_backgrounds_off_draws_print_layout_pages_without_the_page_color() {
    let mut a = app("Text");
    a.run("design.pageColor", json!({"color": "#102040"})).unwrap();
    let paper = |a: &WordApp| page_render_options(a, false).paper_of(&a.session.doc);
    assert_eq!(paper(&a), Rgb(0x10, 0x20, 0x40), "on: the page color shows");
    assert_eq!(a.run("ui.showBackgrounds", json!({"value": false})).unwrap(), json!({"showBackgrounds": false}));
    assert_eq!(paper(&a), Rgb::WHITE, "off: white paper in Print Layout");
    assert_eq!(a.session.doc.settings.page_color, Some(Rgb(0x10, 0x20, 0x40)), "the document keeps it");
    a.run("ui.showBackgrounds", json!({"value": true})).unwrap();
    assert_eq!(paper(&a), Rgb(0x10, 0x20, 0x40));
}

#[test]
fn pdf_export_ignores_the_three_options() {
    let mut a = app("Marked text");
    a.run("select.text", json!({"text": "Marked"})).unwrap();
    a.run("format.highlight", json!({"color": "yellow"})).unwrap();
    a.run("design.pageColor", json!({"color": "#102040"})).unwrap();
    let on = wordcraft_engine::io::save_bytes("out.pdf", &a.session.doc).unwrap();
    for id in ["ui.showHighlight", "ui.showDrawings", "ui.showBackgrounds"] {
        a.run(id, json!({"value": false})).unwrap();
    }
    let off = wordcraft_engine::io::save_bytes("out.pdf", &a.session.doc).unwrap();
    assert_eq!(on, off);
}
