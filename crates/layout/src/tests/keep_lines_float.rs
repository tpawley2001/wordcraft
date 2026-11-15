//! Before compatibility mode 15 a paragraph with Keep lines together (`w:keepLines`, ECMA-376
//! Part 1 §17.3.1.15) whose full-width lines would reach a floating object with Square wrapping
//! (`wp:wrapSquare`, §20.4.2.17) starts beside it: its first line goes to the top of the first
//! line that meets the object (or to the object's top, when that is inside the first line), the
//! room above stays empty and every line of the paragraph wraps. The object does not move. In
//! mode 15, and without Keep lines together (Keep with next alone does not count), the lines
//! above the object stay where they are at full width.
//!
//! Measured in Microsoft 365 Word with small probe documents (US Letter, 1 inch margins, Times
//! New Roman 12, 13.8 pt lines; a 72 x 300 pt Square object at the column's left edge, `t` pt
//! below its paragraph's top): in modes 11, 12 and 14 `t` = 2, 5, 10, 13 put the first line at
//! the object's top, `t` = 20 and 25 one line down, 30 two lines down, 45 none (unless the space
//! after reaches it). Word snaps some offsets to 0.75 pt (9.7 for 10); we use the exact top. The
//! tests use the test fonts, so they measure the line height `h` instead of assuming 13.8.
//!
//! Not measured, decided as follows: Tight and Through wrapping and a nonzero wrap distance use the
//! wrap area's top, as Square does; with several objects the first line that meets any of them
//! decides, once; floating tables are left out; a paragraph that moves to the next page because
//! its taller height no longer fits takes its objects along (they are positioned from where it
//! starts), as any kept paragraph does.

use super::lay;
#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::para::{Anchor, Float, ShapeKind, Wrap};
use wordcraft_doc::props::{Align, LineSpacing};
use wordcraft_doc::{Block, InlineObject, Pos, StoryRef};

const TITLE: &str = "TITLE LINE ONE\nsecond line of the title\nthird line here";
/// The column's width (US Letter, 1 inch margins).
const COL_W: f32 = 468.0;
const OBJ_W: f32 = 72.0;
const OBJ_H: f32 = 300.0;

#[derive(Clone, Copy, PartialEq)]
enum Where {
    /// In the paragraph under test, before its text.
    Start,
    /// In the paragraph under test, after its last character.
    End,
    /// In the paragraph before it, positioned from that paragraph's top.
    Before,
}

#[derive(Clone, Copy)]
struct Probe {
    mode: u32,
    keep_lines: bool,
    keep_next: bool,
    /// Lines of the paragraph under test (1 to 3).
    lines: usize,
    /// The object's top below the paragraph's top; `None`: no object.
    t: Option<f32>,
    wrap: Wrap,
    right: bool,
    anchor: Where,
    space_after: f32,
    /// Plain paragraphs before the one under test.
    before: usize,
    /// Plain paragraphs after it.
    after: usize,
    obj_h: f32,
}

impl Probe {
    fn new(mode: u32, keep_lines: bool, lines: usize, t: f32) -> Self {
        Probe {
            mode,
            keep_lines,
            keep_next: false,
            lines,
            t: Some(t),
            wrap: Wrap::Square,
            right: false,
            anchor: Where::Start,
            space_after: 0.0,
            before: 1,
            after: 10,
            obj_h: OBJ_H,
        }
    }
}

