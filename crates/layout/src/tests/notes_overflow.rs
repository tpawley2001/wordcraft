//! A body line that refers to several footnotes, near the bottom of a page. Word (Microsoft 365,
//! synthetic RTF files: letter, 1 in margins, Times New Roman 12 body, 10 pt notes) keeps the line
//! on its page when the first line of its first note fits below it, places its notes in order
//! while they fit (whole, then the first that doesn't split at a line, or carried whole when not
//! even its first line fits) and carries the rest to the next page; the page ends after that line.
//! When not even the first note can start, the line moves to the next page with its notes.
//!
//! The room below the reference line is set from the heights measured here (the test fonts'
//! metrics), so the tests assert which page and column things land on, not Word's point values.

use super::*;
use crate::notes::{MAX_NOTE_PAGES, NOTE_SEP};
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::section::Columns;
use wordcraft_doc::{Pos, para_block};

const NOTE_SEP_W: f32 = 144.0;

fn chr(size: f32) -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() }
}

fn para(text: &str, size: f32) -> Paragraph {
    let mut p = Paragraph::with_text(text, chr(size));
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p
}

/// The shape of a test document: `k` one-line paragraphs (the first with `lead` points of space
/// before), then "REF" referring to `notes` footnotes of `lines` one-line paragraphs each, then
/// T1, T2, T3; in `cols` columns.
#[derive(Clone, Copy)]
struct Shape {
    k: usize,
    lead: f32,
    notes: usize,
    lines: usize,
    cols: u32,
}

/// The document and its notes' ids in reference order.
fn build(s: Shape) -> (Document, Vec<u32>) {
    let mut d = Document::from_text("");
    let mut body = Vec::new();
    for i in 1..=s.k {
        let mut p = para(&format!("L{i:02}"), 12.0);
        if i == 1 {
            p.props.space_before = Some(s.lead);
        }
        body.push(para_block(p));
    }
    body.push(para_block(para("REF", 12.0)));
    for t in 1..=3 {
        body.push(para_block(para(&format!("T{t}"), 12.0)));
    }
    d.body = body;
    d.settings.compat_mode = 15;
    d.last_section.columns = Columns { count: s.cols, space: 36.0, ..Columns::default() };
    let ids: Vec<u32> = (1..=s.notes)
        .map(|n| {
            let blocks = (1..=s.lines).map(|j| para_block(para(&format!("Note {n} line {j}"), 10.0))).collect();
            d.add_part(wordcraft_doc::PartKind::Footnote, blocks)
        })
        .collect();
    // Inserted last first at the end of "REF", so they read 1..N.
    for id in ids.iter().rev() {
        d.insert_object(
            &Pos::body(s.k, 3),
            InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id: *id, custom: String::new() },
            &Default::default(),
        )
        .unwrap();
    }
    (d, ids)
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

/// Where body paragraph `block` starts: (page, left edge, top, bottom).
fn body_at(l: &DocLayout, block: usize) -> Option<(usize, f32, f32, f32)> {
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Body, path, para, x, y, l0: 0, l1, .. } = it
                && path.0 == [block as u32]
            {
                return Some((pi, *x, *y, item_bottom(*y, para, 0, *l1).unwrap()));
            }
        }
    }
    None
}

/// Every placed line run of note `id`, in page order: (page, top, bottom, lines).
fn note_pieces(l: &DocLayout, id: u32) -> Vec<(usize, f32, f32, usize)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Part(s), para, y, l0, l1, .. } = it
                && *s == id
            {
                out.push((pi, *y, item_bottom(*y, para, *l0, *l1).unwrap(), l1 - l0));
            }
        }
    }
    out
}

/// The pages note `id` has lines on, and how many lines on each.
fn note_lines_by_page(l: &DocLayout, id: u32) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (pg, _, _, n) in note_pieces(l, id) {
        match out.last_mut() {
            Some((p, c)) if *p == pg => *c += n,
            _ => out.push((pg, n)),
        }
    }
    out
}

/// The lengths of the note separator rules on `page`.
fn separators(l: &DocLayout, page: usize) -> Vec<f32> {
    l.pages[page]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Rule { x0, x1, .. } => Some((x1 - x0).abs()),
            _ => None,
        })
        .collect()
}

/// Heights measured with the reference line at the top of the page: the body's top, the line
/// pitch of the L paragraphs, the REF line's height, one note's height and its first line's.
struct Metrics {
    top: f32,
    pitch: f32,
    ref_h: f32,
    note_h: f32,
    line1: f32,
    bottom: f32,
}

