//! Where the body starts below a header taller than the top margin, against values measured from
//! PDFs Word exported (ECMA-376 §17.6.11, §17.10). The fixtures in `header_extent_fixtures.rs`
//! rebuild each header's structure synthetically; see that file for the shapes.
use super::header_extent_fixtures::*;

const TOLERANCE: f32 = 2.0;

fn near(what: &str, got: f32, want: f32) {
    assert!((got - want).abs() <= TOLERANCE, "{what}: got {got:.2}pt, Word {want:.2}pt (±{TOLERANCE})");
}

#[test]
fn nested_header_table_header_baseline() {
    let l = lay_fixture(&nested_header_table_doc());
    near("header text baseline", first_header_baseline(&l, 0), 18.7);
}

#[test]
fn nested_header_table_body_baseline() {
    let l = lay_fixture(&nested_header_table_doc());
    near("first body baseline", first_body_baseline(&l, 0), 41.2);
}

#[test]
fn top_and_bottom_text_box_beside_inline_canvas_body_baseline() {
    let l = lay_fixture(&header_text_box_and_canvas_doc());
    near("first body baseline", first_body_baseline(&l, 0), 105.9);
}

#[test]
fn short_header_keeps_the_top_margin() {
    // A one-line 10pt header at 0.5in in a 1in top margin does not move the body.
    let l = lay_fixture(&short_header_doc());
    let top = first_body_line_top(&l, 0);
    assert!((top - 72.0).abs() < 0.5, "body line top {top}");
}
