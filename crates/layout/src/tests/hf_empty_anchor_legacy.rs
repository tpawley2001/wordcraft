//! An empty header paragraph that anchors a Top and Bottom picture (a letterhead banner). In
//! compatibility modes 11, 12 and 14 Word leaves the header's lines where they are, as it does
//! for a paragraph with text (`hf_top_bottom_wrap.rs`), and starts the body at the picture's
//! bottom; in mode 15 the anchor's line goes below the picture. Expected values measured from
//! PDFs that Microsoft 365 Word exported for synthetic probes (Letter, top margin 50.4pt, header
//! 36pt, Arial 12pt single-spaced = 13.8pt lines, a 400 x 49.8pt box at column +0, paragraph +0,
//! wp:wrapTopAndBottom, ECMA-376 §20.4.2.20). Line tops in pt from the page top; Word's 13.8pt
//! line pitch is replaced by the test font's own (`pitch()`).
use super::rect_shape;
use crate::*;
use wordcraft_doc::para::{Anchor, Float, Wrap};
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{PartKind, para_block};

const TOL: f32 = 1.5;
/// The header distance plus the box's height: where the box ends.
const BOX_BOTTOM: f32 = 36.0 + 49.8;

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

/// `text` (may be empty) with the 400 x 49.8pt Top and Bottom box anchored at its start, `y`
/// below the paragraph's top.
fn anchor_para(text: &str, y: f32) -> Paragraph {
    let mut p = para(text);
    let f = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 0.0, y, ..Default::default() };
    p.insert_object(0, rect_shape(400.0, 49.8, f), &Default::default()).unwrap();
    p
}

fn doc_with(mode: u32, header: Option<Vec<Paragraph>>, body: Vec<Paragraph>) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (50.4, 72.0, 72.0, 72.0);
    (s.header, s.footer) = (36.0, 36.0);
    if let Some(h) = header {
        let id = d.add_part(PartKind::Header, h.into_iter().map(para_block).collect());
        d.last_section.headers.default = Some(id);
    }
    d.body = body.into_iter().map(para_block).collect();
    d
}

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// Tops of every paragraph's first line among `items` (an empty anchor paragraph included), top
/// to bottom.
fn line_tops(items: &[Placed]) -> Vec<f32> {
    let mut v: Vec<f32> = items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { y, .. } => Some(*y),
            _ => None,
        })
        .collect();
    v.sort_by(f32::total_cmp);
    v
}

/// The line pitch of a plain single-spaced line of the fixture font (13.8pt in Word).
fn pitch() -> f32 {
    let t = line_tops(&lay(&doc_with(15, None, vec![para("A"), para("B")])).pages[0].items);
    t[1] - t[0]
}

/// Probes e, k, l: the header is one empty paragraph holding the box. Word (modes 11, 12 and
/// 14): the body's first line starts at 85.8, the box's bottom.
#[test]
fn body_starts_at_the_box_under_an_empty_header_anchor_in_legacy_modes() {
    for mode in [11, 12, 14] {
        let d = doc_with(mode, Some(vec![anchor_para("", 0.0)]), vec![para("BODY1"), para("BODY2")]);
        let l = lay(&d);
        let p = &l.pages[0];
        let header = line_tops(&p.header);
        assert_eq!(header.len(), 1, "mode {mode}: one header line: {header:?}");
        near(&format!("mode {mode}: empty anchor line top"), header[0], 36.0);
        let body = line_tops(&p.items);
        near(&format!("mode {mode}: BODY1 line top"), body[0], BOX_BOTTOM);
        near(&format!("mode {mode}: BODY2 line top"), body[1], BOX_BOTTOM + pitch());
    }
}

/// Probe m: the box 10pt below the paragraph's top. Word (mode 11): body at 95.8.
#[test]
fn body_starts_at_a_lowered_box_under_an_empty_header_anchor() {
    let d = doc_with(11, Some(vec![anchor_para("", 10.0)]), vec![para("BODY1")]);
    let l = lay(&d);
    near("BODY1 line top", line_tops(&l.pages[0].items)[0], BOX_BOTTOM + 10.0);
}

/// Probe o: the empty anchor paragraph, then "HDR2". Word (mode 11): HDR2 at 49.8, right under
/// the empty line; body at 85.8.
#[test]
fn header_lines_follow_an_empty_anchor_line_in_legacy_modes() {
    let d = doc_with(11, Some(vec![anchor_para("", 0.0), para("HDR2")]), vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    let header = line_tops(&p.header);
    assert_eq!(header.len(), 2, "two header lines: {header:?}");
    near("empty anchor line top", header[0], 36.0);
    near("HDR2 line top", header[1], 36.0 + pitch());
    near("BODY1 line top", line_tops(&p.items)[0], BOX_BOTTOM);
}

/// Probe h (unchanged behaviour): an anchor paragraph with text in mode 11. Word: HDR at 36.0,
/// body at 85.8.
#[test]
fn header_anchor_with_text_stays_in_legacy_modes() {
    let d = doc_with(11, Some(vec![anchor_para("HDR", 0.0)]), vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    near("HDR line top", line_tops(&p.header)[0], 36.0);
    near("BODY1 line top", line_tops(&p.items)[0], BOX_BOTTOM);
}

/// Probes a and g (unchanged behaviour): in mode 15 the empty anchor's line goes below the box.
/// Word: body at 99.6; with "HDR2" after it, HDR2 at 99.6 and the body at 113.4.
#[test]
fn empty_header_anchor_line_goes_below_the_box_in_mode_15() {
    let h = pitch();
    let d = doc_with(15, Some(vec![anchor_para("", 0.0)]), vec![para("BODY1")]);
    let l = lay(&d);
    near("BODY1 line top", line_tops(&l.pages[0].items)[0], BOX_BOTTOM + h);

    let d = doc_with(15, Some(vec![anchor_para("", 0.0), para("HDR2")]), vec![para("BODY1")]);
    let l = lay(&d);
    let p = &l.pages[0];
    let header = line_tops(&p.header);
    near("HDR2 line top", *header.last().unwrap(), BOX_BOTTOM + h);
    near("BODY1 line top", line_tops(&p.items)[0], BOX_BOTTOM + 2.0 * h);
}

/// Probes j and p (unchanged behaviour): in the body an empty anchor paragraph's line goes below
/// the box in every mode. Word: BODY1 at 50.4 + 49.8 + 13.8 = 114.0.
#[test]
fn body_empty_anchor_line_still_goes_below_the_box() {
    for mode in [11, 15] {
        let d = doc_with(mode, None, vec![anchor_para("", 0.0), para("BODY1")]);
        let l = lay(&d);
        let tops = line_tops(&l.pages[0].items);
        near(&format!("mode {mode}: BODY1 line top"), *tops.last().unwrap(), 50.4 + 49.8 + pitch());
    }
}
