//! Footnotes in a section with two or more columns, before compatibility mode 15 (ECMA-376 Part 1
//! §17.15.3.4 `compatibilityMode`; every binary .doc and older DOCX): each note sits under the
//! column whose text refers to it (§17.11.14 `w:footnoteReference`), wrapped to that column's
//! width, and only that column's text ends above it. From mode 15 the page keeps one note area.
//!
//! Shapes measured from Word's PDFs of the same documents (US Letter, 1" margins, two 216pt columns
//! 36pt apart, 100 one-line paragraphs): a 40-word note referred to in column 1 took 7 lines under
//! column 1 only (column 1 held 40 lines, column 2 all 46); a second, one-line note referred to in
//! column 2 sat under column 2 (45 lines above it); a note too long for column 1 continued at the
//! foot of column 2 of the same page. These tests use the test fonts, so they assert which column
//! and page, not Word's point values.

use super::*;
use wordcraft_doc::section::Columns;

const COL1_X: f32 = 72.0;
const COL2_X: f32 = 324.0;
const PARAS: usize = 100;

/// 100 one-line body paragraphs in `columns` equal columns 36pt apart, in compatibility `mode`.
fn doc(columns: u32, mode: u32) -> Document {
    let text: Vec<String> = (1..=PARAS).map(|i| format!("Line {i}")).collect();
    let mut d = Document::from_text(&text.join("\n"));
    d.settings.compat_mode = mode;
    d.last_section = SectionProps { columns: Columns { count: columns, space: 36.0, ..Columns::default() }, ..SectionProps::default() };
    d
}

fn lay_out(d: &Document) -> DocLayout {
    crate::test_fonts();
    lay(d)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

/// The body paragraphs (their indexes) placed on `page` in the column whose left edge is `x`.
fn body_in(l: &DocLayout, page: usize, x: f32) -> Vec<u32> {
    l.pages[page]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, x: ix, l0: 0, .. } if path.depth() == 0 && near(*ix, x) => Some(path.0[0]),
            _ => None,
        })
        .collect()
}

/// Every placed piece of note `id`: (page, left edge, top, lines).
fn note_pieces(l: &DocLayout, id: u32) -> Vec<(usize, f32, f32, usize)> {
    let mut out = Vec::new();
    for (pi, p) in l.pages.iter().enumerate() {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Part(s), x, y, l0, l1, .. } = it
                && *s == id
            {
                out.push((pi, *x, *y, l1 - l0));
            }
        }
    }
    out
}

/// The bottom of the lowest body line on `page` in the column at `x`.
fn body_bottom(l: &DocLayout, page: usize, x: f32) -> f32 {
    l.pages[page]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, para, x: ix, y, l0, l1, .. } if near(*ix, x) => {
                Some(*y + para.lines[*l0..*l1].iter().map(|ln| ln.height).sum::<f32>())
            }
            _ => None,
        })
        .fold(0.0, f32::max)
}

fn forty_words() -> String {
    (1..=40).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ")
}

/// How many one-line paragraphs a column holds when the page has no notes.
fn full_column() -> usize {
    let l = lay_out(&doc(2, 14));
    let n = body_in(&l, 0, COL1_X).len();
    assert!(n > 20, "a full column holds {n} lines");
    assert_eq!(body_in(&l, 0, COL2_X).len(), n, "both columns are full without notes");
    n
}

#[test]
fn a_note_sits_under_the_column_that_refers_to_it() {
    let full = full_column();
    for mode in [11, 12, 14] {
        let mut d = doc(2, mode);
        let id = footnote(&mut d, &Pos::body(4, 6), &forty_words());
        let l = lay_out(&d);
        let pieces = note_pieces(&l, id);
        assert!(!pieces.is_empty(), "mode {mode}: the note is placed");
        for (page, x, ..) in &pieces {
            assert_eq!(*page, 0, "mode {mode}: {pieces:?}");
            assert!(near(*x, COL1_X), "mode {mode}: under column 1: {pieces:?}");
        }
        let (c1, c2) = (body_in(&l, 0, COL1_X).len(), body_in(&l, 0, COL2_X).len());
        assert_eq!(c2, full, "mode {mode}: column 2 has no notes and stays full (column 1 holds {c1})");
        assert!(c1 < full, "mode {mode}: column 1 ends above its note: {c1} of {full}");
        let top = pieces.iter().map(|p| p.2).fold(f32::MAX, f32::min);
        assert!(top >= body_bottom(&l, 0, COL1_X), "mode {mode}: the note starts below column 1's text");
        assert!(top < body_bottom(&l, 0, COL2_X), "mode {mode}: column 2's text runs on beside the note");
    }
}

#[test]
fn a_note_is_wrapped_to_its_column() {
    let lines = |columns: u32, mode: u32| {
        let mut d = doc(columns, mode);
        let id = footnote(&mut d, &Pos::body(4, 6), &forty_words());
        note_pieces(&lay_out(&d), id).iter().map(|p| p.3).sum::<usize>()
    };
    let (wide, narrow) = (lines(1, 14), lines(2, 14));
    assert!(wide >= 1);
    // 216pt instead of 468pt: more than twice the lines, give or take one.
    assert!(narrow >= wide * 2 - 1 && narrow > wide, "column-wide {narrow} lines, page-wide {wide}");
}

