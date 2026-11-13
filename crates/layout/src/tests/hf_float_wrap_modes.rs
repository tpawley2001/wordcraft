//! A Top and Bottom object anchored in a header or footer paragraph, by compatibility mode. In
//! mode 15 the header's or footer's own text wraps it as body text does (a line the object's
//! band would cover goes below it); in modes 11, 12 and 14 the text stays where it is
//! (`hf_top_bottom_wrap.rs`). Expected values measured from PDFs that Microsoft 365 Word exported
//! for synthetic probes, pictures and DrawingML shapes alike (Letter, top margin 50.4pt, other
//! margins 72pt, header and footer 36pt, Arial 12pt single-spaced = 13.8pt lines;
//! wp:wrapTopAndBottom / wp:wrapSquare, ECMA-376 §20.4.2.20 / §20.4.2.17). Line tops in pt from
//! the page top; Word's 13.8pt line pitch is replaced by the test font's own (`pitch()`).
use super::rect_shape;
use crate::*;
use wordcraft_doc::para::{Anchor, Float, Wrap};
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{PartKind, para_block};

const TOL: f32 = 1.5;
/// Top margin: where the body starts under a header that ends above it.
const MARGIN_TOP: f32 = 50.4;
/// Header distance: the top of the header's first line when nothing moves it.
const HDR: f32 = 36.0;
/// Height of the big and the Square boxes.
const BOX_H: f32 = 49.8;

fn near(what: &str, got: f32, want: f32) {
    assert!((got - want).abs() <= TOL, "{what}: got {got:.2}pt, Word {want:.2}pt (±{TOL})");
}

fn arial() -> CharProps {
    CharProps { font: Some("Arial".into()), size: Some(12.0), ..Default::default() }
}

fn tight() -> ParaProps {
    ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() }
}

fn para(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, arial());
    p.props = tight();
    p
}

/// A floating box anchored column + `x`, paragraph + `y`, no distance from the text.
fn float_box(wrap: Wrap, w: f32, h: f32, x: f32, y: f32) -> InlineObject {
    let f = Float { wrap, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x, y, dist: 0.0, dist_top: 0.0, dist_bottom: 0.0, ..Default::default() };
    rect_shape(w, h, f)
}

/// `text` (may be empty) with `object` anchored at its start.
fn with_object(text: &str, object: InlineObject) -> Paragraph {
    let mut p = para(text);
    p.insert_object(0, object, &Default::default()).unwrap();
    p
}

/// The 400 x 49.8pt Top and Bottom box at column + 0, paragraph + `y`.
fn big(y: f32) -> InlineObject {
    float_box(Wrap::TopAndBottom, 400.0, BOX_H, 0.0, y)
}

/// The 144 x 30pt Top and Bottom box at column + 144, paragraph + `y`.
fn small(y: f32) -> InlineObject {
    float_box(Wrap::TopAndBottom, 144.0, 30.0, 144.0, y)
}

/// A Square box `w` wide and 49.8pt high at column + 0, paragraph + 0 (468pt is the text width).
fn square(w: f32) -> InlineObject {
    float_box(Wrap::Square, w, BOX_H, 0.0, 0.0)
}

fn doc_with(mode: u32, header: Option<Vec<Paragraph>>, footer: Option<Vec<Paragraph>>, body: Vec<Paragraph>) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (MARGIN_TOP, 72.0, 72.0, 72.0);
    (s.header, s.footer) = (36.0, 36.0);
    if let Some(h) = header {
        let id = d.add_part(PartKind::Header, h.into_iter().map(para_block).collect());
        d.last_section.headers.default = Some(id);
    }
    if let Some(f) = footer {
        let id = d.add_part(PartKind::Footer, f.into_iter().map(para_block).collect());
        d.last_section.footers.default = Some(id);
    }
    d.body = body.into_iter().map(para_block).collect();
    d
}

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// Tops of the lines with text among `items` (every fixture paragraph is one line), top to
/// bottom; an empty anchor paragraph's line is left out.
fn line_tops(items: &[Placed]) -> Vec<f32> {
    let mut v: Vec<f32> = items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, y, .. } if para.text_len > 1 => Some(*y),
            _ => None,
        })
        .collect();
    v.sort_by(f32::total_cmp);
    v
}