fn measure(notes: usize, lines: usize, cols: u32) -> Metrics {
    let (d, ids) = build(Shape { k: 2, lead: 0.0, notes, lines, cols });
    let l = lay_out(&d);
    let (a, b, r) = (body_at(&l, 0).unwrap(), body_at(&l, 1).unwrap(), body_at(&l, 2).unwrap());
    assert_eq!((a.0, b.0, r.0), (0, 0, 0));
    let pieces = note_pieces(&l, ids[0]);
    assert_eq!(pieces.iter().map(|p| p.3).sum::<usize>(), lines, "each note line is one line: {pieces:?}");
    let top = pieces.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let bottom = pieces.iter().map(|p| p.2).fold(f32::MIN, f32::max);
    let first = pieces.iter().min_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
    Metrics {
        top: a.2,
        pitch: b.2 - a.2,
        ref_h: r.3 - r.2,
        note_h: bottom - top,
        line1: (first.2 - first.1) / first.3 as f32,
        bottom: d.last_section.page_h - d.last_section.margin_bottom,
    }
}

/// The document whose REF line would leave `room` points below it on page 1.
fn with_room(notes: usize, lines: usize, cols: u32, room: f32) -> (Document, Vec<u32>, Metrics) {
    let m = measure(notes, lines, cols);
    let above = m.bottom - m.top - m.ref_h - room;
    assert!(above > m.pitch * 2.0, "room {room} leaves space for the paragraphs above");
    let k = (above / m.pitch).floor() as usize;
    let lead = above - k as f32 * m.pitch;
    let (d, ids) = build(Shape { k, lead, notes, lines, cols });
    (d, ids, m)
}

/// The room below the REF line (block `k`) when it is on page 1.
fn room_below(l: &DocLayout, m: &Metrics, k: usize) -> f32 {
    let r = body_at(l, k).unwrap();
    assert_eq!(r.0, 0, "REF is on page 1");
    m.bottom - r.3
}

fn ref_block(d: &Document) -> usize {
    d.body.len() - 4
}

#[test]
fn line_stays_when_first_note_starts() {
    let m = measure(6, 3, 1);
    let room = NOTE_SEP + 3.0 * m.note_h + m.line1 / 2.0;
    let (d, ids, m) = with_room(6, 3, 1, room);
    let l = lay_out(&d);
    let k = ref_block(&d);
    assert!((room_below(&l, &m, k) - room).abs() < 0.5, "the room is set as asked");
    for (i, id) in ids.iter().enumerate() {
        let want = if i < 3 { 0 } else { 1 };
        assert_eq!(note_lines_by_page(&l, *id), vec![(want, 3)], "note {} whole on page {}", i + 1, want + 1);
    }
    assert_eq!(body_at(&l, k + 1).unwrap().0, 1, "T1 starts page 2");
    // Note 4 is carried whole: page 2 has the normal separator.
    assert!(separators(&l, 1).iter().any(|w| (w - NOTE_SEP_W).abs() < 0.5), "{:?}", separators(&l, 1));
}

#[test]
fn overflowing_note_splits_at_a_line() {
    let m = measure(6, 3, 1);
    let room = NOTE_SEP + 2.0 * m.note_h + 1.5 * m.line1;
    let (d, ids, m) = with_room(6, 3, 1, room);
    let l = lay_out(&d);
    let k = ref_block(&d);
    assert!((room_below(&l, &m, k) - room).abs() < 0.5);
    assert_eq!(note_lines_by_page(&l, ids[0]), vec![(0, 3)]);
    assert_eq!(note_lines_by_page(&l, ids[1]), vec![(0, 3)]);
    assert_eq!(note_lines_by_page(&l, ids[2]), vec![(0, 1), (1, 2)], "note 3: one line, then the rest");
    for id in &ids[3..] {
        assert_eq!(note_lines_by_page(&l, *id), vec![(1, 3)]);
    }
    // The rest of note 3 comes first on page 2, under the continuation separator.
    let rest_top = note_pieces(&l, ids[2]).iter().filter(|p| p.0 == 1).map(|p| p.1).fold(f32::MAX, f32::min);
    let four_top = note_pieces(&l, ids[3]).iter().map(|p| p.1).fold(f32::MAX, f32::min);
    assert!(rest_top < four_top);
    let text_w = d.last_section.text_width();
    assert!(separators(&l, 1).iter().any(|w| (w - text_w).abs() < 0.5), "continuation separator: {:?}", separators(&l, 1));
    assert_eq!(body_at(&l, k + 1).unwrap().0, 1);
}

