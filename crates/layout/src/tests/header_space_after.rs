//! The space after a header's last paragraph counts in the header's height, so the body starts
//! below it (ECMA-376 §17.3.1.33 `w:spacing/@w:after`, §17.6.11 `w:pgMar/@w:header`). Values
//! measured from PDFs Word exported of small probe documents: with 13.8pt header lines at 36pt
//! from the edge and a 72pt top margin, the body starts at 72 for "HDR", at 79.8 for "HDR" with
//! 30pt after and at 93.6 for "HDR" followed by an empty paragraph with 30pt after. The
//! documents are built in code and the expected tops are computed from the header's laid-out
//! line heights, so the tests hold with any fallback font.
use super::header_extent_fixtures::{first_body_line_top, lay_fixture};
use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{PartKind, para_block};

const HEADER_DISTANCE: f32 = 36.0;
const TOP_MARGIN: f32 = 72.0;

/// A header paragraph: single spacing, `before` and `after` points of space.
fn header_para(text: &str, before: f32, after: f32) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, CharProps { size: Some(12.0), ..Default::default() });
    p.props =
        ParaProps { space_before: Some(before), space_after: Some(after), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
    para_block(p)
}

/// Letter page, 1in margins, header 0.5in from the edge, the given header paragraphs, and a
/// body of one 12pt paragraph with no space before.
fn doc_with_header(header: Vec<Arc<Block>>) -> Document {
    let mut d = Document::new();
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (TOP_MARGIN, 72.0, 72.0, 72.0);
    s.header = HEADER_DISTANCE;
    let id = d.add_part(PartKind::Header, header);
    d.last_section.headers.default = Some(id);
    let mut body = Paragraph::with_text("BODY", CharProps { size: Some(12.0), ..Default::default() });
    body.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), ..Default::default() };
    d.body = vec![para_block(body)];
    d
}

/// (top of the header's first line, sum of the heights of all the header's lines) on page 0.
fn header_lines(l: &DocLayout) -> (f32, f32) {
    let mut top = f32::MAX;
    let mut height = 0.0;
    for it in &l.pages[0].header {
        if let Placed::Lines { para, l0, l1, y, .. } = it {
            top = top.min(*y);
            height += para.lines[*l0..*l1].iter().map(|line| line.height).sum::<f32>();
        }
    }
    (top, height)
}

fn near(what: &str, got: f32, want: f32) {
    assert!((got - want).abs() <= 0.25, "{what}: got {got:.2}pt, want {want:.2}pt");
}

/// Lays `header` out and checks the body's first line top against
/// max(top margin, header distance + header line heights + `spacing`).
fn check(what: &str, header: Vec<Arc<Block>>, lines: usize, spacing: f32) {
    let l = lay_fixture(&doc_with_header(header));
    let (top, height) = header_lines(&l);
    assert!(height > 10.0 * lines as f32, "{what}: the header's {lines} line(s) are laid out ({height}pt)");
    let want = TOP_MARGIN.max(HEADER_DISTANCE + height + spacing);
    near(what, first_body_line_top(&l, 0), want);
    // The header itself does not move: its first line starts at the header distance, below its
    // own space before.
    let before = if what.contains("space before") { 30.0 } else { 0.0 };
    near(&format!("{what}: header top"), top, HEADER_DISTANCE + before);
}

#[test]
fn a_short_header_without_spacing_keeps_the_top_margin() {
    // Word: 72.0.
    check("HDR", vec![header_para("HDR", 0.0, 0.0)], 1, 0.0);
}

#[test]
fn space_after_the_headers_only_paragraph_pushes_the_body_down() {
    // Word: 79.8 = 36 + 13.8 + 30.
    check("HDR with 30pt after", vec![header_para("HDR", 0.0, 30.0)], 1, 30.0);
}

#[test]
fn space_after_an_empty_last_header_paragraph_pushes_the_body_down() {
    // Word: 93.6 = 36 + 13.8 + 13.8 + 30.
    check("HDR, then an empty paragraph with 30pt after", vec![header_para("HDR", 0.0, 0.0), header_para("", 0.0, 30.0)], 2, 30.0);
}

#[test]
fn only_the_last_paragraphs_space_after_is_added_once() {
    // 20pt between the two paragraphs (already counted today) and 30pt after the last one.
    check("HDR with 20pt after, H2 with 30pt after", vec![header_para("HDR", 0.0, 20.0), header_para("H2", 0.0, 30.0)], 2, 50.0);
}

#[test]
fn a_small_space_after_that_stays_inside_the_top_margin_changes_nothing() {
    // 36 + one line + 6 is still above the 72pt top margin.
    let l = lay_fixture(&doc_with_header(vec![header_para("HDR", 0.0, 6.0)]));
    near("HDR with 6pt after", first_body_line_top(&l, 0), TOP_MARGIN);
}

#[test]
fn space_before_the_headers_first_paragraph_still_counts() {
    // Control (passes before the change). Word: 79.8.
    check("HDR with space before 30pt", vec![header_para("HDR", 30.0, 0.0)], 1, 30.0);
}

#[test]
fn three_header_lines_push_the_body_below_them() {
    // Control (passes before the change). Word: 77.4 = 36 + 3 x 13.8.
    check("HDR, H2, H3", vec![header_para("HDR", 0.0, 0.0), header_para("H2", 0.0, 0.0), header_para("H3", 0.0, 0.0)], 3, 0.0);
}
