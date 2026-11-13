//! A Square-wrapped floating object positioned down from its anchor paragraph that no longer
//! fits on the page: Word starts the anchor paragraph on the next page and the object goes with
//! it. In current documents (`compatibilityMode` 15) the limit is the bottom margin; in older
//! compatibility modes (11 and 12 measured) the object may hang into the bottom margin and only
//! moves when it would pass the bottom edge of the paper. (Microsoft 365 Word's PDF output of
//! small DOCX files: Letter paper, text from y 72 to 720 and 432 pt wide, 13.8 pt lines, the
//! object 16.2 pt below its paragraph's top and 270 pt high.)

use super::{assert_contiguous, lay, shown_and_ranges};
#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::para::{Anchor, Float, ShapeKind, Wrap};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Block, InlineObject, Path, Pos, StoryRef};

/// The line pitch of the probes, the object's offset below its paragraph's top, and its height.
const PITCH: f32 = 13.8;
const DOWN: f32 = 16.2;
const HIGH: f32 = 270.0;

/// A rectangle `w` × `h`, Square wrap on both sides (9 pt left and right, 0 above and below),
/// at the column's left edge and [`DOWN`] below the top of the paragraph it is anchored in.
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

/// `n` one-line paragraphs, the anchor paragraph (`anchor` its text; block `n`) and one more
/// paragraph (block `n + 1`), every line [`PITCH`] high with no space between paragraphs, on
/// Letter paper with the text from y 72 to 720 and 432 pt wide, in compatibility mode `mode`.
fn fixture(n: usize, anchor: &str, mode: u32) -> Document {
    crate::test_fonts();
    let mut text: Vec<String> = (0..n).map(|i| format!("Line {i}")).collect();
    text.push(anchor.to_string());
    text.push("NEXT".to_string());
    let mut d = Document::from_text(&text.join("\n"));
    for block in &mut d.body {
        if let Block::Para(p) = Arc::make_mut(block) {
            p.props.line_spacing = Some(LineSpacing::Exactly(PITCH));
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
        }
    }
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 90.0, 72.0, 90.0);
    d.settings.compat_mode = mode;
    d
}

