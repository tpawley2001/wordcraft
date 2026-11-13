//! A Square-wrapped floating object positioned down from its anchor paragraph that would end
//! below the paper's bottom edge, in documents older than `compatibilityMode` 15: Word moves the
//! object up so it ends at the paper's edge and keeps it, and its paragraph, on the page; text
//! wraps around it there. Only an object that then leaves the paragraph's first line no room
//! beside it (as wide as the column) goes to the next page with its paragraph
//! (`square_float_page_fit`, rows e to g). (Microsoft 365 Word, small DOCX files in mode 11:
//! Letter paper, text from y 72 and 432 pt wide, 13.8 pt lines, the object 200 pt wide at the
//! column's left, 16.2 pt below its paragraph's top, with 9 pt wrap distance left and right.)

use super::lay;
#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::para::{Anchor, Float, ShapeKind, Wrap};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Block, InlineObject, Pos, StoryRef};

/// The object's offset below its paragraph's top, its width, and the paper's height.
const DOWN: f32 = 16.2;
const WIDE: f32 = 200.0;
const PAPER: f32 = 792.0;

/// A `w` × `h` rectangle, Square wrap on both sides (9 pt left and right, 0 above and below), at
/// the column's left edge and [`DOWN`] below the top of its paragraph.
fn para_shape(w: f32, h: f32) -> InlineObject {
    let float = Float {
        wrap: Wrap::Square,
        h_rel: Anchor::Column,
        v_rel: Anchor::Paragraph,
        x: 0.0,
        y: DOWN,
        dist: 9.0,
        dist_top: 0.0,
        dist_bottom: 0.0,
        ..Default::default()
    };
    InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w,
        h,
        fill: None,
        stroke: None,
        stroke_width: 1.0,
        float,
        story: None,
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

/// `n` one-line filler paragraphs, "ANCHOR" (block `n`) and "NEXT" (block `n + 1`), 13.8 pt
/// lines with no space between paragraphs, on Letter paper with margins 72 above, 90 left and
/// right and `bottom` below, in compatibility mode `mode`; a `w` × `h` object anchored at the
/// start of ANCHOR when `h` > 0.
fn fixture(n: usize, mode: u32, bottom: f32, (w, h): (f32, f32)) -> Document {
    crate::test_fonts();
    let mut text: Vec<String> = (0..n).map(|i| format!("Line {i}")).collect();
    text.push("ANCHOR".to_string());
    text.push("NEXT".to_string());
    let mut d = Document::from_text(&text.join("\n"));
    for block in &mut d.body {
        if let Block::Para(p) = Arc::make_mut(block) {
            p.props.line_spacing = Some(LineSpacing::Exactly(13.8));
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
        }
    }
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, PAPER);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 90.0, bottom, 90.0);
    d.settings.compat_mode = mode;
    if h > 0.0 {
        d.insert_object(&Pos::body(n, 0), para_shape(w, h), &Default::default()).unwrap();
    }
    d
}

/// The page (0-based), left and top of the first line of body paragraph `block`.
fn first_line(l: &DocLayout, block: usize) -> (usize, f32, f32) {
    for (pi, page) in l.pages.iter().enumerate() {
        let mut tops: Vec<(f32, f32)> = Vec::new();
        for it in &page.items {
            let Placed::Lines { story: StoryRef::Body, path, para, l0, x, y, .. } = it else { continue };
            if path.0 == [block as u32] {
                tops.push((x + para.lines[*l0].left, *y));
            }
        }
        tops.sort_by(|a, b| a.1.total_cmp(&b.1));
        if let Some(&(x, y)) = tops.first() {
            return (pi, x, y);
        }
    }
    panic!("paragraph {block} is not placed on any page");
}

/// The page (0-based) and rectangle of the one floating shape drawn.
fn shape(l: &DocLayout) -> (usize, Rect) {
    let mut found = Vec::new();
    for (pi, page) in l.pages.iter().enumerate() {
        for it in &page.items {
            if let Placed::Shape { rect, .. } = it {
                found.push((pi, *rect));
            }
        }
    }
    assert_eq!(found.len(), 1, "the object is drawn once: {found:?}");
    found[0]
}

/// The filler's line pitch and the top of the first line, measured from the fixture without
/// an object.
fn pitch(n: usize) -> (f32, f32) {
    let l = lay(&fixture(n, 11, 72.0, (0.0, 0.0)));
    let (_, _, t0) = first_line(&l, 0);
    let (_, _, t1) = first_line(&l, 1);
    (t1 - t0, t0)
}

