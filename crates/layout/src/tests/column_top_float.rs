//! A paragraph that runs on from one column into the next, where a Square-wrapped floating
//! object fills the top of that next column: Word starts the continued lines below the object,
//! as it does for a paragraph that starts there, and the column holds only what fits below it
//! (Microsoft 365 Word's PDF output of two-column documents; a 200 pt high object at the top of
//! column 2 puts the first line's top at its bottom and leaves 32 of the column's 46 lines).

use super::{assert_contiguous, column_wrap_doc, lay, page_shape, shown_and_ranges};
#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::{Path, Pos, StoryRef};

/// Every body line drawn on page `page` in the column at `col_x`: (block, left, top, bottom).
fn column_lines(l: &DocLayout, page: usize, col_x: f32) -> Vec<(u32, f32, f32, f32)> {
    let mut v = Vec::new();
    for it in &l.pages[page].items {
        let Placed::Lines { story: StoryRef::Body, path, para, l0, l1, x, y, .. } = it else { continue };
        if (x - col_x).abs() > 1.0 {
            continue;
        }
        let first = &para.lines[*l0];
        for line in &para.lines[*l0..*l1] {
            let top = y + (line.top - first.top);
            v.push((path.0[0], x + line.left, top, top + line.height));
        }
    }
    v.sort_by(|a, b| a.2.total_cmp(&b.2));
    v
}

/// The page x of column 2 and the body's top and bottom on page 1.
fn column_2(d: &Document, l: &DocLayout) -> (f32, f32, f32) {
    let body = l.pages[0].body;
    (body.x + d.last_section.column_boxes()[1].0, body.y, body.bottom())
}

/// A rectangle as wide as column 2 (and a little more), `h` high, `down` below the body's top.
/// The geometry comes from a layout of the same document without the shape.
fn over_column_2(words: usize, down: f32, h: f32) -> Rect {
    let plain = column_wrap_doc(2, None, words);
    let l = lay(&plain);
    let (x, top, _) = column_2(&plain, &l);
    let w = plain.last_section.column_boxes()[1].1;
    Rect::new(x - 1.0, top + down, w + 2.0, h)
}

/// The paragraph's text is all placed, in order.
fn assert_whole(d: &Document, l: &DocLayout, block: u32) {
    let (_, ranges) = shown_and_ranges(l, block);
    assert_contiguous(&ranges, d.para(StoryRef::Body, &Path::top(block as usize)).map(|p| p.len()).unwrap_or(0));
}

/// Column 2's lines start at the bottom of `r` (not above it, not through it).
fn assert_starts_below(d: &Document, l: &DocLayout, r: Rect, block: u32) {
    let (x, ..) = column_2(d, l);
    let lines = column_lines(l, 0, x);
    assert!(!lines.is_empty(), "the text runs on into column 2");
    assert!(lines.iter().all(|&(b, ..)| b == block), "column 2 holds the continued paragraph: {lines:?}");
    for &(_, left, top, b) in &lines {
        assert!(top >= r.bottom() - 0.01, "the line at {left}, {top}..{b} is drawn through the shape {r:?}");
    }
    let first = lines[0].2;
    assert!((first - r.bottom()).abs() < 0.5, "the first line of column 2 starts at the shape's bottom {}: {first}", r.bottom());
}

#[test]
fn a_paragraph_continued_into_a_column_starts_below_a_shape_filling_its_top() {
    crate::test_fonts();
    // Word (probe a): the first line of column 2 has its top at the rectangle's bottom.
    let r = over_column_2(900, 0.0, 200.0);
    let d = column_wrap_doc(2, Some(page_shape(r.x, r.y, r.w, r.h)), 900);
    let l = lay(&d);
    assert_whole(&d, &l, 1);
    assert_starts_below(&d, &l, r, 1);
}

