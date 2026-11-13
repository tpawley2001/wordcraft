//! A paragraph's second-to-last line refers to more footnotes than fit below it, so the page ends
//! after it with the notes that don't fit carried to the next page. Word (Microsoft 365, synthetic
//! RTF files: letter, 1 in margins, Times New Roman 12 body, 10 pt notes of three lines, widow
//! control on) keeps the paragraph's last line on the page too when it fits above the notes that
//! stay; the overflowing notes give way to it, its own notes are carried after them, and the next
//! paragraph starts the next page. With a fitting note, widow control off, or two lines after the
//! overflowing one, the layout is as before.
//!
//! The room below the overflowing line is set from heights measured here (the test fonts'
//! metrics), so the tests assert which page things land on, not Word's point values.

use super::*;
use crate::notes::{MAX_NOTE_PAGES, NOTE_SEP};
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::para_block;
use wordcraft_doc::props::LineSpacing;

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

/// The test paragraph: AA1, AA2, then "REFA" with `refa` notes, then `after` lines (the last
/// one referring to `last_notes` notes, in `last_size` points); before it `k` one-line
/// paragraphs (the first with `lead` points of space before), after it T1, T2, T3.
#[derive(Clone)]
struct Shape {
    k: usize,
    lead: f32,
    refa: usize,
    lines: usize,
    after: Vec<&'static str>,
    last_notes: usize,
    last_size: f32,
    widow: bool,
}

impl Shape {
    fn new(refa: usize, after: &[&'static str], last_notes: usize) -> Shape {
        Shape { k: 2, lead: 0.0, refa, lines: 3, after: after.to_vec(), last_notes, last_size: 12.0, widow: true }
    }
}

/// The document, the test paragraph's block and its notes' ids in reference order.
fn build(s: &Shape) -> (Document, usize, Vec<u32>) {
    let mut d = Document::from_text("");
    let mut body = Vec::new();
    for i in 1..=s.k {
        let mut p = para(&format!("L{i:02}"), 12.0);
        if i == 1 {
            p.props.space_before = Some(s.lead);
        }
        body.push(para_block(p));
    }
    let mut text = String::from("AA1\nAA2\nREFA");
    let refa_at = text.len();
    for a in &s.after {
        text.push('\n');
        text.push_str(a);
    }
    let last_at = text.len();
    let last_start = text.rfind('\n').map_or(0, |i| i + 1);
    let mut p = para(&text, 12.0);
    p.props.widow_control = Some(s.widow);
    if (s.last_size - 12.0).abs() > 0.01 {
        let size = s.last_size;
        p.format(last_start, last_at, &|c| c.size = Some(size)).unwrap();
    }
    let total = s.refa + s.last_notes;
    let ids: Vec<u32> = (1..=total)
        .map(|n| {
            let blocks = (1..=s.lines).map(|j| para_block(para(&format!("Note {n} line {j}"), 10.0))).collect();
            d.add_part(wordcraft_doc::PartKind::Footnote, blocks)
        })
        .collect();
    // Inserted last first, so they read 1..N.
    for (i, id) in ids.iter().enumerate().rev() {
        let at = if i < s.refa { refa_at } else { last_at };
        p.insert_object(at, InlineObject::NoteRef { kind: NoteKind::Footnote, id: *id, custom: String::new() }, &chr(12.0)).unwrap();
    }
    let block = body.len();
    body.push(para_block(p));
    for t in 1..=3 {
        body.push(para_block(para(&format!("T{t}"), 12.0)));
    }
    d.body = body;
    d.settings.compat_mode = 15;
    (d, block, ids)
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

/// The page line `line` of body paragraph `block` is on, and that piece's top (where its first
/// line starts) and first line index.
fn line_at(l: &DocLayout, block: usize, line: usize) -> Option<(usize, f32, usize)> {
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Body, path, y, l0, l1, .. } = it
                && path.0 == [block as u32]
                && *l0 <= line
                && line < *l1
            {
                return Some((pi, *y, *l0));
            }
        }
    }
    None
}