fn shape(p: &Probe, y: f32) -> InlineObject {
    let x = if p.right { COL_W - OBJ_W } else { 0.0 };
    let float = Float {
        wrap: p.wrap,
        h_rel: Anchor::Column,
        v_rel: Anchor::Paragraph,
        x,
        y,
        dist: 0.0,
        dist_top: 0.0,
        dist_bottom: 0.0,
        ..Default::default()
    };
    InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w: OBJ_W,
        h: p.obj_h,
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

/// `p.before` plain paragraphs, the paragraph under test `T` (block `p.before`), then `p.after`
/// one-word paragraphs; single spacing, no paragraph spacing. `h_before`: the height of a plain
/// paragraph, for an object anchored in the one before `T`.
fn doc(p: &Probe, h_before: f32) -> Document {
    crate::test_fonts();
    let mut text: Vec<String> = (0..p.before).map(|i| if i + 1 == p.before { "Before".into() } else { format!("Line {i}") }).collect();
    text.push(String::new());
    text.extend((0..p.after).map(|i| format!("Word{i}")));
    let mut d = Document::from_text(&text.join("\n"));
    let t_block = p.before;
    let title = TITLE.split('\n').take(p.lines.max(1)).collect::<Vec<_>>().join("\n");
    d.insert_text(&Pos::body(t_block, 0), &title, &Default::default()).unwrap();
    for (i, block) in d.body.iter_mut().enumerate() {
        if let Block::Para(para) = Arc::make_mut(block) {
            para.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            para.props.space_before = Some(0.0);
            para.props.space_after = Some(0.0);
            if i == t_block {
                para.props.align = Some(Align::Center);
                para.props.keep_lines = Some(p.keep_lines);
                para.props.keep_next = Some(p.keep_next);
                para.props.space_after = Some(p.space_after);
            }
        }
    }
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 72.0, 72.0, 72.0);
    d.settings.compat_mode = p.mode;
    if let Some(t) = p.t {
        match p.anchor {
            Where::Start => d.insert_object(&Pos::body(t_block, 0), shape(p, t), &Default::default()).unwrap(),
            Where::End => {
                let end = title.len();
                d.insert_object(&Pos::body(t_block, end), shape(p, t), &Default::default()).unwrap()
            }
            Where::Before => d.insert_object(&Pos::body(t_block - 1, 0), shape(p, h_before + t), &Default::default()).unwrap(),
        };
    }
    d
}

/// Where a paragraph's lines are placed: (page, page top of each line, the paragraph's layout).
struct Placement {
    page: usize,
    tops: Vec<f32>,
    para: Arc<ParaLayout>,
}

fn placed(l: &DocLayout, block: usize) -> Placement {
    for (pi, page) in l.pages.iter().enumerate() {
        for it in &page.items {
            let Placed::Lines { story: StoryRef::Body, path, para, l0, l1, y, .. } = it else { continue };
            if path.0 == [block as u32] {
                let first = para.lines[*l0].top;
                let tops = para.lines[*l0..*l1].iter().map(|line| y + line.top - first).collect();
                return Placement { page: pi, tops, para: para.clone() };
            }
        }
    }
    panic!("paragraph {block} is not placed");
}

