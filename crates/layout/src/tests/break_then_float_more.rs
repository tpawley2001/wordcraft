//! More cases of a floating object anchored in a paragraph with a break (see
//! `tests/break_then_float.rs` for the measured ones): a column break (`w:br w:type="column"`)
//! follows the same rules into the next column, and a square-wrapped object after a page break
//! with text after it keeps the page count it had.

use crate::*;
use wordcraft_doc::para::{COLUMN_BREAK, PAGE_BREAK, ShapeKind};
use wordcraft_doc::section::Columns;
use wordcraft_doc::{Pos, StoryRef};

fn lay(doc: &Document) -> DocLayout {
    crate::test_fonts();
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// `lines` one-line paragraphs, then one holding `brk`, a 200pt wide text box of height `h`
/// wrapped `wrap` at offset 0 from its paragraph and `after`, then "NEXT" and "LAST".
fn doc(lines: usize, brk: char, h: f32, wrap: Wrap, after: &str) -> Document {
    let mut text: Vec<String> = (1..=lines).map(|i| format!("L{i}")).collect();
    text.extend([String::new(), "NEXT".into(), "LAST".into()]);
    let mut d = Document::from_text(&text.join("\n"));
    d.insert_text(&Pos::body(lines, 0), &brk.to_string(), &Default::default()).unwrap();
    let id = d.add_part(wordcraft_doc::PartKind::TextBox, vec![wordcraft_doc::para_block(Paragraph::with_text("", Default::default()))]);
    let float = Float { wrap, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 0.0, y: 0.0, dist: 0.0, ..Default::default() };
    let shape = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w: 200.0,
        h,
        fill: None,
        stroke: None,
        stroke_width: 0.75,
        float,
        story: Some(id),
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    };
    let off = brk.len_utf8();
    d.insert_object(&Pos::body(lines, off), shape, &Default::default()).unwrap();
    if !after.is_empty() {
        let off = d.body.get(lines).and_then(|b| b.as_para()).map_or(0, |p| p.text.len());
        d.insert_text(&Pos::body(lines, off), after, &Default::default()).unwrap();
    }
    d
}

/// Page, left and top of the first piece of body paragraph `i` from line `from` on.
fn para_at(l: &DocLayout, i: usize, from: usize) -> Option<(usize, f32, f32)> {
    l.pages.iter().enumerate().find_map(|(pi, p)| {
        p.items.iter().find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, x, y, l1, .. } if path.0 == [i as u32] && *l1 > from => Some((pi, *x, *y)),
            _ => None,
        })
    })
}

fn boxes(l: &DocLayout) -> Vec<(usize, Rect)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Shape { rect, kind: ShapeKind::TextBox, .. } = it {
                out.push((pi, *rect));
            }
        }
    }
    out
}

#[test]
fn an_anchor_after_a_column_break_starts_no_line_in_the_next_column() {
    for wrap in [Wrap::TopAndBottom, Wrap::Square] {
        let mut d = doc(2, COLUMN_BREAK, 100.0, wrap, "");
        d.last_section.columns = Columns { count: 2, space: 36.0, separator: false, widths: Vec::new() };
        let l = lay(&d);
        assert_eq!(l.pages.len(), 1, "{wrap:?}");
        let first = para_at(&l, 0, 0).unwrap();
        let b = boxes(&l);
        assert_eq!(b.len(), 1, "{wrap:?}: the box is drawn once: {b:?}");
        assert!(b[0].1.x < first.1 + 1.0, "{wrap:?}: in the first column: {b:?}");
        let next = para_at(&l, 3, 0).unwrap();
        assert!(next.1 > first.1 + 100.0, "{wrap:?}: NEXT is in the second column: {next:?}");
        assert!((next.2 - first.2).abs() < 0.5, "{wrap:?}: at its top, with no empty line above: {next:?} {first:?}");
    }
}

#[test]
fn text_after_the_break_beside_a_square_box_keeps_its_place() {
    let l = lay(&doc(2, PAGE_BREAK, 100.0, Wrap::Square, "AFTER"));
    assert_eq!(l.pages.len(), 2);
    let b = boxes(&l);
    assert_eq!(b.len(), 1, "{b:?}");
    assert_eq!(b[0].0, 1, "with the text after the break: {b:?}");
    let line = para_at(&l, 2, 1).unwrap();
    assert_eq!(line.0, 1);
    // Beside the box, not below it.
    assert!(line.2 < b[0].1.bottom() - 1.0, "{line:?} {b:?}");
    let next = para_at(&l, 3, 0).unwrap();
    assert_eq!(next.0, 1);
}