/// [`fixture`] with a `w` × `h` object anchored at the start of the anchor paragraph.
fn with_shape(n: usize, anchor: &str, mode: u32, (w, h): (f32, f32)) -> Document {
    let mut d = fixture(n, anchor, mode);
    d.insert_object(&Pos::body(n, 0), para_shape(w, h), &Default::default()).unwrap();
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

/// Every paragraph's text is placed, in order, exactly once.
fn assert_all_text(d: &Document, l: &DocLayout) {
    for block in 0..d.body.len() {
        let (_, ranges) = shown_and_ranges(l, block as u32);
        assert_contiguous(&ranges, d.para(StoryRef::Body, &Path::top(block)).map(|p| p.len()).unwrap_or(0));
    }
}

/// Where the object's bottom would be on page 1: the probes' `72 + 13.8 n + 16.2 + 270`.
fn bottom_on_page_1(n: usize) -> f32 {
    72.0 + PITCH * n as f32 + DOWN + HIGH
}

/// The anchor paragraph and its object stay on page 1, the object where its offset puts it.
fn assert_stays(d: &Document, n: usize) {
    let l = lay(d);
    let (page, _, top) = first_line(&l, n);
    assert_eq!(page, 0, "the anchor paragraph stays on page 1");
    assert!((top - (72.0 + PITCH * n as f32)).abs() < 0.5, "below the {n} lines before it: {top}");
    let (sp, r) = shape(&l);
    assert_eq!(sp, 0, "the object stays on page 1: {r:?}");
    assert!((r.bottom() - bottom_on_page_1(n)).abs() < 0.5, "its bottom is {}: {r:?}", bottom_on_page_1(n));
    assert_all_text(d, &l);
}

/// The anchor paragraph starts page 2 and its object is drawn there, [`DOWN`] below the text's
/// top; page 1 keeps the `n` lines before it. Returns the layout and the object's rectangle.
fn assert_moves(d: &Document, n: usize) -> (DocLayout, Rect) {
    let l = lay(d);
    let top = l.pages[0].body.y;
    assert!((top - 72.0).abs() < 0.01);
    let (last_page, _, last_top) = first_line(&l, n - 1);
    assert_eq!(last_page, 0, "the lines before the anchor stay on page 1");
    assert!((last_top - (72.0 + PITCH * (n - 1) as f32)).abs() < 0.5, "and stay where they were: {last_top}");
    let (page, _, y) = first_line(&l, n);
    assert_eq!(page, 1, "the anchor paragraph moves to page 2 (its line is at {y})");
    assert!((y - top).abs() < 0.5, "and starts at the top of the text there: {y}");
    let (sp, r) = shape(&l);
    assert_eq!(sp, 1, "the object moves to page 2 with its anchor: {r:?}");
    assert!((r.y - (top + DOWN)).abs() < 0.5, "{DOWN} pt below the paragraph's top: {r:?}");
    assert!(r.bottom() <= l.pages[1].body.bottom() + 0.01, "inside the text area there: {r:?}");
    assert_eq!(l.pages.len(), 2, "nothing else changes page");
    assert_all_text(d, &l);
    (l, r)
}

/// The paragraph after the anchor starts at the object's bottom on page 2 (no room beside it).
fn assert_next_below(l: &DocLayout, r: Rect, n: usize) {
    let (page, _, top) = first_line(l, n + 1);
    assert_eq!(page, 1, "the next paragraph follows on page 2");
    assert!((top - r.bottom()).abs() < 0.5, "its top is the object's bottom {}: {top}", r.bottom());
}

#[test]
fn the_fixture_has_the_probes_geometry() {
    // No object: 13.8 pt lines from y 72, the text area ends at 720, everything on one page.
    let d = fixture(30, "ANCHOR", 15);
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    let body = l.pages[0].body;
    assert!((body.y - 72.0).abs() < 0.01 && (body.bottom() - 720.0).abs() < 0.01 && (body.w - 432.0).abs() < 0.01, "{body:?}");
    for block in [0usize, 1, 30, 31] {
        let (_, x, top) = first_line(&l, block);
        assert!((top - (72.0 + PITCH * block as f32)).abs() < 0.05, "line {block} at {top}");
        assert!((x - 90.0).abs() < 0.5, "line {block} starts at the left margin: {x}");
    }
}

#[test]
fn row_a_current_mode_an_object_ending_above_the_bottom_margin_stays() {
    // Word: 26 lines before it, the object's bottom at 717.0 (the margin is at 720): page 1.
    assert!(bottom_on_page_1(26) < 720.0);
    assert_stays(&with_shape(26, "ANCHOR", 15, (432.0, HIGH)), 26);
}

#[test]
fn row_b_current_mode_an_object_passing_the_bottom_margin_moves_with_its_paragraph() {
    // Word: 30 lines before it, the bottom would be 772.2: the anchor line is at the top of
    // page 2, the object's top at 88.2 and the next paragraph's top at its bottom, 358.2.
    assert!(bottom_on_page_1(30) > 720.0 && bottom_on_page_1(30) < 792.0);
    let d = with_shape(30, "ANCHOR", 15, (432.0, HIGH));
    let (l, r) = assert_moves(&d, 30);
    assert!((r.bottom() - 358.2).abs() < 0.5, "{r:?}");
    assert_next_below(&l, r, 30);
}

#[test]
fn row_c_current_mode_an_anchor_only_paragraph_moves_too() {
    // Word: as row b with nothing but the anchor in the paragraph.
    let d = with_shape(30, "", 15, (432.0, HIGH));
    let (l, r) = assert_moves(&d, 30);
    assert_next_below(&l, r, 30);
}

#[test]
fn row_d_current_mode_a_narrow_object_moves_too_and_the_next_paragraph_goes_beside_it() {
    // Word: a 200 pt wide object leaves room beside it; it still moves with its anchor, and
    // the next paragraph's line is one line below the anchor's, right of the object and its
    // 9 pt wrap distance (x 299 = 90 + 200 + 9).
    let d = with_shape(30, "ANCHOR", 15, (200.0, HIGH));
    let (l, r) = assert_moves(&d, 30);
    let (page, x, top) = first_line(&l, 31);
    assert_eq!(page, 1);
    assert!((top - (72.0 + PITCH)).abs() < 0.5, "one line below the anchor line: {top}");
    assert!(x >= r.right() + 9.0 - 0.5, "beside the object {r:?}: {x}");
    assert!((x - 299.0).abs() < 1.0, "at the object's right edge plus its wrap distance: {x}");
}

#[test]
fn row_e_older_modes_an_object_hanging_into_the_bottom_margin_stays() {
    // Word, modes 11 and 12: the bottom at 772.2 is in the margin but on the paper: page 1.
    for mode in [11, 12] {
        assert_stays(&with_shape(30, "ANCHOR", mode, (432.0, HIGH)), 30);
        assert_stays(&with_shape(30, "", mode, (432.0, HIGH)), 30);
        assert_stays(&with_shape(30, "ANCHOR", mode, (423.0, HIGH)), 30);
    }
}

#[test]
fn row_f_older_mode_an_object_ending_just_on_the_paper_stays() {
    // Word, mode 11: 31 lines before it, the bottom at 786.0 (the paper ends at 792): page 1.
    assert!(bottom_on_page_1(31) < 792.0);
    assert_stays(&with_shape(31, "", 11, (432.0, HIGH)), 31);
}

#[test]
fn row_g_older_mode_an_object_passing_the_paper_s_edge_moves_with_its_paragraph() {
    // Word, mode 11: 32, 33 and 36 lines before it, the bottom would be 799.8, 813.6 and 855.0,
    // off the 792 pt paper: the object's top is 88.2 on page 2, the next paragraph's top 358.2.
    for n in [32, 33, 36] {
        assert!(bottom_on_page_1(n) > 792.0);
        let d = with_shape(n, "", 11, (432.0, HIGH));
        let (l, r) = assert_moves(&d, n);
        assert!((r.bottom() - 358.2).abs() < 0.5, "{n} lines: {r:?}");
        assert_next_below(&l, r, n);
    }
}

#[test]
fn a_paragraph_at_the_top_of_a_page_never_moves_for_its_object() {
    // Not a Word probe: the guard that keeps the layout finite. With no text above it the
    // anchor paragraph stays, however far the object reaches.
    for mode in [11, 15] {
        let d = with_shape(0, "ANCHOR", mode, (432.0, 760.0));
        let l = lay(&d);
        let (page, _, top) = first_line(&l, 0);
        assert_eq!((page, shape(&l).0), (0, 0), "mode {mode}");
        assert!((top - 72.0).abs() < 0.5, "mode {mode}: {top}");
        assert!(l.pages.len() <= 2, "mode {mode}: {} pages", l.pages.len());
        assert_all_text(&d, &l);
    }
}

#[test]
fn an_object_taller_than_any_page_lays_out_once_and_loses_no_text() {
    // Not a Word probe: an object that would not fit on the next page either must not send its
    // paragraph from page to page. Only termination and the text are checked here.
    for mode in [11, 15] {
        let d = with_shape(30, "ANCHOR", mode, (432.0, 900.0));
        let l = lay(&d);
        assert!(l.pages.len() <= 3, "mode {mode}: {} pages", l.pages.len());
        shape(&l);
        assert_all_text(&d, &l);
    }
}

#[test]
fn a_page_break_before_the_anchor_gives_the_same_second_page() {
    // The page-2 geometry rows b, c and g expect is what a paragraph starting the page gets:
    // the anchor line at the text's top, the object 16.2 pt lower, the next paragraph below it.
    for anchor in ["ANCHOR", ""] {
        let mut d = with_shape(30, anchor, 15, (432.0, HIGH));
        if let Some(Block::Para(p)) = d.body.get_mut(30).map(Arc::make_mut) {
            p.props.page_break_before = Some(true);
        }
        let (l, r) = assert_moves(&d, 30);
        assert!((r.bottom() - 358.2).abs() < 0.5, "{r:?}");
        assert_next_below(&l, r, 30);
    }
}
