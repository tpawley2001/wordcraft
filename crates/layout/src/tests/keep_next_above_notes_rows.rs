//! Rows 1, 2 and 5 of the keep-with-next-above-notes task (see `keep_next_above_notes.rs`): the same
//! heading and 2-line paragraph near the bottom of a page whose footnotes take the room the
//! paragraph would need. The page's body is one-line paragraphs, the third of which refers to a
//! footnote of several lines, so a note area sits at the page's foot; the heading's own line fits
//! above it, the paragraph after it does not (it would without the notes). Word moves the heading
//! to the next page with that paragraph, and page 1 keeps its note.
//!
//! With a heading without space after these rows pass as they are: what it took to fail is the
//! heading's space after (12 pt, as the real file's Heading 1 has). The keep check measured the
//! heading's line and the next paragraph's first two lines, but not the space after between them,
//! so with room for the three lines and less than the space after more, the heading stayed and the
//! paragraph went over alone, with or without notes on the page (`row3_space_after_no_notes`).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{Pos, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn plain() -> ParaProps {
    ParaProps {
        space_before: Some(0.0),
        space_after: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(1.0)),
        widow_control: Some(true),
        ..Default::default()
    }
}

fn paragraph(text: &str, p: &ParaProps) -> Paragraph {
    let mut x = Paragraph::with_text(text, Default::default());
    x.props = p.clone();
    x
}

fn para(text: &str, p: &ParaProps) -> Arc<Block> {
    para_block(paragraph(text, p))
}

fn page_of(l: &DocLayout, block: usize) -> usize {
    l.caret(&Pos::body(block, 0)).unwrap().page
}

/// The filler that refers to the footnote.
const NOTED: usize = 2;
/// Lines in the footnote.
const NOTE_LINES: usize = 5;

/// `fillers` one-line paragraphs (filler [`NOTED`] referring to a footnote of [`NOTE_LINES`]
/// lines), `empties` empty paragraphs, then `tail`. Returns the document and the first tail block.
fn doc_with(fillers: usize, empties: usize, tail: Vec<Arc<Block>>, noted: bool, lead: f32) -> (Document, usize) {
    let mut d = Document::new();
    let note = (0..NOTE_LINES).map(|j| para(&format!("Note line {j}"), &plain())).collect();
    let id = d.add_part(wordcraft_doc::PartKind::Footnote, note);
    let mut body: Vec<Arc<Block>> = Vec::new();
    for i in 0..fillers {
        let mut p = paragraph(&format!("F{i}"), &plain());
        if i == 0 {
            p.props.space_before = Some(lead);
        }
        if noted && i == NOTED {
            let at = p.text.len();
            p.insert_object(at, InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &Default::default()).unwrap();
        }
        body.push(para_block(p));
    }
    body.extend((0..empties).map(|_| para("", &plain())));
    let first = body.len();
    body.extend(tail);
    d.body = body;
    (d, first)
}

/// The heading's space after.
const AFTER: f32 = 12.0;

/// One-line plain paragraphs that fit on the first page (above its note when `noted`), the space
/// before the first that leaves (to within 0.25 pt) no room below the last of them, and their pitch.
fn fill(noted: bool) -> (usize, f32, f32) {
    let on_first = |lead: f32| {
        let (d, _) = doc_with(400, 0, Vec::new(), noted, lead);
        let l = lay(&d);
        let pitch = l.caret(&Pos::body(2, 0)).unwrap().top - l.caret(&Pos::body(1, 0)).unwrap().top;
        ((0..400).take_while(|i| page_of(&l, *i) == 0).count(), pitch)
    };
    let (cap, pitch) = on_first(0.0);
    let mut lead = 0.0;
    while on_first(lead + 0.25).0 == cap {
        lead += 0.25;
    }
    (cap, lead, pitch)
}

/// The heading (keep with next, and keep lines when `keep_lines`; 12 pt after) and the 2-line paragraph.
fn tail(keep_lines: bool) -> Vec<Arc<Block>> {
    let heading = ParaProps { keep_next: Some(true), keep_lines: Some(keep_lines), space_after: Some(AFTER), ..plain() };
    vec![para("HEADING", &heading), para("N1\nN2", &plain())]
}

/// The document whose page 1 has `empties` empty paragraphs, then the heading and the 2-line
/// paragraph, with `extra` points of room below the three lines.
fn shaped(noted: bool, empties: usize, keep_lines: bool, extra: f32) -> (Document, usize) {
    let (cap, lead, pitch) = fill(noted);
    // One filler fewer and a line's pitch more before the first, so that stays positive.
    assert!(extra < pitch, "{extra} pt within a line");
    doc_with(cap - 4 - empties, empties, tail(keep_lines), noted, lead + pitch - extra)
}

/// The heading's and next paragraph's pages, and the pages with note lines.
fn pages(d: &Document, h: usize) -> (usize, usize, Vec<usize>) {
    let l = lay(d);
    let note: Vec<usize> = l
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p.items.iter().any(|it| matches!(it, Placed::Lines { story: StoryRef::Part(_), .. })))
        .map(|(i, _)| i)
        .collect();
    (page_of(&l, h), page_of(&l, h + 1), note)
}

#[test]
fn row1_heading_keep_lines_moves_with_the_paragraph_above_notes() {
    let (d, h) = shaped(true, 0, true, AFTER / 2.0);
    assert_eq!(pages(&d, h), (1, 1, vec![0]));
}

#[test]
fn row2_heading_keep_next_only_moves_with_the_paragraph_above_notes() {
    let (d, h) = shaped(true, 0, false, AFTER / 2.0);
    assert_eq!(pages(&d, h), (1, 1, vec![0]));
}

#[test]
fn row3_space_after_no_notes() {
    let (d, h) = shaped(false, 0, true, AFTER / 2.0);
    assert_eq!(pages(&d, h), (1, 1, vec![]));
}

#[test]
fn row4_with_notes_all_on_the_page_when_both_fit() {
    let (d, h) = shaped(true, 0, true, AFTER + 1.0);
    assert_eq!(pages(&d, h), (0, 0, vec![0]));
}

#[test]
fn row4_space_after_no_notes_all_on_the_page() {
    let (d, h) = shaped(false, 0, true, AFTER + 1.0);
    assert_eq!(pages(&d, h), (0, 0, vec![]));
}

#[test]
fn row5_empty_paragraphs_before_the_heading_above_notes() {
    let (d, h) = shaped(true, 12, true, AFTER / 2.0);
    assert_eq!(pages(&d, h), (1, 1, vec![0]));
}

#[test]
fn row5_empty_paragraphs_all_on_the_page_when_both_fit() {
    let (d, h) = shaped(true, 12, true, AFTER + 1.0);
    assert_eq!(pages(&d, h), (0, 0, vec![0]));
}