#[test]
fn each_column_carries_its_own_notes() {
    let full = full_column();
    let mut d = doc(2, 14);
    let first = footnote(&mut d, &Pos::body(4, 6), &forty_words());
    // The paragraph that would start a third column of full ones: column 1 ends early above its
    // note, so it is in column 2 (and it is there too while the notes are page-wide).
    let in_col2 = full;
    let second = footnote(&mut d, &Pos::body(in_col2, 4), "Short.");
    let l = lay_out(&d);
    assert!(body_in(&l, 0, COL2_X).contains(&(in_col2 as u32)), "the second reference is in column 2");
    let (p1, p2) = (note_pieces(&l, first), note_pieces(&l, second));
    assert!(!p1.is_empty() && p1.iter().all(|p| p.0 == 0 && near(p.1, COL1_X)), "note 1 under column 1: {p1:?}");
    assert!(!p2.is_empty() && p2.iter().all(|p| p.0 == 0 && near(p.1, COL2_X)), "note 2 under column 2 of page 1: {p2:?}");
    let (c1, c2) = (body_in(&l, 0, COL1_X).len(), body_in(&l, 0, COL2_X).len());
    assert!(c1 < c2 && c2 < full, "column 1 loses its long note, column 2 its short one: {c1} {c2} of {full}");
    assert!(p2[0].2 >= body_bottom(&l, 0, COL2_X), "note 2 is below column 2's text");
}

#[test]
fn a_note_too_long_for_its_column_continues_at_the_foot_of_the_next() {
    let full = full_column();
    let mut d = doc(2, 14);
    // As many one-line note paragraphs as a column holds lines: below five lines of text and the
    // separator they overrun column 1 by a few rows, which fit at the foot of column 2.
    let rows: Vec<_> =
        (1..=full).map(|i| wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(&format!("Row {i}"), Default::default()))).collect();
    let id = d.add_part(wordcraft_doc::PartKind::Footnote, rows);
    d.insert_object(
        &Pos::body(4, 6),
        InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id, custom: String::new() },
        &Default::default(),
    )
    .unwrap();
    let l = lay_out(&d);
    assert_eq!(body_in(&l, 0, COL1_X), vec![0, 1, 2, 3, 4], "column 1 ends with the reference line");
    let pieces = note_pieces(&l, id);
    assert!(pieces.iter().all(|p| p.0 == 0), "the whole note is on page 1: {pieces:?}");
    let in1 = pieces.iter().filter(|p| near(p.1, COL1_X)).count();
    let in2 = pieces.iter().filter(|p| near(p.1, COL2_X)).count();
    assert!(in1 > full / 2, "most rows are under column 1: {in1}");
    assert!(in2 >= 1 && in1 + in2 == pieces.len(), "the rest continues in column 2: {in1} + {in2} of {}", pieces.len());
    let col2 = body_in(&l, 0, COL2_X);
    assert_eq!(col2.first(), Some(&5), "column 2 starts with the next line");
    assert!(col2.len() >= full / 2 && col2.len() < full, "column 2's text ends above the continued rows: {}", col2.len());
    let cont_top = pieces.iter().filter(|p| near(p.1, COL2_X)).map(|p| p.2).fold(f32::MAX, f32::min);
    assert!(cont_top >= body_bottom(&l, 0, COL2_X), "the continued rows are below column 2's text");
    assert!(!body_in(&l, 1, COL1_X).is_empty(), "page 2 goes on with body text");
}

/// Guard (passes before the change): from mode 15 the page keeps one note area and both columns
/// end above it.
#[test]
fn mode_15_keeps_one_note_area_for_the_page() {
    let full = full_column();
    let mut d = doc(2, 15);
    let id = footnote(&mut d, &Pos::body(4, 6), &forty_words());
    let l = lay_out(&d);
    let (c1, c2) = (body_in(&l, 0, COL1_X).len(), body_in(&l, 0, COL2_X).len());
    assert_eq!(c1, c2, "both columns end above the page's notes");
    assert!(c1 < full);
    assert!(note_pieces(&l, id).iter().all(|p| p.0 == 0));
}

/// Guard (passes before the change): a one-column section is laid out as before in every mode.
#[test]
fn one_column_is_unchanged() {
    let pieces = |mode: u32| {
        let mut d = doc(1, mode);
        let id = footnote(&mut d, &Pos::body(4, 6), &forty_words());
        let l = lay_out(&d);
        (note_pieces(&l, id).iter().map(|p| (p.0, p.1.round() as i32, p.2.round() as i32, p.3)).collect::<Vec<_>>(), body_in(&l, 0, COL1_X).len())
    };
    let (m14, m15) = (pieces(14), pieces(15));
    assert_eq!(m14, m15);
    assert!(!m14.0.is_empty() && m14.0.iter().all(|p| p.0 == 0 && p.1 == 72), "{m14:?}");
}