/// The line pitch of a plain single-spaced line of the fixture font (13.8pt in Word).
fn pitch() -> f32 {
    let t = line_tops(&lay(&doc_with(15, None, None, vec![para("AA"), para("BB")])).pages[0].items);
    t[1] - t[0]
}

/// Header and body line tops of page 1 for a header of `header` paragraphs over BODY1, BODY2.
fn tops(mode: u32, header: Vec<Paragraph>) -> (Vec<f32>, Vec<f32>) {
    let d = doc_with(mode, Some(header), None, vec![para("BODY1"), para("BODY2")]);
    let l = lay(&d);
    let p = &l.pages[0];
    (line_tops(&p.header), line_tops(&p.items))
}

/// Row 1. Word, mode 15: HDR 85.8 (below the box), BODY1 99.6.
#[test]
fn mode_15_header_text_goes_below_a_top_and_bottom_box() {
    let (h, b) = tops(15, vec![with_object("HDR", big(0.0))]);
    near("HDR line top", h[0], HDR + BOX_H);
    near("BODY1 line top", b[0], HDR + BOX_H + pitch());
}

/// Row 2. Word, mode 15, the box 7.2pt below the paragraph's top: HDR 93.0, BODY1 106.8.
#[test]
fn mode_15_header_text_goes_below_a_lowered_box() {
    let (h, b) = tops(15, vec![with_object("HDR", big(7.2))]);
    near("HDR line top", h[0], HDR + 7.2 + BOX_H);
    near("BODY1 line top", b[0], HDR + 7.2 + BOX_H + pitch());
}

/// Rows 3 and 4. Word, mode 15, the small box beside the text's start: HDR still goes below it
/// (73.2 with the box at +7.2, 66.0 at +0), BODY1 one line further down.
#[test]
fn mode_15_header_text_goes_below_a_small_box_wherever_it_stands() {
    for y in [7.2, 0.0] {
        let (h, b) = tops(15, vec![with_object("HDR", small(y))]);
        near(&format!("box +{y}: HDR line top"), h[0], HDR + y + 30.0);
        near(&format!("box +{y}: BODY1 line top"), b[0], HDR + y + 30.0 + pitch());
    }
}

/// Row 5. Word, mode 15: HDR 85.8, HDR2 99.6, BODY1 113.4.
#[test]
fn mode_15_the_next_header_paragraph_follows_the_moved_line() {
    let (h, b) = tops(15, vec![with_object("HDR", big(0.0)), para("HDR2")]);
    assert_eq!(h.len(), 2, "two header lines: {h:?}");
    near("HDR line top", h[0], HDR + BOX_H);
    near("HDR2 line top", h[1], HDR + BOX_H + pitch());
    near("BODY1 line top", b[0], HDR + BOX_H + 2.0 * pitch());
}

/// Row 6. Word, mode 15, footer FFIRST / FANCHOR with the small box at +7.2 / FNEXT: plain
/// top-down flow ending at 792 - 36: FNEXT 742.2, FANCHOR 728.4 right below the box
/// (698.4..728.4), FFIRST 677.4; the body's last line ends at or above FFIRST.
#[test]
fn mode_15_footer_text_goes_below_a_top_and_bottom_box_and_the_footer_grows() {
    let footer = vec![para("FFIRST"), with_object("FANCHOR", small(7.2)), para("FNEXT")];
    let body: Vec<Paragraph> = (1..=60).map(|i| para(&format!("L{i:02}"))).collect();
    let d = doc_with(15, None, Some(footer), body);
    let l = lay(&d);
    let p = &l.pages[0];
    let f = line_tops(&p.footer);
    let h = pitch();
    assert_eq!(f.len(), 3, "three footer lines: {f:?}");
    near("FNEXT line top", f[2], 756.0 - h);
    near("FANCHOR line top", f[1], 756.0 - 2.0 * h);
    near("FFIRST line top", f[0], 756.0 - 3.0 * h - 37.2);
    let last = *line_tops(&p.items).last().expect("body lines on page 1");
    assert!(last + h <= f[0] + TOL, "the body's last line (top {last:.2}) ends above the footer's first line ({:.2})", f[0]);
}

