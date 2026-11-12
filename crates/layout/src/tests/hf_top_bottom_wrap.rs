//! A Top and Bottom object anchored in a header or footer doesn't move that header's or footer's
//! own lines (they lay out as if it weren't there), but the body still starts below it. In the
//! body, the anchor's line still goes below the object. Expected values measured from PDFs that
//! Microsoft 365 Word exported for synthetic probes (Letter, 1in margins, header/footer 0.5in,
//! Arial 12pt single-spaced = 13.8pt lines, a 144 x 30pt box at column +144pt, paragraph +7.2pt,
//! wp:wrapTopAndBottom, ECMA-376 §20.4.2.20). Line tops in pt from the page top; Word's
//! 13.8pt line pitch is replaced by the test font's own (`pitch()`).
use super::{rect_shape, shapes};
use crate::*;
use wordcraft_doc::para::{Anchor, Float, Wrap};
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{PartKind, para_block};

const TOL: f32 = 1.5;

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

/// The 144 x 30pt Top and Bottom box, `rel` + `y` vertically, column + 144pt across.
fn tb_box(v_rel: Anchor, y: f32) -> InlineObject {
    let f = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel, x: 144.0, y, ..Default::default() };
    rect_shape(144.0, 30.0, f)
}

/// `text` with the box anchored at its start.
fn anchor_para(text: &str, v_rel: Anchor, y: f32) -> Paragraph {
    let mut p = para(text);
    p.insert_object(0, tb_box(v_rel, y), &Default::default()).unwrap();
    p
}

fn letter(d: &mut Document) {
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 72.0, 72.0, 72.0);
    (s.header, s.footer) = (36.0, 36.0);
}

fn doc_with(header: Option<Vec<Paragraph>>, footer: Option<Vec<Paragraph>>, body: Vec<Paragraph>) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    // The probes were DOCX files with no compatibilityMode setting, which Word opens in mode 12;
    // from mode 15 a header's own text wraps the box as body text does (`hf_float_wrap_modes`).
    d.settings.compat_mode = wordcraft_doc::LEGACY_COMPAT_MODE;
    letter(&mut d);
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

/// Tops of the text lines among `items` (every fixture paragraph is one line), top to bottom.
fn line_tops(items: &[Placed]) -> Vec<f32> {
    let mut v: Vec<f32> = items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, y, .. } if para.text_len > 0 => Some(*y),
            _ => None,
        })
        .collect();
    v.sort_by(f32::total_cmp);
    v
}

/// The line pitch of a plain single-spaced line of the fixture font (13.8pt for Arial 12pt in
/// Word; the test fonts may differ, so expected tops are built from it).
fn pitch() -> f32 {
    let t = line_tops(&lay(&doc_with(None, None, vec![para("A"), para("B")])).pages[0].items);
    t[1] - t[0]
}

