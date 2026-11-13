//! A line whose first word fits neither side of a floating object with Square wrapping (ECMA-376
//! Part 1 §20.4.2.17 `wp:wrapSquare`) moves down one row height at a time until the row clears
//! the object, as Word lays it out: it lands on `natural top + k * row height` (k the smallest
//! count that clears the object and its distance from text below), up to one row below the
//! object, not flush with its bottom. A Top and Bottom object (§20.4.2.20) still starts the next
//! line at its bottom.
//!
//! Measured in Word from its PDFs of probe files: a 10 pt line anchors a box 3 pt below its top,
//! leaving 31 pt on the left and 23 pt on the right of a 216 pt column; a heading whose first
//! word is about 40 pt wide follows. With a 225 pt box the heading's line top was 20 rows below
//! the anchor line's top (2 pt below the box), with a 215 pt box 19 rows, with a 220 pt box 20,
//! with 5 pt distance below 21; a 14 pt heading stepped by its own row height. The number of
//! empty paragraphs between (they fit beside the box) did not change it. These tests read the
//! row heights from the layout with the test fonts, so they assert the rule, not Word's points.

use super::*;
use wordcraft_doc::para::{Anchor, Float, Wrap};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Path, StoryRef};

const HEADING: &str = "Alternate Names";
/// The room the box leaves on its left (40 pt from the column's left, 9 pt distance).
const LEFT_ROOM: f32 = 31.0;

struct Probe {
    /// The box's height.
    h: f32,
    /// Its distance from text below.
    d: f32,
    /// Empty paragraphs between the anchor and the heading.
    e: usize,
    /// The heading's font size.
    heading: f32,
    /// The line after the anchor (instead of the heading).
    text: &'static str,
    wrap: Wrap,
}

const BASE: Probe = Probe { h: 225.0, d: 0.0, e: 1, heading: 10.0, text: HEADING, wrap: Wrap::Square };

/// The paragraphs: P (the anchor), `e` empty ones, the heading, two 10 pt lines and two lines at
/// the heading's size (to read the row heights). The heading's index is `e + 1`.
fn doc(p: &Probe) -> Document {
    let mut text = vec!["P".to_string()];
    text.extend(std::iter::repeat_n(String::new(), p.e));
    text.extend([p.text.to_string(), "ten one".into(), "ten two".into(), "big one".into(), "big two".into()]);
    let mut d = Document::from_text(&text.join("\n"));
    {
        let st = Arc::make_mut(&mut d.styles);
        st.default_chr.font = Some(WRAP_TEST_FONT.into());
        st.default_chr.size = Some(10.0);
    }
    let s = &mut d.last_section;
    s.page_w = 360.0;
    s.margin_left = 72.0;
    s.margin_right = 72.0;
    let n = d.body.len();
    for (i, block) in d.body.iter_mut().enumerate() {
        if let Block::Para(para) = Arc::make_mut(block) {
            para.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            para.props.space_before = Some(0.0);
            para.props.space_after = Some(0.0);
            if i == p.e + 1 || i + 2 >= n {
                para.mark.size = Some(p.heading);
                if let Some(run) = para.runs.first_mut() {
                    run.props.size = Some(p.heading);
                }
            }
        }
    }
    let float =
        Float { wrap: p.wrap, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 40.0, y: 3.0, dist: 9.0, dist_bottom: p.d, ..Default::default() };
    text_box_at(&mut d, &Pos::body(0, 1), "", 144.0, p.h, float);
    d
}

/// Page top of the first line of body paragraph `i`, and the paragraph's layout.
fn top_of(l: &DocLayout, i: usize) -> (f32, ParaLayout) {
    l.pages
        .iter()
        .flat_map(|p| &p.items)
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, y, para, l0: 0, .. } if path.0 == [i as u32] => Some((*y, (**para).clone())),
            _ => None,
        })
        .unwrap_or_else(|| panic!("paragraph {i} is placed"))
}

/// The box's page rectangle (top, bottom).
fn box_rect(l: &DocLayout) -> (f32, f32) {
    let r = l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Shape { rect, kind: wordcraft_doc::para::ShapeKind::TextBox, .. } => Some(*rect),
            _ => None,
        })
        .expect("the box is drawn");
    (r.y, r.y + r.h)
}

struct Laid {
    /// The anchor line's top.
    t: f32,
    /// The 10 pt row height and the heading-size row height.
    h10: f32,
    hh: f32,
    /// The top of the line after the anchor and the empty paragraphs.
    heading: f32,
    /// Where it would start without the box.
    natural: f32,
}

