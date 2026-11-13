//! Contextual spacing (`w:contextualSpacing`: no space between neighbouring paragraphs of one
//! style) between footnotes. Word (Microsoft 365, synthetic one-paragraph RTF files: letter, 1 in margins,
//! Times New Roman 10 pt notes at the page bottom) lays the footnotes of a document out as one
//! run of paragraphs in document order: the last paragraph of a note and the first of the next
//! note are spaced as two neighbouring body paragraphs, wherever the two notes land.
//!
//! Styles: A and B = space after 12 pt with contextual spacing, C = space after 12 pt without it,
//! D = space before 12 pt with contextual spacing.

use super::*;
use std::sync::Arc;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::styles::{Style, StyleKind};
use wordcraft_doc::{Pos, para_block};

const TOL: f32 = 0.05;

fn chr(size: f32) -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(size), ..Default::default() }
}

fn para(text: &str, size: f32, style: Option<&str>) -> Paragraph {
    let mut p = Paragraph::with_text(text, chr(size));
    p.props.style = style.map(Into::into);
    if style.is_none() {
        p.props.space_before = Some(0.0);
        p.props.space_after = Some(0.0);
    }
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p
}

fn add_styles(d: &mut Document) {
    let style = |id: &str, before: f32, after: f32, contextual: bool| Style {
        id: id.into(),
        name: id.into(),
        kind: StyleKind::Paragraph,
        para: ParaProps {
            space_before: Some(before),
            space_after: Some(after),
            contextual_spacing: Some(contextual),
            line_spacing: Some(LineSpacing::Multiple(1.0)),
            ..Default::default()
        },
        ..Style::default()
    };
    let styles = Arc::make_mut(&mut d.styles);
    styles.upsert(style("A", 0.0, 12.0, true));
    styles.upsert(style("B", 0.0, 12.0, true));
    styles.upsert(style("C", 0.0, 12.0, false));
    styles.upsert(style("D", 12.0, 0.0, true));
}

/// A note: its paragraphs' styles.
type Note<'a> = &'a [&'a str];

/// A document of `body` one-line body paragraphs; body paragraph `at` refers to `notes[..]` (in
/// order); with `page2`, the last note is referred to from a paragraph on page 2 (after a page
/// break). Returns the document and the notes' ids in order.
fn build(body: usize, notes: &[Note], page2: bool) -> (Document, Vec<u32>) {
    let mut d = Document::from_text("");
    add_styles(&mut d);
    d.settings.compat_mode = 15;
    let mut blocks: Vec<_> = (0..body.max(1)).map(|i| para_block(para(&format!("Body {i}"), 12.0, None))).collect();
    if page2 {
        let mut p = para("Second page", 12.0, None);
        p.props.page_break_before = Some(true);
        blocks.push(para_block(p));
    }
    d.body = blocks;
    let ids: Vec<u32> = notes
        .iter()
        .enumerate()
        .map(|(n, styles)| {
            let blocks = styles.iter().enumerate().map(|(j, s)| para_block(para(&format!("Note {n} paragraph {j}"), 10.0, Some(s)))).collect();
            d.add_part(wordcraft_doc::PartKind::Footnote, blocks)
        })
        .collect();
    let last_block = d.body.len() - 1;
    let refs: Vec<(usize, u32)> = ids.iter().enumerate().map(|(i, id)| (if page2 && i + 1 == ids.len() { last_block } else { 0 }, *id)).collect();
    // Inserted last first at the end of their paragraph's text, so they read 1..N.
    let lens: Vec<usize> = d
        .body
        .iter()
        .map(|b| match &**b {
            Block::Para(p) => p.text.len(),
            Block::Table(_) => 0,
        })
        .collect();
    for (block, id) in refs.iter().rev() {
        let len = lens[*block];
        d.insert_object(
            &Pos::body(*block, len),
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

/// Every placed note line of note `id`: (page, top, bottom), in page and item order.
fn note_lines(l: &DocLayout, id: u32) -> Vec<(usize, f32, f32)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Part(s), para, y, l0, l1, .. } = it
                && *s == id
            {
                let Some(first) = para.lines.get(*l0) else { continue };
                for k in *l0..*l1 {
                    let line = &para.lines[k];
                    let top = y + line.top - first.top;
                    out.push((pi, top, top + line.height));
                }
            }
        }
    }
    out
}

/// The lines of all `ids`, in note order.
fn all_lines(l: &DocLayout, ids: &[u32]) -> Vec<(usize, f32, f32)> {
    ids.iter().flat_map(|id| note_lines(l, *id)).collect()
}

fn text_bottom(d: &Document) -> f32 {
    d.last_section.page_h - d.last_section.margin_bottom
}

