//! A floating object (`wp:anchor`, ECMA-376 Part 1 §20.4.2.3) anchored after a manual page break
//! (`w:br w:type="page"`, §17.3.3.1) with nothing after it but the paragraph mark, in a document
//! with `w:splitPgBreakAndParaMark` below compatibility mode 15: the mark makes an empty line at
//! the top of the next page and the object is anchored to that line, so it is placed on the next
//! page. A top-and-bottom object sits at the page top with the mark's line below it; a square or
//! wrap-none object sits at the page top beside the mark's line. In mode 15, or without the
//! setting, the object stays with the break (`tests/break_then_float.rs`).
//!
//! Measured in Word's PDFs of synthetic documents (US Letter, 13.8 pt lines, the paragraph on
//! line 41 of 48, or line 3): pages, the object's page and the next paragraph's position. These
//! tests use the test fonts and text boxes, so they assert pages and order, and offsets from the
//! fixture's own line height.

use super::*;
use wordcraft_doc::para::{Anchor, Float, PAGE_BREAK, Wrap};

#[derive(Clone, Copy)]
enum Piece {
    Break,
    /// A floating text box `w` x `h` with this wrapping, `y` from its paragraph.
    Box(f32, f32, f32, Wrap),
    Text(&'static str),
    /// A 200 x 100 pt top-and-bottom text box 100 pt below the top margin.
    MarginBox,
}

/// `lines` one-line paragraphs, then one paragraph built from `pieces`, then "NEXT" and "LAST".
fn doc(lines: usize, pieces: &[Piece], mode: u32, split: bool) -> Document {
    let mut text: Vec<String> = (1..=lines).map(|i| format!("L{i}")).collect();
    text.extend([String::new(), "NEXT".into(), "LAST".into()]);
    let mut d = Document::from_text(&text.join("\n"));
    d.settings.compat_mode = mode;
    d.settings.split_pg_break_and_para_mark = split;
    for piece in pieces {
        let off = d.para_mut(StoryRef::Body, &Path::top(lines)).unwrap().text.len();
        let pos = Pos::body(lines, off);
        match *piece {
            Piece::Break => {
                d.insert_text(&pos, &PAGE_BREAK.to_string(), &Default::default()).unwrap();
            }
            Piece::Text(s) => {
                d.insert_text(&pos, s, &Default::default()).unwrap();
            }
            Piece::Box(w, h, y, wrap) => {
                let float = Float { wrap, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 0.0, y, dist: 0.0, ..Default::default() };
                text_box_at(&mut d, &pos, "", w, h, float);
            }
            Piece::MarginBox => {
                let float = Float { wrap: TB, h_rel: Anchor::Column, v_rel: Anchor::Margin, x: 0.0, y: 100.0, dist: 0.0, ..Default::default() };
                text_box_at(&mut d, &pos, "", 200.0, 100.0, float);
            }
        }
    }
    d
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

/// Page and top of the first placed piece of body paragraph `i`.
fn para_at(l: &DocLayout, i: usize) -> Option<(usize, f32)> {
    l.pages.iter().enumerate().find_map(|(pi, p)| {
        p.items.iter().find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, y, .. } if path.0 == [i as u32] => Some((pi, *y)),
            _ => None,
        })
    })
}

/// Page, top and bottom of every text box drawn.
fn boxes(l: &DocLayout) -> Vec<(usize, f32, f32)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Shape { rect, kind: wordcraft_doc::para::ShapeKind::TextBox, .. } = it {
                out.push((pi, rect.y, rect.y + rect.h));
            }
        }
    }
    out
}

/// How many one-line paragraphs a page holds, the top of its first line and the line height.
fn page_shape() -> (usize, f32, f32) {
    let l = lay_out(&doc(100, &[], 11, true));
    let n = l.pages[0].items.iter().filter(|it| matches!(it, Placed::Lines { story: StoryRef::Body, .. })).count();
    assert!(n > 15, "a page holds {n} lines");
    let (top, second) = (para_at(&l, 0).unwrap().1, para_at(&l, 1).unwrap().1);
    (n, top, second - top)
}

const TB: Wrap = Wrap::TopAndBottom;

/// The paragraph sits on line 41 of 48 in Word: about seven lines of room left.
fn lines_before() -> usize {
    page_shape().0 - 8
}

