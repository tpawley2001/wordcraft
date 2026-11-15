//! A floating object whose wrap area, known from the top of the page, pushes its own anchor
//! paragraph off that page: Word lays the page out as if the object were not there, ends the page
//! right before the anchor paragraph (even with room left), and starts the next page with the
//! paragraph and its object. Expected values measured from PDFs that Microsoft 365 Word exported
//! for synthetic probes (Letter, 1in margins, 12pt single-spaced one-line paragraphs, ECMA-376
//! §20.4.2.3, §20.4.2.17, §20.4.2.20); Word's 13.8pt line pitch is replaced by the test font's own.
use super::{rect_shape, shapes};
use crate::*;
use wordcraft_doc::para::{Anchor, Float, Wrap};
use wordcraft_doc::para_block;
use wordcraft_doc::props::{LineSpacing, ParaProps};

const TOL: f32 = 1.5;
const TOP: f32 = 72.0;

fn near(what: &str, got: f32, want: f32) {
    assert!((got - want).abs() <= TOL, "{what}: got {got:.2}pt, Word {want:.2}pt (±{TOL})");
}

fn para(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps { font: Some("Arial".into()), size: Some(12.0), ..Default::default() });
    p.props = ParaProps { space_before: Some(0.0), space_after: Some(0.0), line_spacing: Some(LineSpacing::Multiple(1.0)), ..Default::default() };
    p
}

/// `text` holding a `w` x `h` rectangle floated by `f`.
fn anchor(text: &str, w: f32, h: f32, f: Float) -> Paragraph {
    let mut p = para(text);
    p.insert_object(0, rect_shape(w, h, f), &Default::default()).unwrap();
    p
}

fn doc(body: Vec<Paragraph>, mode: u32, cols: u32) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (612.0, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (72.0, 72.0, 72.0, 72.0);
    if cols > 1 {
        s.columns.count = cols;
        s.columns.space = 36.0;
    }
    d.body = body.into_iter().map(para_block).collect();
    d
}

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// (body block, line top) of every text line on `page`, top to bottom.
fn lines(l: &DocLayout, page: usize) -> Vec<(usize, f32)> {
    let mut v: Vec<(usize, f32)> = l.pages[page]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, path, y, story: StoryRef::Body, .. } if para.text_len > 0 => Some((path.0[0] as usize, *y)),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v
}

/// The test font's single-spaced line pitch and how many one-line paragraphs a plain page holds.
fn metrics() -> (f32, usize) {
    let body: Vec<Paragraph> = (0..120).map(|i| para(&format!("P{i}"))).collect();
    let l = lay(&doc(body, 15, 1));
    let t = lines(&l, 0);
    (t[1].1 - t[0].1, t.len())
}

fn fillers(n: usize, prefix: &str) -> Vec<Paragraph> {
    (1..=n).map(|i| para(&format!("{prefix}{i:03}"))).collect()
}

/// The page-relative probe of part A: `n` filler lines, ANCHOR with a 468pt-wide box at page
/// y 72 (column x 0) of height `h`, distance below `dist_b`, then 40 more lines.
fn probe_a(n: usize, h: f32, dist_b: f32, wrap: Wrap, mode: u32) -> (Document, usize) {
    let f = Float { wrap, h_rel: Anchor::Column, v_rel: Anchor::Page, x: 0.0, y: 72.0, dist_bottom: dist_b, ..Default::default() };
    let mut body = fillers(n, "L");
    body.push(anchor("ANCHOR", 468.0, h, f));
    body.extend(fillers(40, "M"));
    (doc(body, mode, 1), n)
}

/// The anchor paragraph left page 0: page 0 holds exactly the `n` filler lines from the top
/// margin and no object; page 1 has the object and starts with ANCHOR at `anchor_top`.
fn assert_moved(name: &str, d: &Document, n: usize, anchor_top: f32) {
    let l = lay(d);
    let p0 = lines(&l, 0);
    assert!(shapes(&l.pages[0].items).is_empty(), "{name}: no object on the page the anchor left");
    assert_eq!(p0.len(), n, "{name}: page 1 holds the {n} lines before the anchor and ends there: {p0:?}");
    assert!(p0.iter().all(|(b, _)| *b < n), "{name}: only the lines before the anchor on page 1: {p0:?}");
    near(&format!("{name}: first line of page 1 (no band)"), p0[0].1, TOP);
    assert!(l.pages.len() > 1, "{name}: a second page");
    assert_eq!(shapes(&l.pages[1].items).len(), 1, "{name}: the object is on page 2");
    let p1 = lines(&l, 1);
    assert_eq!(p1[0].0, n, "{name}: page 2 starts with ANCHOR: {p1:?}");
    near(&format!("{name}: ANCHOR on page 2"), p1[0].1, anchor_top);
}

