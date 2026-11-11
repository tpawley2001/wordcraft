//! Ruler margin drags with a right-to-left gutter (`w:rtlGutter`, ECMA-376 §17.6.16): the gutter
//! widens the right margin, so the visible right edge of the text area is `page_w - right -
//! gutter` and the left edge is the left margin itself.

use super::*;
use wordcraft_doc::section::SectionProps;

fn sect(rtl_gutter: bool) -> SectionProps {
    SectionProps {
        page_w: 612.0,
        page_h: 792.0,
        margin_left: 72.0,
        margin_right: 90.0,
        margin_top: 72.0,
        margin_bottom: 72.0,
        gutter: 36.0,
        rtl_gutter,
        ..Default::default()
    }
}

#[test]
fn dragging_the_left_edge_with_a_right_gutter_sets_the_margin_to_the_pointer() {
    assert_eq!(dragged_margin(MarginEdge::Left, 100.0, &sect(true)), 99.0, "snapped to 1/16 inch, no gutter to subtract");
}

#[test]
fn dragging_the_right_edge_with_a_right_gutter_leaves_the_gutter_out_of_the_margin() {
    assert_eq!(dragged_margin(MarginEdge::Right, 450.0, &sect(true)), 126.0);
    assert_eq!(margin_drag(MarginEdge::Right, 450.0, &sect(true), false), (MarginEdge::Right, 126.0));
}

#[test]
fn a_left_gutter_drags_as_before() {
    assert_eq!(dragged_margin(MarginEdge::Right, 450.0, &sect(false)), 162.0);
    assert_eq!(dragged_margin(MarginEdge::Left, 144.0, &sect(false)), 108.0, "the gutter isn't margin");
}

#[test]
fn a_right_gutter_drag_still_leaves_an_inch_of_text() {
    assert_eq!(dragged_margin(MarginEdge::Left, 600.0, &sect(true)), 414.0);
    assert_eq!(dragged_margin(MarginEdge::Right, 0.0, &sect(true)), 432.0);
}

/// Through the app: the page body shows the gutter on the right, and dragging the visible right
/// edge puts it under the pointer without touching the left margin.
#[test]
fn ruler_drag_of_the_right_edge_follows_the_pointer_with_a_right_gutter() {
    let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("one")), Default::default());
    let mut s = wordcraft_engine::cmd::page::sect(&app.session);
    s.gutter = 36.0;
    s.rtl_gutter = true;
    app.run("layout.pageSetup", json!({"section": s})).unwrap();
    let sect = wordcraft_engine::cmd::page::sect(&app.session);
    assert!(sect.rtl_gutter && sect.gutter == 36.0, "the section took the right gutter");
    let body = |app: &mut WordApp| {
        let l = app.session.layout();
        let b = l.pages.first().expect("a page").body;
        (b.x, b.right())
    };
    let (x, right) = body(&mut app);
    assert!((x - sect.margin_left).abs() < 0.01, "no gutter on the left ({x})");
    assert!((right - (sect.page_w - sect.margin_right - 36.0)).abs() < 0.01, "the gutter widens the right margin ({right})");

    let (edge, v) = margin_drag(MarginEdge::Right, 450.0, &sect, false);
    assert_eq!(edge, MarginEdge::Right);
    app.run("layout.margins", json!({edge.param(): v})).unwrap();
    let after = wordcraft_engine::cmd::page::sect(&app.session);
    assert_eq!(after.margin_left, sect.margin_left, "the left margin was not touched");
    let (x, right) = body(&mut app);
    assert!((right - 450.0).abs() <= 2.25, "the visible right edge is at the pointer ({right})");
    assert!((x - sect.margin_left).abs() < 0.01, "the left edge stays ({x})");
}
