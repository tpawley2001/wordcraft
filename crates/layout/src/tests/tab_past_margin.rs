//! A tab that starts at the right edge of the line, where an earlier custom stop (a right stop at
//! the margin) put the text, goes on to the next default stop past the edge, and the rest of the
//! line stays on it, running past the margin, instead of wrapping. A tab that starts before the
//! edge and whose default stop is at or past it still wraps.
//!
//! Measured in Word (16.0.20527), compatibility-mode documents: US Letter, 1 in margins (text
//! width 468 pt), 12 pt text, default stops every 36 pt; custom stops: centre at half the text
//! width, right at the text width. `Left⇥Mid⇥⇥Right text` is one line with `Right` at the
//! margin + 504 pt; `A` and 14 tabs then `Past` is two lines with `Past` at the margin + 36 pt.

use super::*;
use wordcraft_doc::props::{TabAlign, TabLeader, TabStop};
use wordcraft_doc::{PartKind, para_block};

const MARGIN: f32 = 72.0;
const PAGE_W: f32 = 612.0;
const DEFAULT_TAB: f32 = 36.0;

fn text_width() -> f32 {
    PAGE_W - 2.0 * MARGIN
}

/// The centre stop at half the text width and the right stop at the text width.
fn footer_stops() -> Vec<TabStop> {
    vec![
        TabStop { pos: text_width() / 2.0, align: TabAlign::Center, leader: TabLeader::None },
        TabStop { pos: text_width(), align: TabAlign::Right, leader: TabLeader::None },
    ]
}

/// The first default stop past the right margin, from the left margin.
fn past_margin_stop() -> f32 {
    ((text_width() / DEFAULT_TAB).floor() + 1.0) * DEFAULT_TAB
}

fn para(text: &str, tabs: Option<Vec<TabStop>>) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.tabs = tabs;
    para_block(p)
}

/// A compatibility-mode document on a Letter page with 1 in margins and the pinned test font.
fn base() -> Document {
    let mut d = Document::new();
    d.settings.compat_mode = 14;
    d.settings.default_tab = DEFAULT_TAB;
    Arc::make_mut(&mut d.styles).default_chr.font = Some(WRAP_TEST_FONT.into());
    Arc::make_mut(&mut d.styles).default_chr.size = Some(12.0);
    let s = &mut d.last_section;
    (s.page_w, s.page_h) = (PAGE_W, 792.0);
    (s.margin_top, s.margin_right, s.margin_bottom, s.margin_left) = (MARGIN, MARGIN, MARGIN, MARGIN);
    d
}

/// `text` (with `tabs`) then a paragraph `Next`.
fn body_doc(text: &str, tabs: Option<Vec<TabStop>>) -> Document {
    let mut d = base();
    d.body = vec![para(text, tabs), para("Next", None)];
    d
}

/// The first paragraph's line count, and the tops of its first line and of the next paragraph.
fn lines_and_next(l: &DocLayout) -> (usize, f32, f32) {
    let mut found =
        l.pages[0].items.iter().filter_map(|i| if let Placed::Lines { para, l0, l1, y, .. } = i { Some((para, *l0, *l1, *y)) } else { None });
    let (_, l0, l1, y) = found.next().expect("first paragraph laid out");
    let (_, _, _, next_y) = found.next().expect("second paragraph laid out");
    assert_eq!(l0, 0);
    (l1 - l0, y, next_y)
}

fn line_height(l: &DocLayout) -> f32 {
    l.pages[0]
        .items
        .iter()
        .find_map(|i| if let Placed::Lines { para, .. } = i { para.lines.first().map(|ln| ln.height) } else { None })
        .expect("a line")
}

/// One line; the text after the last tab starts at the margin + the first default stop past the
/// margin; the next paragraph is one line lower.
fn check_one_line(text: &str) {
    let d = body_doc(text, Some(footer_stops()));
    let l = lay(&d);
    let (n, top, next) = lines_and_next(&l);
    assert_eq!(n, 1, "{text:?} takes one line");
    let after = text.rfind('\t').expect("a tab") + 1;
    let x = l.caret(&Pos::body(0, after)).expect("caret").x;
    let want = MARGIN + past_margin_stop();
    assert!((x - want).abs() <= 0.1, "{text:?}: the text after the last tab starts at {x}, want {want}");
    let h = line_height(&l);
    assert!((next - top - h).abs() <= 0.1, "{text:?}: the next paragraph starts one line lower ({top} -> {next}, line {h})");
}

#[test]
fn text_after_a_tab_from_an_empty_right_stop_stays_on_the_line() {
    check_one_line("Left\tMid\t\tRight text");
}

#[test]
fn many_words_after_that_tab_stay_on_the_line() {
    check_one_line("Left\tMid\t\tone two three four five six seven eight nine ten");
}

#[test]
fn text_after_right_aligned_text_and_a_tab_stays_on_the_line() {
    check_one_line("Left\tMid\tRight\tafter");
}

#[test]
fn a_default_tab_that_reaches_the_margin_still_wraps() {
    let text = format!("A{}Past", "\t".repeat(14));
    let d = body_doc(&text, None);
    let l = lay(&d);
    let (n, top, next) = lines_and_next(&l);
    assert_eq!(n, 2, "the tab past the margin wraps");
    let x = l.caret(&Pos::body(0, text.len() - "Past".len())).expect("caret").x;
    let want = MARGIN + DEFAULT_TAB;
    assert!((x - want).abs() <= 0.1, "Past starts at the first stop of line 2: {x}, want {want}");
    let h = line_height(&l);
    assert!(next - top > 1.5 * h, "the next paragraph is below both lines");
}

#[test]
fn a_footer_with_a_right_stop_at_the_margin_and_one_more_tab_is_one_line() {
    let mut d = base();
    d.last_section.footer = 36.0;
    let id = d.add_part(PartKind::Footer, vec![para("a\tb\t\tc d", Some(footer_stops()))]);
    d.last_section.footers.default = Some(id);
    d.body = vec![para("Body", None)];
    let l = lay(&d);
    let lines: Vec<f32> = l.pages[0]
        .footer
        .iter()
        .filter_map(|i| {
            if let Placed::Lines { para, l0, l1, .. } = i { Some(para.lines[*l0..*l1].iter().map(|ln| ln.height).collect::<Vec<_>>()) } else { None }
        })
        .flatten()
        .collect();
    assert_eq!(lines.len(), 1, "the footer is one line: {lines:?}");
}