/// Lays out `[break][box]` after `lines` paragraphs and checks: the page count, the box on page 2
/// at its top, and NEXT on page 2 at `next` (given the box's bottom, the body top and a line).
fn check(lines: usize, bx: Piece, mode: u32, pages: usize, next: impl Fn(f32, f32, f32) -> f32) {
    let (_, top, line) = page_shape();
    let l = lay_out(&doc(lines, &[Piece::Break, bx], mode, true));
    assert_eq!(l.pages.len(), pages, "mode {mode}");
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "mode {mode}: the box is drawn once: {b:?}");
    assert_eq!(b[0].0, 1, "mode {mode}: the box is on page 2: {b:?}");
    assert!((b[0].1 - top).abs() < 3.0, "mode {mode}: at the top of the page: {b:?} (body top {top})");
    let got = para_at(&l, lines + 1).unwrap();
    assert_eq!(got.0, 1, "mode {mode}: NEXT is on page 2: {got:?}");
    let want = next(b[0].2, top, line);
    assert!((got.1 - want).abs() < 0.5, "mode {mode}: NEXT at {got:?}, want {want} (box {b:?}, line {line})");
}

#[test]
fn row1_the_break_alone_puts_the_marks_line_above_next() {
    let (_, top, line) = page_shape();
    let lines = lines_before();
    let l = lay_out(&doc(lines, &[Piece::Break], 11, true));
    assert_eq!(l.pages.len(), 2);
    let next = para_at(&l, lines + 1).unwrap();
    assert_eq!(next.0, 1);
    assert!((next.1 - (top + line)).abs() < 0.5, "{next:?}");
}

#[test]
fn row2_a_full_width_figure_and_next_share_page_2() {
    check(lines_before(), Piece::Box(467.7, 361.0, -1.35, TB), 11, 2, |bottom, _, line| bottom + line);
}

#[test]
fn row3_mode_14_is_the_same() {
    check(lines_before(), Piece::Box(467.7, 361.0, -1.35, TB), 14, 2, |bottom, _, line| bottom + line);
}

#[test]
fn row4_mode_15_keeps_the_figure_alone_on_page_2() {
    let (_, top, _) = page_shape();
    let lines = lines_before();
    let l = lay_out(&doc(lines, &[Piece::Break, Piece::Box(467.7, 361.0, -1.35, TB)], 15, true));
    assert_eq!(l.pages.len(), 3);
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 1, "{b:?}");
    let next = para_at(&l, lines + 1).unwrap();
    assert_eq!(next.0, 2, "{next:?}");
    assert!((next.1 - top).abs() < 0.5, "{next:?}");
}

#[test]
fn row5_a_small_top_and_bottom_box_goes_to_page_2() {
    check(lines_before(), Piece::Box(200.0, 100.0, 0.0, TB), 11, 2, |bottom, _, line| bottom + line);
}

#[test]
fn row6_a_square_box_sits_beside_the_marks_line() {
    check(lines_before(), Piece::Box(200.0, 100.0, 0.0, Wrap::Square), 11, 2, |_, top, line| top + line);
}

#[test]
fn row7_a_box_in_front_of_text_sits_over_the_marks_line() {
    check(lines_before(), Piece::Box(200.0, 100.0, 0.0, Wrap::InFrontOfText), 11, 2, |_, top, line| top + line);
}

#[test]
fn row8_a_box_on_a_nearly_empty_page_still_goes_to_page_2() {
    check(2, Piece::Box(200.0, 100.0, 0.0, TB), 11, 2, |bottom, _, line| bottom + line);
}

#[test]
fn without_the_setting_the_box_stays_with_the_break() {
    let (_, top, _) = page_shape();
    let lines = lines_before();
    let l = lay_out(&doc(lines, &[Piece::Break, Piece::Box(200.0, 100.0, 0.0, TB)], 11, false));
    assert_eq!(l.pages.len(), 2);
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 0, "the box is on page 1: {b:?}");
    let next = para_at(&l, lines + 1).unwrap();
    assert_eq!(next.0, 1, "{next:?}");
    assert!((next.1 - top).abs() < 0.5, "NEXT starts page 2: {next:?}");
}

#[test]
fn text_after_the_box_is_unchanged_by_the_setting() {
    let lines = lines_before();
    let pieces = [Piece::Break, Piece::Box(200.0, 100.0, 0.0, TB), Piece::Text("AFTER")];
    let on = lay_out(&doc(lines, &pieces, 11, true));
    let off = lay_out(&doc(lines, &pieces, 11, false));
    assert_eq!(on.pages.len(), off.pages.len());
    assert_eq!(boxes(&on), boxes(&off));
    assert_eq!(para_at(&on, lines + 1), para_at(&off, lines + 1));
}

#[test]
fn a_box_positioned_from_the_margin_is_unchanged_by_the_setting() {
    let lines = lines_before();
    let pieces = [Piece::Break, Piece::MarginBox];
    let on = lay_out(&doc(lines, &pieces, 11, true));
    let off = lay_out(&doc(lines, &pieces, 11, false));
    assert_eq!(on.pages.len(), off.pages.len());
    assert_eq!(boxes(&on), boxes(&off));
    assert_eq!(para_at(&on, lines + 1), para_at(&off, lines + 1));
}
