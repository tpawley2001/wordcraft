//! A floating object with text wrapping, anchored in a section whose columns are balanced
//! because a continuous section break follows, in compatibility mode below 15 (ECMA-376 Part 1
//! §17.15.3.4 `compatibilityMode`; every .doc): the object keeps the place the section's first,
//! unbalanced layout gives it, balancing moves only the text around it, and the next section
//! starts below the balanced text, wrapping around the object where it overlaps it.
//!
//! Expected values are line tops measured in Microsoft 365 Word's PDF output of small documents
//! (US Letter, left and right margins 90 pt, Times New Roman 12 pt with 13.8 pt lines, no
//! paragraph spacing; the test fonts' lines are scaled to that pitch). Section 1 has 2 equal columns 36 pt apart (x 90 and 324) and an 85 x 58.6
//! pt object wrapped top and bottom, at 0 from its column and its paragraph. Section 2 is
//! continuous with columns of 120 and 276 pt (x 90 and 246): `b0`..`b3`, then `c0` after a
//! column break, then `c1`. A third section starts on a new page.

use super::*;
use wordcraft_doc::para::{Anchor, COLUMN_BREAK, Float, ShapeKind, Wrap};
use wordcraft_doc::para_block;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::section::{Columns, SectionStart};

const TOL: f32 = 0.6;

fn chr() -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str) -> Paragraph {
    let mut p = if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, chr()) };
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    // Word's single-spaced 13.8 pt: the test fonts' single spacing is taller, so a multiple below
    // single shrinks it to that (the whole line box, as text wrapping measures it too).
    p.props.line_spacing = Some(LineSpacing::Multiple(13.8 / single_pitch()));
    p.mark = chr();
    p
}

/// The test fonts' single-spaced line pitch for [`chr`].
fn single_pitch() -> f32 {
    static PITCH: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *PITCH.get_or_init(|| {
        let single = |t: &str| {
            let mut p = Paragraph::with_text(t, chr());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            p
        };
        let mut d = Document::new();
        d.body = [single("X"), single("Y")].into_iter().map(para_block).collect();
        let l = lay_out(&d);
        l.caret(&Pos::body(1, 0)).unwrap().top - l.caret(&Pos::body(0, 0)).unwrap().top
    })
}

fn sect(start: SectionStart, columns: Columns) -> SectionProps {
    SectionProps { start, margin_left: 90.0, margin_right: 90.0, columns, ..SectionProps::default() }
}

fn object() -> InlineObject {
    let float = Float {
        wrap: Wrap::TopAndBottom,
        h_rel: Anchor::Column,
        v_rel: Anchor::Paragraph,
        x: 0.0,
        y: 0.0,
        dist: 0.0,
        dist_top: 0.0,
        dist_bottom: 0.0,
        ..Default::default()
    };
    InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w: 85.0,
        h: 58.6,
        fill: None,
        stroke: None,
        stroke_width: 0.0,
        float,
        story: None,
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

/// The fixture: section 1 `first` (the object anchored at the start of paragraph `anchor`, with
/// `cols` equal columns), section 2 as described above, then a one-paragraph third section.
fn doc(first: &[&str], anchor: usize, cols: u32, mode: u32) -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    let mut body: Vec<Paragraph> = first.iter().map(|t| para(t)).collect();
    if let Some(p) = body.last_mut() {
        p.section = Some(Box::new(sect(SectionStart::NextPage, Columns { count: cols, space: 36.0, ..Columns::default() })));
    }
    let second = ["b0", "b1", "b2", "b3"].map(para);
    body.extend(second);
    body.push(para(&format!("{COLUMN_BREAK}c0")));
    let mut c1 = para("c1");
    c1.section = Some(Box::new(sect(
        SectionStart::Continuous,
        Columns { count: 2, space: 36.0, widths: vec![(120.0, 36.0), (276.0, 0.0)], ..Columns::default() },
    )));
    body.push(c1);
    body.push(para("z"));
    d.body = body.into_iter().map(para_block).collect();
    d.last_section = sect(SectionStart::NextPage, Columns::default());
    d.insert_object(&Pos::body(anchor, 0), object(), &chr()).unwrap();
    d
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

/// (x, top) of every line of body paragraph `block` on the first page, in order.
fn lines(l: &DocLayout, block: usize) -> Vec<(f32, f32)> {
    let mut v = Vec::new();
    for it in &l.pages[0].items {
        let Placed::Lines { story: StoryRef::Body, path, para, l0, l1, x, y, .. } = it else { continue };
        if path.0 != [block as u32] {
            continue;
        }
        let first = &para.lines[*l0];
        for line in &para.lines[*l0..*l1] {
            v.push((*x, y + (line.top - first.top)));
        }
    }
    v
}

/// The (x, top) of the line of `block` holding its text: its last line (`c0` follows a column
/// break, which may leave an empty line before it).
fn at(l: &DocLayout, block: usize) -> (f32, f32) {
    *lines(l, block).last().unwrap_or_else(|| panic!("paragraph {block} is laid out on page 1"))
}

fn objects(l: &DocLayout) -> Vec<Rect> {
    l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Object { rect, story: StoryRef::Body, .. } => Some(*rect),
            _ => None,
        })
        .collect()
}