fn run(p: &Probe) -> Laid {
    crate::test_fonts();
    let d = doc(p);
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1, "the probe fits one page");
    let n = d.body.len();
    let (t, _) = top_of(&l, 0);
    let h10 = top_of(&l, n - 3).0 - top_of(&l, n - 4).0;
    let hh = top_of(&l, n - 1).0 - top_of(&l, n - 2).0;
    assert!(h10 > 5.0 && hh > 5.0, "row heights {h10} {hh}");
    let (heading, para) = top_of(&l, p.e + 1);
    assert_eq!(d.para(StoryRef::Body, &Path::top(p.e + 1)).map(|q| q.text.clone()).as_deref(), Some(p.text));
    let (bt, bb) = box_rect(&l);
    assert!((bt - (t + 3.0)).abs() < 0.05, "the box starts 3 pt below the anchor line: {bt} {t}");
    assert!((bb - (t + 3.0 + p.h)).abs() < 0.05, "the box keeps its height: {bb}");
    if p.text == HEADING {
        let first_word: f32 = para.clusters.iter().filter(|c| c.start < "Alternate".len()).map(|c| c.adv).sum();
        assert!(first_word > LEFT_ROOM, "the heading's first word ({first_word} pt) must fit neither side of the box");
    }
    Laid { t, h10, hh, heading, natural: t + (p.e as f32 + 1.0) * h10 }
}

/// `natural + k * h` for the smallest `k` that reaches `clear`.
fn stepped(natural: f32, h: f32, clear: f32) -> f32 {
    let mut k = 0.0;
    while natural + k * h < clear - 1e-3 {
        k += 1.0;
    }
    natural + k * h
}

fn assert_top(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() < 0.05, "{what}: line top {got}, want {want}");
}

#[test]
fn steps_by_row_height_below_a_square_object() {
    let r = run(&BASE);
    let want = stepped(r.natural, r.h10, r.t + 228.0);
    assert_top(r.heading, want, "#1");
    assert!(r.heading > r.t + 228.0 + 0.05, "below the box's bottom, not flush with it: {} {}", r.heading, r.t + 228.0);
    // With the test font's row height this is 20 rows below the anchor line, as in Word.
    let rows = ((r.heading - r.t) / r.h10).round();
    assert!((r.heading - r.t - rows * r.h10).abs() < 0.05, "a whole number of rows below the anchor line");
}

#[test]
fn empty_paragraphs_before_do_not_move_it() {
    let one = run(&BASE);
    for e in [0, 2] {
        let r = run(&Probe { e, ..BASE });
        assert_top(r.heading - r.t, one.heading - one.t, &format!("E = {e}"));
        assert_top(r.heading, stepped(r.natural, r.h10, r.t + 228.0), &format!("E = {e}"));
    }
}

#[test]
fn shorter_object_one_row_less() {
    for h in [215.0, 220.0] {
        let r = run(&Probe { h, ..BASE });
        assert_top(r.heading, stepped(r.natural, r.h10, r.t + 3.0 + h), &format!("H = {h}"));
        assert!(r.heading > r.t + 3.0 + h, "H = {h}: below the box");
    }
}

#[test]
fn distance_below_counts() {
    let r = run(&Probe { d: 5.0, ..BASE });
    let want = stepped(r.natural, r.h10, r.t + 233.0);
    assert_top(r.heading, want, "D = 5");
    assert!(r.heading > r.t + 233.0, "below the distance");
}

#[test]
fn taller_row_steps_by_its_own_height() {
    let r = run(&Probe { heading: 14.0, ..BASE });
    assert!(r.hh > r.h10 + 2.0, "the 14 pt rows are taller: {} {}", r.hh, r.h10);
    assert_top(r.heading, stepped(r.natural, r.hh, r.t + 228.0), "14 pt heading");
}

#[test]
fn text_that_fits_beside_is_untouched() {
    let r = run(&Probe { text: "a", ..BASE });
    assert_top(r.heading, r.natural, "a one-letter word beside the box");
}

#[test]
fn top_and_bottom_object_still_starts_flush() {
    crate::test_fonts();
    for d in [0.0, 5.0] {
        let doc = doc(&Probe { wrap: Wrap::TopAndBottom, e: 0, d, ..BASE });
        let l = lay(&doc);
        let (_, bottom) = box_rect(&l);
        // The line that meets the object (here its own anchor line) starts at its bottom plus
        // the distance, not a whole number of rows lower.
        let (first, _) = top_of(&l, 0);
        assert_top(first, bottom + d, &format!("Top and Bottom, D = {d}"));
    }
}