fn page_of(l: &DocLayout, block: usize, line: usize) -> usize {
    line_at(l, block, line).unwrap().0
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

/// Heights measured with everything on page 1: the body's top, the line pitch, one note's
/// height and its first line's, and the body's bottom.
struct Metrics {
    top: f32,
    pitch: f32,
    note_h: f32,
    line1: f32,
    bottom: f32,
}

fn measure(lines: usize) -> Metrics {
    let mut s = Shape::new(1, &["ZZ1"], 0);
    s.lines = lines;
    let (d, block, ids) = build(&s);
    let l = lay_out(&d);
    let (a, b) = (line_at(&l, 0, 0).unwrap(), line_at(&l, 1, 0).unwrap());
    let p = line_at(&l, block, 0).unwrap();
    assert_eq!((a.0, b.0, p.0), (0, 0, 0));
    let pitch = b.1 - a.1;
    assert!(((p.1 - a.1) - 2.0 * pitch).abs() < 0.01, "the paragraph's lines have the same pitch");
    let pieces = note_pieces(&l, ids[0]);
    assert_eq!(pieces.iter().map(|p| p.3).sum::<usize>(), lines);
    let top = pieces.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let bottom = pieces.iter().map(|p| p.2).fold(f32::MIN, f32::max);
    let first = pieces.iter().min_by(|x, y| x.1.total_cmp(&y.1)).unwrap();
    Metrics {
        top: a.1,
        pitch,
        note_h: bottom - top,
        line1: (first.2 - first.1) / first.3 as f32,
        bottom: d.last_section.page_h - d.last_section.margin_bottom,
    }
}

/// The document whose REFA line would leave `room` points below it on page 1.
fn with_room(mut s: Shape, m: &Metrics, room: f32) -> (Document, usize, Vec<u32>) {
    // REFA is the paragraph's third line.
    let above = m.bottom - m.top - 3.0 * m.pitch - room;
    assert!(above > m.pitch * 2.0, "room {room} leaves space for the paragraphs above");
    s.k = (above / m.pitch).floor() as usize;
    s.lead = above - s.k as f32 * m.pitch;
    build(&s)
}

/// The room below REFA (line 2 of `block`) when it is on page 1.
fn room_below(l: &DocLayout, m: &Metrics, block: usize) -> f32 {
    let (pg, y, l0) = line_at(l, block, 2).unwrap();
    assert_eq!(pg, 0, "REFA is on page 1");
    m.bottom - (y + (3 - l0) as f32 * m.pitch)
}

/// p1: below REFA, the separator, notes 1 and 2, and room for the last line but not for it and
/// note 3's first line.
fn p1_room(m: &Metrics) -> f32 {
    let spare = m.pitch + m.line1 / 2.0;
    assert!(spare < m.line1 + m.pitch && spare >= m.pitch);
    NOTE_SEP + 2.0 * m.note_h + spare
}

#[test]
fn last_line_stays_after_an_overflowing_line() {
    let m = measure(3);
    let room = p1_room(&m);
    let (d, b, ids) = with_room(Shape::new(3, &["ZZ1"], 0), &m, room);
    let l = lay_out(&d);
    assert!((room_below(&l, &m, b) - room).abs() < 0.5, "the room is set as asked");
    for line in 0..4 {
        assert_eq!(page_of(&l, b, line), 0, "line {line} on page 1");
    }
    assert_eq!(note_lines_by_page(&l, ids[0]), vec![(0, 3)]);
    assert_eq!(note_lines_by_page(&l, ids[1]), vec![(0, 3)]);
    assert_eq!(note_lines_by_page(&l, ids[2]).first().map(|p| p.0), Some(1), "note 3 starts on page 2");
    assert_eq!(page_of(&l, b + 1, 0), 1, "T1 on page 2");
}

#[test]
fn last_line_notes_are_carried_in_order() {
    let m = measure(3);
    let room = p1_room(&m);
    let (d, b, ids) = with_room(Shape::new(3, &["REFB"], 1), &m, room);
    let l = lay_out(&d);
    assert_eq!(page_of(&l, b, 2), 0, "REFA on page 1");
    assert_eq!(page_of(&l, b, 3), 0, "REFB on page 1");
    assert_eq!(note_lines_by_page(&l, ids[0]), vec![(0, 3)]);
    assert_eq!(note_lines_by_page(&l, ids[1]), vec![(0, 3)]);
    let mut page2: Vec<(f32, u32)> = Vec::new();
    for id in &ids {
        for (pg, top, ..) in note_pieces(&l, *id) {
            if pg == 1 {
                page2.push((top, *id));
            }
        }
    }
    page2.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut order: Vec<u32> = page2.iter().map(|p| p.1).collect();
    order.dedup();
    assert_eq!(order, vec![ids[2], ids[3]], "page 2: note 3, then note 4");
    assert!(note_pieces(&l, ids[3]).iter().all(|p| p.0 == 1), "note 4 is whole on page 2");
    assert_eq!(page_of(&l, b + 1, 0), 1, "T1 on page 2");
}

#[test]
fn overflow_gives_way_to_the_last_line() {
    let m = measure(3);
    let spare = m.line1 + m.pitch - 1.0;
    assert!(spare >= m.line1 && spare >= m.pitch);
    let room = NOTE_SEP + 2.0 * m.note_h + spare;
    let (d, b, ids) = with_room(Shape::new(3, &["ZZ1"], 0), &m, room);
    let l = lay_out(&d);
    assert!((room_below(&l, &m, b) - room).abs() < 0.5);
    assert_eq!(page_of(&l, b, 3), 0, "ZZ1 on page 1");
    for (pg, _, bottom, _) in note_pieces(&l, ids[2]) {
        assert!(pg != 0 || bottom <= m.bottom + 0.01, "note 3 ends at {bottom} on page 1, below {}", m.bottom);
    }
    let foot = l.pages[0].items.iter().map(|it| crate::notes::v_span(it).1).fold(f32::MIN, f32::max);
    assert!(foot <= m.bottom + 0.01, "page 1's content ends at {foot}, below {}", m.bottom);
    assert_eq!(page_of(&l, b + 1, 0), 1, "T1 on page 2");
}

#[test]
fn fitting_notes_keep_the_ordinary_rule() {
    let m = measure(3);
    // REFA's one note fits whole; REFB's cannot start below it.
    let room = NOTE_SEP + m.note_h + m.pitch + m.line1 / 2.0;
    let (d, b, ids) = with_room(Shape::new(1, &["REFB"], 1), &m, room);
    let l = lay_out(&d);
    assert_eq!(page_of(&l, b, 0), 0);
    assert_eq!(page_of(&l, b, 1), 0);
    assert_eq!(page_of(&l, b, 2), 1, "REFA moves with REFB");
    assert_eq!(page_of(&l, b, 3), 1);
    for id in &ids {
        assert_eq!(note_lines_by_page(&l, *id), vec![(1, 3)]);
    }
}

#[test]
fn widow_control_off_is_unchanged() {
    let m = measure(3);
    let room = p1_room(&m);
    let mut s = Shape::new(3, &["ZZ1"], 0);
    s.widow = false;
    let (d, b, _) = with_room(s, &m, room);
    let l = lay_out(&d);
    assert_eq!(page_of(&l, b, 2), 0, "REFA stays on page 1");
    assert_eq!(page_of(&l, b, 3), 1, "ZZ1 goes to page 2");
}

#[test]
fn two_lines_after_are_unchanged() {
    let m = measure(3);
    let room = p1_room(&m);
    let (d, b, _) = with_room(Shape::new(3, &["ZZ1", "ZZ2"], 0), &m, room);
    let l = lay_out(&d);
    assert_eq!(page_of(&l, b, 2), 0, "REFA stays on page 1");
    assert_eq!(page_of(&l, b, 3), 1, "ZZ1 starts page 2");
    assert_eq!(page_of(&l, b, 4), 1, "ZZ2 on page 2");
}

#[test]
fn last_line_too_tall_keeps_old_rule() {
    let m = measure(3);
    let room = p1_room(&m);
    let mut s = Shape::new(3, &["ZZ1"], 0);
    s.last_size = 96.0;
    let (d, b, ids) = with_room(s, &m, room);
    let l = lay_out(&d);
    assert_eq!(page_of(&l, b, 1), 0, "AA2 stays on page 1");
    assert_eq!(page_of(&l, b, 2), 1, "REFA starts page 2");
    assert_eq!(page_of(&l, b, 3), 1, "the last line is on page 2");
    assert_eq!(note_lines_by_page(&l, ids[0]).first().map(|p| p.0), Some(1));
}

#[test]
fn thousand_notes_then_last_line_end() {
    const N: usize = 1000;
    let mut s = Shape::new(N, &["ZZ1"], 0);
    s.lines = 1;
    let (d, b, ids) = build(&s);
    let l = lay_out(&d);
    let m = measure(1);
    let per_page = ((m.bottom - m.top - NOTE_SEP) / m.line1).floor().max(1.0) as usize;
    let bound = 1 + MAX_NOTE_PAGES + N.div_ceil(per_page);
    assert!(l.pages.len() <= bound, "{} pages, at most {bound}", l.pages.len());
    assert!(!note_pieces(&l, ids[0]).is_empty());
    assert!(line_at(&l, b, 3).is_some(), "the last line is placed");
}
