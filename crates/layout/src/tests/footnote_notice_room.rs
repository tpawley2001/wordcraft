//! A document with a footnote continuation notice keeps the notice's room under the footnotes on
//! every page that has footnotes, whether or not a note continues (measured in Word: the notes end
//! the notice's height above the bottom margin). The notice is only drawn under a note that
//! continues. A page without footnotes reserves nothing, and a document without a notice lays out
//! as before.
use super::default_note_separator::{normal_line, sep_height};
use crate::notes::{DECOR_STORY, NOTE_SEP};
use crate::page::item_bottom;
use crate::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{NoteSeparators, PartKind, Pos, para_block};

const TOL: f32 = 0.05;
const BODY: f32 = 12.0;
const NOTE: f32 = 10.0;
const NOTICE: f32 = 13.0;

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn exactly(h: f32) -> ParaProps {
    ParaProps { line_spacing: Some(LineSpacing::Exactly(h)), space_before: Some(0.0), space_after: Some(0.0), ..Default::default() }
}

fn para(text: &str, h: f32) -> std::sync::Arc<Block> {
    let mut p = Paragraph::with_text(text, Default::default());
    p.props = exactly(h);
    para_block(p)
}

/// Letter, 72 pt margins, `lines` body lines of 12 pt; a footnote of `note_lines` lines (10 pt)
/// referenced at the end of each body line in `refs` (0-based); the notice when `notice`.
fn build(lines: usize, refs: &[usize], note_lines: usize, notice: bool) -> (Document, Vec<u32>) {
    crate::test_fonts();
    let mut d = Document::from_text("");
    let s = &mut d.last_section;
    (s.page_w, s.page_h, s.margin_top, s.margin_bottom, s.margin_left, s.margin_right) = (612.0, 792.0, 72.0, 72.0, 72.0, 72.0);
    d.body = (1..=lines).map(|i| para(&format!("L{i:02}"), BODY)).collect();
    if notice {
        d.footnote_separators = NoteSeparators { continuation_notice: Some(vec![para("(continued)", NOTICE)]), ..Default::default() };
    }
    let mut ids = Vec::new();
    for (k, &i) in refs.iter().enumerate() {
        let body = (1..=note_lines).map(|j| para(&format!("Note {} line {j}", k + 1), NOTE)).collect();
        let id = d.add_part(PartKind::Footnote, body);
        let off = format!("L{:02}", i + 1).len();
        d.insert_object(&Pos::body(i, off), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &Default::default())
            .unwrap();
        ids.push(id);
    }
    (d, ids)
}

fn margin(d: &Document) -> f32 {
    d.last_section.page_h - d.last_section.margin_bottom
}

/// The bottom of footnote `id`'s lines on page `pi` (`None`: none there).
fn note_bottom(l: &DocLayout, pi: usize, id: u32) -> Option<f32> {
    l.pages[pi]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Part(s), y, para, l0, l1, .. } if *s == id => item_bottom(*y, para, *l0, *l1),
            _ => None,
        })
        .reduce(f32::max)
}

/// The number of lines of footnote `id` on page `pi`.
fn note_lines(l: &DocLayout, pi: usize, id: u32) -> usize {
    l.pages[pi]
        .items
        .iter()
        .map(|it| match it {
            Placed::Lines { story: StoryRef::Part(s), l0, l1, .. } if *s == id => l1 - l0,
            _ => 0,
        })
        .sum()
}

/// The number of body lines on page `pi`.
fn body_lines(l: &DocLayout, pi: usize) -> usize {
    l.pages[pi]
        .items
        .iter()
        .map(|it| match it {
            Placed::Lines { story: StoryRef::Body, l0, l1, .. } => l1 - l0,
            _ => 0,
        })
        .sum()
}

/// The tops and bottoms of the continuation notices drawn on page `pi`.
fn notices(l: &DocLayout, pi: usize) -> Vec<(f32, f32)> {
    l.pages[pi]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story, y, para, l0, l1, .. } if *story == DECOR_STORY => Some((*y, item_bottom(*y, para, *l0, *l1)?)),
            _ => None,
        })
        .collect()
}