#[test]
fn the_page_the_anchor_left_has_no_band() {
    let (_, cap) = metrics();
    for mode in [15, 11] {
        let (d, n) = probe_a(cap - 1, 300.0, 0.0, Wrap::TopAndBottom, mode);
        assert_moved(&format!("a{mode}"), &d, n, 72.0 + 300.0);
    }
}

#[test]
fn the_page_ends_before_the_anchor_paragraph() {
    let (pitch, cap) = metrics();
    // Fewer lines than the page holds, but too many for the 300pt band above them (Word: 29 of 46).
    let n = ((648.0 - 300.0) / pitch).ceil() as usize + 3;
    assert!(n < cap - 1);
    for mode in [15, 11] {
        let (d, n) = probe_a(n, 300.0, 0.0, Wrap::TopAndBottom, mode);
        assert_moved(&format!("c{mode}"), &d, n, 72.0 + 300.0);
    }
}

#[test]
fn a_small_box_moves_with_its_paragraph() {
    let (_, cap) = metrics();
    let (d, n) = probe_a(cap - 1, 27.6, 0.0, Wrap::TopAndBottom, 15);
    assert_moved("d15", &d, n, 72.0 + 27.6);
}

#[test]
fn distance_below_counts_on_the_new_page() {
    let (_, cap) = metrics();
    let (d, n) = probe_a(cap - 1, 345.6, 14.4, Wrap::TopAndBottom, 11);
    assert_moved("e11", &d, n, 72.0 + 345.6 + 14.4);
}

#[test]
fn an_anchor_that_fits_below_the_box_stays() {
    let (pitch, _) = metrics();
    // b15: 4 lines; f15: the lines + the 300pt box just fit the page.
    let fit = ((648.0 - 300.0) / pitch).floor() as usize - 1;
    for (name, n) in [("b15", 4usize), ("f15", fit)] {
        let (d, n) = probe_a(n, 300.0, 0.0, Wrap::TopAndBottom, 15);
        let l = lay(&d);
        assert_eq!(shapes(&l.pages[0].items).len(), 1, "{name}: the box stays on page 1");
        let p0 = lines(&l, 0);
        near(&format!("{name}: L001 below the box"), p0[0].1, 72.0 + 300.0);
        assert!(p0.iter().any(|(b, _)| *b == n), "{name}: ANCHOR stays on page 1: {p0:?}");
    }
}

#[test]
fn a_square_box_as_wide_as_the_column_moves_the_same_way() {
    let (_, cap) = metrics();
    let (d, n) = probe_a(cap - 1, 300.0, 0.0, Wrap::Square, 15);
    assert_moved("a15 square", &d, n, 72.0 + 300.0);
}

/// Part B (g11, g15): a paragraph-relative offset that puts the box above the page moves it onto
/// the page: the paper's top edge below mode 15, the top margin in mode 15; the text goes below.
#[test]
fn a_box_above_the_page_moves_onto_it() {
    let (pitch, _) = metrics();
    for (mode, box_top) in [(11, 0.0), (15, 72.0)] {
        let f = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 13.0, y: -350.0, ..Default::default() };
        let mut body = fillers(2, "L");
        body.push(anchor("ANCHOR", 180.0, 300.0, f));
        body.extend(fillers(10, "M"));
        let l = lay(&doc(body, mode, 1));
        let boxes = shapes(&l.pages[0].items);
        assert_eq!(boxes.len(), 1, "g{mode}: the box is drawn on page 1");
        near(&format!("g{mode}: box top"), boxes[0].y, box_top);
        let p0 = lines(&l, 0);
        near(&format!("g{mode}: L001 below the box"), p0[0].1, box_top + 300.0);
        near(&format!("g{mode}: ANCHOR"), p0[2].1, box_top + 300.0 + 2.0 * pitch);
    }
}

