//! A Top and Bottom object placed above its paragraph (negative paragraph-relative offset, over the
//! line(s) before its anchor) stays where it is first placed, from its anchor's top laid out
//! without it, and the lines it covers move to just below it (+ its distance below), as in Word.
//! Expected values measured from PDFs that Microsoft 365 Word exported for synthetic probes
//! (Letter, 1in margins, Arial 12pt single-spaced = 13.8pt lines, a 144 x 30pt rectangle at
//! column +144pt, wp:wrapTopAndBottom, ECMA-376 §20.4.2.20). Line tops in pt from the page top;
//! Word's 13.8pt line pitch is replaced by the test font's own (`pitch()`).
use super::{rect_shape, shapes};
use crate::*;
use wordcraft_doc::para::{Anchor, Float, Wrap};
use wordcraft_doc::para_block;
use wordcraft_doc::props::{LineSpacing, ParaProps};

const TOL: f32 = 1.5;
const TOP: f32 = 72.0;

fn near(what: &str, got: f32, want: f32) {
    assert!((got - want).abs() <= TOL, "{what}: got {got:.2}pt, Word {want:.2}pt (±{TOL})");
}

fn para(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps { font: Some("Arial".into()), size: Some(12.0), ..Default::default() });
    p.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
    p
}

/// "ANCHOR1 line" with the 144 x 30pt Top and Bottom box at paragraph + `y`, column + 144pt.
fn anchor_para(y: f32, dist_bottom: f32) -> Paragraph {
    let mut p = para("ANCHOR1 line");
    let f = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 144.0, y, dist_bottom, ..Default::default() };
    p.insert_object(0, rect_shape(144.0, 30.0, f), &Default::default()).unwrap();
    p
}

fn doc(body: Vec<Paragraph>) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 72.0, 72.0, 72.0);
    d.body = body.into_iter().map(para_block).collect();
    d
}

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// Tops of the text lines on page 0 (every fixture paragraph is one line), top to bottom.
fn line_tops(l: &DocLayout) -> Vec<f32> {
    let mut v: Vec<f32> = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, y, .. } if para.text_len > 0 => Some(*y),
            _ => None,
        })
        .collect();
    v.sort_by(f32::total_cmp);
    v
}

/// The test font's single-spaced line pitch (Word: 13.8pt for Arial 12pt).
fn pitch() -> f32 {
    let t = line_tops(&lay(&doc(vec![para("A"), para("B")])));
    t[1] - t[0]
}

/// One paragraph "BEFORE" (or two, with "BEFORE0" first), the anchor paragraph with the box at
/// `y` (distance below `dist_bottom`), then "NEXT1" and "NEXT2". Checks the box and the lines.
fn check(name: &str, two_before: bool, y: f32, dist_bottom: f32) {
    let p = pitch();
    let mut body = vec![];
    if two_before {
        body.push(para("BEFORE0"));
    }
    body.extend([para("BEFORE"), anchor_para(y, dist_bottom), para("NEXT1"), para("NEXT2")]);
    let l = lay(&doc(body));
    assert_eq!(l.pages.len(), 1, "{name}: one page");
    let boxes = shapes(&l.pages[0].items);
    assert_eq!(boxes.len(), 1, "{name}: one box");
    let before = if two_before { 1 } else { 0 };
    // Where the anchor paragraph starts when laid out without the box.
    let anchor_top = TOP + (before as f32 + 1.0) * p;
    let (box_top, box_bottom) = (anchor_top + y, anchor_top + y + 30.0);
    near(&format!("{name}: box top"), boxes[0].y, box_top);
    near(&format!("{name}: box bottom"), boxes[0].bottom(), box_bottom);
    let tops = line_tops(&l);
    assert_eq!(tops.len(), 4 + before, "{name}: lines {tops:?}");
    if two_before {
        near(&format!("{name}: BEFORE0 top (above the box, stays)"), tops[0], TOP);
    }
    let first = box_bottom + dist_bottom;
    for (i, what) in ["BEFORE", "ANCHOR1 line", "NEXT1", "NEXT2"].iter().enumerate() {
        near(&format!("{name}: {what} top"), tops[before + i], first + i as f32 * p);
    }
}

/// Probe f. Word: box 78.6..108.6; BEFORE 108.6, ANCHOR1 122.4, NEXT1 136.2, NEXT2 150.0.
#[test]
fn a_box_above_its_paragraph_moves_the_line_before_to_its_bottom() {
    check("f", false, -7.2, 0.0);
}

/// Probe p. Word: box 82.8..112.8; BEFORE 112.8, ANCHOR1 126.6.
#[test]
fn a_box_a_little_above_its_paragraph_moves_the_line_before_to_its_bottom() {
    check("p", false, -3.0, 0.0);
}

/// Probe q. Word: box 65.8..95.8, distB 9pt; BEFORE 104.8, ANCHOR1 118.6.
#[test]
fn the_line_before_goes_below_the_box_and_its_distance_below() {
    check("q", false, -20.0, 9.0);
}

/// Probe r. Word: BEFORE0 72.0 (the box doesn't reach it); box 92.4..122.4; BEFORE 122.4,
/// ANCHOR1 136.2.
#[test]
fn only_the_lines_the_box_covers_move() {
    check("r", true, -7.2, 0.0);
}

/// Probe a (regression guard, positive offset): Word: BEFORE 72.0, box 92.99..122.99, ANCHOR1
/// line 123.0 (the anchor's own line goes below the box), NEXT1 136.8.
#[test]
fn a_box_below_its_paragraph_top_still_moves_its_own_line_only() {
    let p = pitch();
    let l = lay(&doc(vec![para("BEFORE"), anchor_para(7.2, 0.0), para("NEXT1"), para("NEXT2")]));
    let b = shapes(&l.pages[0].items)[0];
    near("a: box top", b.y, TOP + p + 7.2);
    let tops = line_tops(&l);
    near("a: BEFORE top", tops[0], TOP);
    near("a: ANCHOR1 line top", tops[1], b.bottom());
    near("a: NEXT1 top", tops[2], b.bottom() + p);
}