fn near(got: f32, want: f32, what: &str) {
    assert!((got - want).abs() <= TOL, "{what}: {got}, want {want}");
}

#[test]
fn a_page_with_a_footnote_keeps_the_notice_room_under_it() {
    let (d, ids) = build(10, &[0], 1, true);
    let l = lay(&d);
    near(note_bottom(&l, 0, ids[0]).unwrap(), margin(&d) - NOTICE, "note bottom");
    assert!(notices(&l, 0).is_empty(), "no notice drawn when nothing continues");
}

#[test]
fn without_a_notice_the_footnote_ends_at_the_margin() {
    let (d, ids) = build(10, &[0], 1, false);
    let l = lay(&d);
    near(note_bottom(&l, 0, ids[0]).unwrap(), margin(&d), "note bottom");
}

/// The room the default separator takes above note `id` on page 0 (the fixture has no separator
/// story, only a notice): one single-spaced line of the Normal style, not the old 12 pt.
fn default_sep(d: &Document, l: &DocLayout, id: u32) -> f32 {
    let h = sep_height(l, id);
    near(h, normal_line(d), "default separator: one Normal line");
    assert!((h - NOTE_SEP).abs() > TOL, "default separator {h} is not the 12 pt fallback");
    h
}

fn room(d: &Document) -> f32 {
    margin(d) - d.last_section.margin_top
}

#[test]
fn a_full_page_with_a_footnote_holds_a_line_less_for_the_notice() {
    let (d, ids) = build(60, &[0], 1, true);
    let l = lay(&d);
    let want = ((room(&d) - default_sep(&d, &l, ids[0]) - NOTE - NOTICE) / BODY + 0.001).floor() as usize;
    assert_eq!(body_lines(&l, 0), want);
}

#[test]
fn a_full_page_with_a_footnote_and_no_notice_is_unchanged() {
    let (d, ids) = build(60, &[0], 1, false);
    let l = lay(&d);
    let want = ((room(&d) - default_sep(&d, &l, ids[0]) - NOTE) / BODY + 0.001).floor() as usize;
    assert_eq!(body_lines(&l, 0), want);
}

#[test]
fn a_page_without_footnotes_reserves_nothing() {
    // The note is referenced from a line on page 2.
    let (d, ids) = build(60, &[57], 1, true);
    let l = lay(&d);
    assert_eq!(body_lines(&l, 0), (room(&d) / BODY + 0.001).floor() as usize);
    assert_eq!(note_lines(&l, 0, ids[0]), 0);
    assert_eq!(note_lines(&l, 1, ids[0]), 1);
}

#[test]
fn a_split_footnote_reserves_the_notice_once() {
    let (d, ids) = build(1, &[0], 100, true);
    let l = lay(&d);
    assert!(l.pages.len() >= 2 && note_lines(&l, 1, ids[0]) > 0, "the note continues");
    let bottom = note_bottom(&l, 0, ids[0]).unwrap();
    let drawn = notices(&l, 0);
    assert_eq!(drawn.len(), 1, "one notice: {drawn:?}");
    near(drawn[0].0, bottom, "notice right under the note");
    assert!(bottom + NOTICE <= margin(&d) + TOL, "piece {bottom} + notice inside the page");
    near(drawn[0].1, drawn[0].0 + NOTICE, "notice height");
    // As before: the piece fills the room under the body line and separator, less the notice.
    let top = d.last_section.margin_top + BODY + default_sep(&d, &l, ids[0]);
    let want = ((margin(&d) - NOTICE - top) / NOTE + 0.001).floor() as usize;
    assert_eq!(note_lines(&l, 0, ids[0]), want);
}

#[test]
fn every_page_with_footnotes_keeps_the_notice_room() {
    let (d, ids) = build(60, &[0, 57], 1, true);
    let l = lay(&d);
    near(note_bottom(&l, 0, ids[0]).unwrap(), margin(&d) - NOTICE, "page 1 note bottom");
    near(note_bottom(&l, 1, ids[1]).unwrap(), margin(&d) - NOTICE, "page 2 note bottom");
}