#[test]
fn second_note_carried_whole() {
    let m = measure(2, 1, 1);
    let room = NOTE_SEP + 1.5 * m.note_h;
    let (d, ids, m) = with_room(2, 1, 1, room);
    let l = lay_out(&d);
    let k = ref_block(&d);
    assert!((room_below(&l, &m, k) - room).abs() < 0.5);
    assert_eq!(note_lines_by_page(&l, ids[0]), vec![(0, 1)]);
    assert_eq!(note_lines_by_page(&l, ids[1]), vec![(1, 1)]);
    assert!(separators(&l, 1).iter().any(|w| (w - NOTE_SEP_W).abs() < 0.5), "normal separator: {:?}", separators(&l, 1));
    assert_eq!(body_at(&l, k + 1).unwrap().0, 1, "T1 on page 2");
}

#[test]
fn line_moves_when_no_note_line_fits() {
    let m = measure(2, 1, 1);
    let room = NOTE_SEP + m.line1 / 2.0;
    let (d, ids, _) = with_room(2, 1, 1, room);
    let l = lay_out(&d);
    let k = ref_block(&d);
    assert_eq!(body_at(&l, k - 1).unwrap().0, 0, "the line above stays on page 1");
    assert_eq!(body_at(&l, k).unwrap().0, 1, "REF moves to page 2");
    for id in &ids {
        assert_eq!(note_lines_by_page(&l, *id), vec![(1, 1)]);
    }
}

#[test]
fn single_long_note_still_splits() {
    let m = measure(1, 6, 1);
    let room = NOTE_SEP + 2.5 * m.line1;
    assert!(room < NOTE_SEP + m.note_h);
    let (d, ids, m) = with_room(1, 6, 1, room);
    let l = lay_out(&d);
    let k = ref_block(&d);
    assert!((room_below(&l, &m, k) - room).abs() < 0.5);
    let by_page = note_lines_by_page(&l, ids[0]);
    assert_eq!(by_page.len(), 2, "split: {by_page:?}");
    assert_eq!(by_page[0].0, 0);
    assert_eq!(by_page[1].0, 1);
    assert_eq!(by_page[0].1 + by_page[1].1, 6);
}

#[test]
fn notes_stay_in_reference_order() {
    let m = measure(6, 3, 1);
    let room = NOTE_SEP + 3.0 * m.note_h + m.line1 / 2.0;
    let (d, ids, _) = with_room(6, 3, 1, room);
    let l = lay_out(&d);
    let mut seen: Vec<(usize, f32, u32)> = Vec::new();
    for id in &ids {
        for (pg, top, ..) in note_pieces(&l, *id) {
            seen.push((pg, top, *id));
        }
    }
    seen.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut order: Vec<u32> = seen.iter().map(|s| s.2).collect();
    order.dedup();
    assert_eq!(order, ids);
}

#[test]
fn two_columns_end_the_column_only() {
    let m = measure(6, 3, 2);
    let room = NOTE_SEP + 3.0 * m.note_h + m.line1 / 2.0;
    let (d, ids, m) = with_room(6, 3, 2, room);
    let l = lay_out(&d);
    let k = ref_block(&d);
    assert!((room_below(&l, &m, k) - room).abs() < 0.5);
    let (r, t1) = (body_at(&l, k).unwrap(), body_at(&l, k + 1).unwrap());
    assert_eq!(t1.0, 0, "T1 stays on page 1");
    assert!(t1.1 > r.1 + 100.0, "T1 starts column 2: REF at x {}, T1 at x {}", r.1, t1.1);
    assert!((t1.2 - m.top).abs() < 0.5, "T1 is at the top of column 2");
    assert_eq!(note_lines_by_page(&l, ids[0]), vec![(0, 3)]);
    assert_eq!(note_lines_by_page(&l, ids[5]), vec![(1, 3)]);
}

#[test]
fn thousand_notes_on_one_line_end() {
    const N: usize = 1000;
    let (d, ids) = build(Shape { k: 3, lead: 0.0, notes: N, lines: 1, cols: 1 });
    let l = lay_out(&d);
    let m = measure(1, 1, 1);
    let per_page = ((m.bottom - m.top - NOTE_SEP) / m.line1).floor().max(1.0) as usize;
    let bound = 1 + MAX_NOTE_PAGES + N.div_ceil(per_page);
    assert!(l.pages.len() <= bound, "{} pages, at most {bound}", l.pages.len());
    assert!(!note_pieces(&l, ids[0]).is_empty());
}
