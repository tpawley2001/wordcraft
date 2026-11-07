//! File › Options › Advanced › Show document content on the canvas: text boundaries, picture
//! placeholders and the draft font. All screen only: Print Layout's layout, printing and PDF
//! export don't see them.

use super::pages::{page_render_options, text_boundaries};
use super::*;
use wordcraft_layout::display::{Draw, page_layers};

fn app(text: &str) -> WordApp {
    WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text(text)), crate::Services::default())
}

fn with_picture() -> WordApp {
    let mut a = app("Before the picture ");
    let mut png = Vec::new();
    let img = image::RgbaImage::from_pixel(40, 20, image::Rgba([200, 30, 30, 255]));
    image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    a.run("insert.picture", json!({"data": wordcraft_engine::cmd::insert::base64_encode(&png), "width": 160})).unwrap();
    a
}

/// Every draw, inside rotated, clipped and turned groups too.
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

fn screen_draws(a: &mut WordApp) -> Vec<Draw> {
    let layout = a.session.layout();
    let opts = page_render_options(a, false);
    let page = layout.pages.first().unwrap();
    let mut out = Vec::new();
    for (_, layer) in page_layers(&a.session.doc, page, &opts.display) {
        flat(&layer, &mut out);
    }
    out
}

#[test]
fn text_boundaries_outline_the_text_area_or_each_column() {
    let mut a = app("Columns of text");
    let layout = a.session.layout();
    let page = layout.pages.first().unwrap();
    let sections = a.session.doc.sections();
    let cols = sections.first().map(|(_, s)| s.column_boxes()).unwrap();
    assert_eq!(text_boundaries(page, &cols), vec![page.body], "one column: the text area");
    a.run("select.all", json!({})).unwrap();
    a.run("layout.columns", json!({"count": 2, "space": 36, "apply": "document"})).unwrap();
    let layout = a.session.layout();
    let page = layout.pages.first().unwrap();
    let sections = a.session.doc.sections();
    let cols = sections.get(page.section).map(|(_, s)| s.column_boxes()).unwrap();
    let rects = text_boundaries(page, &cols);
    assert_eq!(rects.len(), 2, "{rects:?}");
    let body = page.body;
    let half = (body.w - 36.0) / 2.0;
    for (r, x) in rects.iter().zip([body.x, body.x + half + 36.0]) {
        assert!((r.x - x).abs() < 0.01 && (r.w - half).abs() < 0.01, "{r:?}");
        assert_eq!((r.y, r.h), (body.y, body.h));
    }
}

#[test]
fn picture_placeholders_draw_an_outline_instead_of_the_picture() {
    let mut a = with_picture();
    let shown = screen_draws(&mut a);
    let rect = shown.iter().find_map(|d| if let Draw::Image { rect, .. } = d { Some(*rect) } else { None }).expect("the picture is drawn");
    let lines_before = shown.iter().filter(|d| matches!(d, Draw::Line { .. })).count();
    let layout = a.session.layout();
    a.run("ui.picturePlaceholders", json!({"value": true})).unwrap();
    assert!(std::sync::Arc::ptr_eq(&layout, &a.session.layout()), "the layout doesn't change");
    let placeholders = screen_draws(&mut a);
    assert!(!placeholders.iter().any(|d| matches!(d, Draw::Image { .. })), "no picture");
    let outline: Vec<_> =
        placeholders.iter().filter_map(|d| if let Draw::Line { x0, y0, x1, y1, .. } = d { Some((*x0, *y0, *x1, *y1)) } else { None }).collect();
    assert_eq!(outline.len(), lines_before + 4, "four sides");
    for (x, y) in [(rect.x, rect.y), (rect.right(), rect.y), (rect.right(), rect.bottom()), (rect.x, rect.bottom())] {
        assert!(outline.iter().any(|&(x0, y0, _, _)| x0 == x && y0 == y), "a side starts at ({x}, {y})");
    }
}

fn first_run_font(l: &DocLayout) -> (String, f32) {
    let pl = l
        .pages
        .iter()
        .flat_map(|p| p.items.iter())
        .find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None })
        .expect("a paragraph");
    let s = pl.styles.first().expect("a run");
    (s.rc.font.clone(), s.rc.size)
}

#[test]
fn the_draft_font_shows_in_draft_and_outline_views_only() {
    let mut a = app("Draft text");
    a.run("select.all", json!({})).unwrap();
    a.run("format.bold", json!({})).unwrap();
    let (doc, undo) = (a.session.doc.clone(), a.session.can_undo());
    a.run("ui.draftFont", json!({"value": true, "name": "Consolas", "size": 12})).unwrap();
    let print = first_run_font(&a.session.layout());
    assert_ne!(print.0, "Consolas", "Print Layout keeps the document's font");
    a.run("view.draft", json!({})).unwrap();
    let l = a.session.layout();
    assert_eq!(first_run_font(&l), ("Consolas".to_string(), 12.0));
    let pl = l.pages.iter().flat_map(|p| p.items.iter()).find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None });
    assert!(pl.unwrap().styles.first().unwrap().rc.bold, "bold is kept");
    a.run("view.outline", json!({})).unwrap();
    assert_eq!(first_run_font(&a.session.layout()).0, "Consolas");
    // Off again: the document's own font, in Draft view too.
    a.run("view.draft", json!({})).unwrap();
    a.run("ui.draftFont", json!({"value": false})).unwrap();
    assert_eq!(first_run_font(&a.session.layout()), print);
    // The document and its undo history never changed.
    assert_eq!((&a.session.doc, a.session.can_undo()), (&doc, undo));
}

/// Printing and export see none of the four: the exported layout keeps the document's fonts and
/// the PDF is the same with every option on.
#[test]
fn pdf_export_ignores_the_show_content_options() {
    let mut a = with_picture();
    a.run("view.draft", json!({})).unwrap();
    let off = wordcraft_engine::io::save_bytes("out.pdf", &a.session.doc).unwrap();
    let off_font = first_run_font(&a.session.export_layout());
    a.run("ui.cropMarks", json!({"value": true})).unwrap();
    a.run("ui.textBoundaries", json!({"value": true})).unwrap();
    a.run("ui.picturePlaceholders", json!({"value": true})).unwrap();
    a.run("ui.draftFont", json!({"value": true, "name": "Consolas", "size": 12})).unwrap();
    assert_eq!(first_run_font(&a.session.export_layout()), off_font);
    assert_ne!(off_font.0, "Consolas");
    let on = wordcraft_engine::io::save_bytes("out.pdf", &a.session.doc).unwrap();
    assert_eq!(off, on);
}