/// The tops of the lines of one-paragraph `notes` laid out on one page, their pitch and the bottom
/// of the last.
fn stacked(notes: &[Note]) -> (Vec<f32>, f32, f32, f32) {
    let (d, ids) = build(1, notes, false);
    let l = lay_out(&d);
    let lines = all_lines(&l, &ids);
    assert_eq!(lines.len(), notes.iter().map(|n| n.len()).sum::<usize>(), "one line per note paragraph: {lines:?}");
    assert!(lines.iter().all(|(p, ..)| *p == 0), "all on page 1: {lines:?}");
    let pitch = lines[0].2 - lines[0].1;
    let last = lines.last().unwrap().2;
    (lines.iter().map(|l| l.1).collect(), pitch, last, text_bottom(&d))
}

/// The bottom of the single note's line in the one-note control document of style C.
fn control_bottom() -> f32 {
    let (tops, pitch, bottom, _) = stacked(&[&["C"]]);
    assert_eq!(tops.len(), 1);
    assert!(pitch > 0.0);
    bottom
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOL
}

#[test]
fn same_style_notes_join() {
    let (tops, pitch, bottom, text) = stacked(&[&["A"], &["A"], &["A"]]);
    assert!(near(tops[1] - tops[0], pitch), "{tops:?} pitch {pitch}");
    assert!(near(tops[2] - tops[1], pitch), "{tops:?} pitch {pitch}");
    assert!(near(bottom, text - 12.0) || near(bottom, control_bottom()), "the last note keeps its space after: {bottom} vs {text}");
}

#[test]
fn no_contextual_spacing_keeps_space() {
    let (tops, pitch, ..) = stacked(&[&["C"], &["C"], &["C"]]);
    assert!(near(tops[1] - tops[0], pitch + 12.0), "{tops:?} pitch {pitch}");
    assert!(near(tops[2] - tops[1], pitch + 12.0), "{tops:?} pitch {pitch}");
}

#[test]
fn different_styles_keep_space() {
    let (tops, pitch, ..) = stacked(&[&["A"], &["B"], &["A"]]);
    assert!(near(tops[1] - tops[0], pitch + 12.0), "{tops:?} pitch {pitch}");
    assert!(near(tops[2] - tops[1], pitch + 12.0), "{tops:?} pitch {pitch}");
}

#[test]
fn inside_and_across_notes_alike() {
    let (tops, pitch, ..) = stacked(&[&["A", "A"], &["A"]]);
    assert_eq!(tops.len(), 3);
    assert!(near(tops[1] - tops[0], pitch), "{tops:?} pitch {pitch}");
    assert!(near(tops[2] - tops[1], pitch), "{tops:?} pitch {pitch}");
}

#[test]
fn space_before_dropped_between_notes() {
    let (tops, pitch, bottom, text) = stacked(&[&["D"], &["D"], &["D"]]);
    assert!(near(tops[1] - tops[0], pitch), "{tops:?} pitch {pitch}");
    assert!(near(tops[2] - tops[1], pitch), "{tops:?} pitch {pitch}");
    assert!(near(bottom, text) || near(bottom, control_bottom() + 12.0), "no space after: {bottom} vs {text}");
}

#[test]
fn next_note_on_next_page_still_joins() {
    let (d, ids) = build(1, &[&["A"], &["A"]], true);
    let l = lay_out(&d);
    let (a, b) = (note_lines(&l, ids[0]), note_lines(&l, ids[1]));
    assert_eq!((a.len(), b.len()), (1, 1), "{a:?} {b:?}");
    assert_eq!((a[0].0, b[0].0), (0, 1), "one note on each page");
    let text = text_bottom(&d);
    let control = control_bottom();
    assert!(near(a[0].2, text) || near(a[0].2, control + 12.0), "page 1's note loses its space after: {} vs {text}", a[0].2);
    assert!(near(b[0].2, text - 12.0) || near(b[0].2, control), "the last note keeps it: {} vs {text}", b[0].2);
}

#[test]
fn single_note_keeps_space_after() {
    let (_, _, bottom_a, text) = stacked(&[&["A"]]);
    let (_, _, bottom_c, _) = stacked(&[&["C"]]);
    assert!(near(bottom_a, text - 12.0) || near(bottom_a, bottom_c), "A: {bottom_a} vs {text}");
    assert!(near(bottom_c, text - 12.0), "C: {bottom_c} vs {text}");
}

/// Body paragraphs on page 1.
fn page1_body_lines(l: &DocLayout) -> usize {
    l.pages[0].items.iter().filter(|it| matches!(it, Placed::Lines { story: StoryRef::Body, .. })).count()
}

#[test]
fn joined_notes_leave_room_for_body_text() {
    let notes: [Note; 3] = [&["A"], &["A"], &["A"]];
    let (d, _) = build(60, &notes, false);
    let joined = page1_body_lines(&lay_out(&d));
    let notes: [Note; 3] = [&["C"], &["C"], &["C"]];
    let (d, _) = build(60, &notes, false);
    let spaced = page1_body_lines(&lay_out(&d));
    assert!((20..60).contains(&spaced), "the page is full of body text: {spaced}");
    assert!(joined > spaced, "{joined} body lines with joined notes, {spaced} with spaced ones");
}