#[test]
fn the_column_then_holds_only_the_lines_that_fit_below_the_shape() {
    crate::test_fonts();
    // Word (probes a and e): 46 lines in a free column, 32 below a 200 pt rectangle: the room
    // left, in whole lines. Here: as many lines as fit between the shape's bottom and the body's.
    let r = over_column_2(900, 0.0, 200.0);
    let plain = column_wrap_doc(2, None, 900);
    let lp = lay(&plain);
    let (x, top, bottom) = column_2(&plain, &lp);
    let free = column_lines(&lp, 0, x);
    assert!(free.len() > 20 && (free[0].2 - top).abs() < 0.5, "without a shape column 2 is full from its top: {}", free.len());
    let pitch = free[1].2 - free[0].2;
    let d = column_wrap_doc(2, Some(page_shape(r.x, r.y, r.w, r.h)), 900);
    let l = lay(&d);
    let lines = column_lines(&l, 0, x);
    let fit = ((bottom - r.bottom() + 0.01) / pitch).floor() as usize;
    assert_eq!(lines.len(), fit, "lines of {pitch} pt between {} and {bottom}", r.bottom());
    assert!(lines.len() < free.len());
    assert!(l.pages.len() >= 2, "what no longer fits goes on to the next page");
    assert_whole(&d, &l, 1);
}

#[test]
fn a_shape_anchored_in_the_running_paragraph_itself_is_cleared_the_same_way() {
    crate::test_fonts();
    // Word (probe f): the rectangle is anchored at the start of the long paragraph.
    let r = over_column_2(900, 0.0, 200.0);
    let mut d = column_wrap_doc(2, None, 900);
    d.insert_object(&Pos::body(1, 0), page_shape(r.x, r.y, r.w, r.h), &Default::default()).unwrap();
    let l = lay(&d);
    assert_whole(&d, &l, 1);
    assert_starts_below(&d, &l, r, 1);
}

#[test]
fn a_shape_lower_in_the_column_is_jumped_over_and_the_top_lines_stay() {
    crate::test_fonts();
    // Word (probe d): column 2 starts at its top; the lines go on below the rectangle.
    let r = over_column_2(900, 128.0, 200.0);
    let d = column_wrap_doc(2, Some(page_shape(r.x, r.y, r.w, r.h)), 900);
    let l = lay(&d);
    assert_whole(&d, &l, 1);
    let (x, top, _) = column_2(&d, &l);
    let lines = column_lines(&l, 0, x);
    assert!(!lines.is_empty() && (lines[0].2 - top).abs() < 0.5, "column 2 starts at its top: {lines:?}");
    let (mut above, mut below) = (0, 0);
    for &(_, _, t, b) in &lines {
        assert!(b <= r.y + 0.01 || t >= r.bottom() - 0.01, "the line at {t}..{b} is drawn through the shape {r:?}");
        if b <= r.y + 0.01 { above += 1 } else { below += 1 }
    }
    assert!(above > 0 && below > 0, "lines above ({above}) and below ({below}) the shape");
}

#[test]
fn paragraphs_that_start_in_the_column_begin_below_the_shape() {
    crate::test_fonts();
    // Word (probe b): one-line paragraphs; the first one in column 2 has its top at the
    // rectangle's bottom.
    let r = over_column_2(900, 0.0, 200.0);
    let text: Vec<String> = (0..79).map(|i| format!("line{i}")).collect();
    let mut d = Document::from_text(&format!("Anchor\n{}", text.join("\n")));
    d.last_section.columns.count = 2;
    d.insert_object(&Pos::body(0, 0), page_shape(r.x, r.y, r.w, r.h), &Default::default()).unwrap();
    let l = lay(&d);
    let (x, ..) = column_2(&d, &l);
    let lines = column_lines(&l, 0, x);
    assert!(!lines.is_empty(), "the paragraphs run on into column 2");
    for &(_, _, t, b) in &lines {
        assert!(t >= r.bottom() - 0.01, "the line at {t}..{b} is clear of the shape {r:?}");
    }
    assert!((lines[0].2 - r.bottom()).abs() < 0.5, "the first starts at the shape's bottom: {}", lines[0].2);
}