/// Part B (a15): the same box laid out from below its paragraph, 30 lines before it: page 1 ends
/// before ANCHOR with no box; page 2 has the box at the top margin and ANCHOR below it.
#[test]
fn a_box_above_its_paragraph_that_pushes_it_off_moves_with_it() {
    let f = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 13.0, y: -350.0, ..Default::default() };
    let n = 30;
    let mut body = fillers(n, "L");
    body.push(anchor("ANCHOR", 180.0, 300.0, f));
    body.extend(fillers(40, "M"));
    let d = doc(body, 15, 1);
    assert_moved("B a15", &d, n, 72.0 + 300.0);
    let l = lay(&d);
    near("B a15: box at the top margin", shapes(&l.pages[1].items)[0].y, 72.0);
}

/// Part B (b15): two columns; the box stays in column 1 where the first layout put it and its
/// paragraph starts column 2; no band across both columns.
#[test]
fn in_columns_the_box_stays_and_its_paragraph_starts_the_next_column() {
    let (pitch, _) = metrics();
    let f = Float { wrap: Wrap::TopAndBottom, h_rel: Anchor::Column, v_rel: Anchor::Paragraph, x: 13.0, y: -350.0, ..Default::default() };
    let n = 30;
    let mut body = fillers(n, "L");
    body.push(anchor("ANCHOR", 180.0, 300.0, f));
    body.extend(fillers(40, "M"));
    let l = lay(&doc(body, 15, 2));
    let boxes = shapes(&l.pages[0].items);
    assert_eq!(boxes.len(), 1, "b15: the box is on page 1");
    // Where ANCHOR sits in column 1 without the box, minus 350.
    near("b15: box top", boxes[0].y, TOP + n as f32 * pitch - 350.0);
    assert!(boxes[0].x < 72.0 + 216.0, "b15: the box stays in column 1 (x {})", boxes[0].x);
    let p0 = lines(&l, 0);
    let first_col1 = p0.iter().filter(|(_, y)| *y < boxes[0].y).count();
    assert!(first_col1 > 0, "b15: column 1 has lines above the box: {p0:?}");
    let anchor_line = p0.iter().find(|(b, _)| *b == n).expect("b15: ANCHOR on page 1");
    // Word: box 136..436, ANCHOR at the top of column 2 @155.3 (below the box top, far above its
    // bottom); the wrong layout starts both columns under an empty band, ANCHOR @576.8.
    assert!(anchor_line.1 < boxes[0].y + 300.0, "b15: ANCHOR starts column 2 near its top, not under a band: {anchor_line:?}");
}

/// Part C (q1, q2, q6): a Square box (page x 278, y 277, 220 x 195pt) whose wrap area pushes its
/// paragraph off the page: the page holds the long lines full width, no box, and ends before ANCHOR.
#[test]
fn a_square_box_that_pushes_its_paragraph_off_moves_with_it() {
    let (_, cap) = metrics();
    let long = "word ".repeat(14);
    for (name, mode, text) in [("q1", 15, "ANCHOR"), ("q2", 11, "ANCHOR"), ("q6", 15, "")] {
        let f = Float { wrap: Wrap::Square, h_rel: Anchor::Page, v_rel: Anchor::Page, x: 278.0, y: 277.0, dist: 9.0, ..Default::default() };
        let n = cap - 6;
        let mut body: Vec<Paragraph> = (1..=n).map(|i| para(&format!("L{i:03} {long}"))).collect();
        body.push(anchor(text, 220.0, 195.0, f));
        body.extend(fillers(10, "M"));
        let l = lay(&doc(body, mode, 1));
        assert!(shapes(&l.pages[0].items).is_empty(), "{name}: no box on page 1");
        let p0 = lines(&l, 0);
        assert!(p0.iter().all(|(b, _)| *b < n), "{name}: page 1 ends before the anchor: {p0:?}");
        assert_eq!(p0.iter().filter(|(b, _)| *b < n).count(), n, "{name}: the {n} long lines one row each, full width");
        assert_eq!(shapes(&l.pages[1].items).len(), 1, "{name}: the box is on page 2");
        near(&format!("{name}: box on page 2"), shapes(&l.pages[1].items)[0].y, 277.0);
    }
}
