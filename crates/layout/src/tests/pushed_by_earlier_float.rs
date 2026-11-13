//! An empty paragraph that ends a section (`w:sectPr` in its `w:pPr`, ECMA-376 Part 1 §17.6.17)
//! or a line holding only a page break, pushed down past the page foot by a top-and-bottom
//! text box (`wp:wrapTopAndBottom`) anchored in an EARLIER paragraph, stays at the foot of its
//! page as Word lays it out: it has no object of its own to move with, so a next-page section
//! after it starts the next page and no blank page appears in between. Only a paragraph that
//! anchors the top-and-bottom object itself goes to the next page with it (see
//! `tests/break_then_float.rs`). Test fonts: these tests assert pages and order, not points.

use super::*;
use wordcraft_doc::para::{Anchor, Float, PAGE_BREAK, Wrap};
use wordcraft_doc::section::SectionStart;

#[derive(Clone, Copy, PartialEq)]
enum Tail {
    /// C carries a continuous section break; the last section starts with `start`.
    Section(SectionStart),
    /// C holds only a page break.
    Break,
}

/// `lines` one-line paragraphs, then A ("A", anchoring a top-and-bottom text box `h` tall at `y`
/// below the paragraph when `h > 0`), then C (empty), then "NEXT". A's index is `lines`.
fn doc(lines: usize, y: f32, h: f32, tail: Tail) -> Document {
    let mut text: Vec<String> = (1..=lines).map(|i| format!("L{i}")).collect();
    text.extend(["A".into(), String::new(), "NEXT".into()]);
    let mut d = Document::from_text(&text.join("\n"));
    if h > 0.0 {
        let float = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 0.0, y, dist: 0.0, ..Default::default() };
        text_box_at(&mut d, &Pos::body(lines, 1), "", 200.0, h, float);
    }
    match tail {
        Tail::Section(start) => {
            d.para_mut(StoryRef::Body, &Path::top(lines + 1)).unwrap().section =
                Some(Box::new(SectionProps { start: SectionStart::Continuous, ..SectionProps::default() }));
            d.last_section.start = start;
        }
        Tail::Break => {
            d.insert_text(&Pos::body(lines + 1, 0), &PAGE_BREAK.to_string(), &Default::default()).unwrap();
        }
    }
    d
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

/// Page and top of the first placed lines of body paragraph `i`.
fn para_at(l: &DocLayout, i: usize) -> Option<(usize, f32)> {
    l.pages.iter().enumerate().find_map(|(pi, p)| {
        p.items.iter().find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, y, .. } if path.0 == [i as u32] => Some((pi, *y)),
            _ => None,
        })
    })
}

/// The number of one-line paragraphs before A, the box's offset below A and its height: A
/// lands about three lines from the foot of page 1, the box starts one line below A's top and
/// fits on the page, and C, pushed below the box, runs half a line past the page foot.
fn shape() -> (usize, f32, f32) {
    let probe = lay_out(&doc(100, 0.0, 0.0, Tail::Break));
    let per_page = probe.pages[0].items.iter().filter(|it| matches!(it, Placed::Lines { story: StoryRef::Body, .. })).count();
    assert!(per_page > 10, "a page holds {per_page} lines");
    let lines = per_page - 4;
    let l = lay_out(&doc(lines, 0.0, 0.0, Tail::Break));
    let a = para_at(&l, lines).unwrap();
    let c = para_at(&l, lines + 1).unwrap();
    assert_eq!((a.0, c.0), (0, 0));
    let line = c.1 - a.1;
    let foot = l.pages[0].h - 72.0;
    let room = foot - (a.1 + line);
    assert!(room > 2.0 * line, "room {room} line {line}");
    (lines, line, room - line / 2.0)
}

/// Pages that hold no body text.
fn blank_pages(l: &DocLayout) -> Vec<usize> {
    (0..l.pages.len()).filter(|&pi| !l.pages[pi].items.iter().any(|it| matches!(it, Placed::Lines { story: StoryRef::Body, .. }))).collect()
}

/// Every text box drawn: its page and bottom.
fn boxes(l: &DocLayout) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Shape { rect, kind: wordcraft_doc::para::ShapeKind::TextBox, .. } = it {
                out.push((pi, rect.y + rect.h));
            }
        }
    }
    out
}

/// Row 1 (fails before the change: three pages, page 2 blank): an empty paragraph with a
/// continuous section break, pushed down by an earlier paragraph's box, stays on page 1; the
/// next-page section starts page 2.
#[test]
fn a_section_break_pushed_by_an_earlier_box_stays_at_the_page_foot() {
    let (lines, y, h) = shape();
    let l = lay_out(&doc(lines, y, h, Tail::Section(SectionStart::NextPage)));
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 0, "the box fits on page 1: {b:?}");
    let c = para_at(&l, lines + 1).unwrap();
    assert_eq!(c.0, 0, "C stays on page 1: {c:?}");
    assert!(c.1 >= b[0].1 - 0.5, "C is pushed below the box: {c:?} {b:?}");
    assert_eq!(para_at(&l, lines + 2).map(|p| p.0), Some(1), "NEXT starts page 2");
    assert_eq!(l.pages.len(), 2);
    assert_eq!(blank_pages(&l), Vec::<usize>::new());
}

/// Row 2: with a continuous last section the layout is what it was before the change: two
/// pages, C at the foot of page 1 and NEXT at the top of page 2 (measured by running this
/// document at the change's base commit; the same document without the box fits on one page,
/// so that copy cannot stand in for it).
#[test]
fn before_a_continuous_section_the_break_moves_on_as_before() {
    let (lines, y, h) = shape();
    let without = lay_out(&doc(lines, y, 0.0, Tail::Section(SectionStart::Continuous)));
    assert_eq!(without.pages.len(), 1);
    let l = lay_out(&doc(lines, y, h, Tail::Section(SectionStart::Continuous)));
    assert_eq!(l.pages.len(), 2);
    assert_eq!(para_at(&l, lines + 1).map(|p| p.0), Some(0), "C stays on page 1");
    assert_eq!(para_at(&l, lines + 2).map(|p| p.0), Some(1), "NEXT starts page 2");
    assert_eq!(blank_pages(&l), Vec::<usize>::new());
}

/// Row 3: a line holding only a page break, pushed down by an earlier paragraph's box, stays on
/// page 1 and NEXT starts page 2.
#[test]
fn a_page_break_pushed_by_an_earlier_box_stays_at_the_page_foot() {
    let (lines, y, h) = shape();
    let l = lay_out(&doc(lines, y, h, Tail::Break));
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 0, "{b:?}");
    assert_eq!(para_at(&l, lines + 1).map(|p| p.0), Some(0), "the break stays on page 1");
    assert_eq!(para_at(&l, lines + 2).map(|p| p.0), Some(1), "NEXT starts page 2");
    assert_eq!(l.pages.len(), 2);
    assert_eq!(blank_pages(&l), Vec::<usize>::new());
}