/// Probe g: header "HANCHOR line" (box +7.2pt) then "HNEXT". Word: 36.0, 49.8; body at 72.0.
#[test]
fn header_anchor_line_stays_at_the_header_distance() {
    let d = doc_with(Some(vec![anchor_para("HANCHOR line", Anchor::Paragraph, 7.2), para("HNEXT")]), None, vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    assert_eq!(shapes(&p.header).len(), 1, "the box is still drawn in the header");
    let tops = line_tops(&p.header);
    assert_eq!(tops.len(), 2, "two header lines: {tops:?}");
    near("HANCHOR line top", tops[0], 36.0);
    near("HNEXT line top", tops[1], 36.0 + pitch());
    near("BODY1 line top", line_tops(&p.items)[0], 72.0);
}

/// Probe h: the anchor paragraph also holds an inline 40.05 x 38.95pt shape before "HAFTER".
/// Word: that line starts at 36.0 beside the box; HNEXT 77.5; body 91.3.
#[test]
fn header_inline_shape_stays_beside_a_top_and_bottom_box() {
    let mut first = anchor_para("HAFTER", Anchor::Paragraph, 7.2);
    let at = first.len() - "HAFTER".len();
    first.insert_object(at, rect_shape(40.05, 38.95, Float::default()), &Default::default()).unwrap();
    let d = doc_with(Some(vec![first, para("HNEXT")]), None, vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    let tops = line_tops(&p.header);
    assert_eq!(tops.len(), 2, "two header lines: {tops:?}");
    near("anchor line top", tops[0], 36.0);
    // Word: HNEXT 77.5 (the 38.95pt shape plus the font's descent), body 91.3 = 77.5 + 13.8.
    assert!(tops[1] > 36.0 + 38.95 - TOL && tops[1] < 36.0 + 38.95 + pitch(), "HNEXT right below the shape's line: {tops:?}");
    near("BODY1 line top", line_tops(&p.items)[0], tops[1] + pitch());
}

/// Probe m: the box is anchored on the second header paragraph (57.0..87.0). Header lines stay
/// consecutive (36.0, 49.8, 63.6); the body starts at the box's bottom, 87.0.
#[test]
fn body_still_clears_a_header_top_and_bottom_box() {
    let header = vec![para("HFIRST"), anchor_para("HANCHOR line", Anchor::Paragraph, 7.2), para("HNEXT")];
    let d = doc_with(Some(header), None, vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    let tops = line_tops(&p.header);
    assert_eq!(tops.len(), 3, "three header lines: {tops:?}");
    near("HFIRST line top", tops[0], 36.0);
    near("HANCHOR line top", tops[1], 36.0 + pitch());
    near("HNEXT line top", tops[2], 36.0 + 2.0 * pitch());
    // The box: anchor paragraph top + 7.2, 30pt tall (Word 49.8 + 7.2 + 30 = 87.0).
    near("BODY1 line top", line_tops(&p.items)[0], 36.0 + pitch() + 37.2);
}

/// Probe l: the box placed from the page (40..70pt) ends above the top margin. Header 36.0, 49.8;
/// body 72.0.
#[test]
fn page_placed_top_and_bottom_box_in_a_header() {
    let d = doc_with(Some(vec![anchor_para("HANCHOR line", Anchor::Page, 40.0), para("HNEXT")]), None, vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    let tops = line_tops(&p.header);
    near("HANCHOR line top", tops[0], 36.0);
    near("HNEXT line top", tops[1], 36.0 + pitch());
    near("BODY1 line top", line_tops(&p.items)[0], 72.0);
}

/// Probe o: footer "FFIRST", "FANCHOR line" (box +7.2pt), "FNEXT". Word keeps the three lines
/// consecutive, the last ending at 792 - 36 = 756: tops 714.6, 728.4, 742.2.
#[test]
fn footer_lines_stay_together_around_a_top_and_bottom_box() {
    let footer = vec![para("FFIRST"), anchor_para("FANCHOR line", Anchor::Paragraph, 7.2), para("FNEXT")];
    let d = doc_with(None, Some(footer), vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    assert_eq!(shapes(&p.footer).len(), 1, "the box is still drawn in the footer");
    let tops = line_tops(&p.footer);
    assert_eq!(tops.len(), 3, "three footer lines: {tops:?}");
    let h = pitch();
    near("FFIRST line top", tops[0], 756.0 - 3.0 * h);
    near("FANCHOR line top", tops[1], 756.0 - 2.0 * h);
    near("FNEXT line top", tops[2], 756.0 - h);
}

/// Probe a (unchanged behaviour): in the body the anchor's line goes below the box. "BEFORE" at
/// 72.0, the anchor paragraph at 85.8, box 93.0..123.0, "ANCHOR1 line" at 123.0.
#[test]
fn body_anchor_line_still_goes_below_a_top_and_bottom_box() {
    let d = doc_with(None, None, vec![para("BEFORE"), anchor_para("ANCHOR1 line", Anchor::Paragraph, 7.2), para("NEXT1")]);
    let l = lay(&d);
    let tops = line_tops(&l.pages[0].items);
    assert_eq!(tops.len(), 3, "three body lines: {tops:?}");
    near("BEFORE line top", tops[0], 72.0);
    let h = pitch();
    near("ANCHOR1 line top", tops[1], 72.0 + h + 37.2);
    near("NEXT1 line top", tops[2], 72.0 + 2.0 * h + 37.2);
}
