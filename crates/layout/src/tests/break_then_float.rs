//! A floating object anchored in a paragraph that also holds a manual page break (`w:br
//! w:type="page"`, ECMA-376 Part 1 §17.3.3.1; `wp:anchor`, §20.4.2.3): the anchor makes no line of
//! its own after the break, a top-and-bottom object (`wp:wrapTopAndBottom`) that does not fit
//! above its anchor line in the room left moves to the next page with that line instead of being
//! dropped, and text after the break starts below the object.
//!
//! Page counts and order measured from Word's PDFs of the same documents (US Letter, 13.8pt
//! lines, the paragraph on line 41 of page 1 with about 118pt left, compatibility modes 11 and
//! 15): `[break][object 361pt tall]` gave three pages (the object alone at the top of page 2);
//! with text after the object two pages, the text directly below the object; a 100pt object after
//! two lines stayed on page 1 and the next paragraph started at the top of page 2. These tests
//! use the test fonts and text boxes, so they assert pages and order, not Word's point values.

use super::*;
use wordcraft_doc::para::{Anchor, Float, PAGE_BREAK, Wrap};

#[derive(Clone, Copy)]
enum Piece {
    Break,
    /// A 200pt wide floating text box of this height and wrapping at offset 0 from its paragraph.
    Box(f32, Wrap),
    Text(&'static str),
}

/// `lines` one-line paragraphs, then one paragraph built from `pieces`, then "NEXT" and "LAST".
/// The built paragraph's index is `lines`.
fn doc(lines: usize, pieces: &[Piece], mode: u32) -> Document {
    let mut text: Vec<String> = (1..=lines).map(|i| format!("L{i}")).collect();
    text.extend([String::new(), "NEXT".into(), "LAST".into()]);
    let mut d = Document::from_text(&text.join("\n"));
    d.settings.compat_mode = mode;
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
            Piece::Box(h, wrap) => {
                let float = Float { wrap, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 0.0, y: 0.0, dist: 0.0, ..Default::default() };
                text_box_at(&mut d, &pos, "", 200.0, h, float);
            }
        }
    }
    d
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

/// Page and top of the first placed piece of body paragraph `i` that starts at line `from` or later.
fn para_at(l: &DocLayout, i: usize, from: usize) -> Option<(usize, f32)> {
    l.pages.iter().enumerate().find_map(|(pi, p)| {
        p.items.iter().find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, y, l0, l1, .. } if path.0 == [i as u32] && *l1 > from => Some((pi, *y)),
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

/// How many one-line paragraphs a page holds, and the top of its first line.
fn page_shape() -> (usize, f32) {
    let l = lay_out(&doc(100, &[], 15));
    let n = l.pages[0].items.iter().filter(|it| matches!(it, Placed::Lines { story: StoryRef::Body, .. })).count();
    assert!(n > 15, "a page holds {n} lines");
    (n, para_at(&l, 0, 0).unwrap().1)
}

const TB: Wrap = Wrap::TopAndBottom;

#[test]
fn an_anchor_after_the_break_starts_no_line_on_the_next_page() {
    for mode in [11, 15] {
        let control = lay_out(&doc(2, &[Piece::Break], mode));
        assert_eq!(control.pages.len(), 2, "mode {mode}");
        let want = para_at(&control, 3, 0).unwrap();
        for wrap in [TB, Wrap::Square, Wrap::InFrontOfText] {
            let l = lay_out(&doc(2, &[Piece::Break, Piece::Box(100.0, wrap)], mode));
            assert_eq!(l.pages.len(), 2, "mode {mode} {wrap:?}");
            let b = boxes(&l);
            assert_eq!(b.len(), 1, "mode {mode} {wrap:?}: the box is drawn once: {b:?}");
            assert_eq!(b[0].0, 0, "mode {mode} {wrap:?}: on the break's page: {b:?}");
            let next = para_at(&l, 3, 0).unwrap();
            assert_eq!(next.0, 1, "mode {mode} {wrap:?}");
            assert!((next.1 - want.1).abs() < 0.5, "mode {mode} {wrap:?}: NEXT starts the page as without the box: {next:?} {want:?}");
        }
    }
}

#[test]
fn a_box_that_does_not_fit_moves_to_the_next_page_with_its_line() {
    let (per_page, top) = page_shape();
    for mode in [11, 15] {
        // Six lines of room left: far less than the box's 361pt.
        let lines = per_page - 6;
        let l = lay_out(&doc(lines, &[Piece::Break, Piece::Box(361.0, TB)], mode));
        let b = boxes(&l);
        assert_eq!(b.len(), 1, "mode {mode}: the box is drawn once: {b:?}");
        assert_eq!(b[0].0, 1, "mode {mode}: on page 2: {b:?}");
        assert!(b[0].1 < top + 3.0, "mode {mode}: at the top of the page: {b:?} (body top {top})");
        assert_eq!(l.pages.len(), 3, "mode {mode}: the break below the box ends page 2");
        assert_eq!(para_at(&l, lines - 1, 0).map(|p| p.0), Some(0), "mode {mode}: the line before stays on page 1");
        let next = para_at(&l, lines + 1, 0).unwrap();
        assert_eq!(next.0, 2, "mode {mode}: NEXT starts page 3");
        assert!((next.1 - top).abs() < 0.5, "mode {mode}: at its top: {next:?}");
    }
}

#[test]
fn text_after_the_break_starts_below_a_top_and_bottom_box() {
    let (per_page, top) = page_shape();
    for mode in [11, 15] {
        let lines = per_page - 6;
        for after in ["AFTER", " "] {
            let l = lay_out(&doc(lines, &[Piece::Break, Piece::Box(361.0, TB), Piece::Text(after)], mode));
            assert_eq!(l.pages.len(), 2, "mode {mode} {after:?}");
            let b = boxes(&l);
            assert_eq!(b.len(), 1, "mode {mode} {after:?}: {b:?}");
            assert_eq!(b[0].0, 1, "mode {mode} {after:?}: the box is on page 2: {b:?}");
            assert!(b[0].1 < top + 3.0, "mode {mode} {after:?}: at the top of the page: {b:?}");
            let line = para_at(&l, lines, 1).expect("the line after the break is placed");
            assert_eq!(line.0, 1, "mode {mode} {after:?}");
            assert!(line.1 >= b[0].2 - 0.5, "mode {mode} {after:?}: the line after the break is below the box: {line:?} {b:?}");
            let next = para_at(&l, lines + 1, 0).unwrap();
            assert_eq!(next.0, 1, "mode {mode} {after:?}");
            assert!(next.1 > line.1 + 5.0, "mode {mode} {after:?}: NEXT is a line further down: {next:?} {line:?}");
        }
    }
}

/// Guard (passes before the change): a top-and-bottom box that fits with its line stays on the
/// break's page.
#[test]
fn a_box_that_fits_stays_on_the_breaks_page() {
    let l = lay_out(&doc(2, &[Piece::Box(100.0, TB), Piece::Break], 15));
    assert_eq!(l.pages.len(), 2);
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 0, "{b:?}");
    assert_eq!(para_at(&l, 3, 0).map(|p| p.0), Some(1));
}
