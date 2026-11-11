//! Header & Footer › Show Document Text off ([`DisplayOptions::hide_body`]): the body's text is
//! left out while the header, the footer and the page borders still draw.
use crate::display::{DisplayOptions, Draw, page_display, page_layers};
use crate::*;
use wordcraft_doc::props::{Border, Borders};
use wordcraft_doc::{PartKind, para_block};

fn doc() -> Document {
    let mut d = Document::from_text("Body text here");
    let header = d.add_part(PartKind::Header, vec![para_block(Paragraph::with_text("Header", Default::default()))]);
    d.last_section.headers.default = Some(header);
    let b = Some(Border::single(1.0));
    d.last_section.page_borders = Some(Borders { top: b, bottom: b, left: b, right: b, ..Default::default() });
    d
}

/// Baselines of the glyph runs drawn, and how many lines.
fn census(draws: &[Draw]) -> (Vec<f32>, usize) {
    let mut ys = Vec::new();
    let mut lines = 0;
    for d in draws {
        match d {
            Draw::Glyphs { glyphs, .. } => ys.extend(glyphs.first().map(|g| g.2)),
            Draw::Line { .. } => lines += 1,
            _ => {}
        }
    }
    (ys, lines)
}

#[test]
fn hide_body_keeps_headers_and_page_borders_only() {
    let d = doc();
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = &l.pages[0];
    let in_body = |y: &f32| *y > page.body.y && *y < page.body.bottom();
    let (shown, borders) = census(&page_display(&d, page, &DisplayOptions { dim_header: false, ..Default::default() }));
    assert!(shown.iter().any(in_body), "the body is drawn normally: {shown:?}");
    assert!(shown.iter().any(|y| !in_body(y)), "the header is drawn: {shown:?}");
    assert!(borders >= 4, "the page borders are drawn ({borders})");

    let opts = DisplayOptions { dim_header: false, dim_body: true, hide_body: true, ..Default::default() };
    let (hidden, kept) = census(&page_display(&d, page, &opts));
    assert!(!hidden.iter().any(in_body), "no body text: {hidden:?}");
    assert!(hidden.iter().any(|y| !in_body(y)), "the header still draws: {hidden:?}");
    assert_eq!(kept, borders, "the page borders still draw");
    let [(_, hf), (_, body)] = page_layers(&d, page, &opts);
    assert!(!census(&hf).0.is_empty());
    assert!(census(&body).0.is_empty());
}