/// The object's rectangle as drawn (page, rect).
fn object(l: &DocLayout) -> (usize, Rect) {
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

/// The height of a plain line and of the title's first line, laid out without an object.
fn heights() -> (f32, f32) {
    let mut p = Probe::new(11, true, 3, 0.0);
    p.t = None;
    let l = lay(&doc(&p, 0.0));
    let before = placed(&l, 0);
    let t = placed(&l, 1);
    (before.para.height, t.para.lines[0].height)
}

struct Outcome {
    /// The paragraph's top `P` and the offset of its first line below it.
    p: f32,
    offset: f32,
    /// For each line of `T`: narrowed by the object (text beside it).
    beside: Vec<bool>,
    /// The page top of the paragraph after `T`.
    next: f32,
    t_page: usize,
    layout: DocLayout,
}

fn run(p: &Probe) -> Outcome {
    let (h_before, _) = heights();
    let l = lay(&doc(p, h_before));
    let before = placed(&l, p.before - 1);
    let t = placed(&l, p.before);
    let t_page = t.page;
    // The paragraph's top: right below the one before it, on the same page; else the body top.
    let top = if before.page == t.page { before.tops[0] + before.para.height } else { 72.0 };
    let beside = t
        .para
        .lines
        .iter()
        .map(|line| {
            let narrowed = if p.right { line.right < COL_W - OBJ_W + 0.5 } else { line.left > OBJ_W - 0.5 };
            if !narrowed {
                assert!(line.left < 0.5 && line.right > COL_W - 0.5, "a line not beside the object is full width: {}..{}", line.left, line.right);
            }
            narrowed
        })
        .collect();
    let next = if p.after > 0 { placed(&l, p.before + 1).tops[0] } else { 0.0 };
    Outcome { p: top, offset: t.tops[0] - top, beside, next, t_page, layout: l }
}

fn assert_offset(r: &Outcome, want: f32, what: &str) {
    assert!((r.offset - want).abs() < 0.5, "{what}: line 1 is {} below the paragraph's top, want {want}", r.offset);
}

fn all(r: &Outcome, beside: bool, n: usize, what: &str) {
    assert_eq!(r.beside, vec![beside; n], "{what}: lines beside the object");
}

/// Rows 1-8, 12: the first line starts at the object's top (inside the first line), else at
/// the top of the first full-width line that meets it; every line is beside it.
#[test]
fn a_kept_paragraph_starts_beside_the_object() {
    let (_, h) = heights();
    for (t, want) in [(0.0, 0.0), (2.0, 2.0), (5.0, 5.0), (10.0, 10.0), (13.0, 13.0), (20.0, h), (25.0, h), (30.0, 2.0 * h)] {
        let r = run(&Probe::new(11, true, 3, t));
        let what = format!("t = {t}");
        assert_offset(&r, want, &what);
        all(&r, true, 3, &what);
        // The object stays where its position puts it, measured from the paragraph's top.
        let (_, rect) = object(&r.layout);
        assert!((rect.y - (r.p + t)).abs() < 0.5, "{what}: the object does not move: {rect:?} (P {})", r.p);
        // The next paragraph follows the taller paragraph.
        assert!((r.next - (r.p + want + 3.0 * h)).abs() < 0.5, "{what}: the next paragraph at {}", r.next);
    }
    // Row 12: one line.
    let r = run(&Probe::new(11, true, 1, 10.0));
    assert_offset(&r, 10.0, "one line, t = 10");
    all(&r, true, 1, "one line, t = 10");
}

/// Rows 9, 11, 13: an object below the paragraph's lines changes nothing (the paragraphs after
/// it wrap).
#[test]
fn an_object_below_the_lines_changes_nothing() {
    let (_, h) = heights();
    for (n, t) in [(3, 45.0), (2, 30.0), (1, 20.0)] {
        let r = run(&Probe::new(11, true, n, t));
        let what = format!("{n} lines, t = {t}");
        assert_offset(&r, 0.0, &what);
        all(&r, false, n, &what);
        assert!((r.next - (r.p + n as f32 * h)).abs() < 0.5, "{what}: the next paragraph right below: {}", r.next);
    }
    let r = run(&Probe::new(11, true, 3, 45.0));
    let next = placed(&r.layout, 2);
    assert!(next.para.lines[0].left > OBJ_W - 0.5, "the paragraph after it wraps around the object");
}

/// Row 10: the last line's band with the space after meets the object: every line goes beside
/// it, line 1 too.
#[test]
fn the_space_after_reaching_the_object_counts() {
    let (_, h) = heights();
    let mut p = Probe::new(11, true, 3, 45.0);
    p.space_after = 12.0;
    let r = run(&p);
    assert_offset(&r, 2.0 * h, "space after 12");
    all(&r, true, 3, "space after 12");
    assert!((r.next - (r.p + 5.0 * h + 12.0)).abs() < 0.5, "the next paragraph at P + 2h + 3h + 12: {} (P {})", r.next, r.p);
}

/// Rows 14-17: anchored after the last character, at the right edge, anchored in the paragraph
/// before, and modes 14 and 12: the same.
#[test]
fn anchor_side_and_older_modes_do_not_matter() {
    let (_, h) = heights();
    let mut cases = Vec::new();
    let mut p = Probe::new(11, true, 3, 20.0);
    p.anchor = Where::End;
    cases.push(("anchored at the end", p));
    let mut p = Probe::new(11, true, 3, 20.0);
    p.right = true;
    cases.push(("at the right edge", p));
    let mut p = Probe::new(11, true, 3, 20.0);
    p.anchor = Where::Before;
    cases.push(("anchored in the paragraph before", p));
    cases.push(("mode 14", Probe::new(14, true, 3, 20.0)));
    cases.push(("mode 12", Probe::new(12, true, 3, 20.0)));
    let mut p = Probe::new(11, true, 3, 20.0);
    p.keep_next = true;
    cases.push(("keep with next too", p));
    for (what, p) in cases {
        let r = run(&p);
        assert_offset(&r, h, what);
        all(&r, true, 3, what);
    }
}

/// Rows 18-23 (guards, as before the change): mode 15, and paragraphs without Keep lines
/// together (Keep with next alone does not count), keep the lines above the object at full width.
#[test]
fn mode_15_and_unkept_paragraphs_are_unchanged() {
    let mut cases = Vec::new();
    let mut p = Probe::new(15, true, 3, 20.0);
    p.keep_next = true;
    cases.push(("mode 15, keep lines and next, t = 20", p, false));
    cases.push(("mode 15, t = 10", Probe::new(15, true, 3, 10.0), true));
    cases.push(("no keep, t = 20", Probe::new(11, false, 3, 20.0), false));
    cases.push(("no keep, t = 10", Probe::new(11, false, 3, 10.0), true));
    for (t, line1) in [(10.0, true), (20.0, false)] {
        let mut p = Probe::new(11, false, 3, t);
        p.keep_next = true;
        cases.push((if t < 15.0 { "keep next only, t = 10" } else { "keep next only, t = 20" }, p, line1));
    }
    for (what, p, line1) in cases {
        let r = run(&p);
        assert_offset(&r, 0.0, what);
        assert_eq!(r.beside, vec![line1, true, true], "{what}: lines beside the object");
    }
}

/// Row 24: with Top and Bottom wrapping the restarted paragraph goes below the object, nothing
/// above it.
#[test]
fn a_top_and_bottom_object_takes_the_whole_paragraph_below_it() {
    let mut p = Probe::new(11, true, 3, 20.0);
    p.wrap = Wrap::TopAndBottom;
    let r = run(&p);
    assert_offset(&r, 20.0 + OBJ_H, "top and bottom");
    all(&r, false, 3, "top and bottom");
    // Mode 15: line 1 stays above it, as before.
    p.mode = 15;
    let r = run(&p);
    assert_offset(&r, 0.0, "top and bottom, mode 15");
}

/// Row 25: near the foot of the page, the paragraph's three lines fit but not with the empty
/// room above them: being kept together, it goes to the next page whole, and its object with it
/// (positioned from the paragraph's new top, starting it beside the object again there).
#[test]
fn the_taller_kept_paragraph_moves_to_the_next_page() {
    let (h_plain, h) = heights();
    assert!((h_plain - h).abs() < 0.01, "plain and title lines are as high: {h_plain} {h}");
    let per_page = ((648.0 + 0.01) / h).floor() as usize;
    let mut p = Probe::new(11, true, 3, 20.0);
    (p.before, p.after, p.obj_h) = (per_page - 3, 1, 30.0);
    // Without Keep lines together the paragraph stays on page 1.
    p.keep_lines = false;
    let control = run(&p);
    assert_eq!(control.t_page, 0, "unkept: stays on page 1");
    p.keep_lines = true;
    let r = run(&p);
    assert_eq!(r.t_page, 1, "kept: moves to page 2");
    assert_offset(&r, h, "kept, on page 2");
    all(&r, true, 3, "kept, on page 2");
    let (page, rect) = object(&r.layout);
    assert_eq!(page, 1, "the object goes along");
    assert!((rect.y - (72.0 + 20.0)).abs() < 0.5, "20 pt below the paragraph's new top: {rect:?}");
}

/// Row 26: the extra line height pushes the last paragraph to a second page in mode 11, not in
/// mode 15.
#[test]
fn the_empty_room_can_add_a_page() {
    let (_, h) = heights();
    let per_page = ((648.0 + 0.01) / h).floor() as usize;
    let mut p = Probe::new(11, true, 3, 20.0);
    (p.before, p.after, p.obj_h) = (per_page - 4, 1, 30.0);
    let r = run(&p);
    assert_eq!(r.t_page, 0);
    assert_offset(&r, h, "mode 11");
    assert_eq!(r.layout.pages.len(), 2, "mode 11: two pages");
    p.mode = 15;
    let r = run(&p);
    assert_eq!(r.layout.pages.len(), 1, "mode 15: one page");
}
