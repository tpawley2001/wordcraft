//! Synthetic fixtures for `header_extent.rs`. Structures are described in that task's spec
//! (case A: nested header table on a landscape letter page; case B: a first-page header whose
//! paragraph holds a top-and-bottom text box anchored 7.2pt below it and an inline 40 x 39pt
//! drawing canvas). Built in code; never from files Word produced.
use crate::*;

pub(super) fn lay_fixture(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// Case A of the spec.
pub(super) fn nested_header_table_doc() -> Document {
    todo!("build case A")
}

/// Case B of the spec.
pub(super) fn header_text_box_and_canvas_doc() -> Document {
    todo!("build case B")
}

/// Letter page, 1in margins, header distance 0.5in, header = one 10pt paragraph, body = one
/// 10pt paragraph with no spacing before.
pub(super) fn short_header_doc() -> Document {
    todo!("build the control case")
}

/// Baseline (pt from the page top) of the first line of the header on page `page`.
pub(super) fn first_header_baseline(_l: &DocLayout, _page: usize) -> f32 {
    todo!()
}

/// Baseline (pt from the page top) of the first body line on page `page` (inside a table cell
/// when the body starts with a table).
pub(super) fn first_body_baseline(_l: &DocLayout, _page: usize) -> f32 {
    todo!()
}

/// Top of the first body line box on page `page`.
pub(super) fn first_body_line_top(_l: &DocLayout, _page: usize) -> f32 {
    todo!()
}