/// Where the anchor paragraph's top is after `n` filler lines, and the unshifted object's bottom.
fn unshifted_bottom(n: usize, h: f32) -> (f32, f32) {
    let (p, t0) = pitch(n);
    let top = t0 + p * n as f32;
    (top, top + DOWN + h)
}

/// One page; the object on it ending at the paper's edge, ANCHOR and NEXT beside it. Returns the
/// layout and the object's rectangle.
fn assert_shifted(d: &Document, n: usize, h: f32) -> (DocLayout, Rect) {
    let (top, bottom) = unshifted_bottom(n, h);
    assert!(bottom > PAPER, "the object would pass the paper's edge: {bottom}");
    let l = lay(d);
    assert_eq!(l.pages.len(), 1, "everything stays on one page");
    let (sp, r) = shape(&l);
    assert_eq!(sp, 0, "the object stays on page 1: {r:?}");
    assert!((r.bottom() - PAPER).abs() < 0.5, "it ends at the paper's edge: {r:?}");
    assert!((r.h - h).abs() < 0.01 && (r.w - WIDE).abs() < 0.01 && (r.x - 90.0).abs() < 0.5, "same size and place across: {r:?}");
    for block in [n, n + 1] {
        let (page, x, _) = first_line(&l, block);
        assert_eq!(page, 0, "paragraph {block} stays on page 1");
        assert!(x >= r.right() + 9.0 - 0.5, "paragraph {block} starts right of the object {r:?}: {x}");
    }
    let (_, _, at) = first_line(&l, n);
    assert!((at - top).abs() < 0.5, "ANCHOR stays below the filler: {at} (expected {top})");
    (l, r)
}

#[test]
fn row_1_an_object_ending_on_the_paper_is_not_moved() {
    // Word: 31 lines, 200 x 275, bottom 791.0: not shifted; ANCHOR at the column's left.
    let (_, bottom) = unshifted_bottom(31, 275.0);
    assert!(bottom < PAPER);
    let d = fixture(31, 11, 72.0, (WIDE, 275.0));
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    let (sp, r) = shape(&l);
    assert_eq!(sp, 0);
    assert!((r.bottom() - bottom).abs() < 0.5, "where its offset puts it, {bottom}: {r:?}");
    let (page, x, _) = first_line(&l, 31);
    assert_eq!(page, 0);
    assert!((x - 90.0).abs() < 0.5, "ANCHOR starts at the column's left: {x}");
}

#[test]
fn rows_2_to_4_an_object_passing_the_paper_s_edge_moves_up_onto_it() {
    // Word: 200 x 278, 281 and 284 (bottom 794.0, 797.0, 800.0): one page, ANCHOR and NEXT
    // beside the shifted object (x 299).
    for h in [278.0, 281.0, 284.0] {
        assert_shifted(&fixture(31, 11, 72.0, (WIDE, h)), 31, h);
    }
}

#[test]
fn row_5_the_bottom_margin_does_not_matter() {
    // Word: as rows 2 to 4 with a 13.7 pt bottom margin.
    for h in [278.0, 281.0, 284.0] {
        assert_shifted(&fixture(31, 11, 13.7, (WIDE, h)), 31, h);
    }
}

#[test]
fn row_6_lines_above_the_paragraph_wrap_around_the_shifted_object() {
    // Word: 200 x 314 (bottom 830.0) is shifted up 38 pt: fillers 30 and 31 go beside it too,
    // filler 29 (above it) does not.
    let (l, r) = assert_shifted(&fixture(31, 11, 13.7, (WIDE, 314.0)), 31, 314.0);
    // Word's fillers 30 and 31 are blocks 29 and 30 (tops 472.2 and 486.0), its filler 29 block
    // 28 (top 458.4).
    for block in [29, 30] {
        let (page, x, _) = first_line(&l, block);
        assert_eq!(page, 0);
        assert!(x >= r.right() + 9.0 - 0.5, "block {block} beside the object {r:?} wraps: {x}");
    }
    let (_, x, _) = first_line(&l, 28);
    assert!((x - 90.0).abs() < 0.5, "block 28, above the object, stays at the column's left: {x}");
}

#[test]
fn current_mode_keeps_moving_the_paragraph() {
    // Mode 15, row 4's object: the paragraph and the object go to page 2 (`square_float_page_fit`
    // row d).
    let l = lay(&fixture(31, 15, 72.0, (WIDE, 284.0)));
    let (page, _, y) = first_line(&l, 31);
    assert_eq!(page, 1, "ANCHOR starts page 2: {y}");
    let (sp, r) = shape(&l);
    assert_eq!(sp, 1, "the object goes with it: {r:?}");
    assert!((r.y - (72.0 + DOWN)).abs() < 0.5, "{DOWN} below the paragraph's top: {r:?}");
}