fn check(l: &DocLayout, name: &str, block: usize, want: (f32, f32)) {
    let got = at(l, block);
    assert!((got.0 - want.0).abs() <= 1.0 && (got.1 - want.1).abs() <= TOL, "{name}: at {got:?}, Word {want:?}");
}

fn check_object(l: &DocLayout, want: (f32, f32)) {
    let objs = objects(l);
    assert_eq!(objs.len(), 1, "one object drawn on page 1: {objs:?}");
    let r = objs[0];
    assert!((r.x - want.0).abs() <= 1.0 && (r.y - want.1).abs() <= TOL, "object at {r:?}, Word {want:?}");
}

const A: [&str; 4] = ["a0", "a1", "a2", "a3"];

/// Section 2's paragraphs `b0`.. start after section 1's `n` paragraphs.
fn check_second(l: &DocLayout, n: usize, b: [f32; 4], c: [f32; 2]) {
    for (k, y) in b.iter().enumerate() {
        check(l, &format!("b{k}"), n + k, (90.0, *y));
    }
    for (k, y) in c.iter().enumerate() {
        check(l, &format!("c{k}"), n + 4 + k, (246.0, *y));
    }
}

#[test]
fn pinned_object_stays_in_first_column_and_text_balances_around_it() {
    let l = lay_out(&doc(&A, 2, 2, 11));
    for (k, want) in [(90.0, 72.0), (90.0, 85.8), (324.0, 72.0), (324.0, 85.8)].into_iter().enumerate() {
        check(&l, A[k], k, want);
    }
    check_object(&l, (90.0, 99.6));
    check_second(&l, 4, [158.2, 172.0, 185.8, 199.6], [99.6, 113.4]);
}

#[test]
fn empty_cover_paragraphs_start_the_next_section_two_lines_down() {
    let l = lay_out(&doc(&["", "", "", ""], 2, 2, 11));
    check_object(&l, (90.0, 99.6));
    check_second(&l, 4, [158.2, 172.0, 185.8, 199.6], [99.6, 113.4]);
}

#[test]
fn empty_last_paragraph_with_the_section_break() {
    let l = lay_out(&doc(&["a0", "a1", "a2", ""], 2, 2, 11));
    for (k, want) in [(90.0, 72.0), (90.0, 85.8), (324.0, 72.0)].into_iter().enumerate() {
        check(&l, A[k], k, want);
    }
    check_object(&l, (90.0, 99.6));
    check_second(&l, 4, [158.2, 172.0, 185.8, 199.6], [99.6, 113.4]);
}

#[test]
fn object_in_the_first_paragraph_sends_every_line_to_the_second_column() {
    let l = lay_out(&doc(&A, 0, 2, 11));
    for (k, want) in [(324.0, 72.0), (324.0, 85.8), (324.0, 99.6), (324.0, 113.4)].into_iter().enumerate() {
        check(&l, A[k], k, want);
    }
    check_object(&l, (90.0, 72.0));
    check_second(&l, 4, [130.6, 144.4, 158.2, 172.0], [127.2, 141.0]);
}

#[test]
fn object_below_the_balanced_text_still_wraps_the_next_section() {
    let eight = ["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"];
    let l = lay_out(&doc(&eight, 5, 2, 11));
    for (k, name) in eight.iter().enumerate() {
        let x = if k < 4 { 90.0 } else { 324.0 };
        check(&l, name, k, (x, 72.0 + 13.8 * (k % 4) as f32));
    }
    check_object(&l, (90.0, 141.0));
    check_second(&l, 8, [127.2, 199.6, 213.4, 227.2], [127.2, 141.0]);
}

#[test]
fn single_column_section_is_unchanged() {
    let l = lay_out(&doc(&A, 2, 1, 11));
    for (k, y) in [72.0, 85.8, 158.2, 172.0].into_iter().enumerate() {
        check(&l, A[k], k, (90.0, y));
    }
    check_object(&l, (90.0, 99.6));
    check_second(&l, 4, [185.8, 199.6, 213.4, 227.2], [185.8, 199.6]);
}

#[test]
fn mode_15_layout_is_unchanged() {
    // From mode 15 the object goes with its paragraph in each balancing pass, as before.
    let l = lay_out(&doc(&A, 2, 2, 15));
    for (k, want) in [(90.0, 72.0), (90.0, 85.8), (324.0, 130.6), (324.0, 144.4)].into_iter().enumerate() {
        check(&l, A[k], k, want);
    }
    check_object(&l, (324.0, 72.0));
    check(&l, "b0", 4, (90.0, 158.2));
    check(&l, "c0", 8, (246.0, 158.2));
}

#[test]
fn object_is_drawn_once() {
    let l = lay_out(&doc(&A, 2, 2, 11));
    assert_eq!(objects(&l).len(), 1);
    let shapes = l.pages.iter().flat_map(|p| &p.items).filter(|it| matches!(it, Placed::Shape { kind: ShapeKind::Rectangle, .. })).count();
    assert_eq!(shapes, 1, "the rectangle is drawn once");
}