/// Row 7 (right today). Word, mode 15, the box 20pt down: the line fits above it and stays at
/// 36.0; BODY1 at the box's bottom, 105.8.
#[test]
fn mode_15_a_header_line_that_fits_above_the_box_stays() {
    let (h, b) = tops(15, vec![with_object("HDR", big(20.0))]);
    near("HDR line top", h[0], HDR);
    near("BODY1 line top", b[0], HDR + 20.0 + BOX_H);
}

/// Row 8 (right today). Word, mode 15, an empty anchor paragraph: its line is below the box, so
/// BODY1 is at 99.6.
#[test]
fn mode_15_an_empty_anchor_line_goes_below_the_box() {
    let (_, b) = tops(15, vec![with_object("", big(0.0))]);
    near("BODY1 line top", b[0], HDR + BOX_H + pitch());
}

/// Rows 9 and 10 (right today). Word, modes 11, 12 and 14: the header's text stays at 36.0 and
/// the body starts at the box's bottom (85.8; 73.2 for the small box at +7.2).
#[test]
fn legacy_modes_keep_header_text_in_place_over_a_top_and_bottom_box() {
    for mode in [11, 12, 14] {
        let (h, b) = tops(mode, vec![with_object("HDR", big(0.0))]);
        near(&format!("mode {mode}: HDR line top"), h[0], HDR);
        near(&format!("mode {mode}: BODY1 line top"), b[0], HDR + BOX_H);
        let (h, b) = tops(mode, vec![with_object("HDR", small(7.2))]);
        near(&format!("mode {mode}, small box: HDR line top"), h[0], HDR);
        near(&format!("mode {mode}, small box: BODY1 line top"), b[0], HDR + 7.2 + 30.0);
    }
}

/// Row 11 (right today). Word, mode 15: header text wraps a Square box as body text does, so it
/// goes below one as wide as the text: HDR 85.8, BODY1 99.6.
#[test]
fn mode_15_header_text_goes_below_a_full_width_square_box() {
    let (h, b) = tops(15, vec![with_object("HDR", square(468.0))]);
    near("HDR line top", h[0], HDR + BOX_H);
    near("BODY1 line top", b[0], HDR + BOX_H + pitch());
}

/// A letterhead: the small Top and Bottom box (paragraph + 7.2pt) and, after it, an inline
/// 40.05 x 38.95pt picture, no text.
fn logo_beside_box() -> Paragraph {
    let mut p = with_object("", small(7.2));
    p.insert_object(p.len(), rect_shape(40.05, 38.95, Float::default()), &Default::default()).unwrap();
    p
}

/// Word, mode 15: the line holding only the inline picture goes below the box (the probe's body
/// started 36.7pt lower than in mode 14); HNEXT follows the picture's line.
#[test]
fn mode_15_header_line_with_only_an_inline_picture_goes_below_a_top_and_bottom_box() {
    let (h, b) = tops(15, vec![logo_beside_box(), para("HNEXT")]);
    assert_eq!(h.len(), 2, "the picture's line and HNEXT: {h:?}");
    near("picture line top", h[0], HDR + 7.2 + 30.0);
    assert!(h[1] > h[0] + 38.95 - TOL && h[1] < h[0] + 38.95 + pitch(), "HNEXT right below the picture's line: {h:?}");
    near("BODY1 line top", b[0], h[1] + pitch());
}

/// Word, mode 14, the same paragraph in a footer: the picture's line stays beside the box, which
/// sits 7.2pt below the paragraph's top, and FNEXT ends at the footer distance.
#[test]
fn mode_14_footer_line_with_only_an_inline_picture_stays_beside_a_top_and_bottom_box() {
    let d = doc_with(14, None, Some(vec![logo_beside_box(), para("FNEXT")]), vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    let f = line_tops(&p.footer);
    assert_eq!(f.len(), 2, "the picture's line and FNEXT: {f:?}");
    near("FNEXT line top", f[1], 792.0 - 36.0 - pitch());
    assert!(f[1] > f[0] + 38.95 - TOL && f[1] < f[0] + 38.95 + pitch(), "FNEXT right below the picture's line: {f:?}");
    let boxes: Vec<Rect> = super::shapes(&p.footer).into_iter().filter(|r| (r.w - 144.0).abs() < 0.5).collect();
    assert_eq!(boxes.len(), 1, "the box is drawn in the footer: {boxes:?}");
    near("box top", boxes[0].y, f[0] + 7.2);
}
