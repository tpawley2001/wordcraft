//! A heading kept with the next paragraph moves to the next page with it when that paragraph does
//! not fit in the room left on the page: with footnotes on the page, the room is what is left above
//! the note area (separator and notes). Word (Microsoft 365, a Word 97-2003 report: a keep-with-next
//! Heading 1 near the bottom of a page with footnotes, then a 2-line paragraph) starts the next page
//! with the heading; we left the heading alone at the bottom of the page above the notes.
//!
//! Rows 3 and 4 (no notes) are the controls; rows 1, 2 and 5 (with notes) are to be added per the
//! task spec, building the body with footnote references as `notes_overflow_last_line.rs` does.

#[allow(unused_imports)]
use crate::*;
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

fn para(text: &str, p: &ParaProps) -> Arc<Block> {
    let mut x = Paragraph::with_text(text, Default::default());
    x.props = p.clone();
    para_block(x)
}

fn page_of(l: &DocLayout, block: usize) -> usize {
    l.caret(&Pos::body(block, 0)).unwrap().page
}

/// One-line plain paragraphs that fit on the first page.
fn lines_per_page() -> usize {
    let mut d = Document::new();
    d.body = (0..400).map(|i| para(&format!("F{i}"), &plain())).collect();
    let l = lay(&d);
    (0..400).take_while(|i| page_of(&l, *i) == 0).count()
}

/// `fillers` one-line paragraphs, a one-line keep-with-next (+ keep lines) heading, then a
/// 2-line paragraph with widow control. Returns the document and the heading's block.
fn heading_doc(fillers: usize) -> (Document, usize) {
    let mut d = Document::new();
    d.body = (0..fillers).map(|i| para(&format!("F{i}"), &plain())).collect();
    let heading = ParaProps { keep_next: Some(true), keep_lines: Some(true), ..plain() };
    d.body.push(para("HEADING", &heading));
    d.body.push(para("N1\nN2", &plain()));
    (d, fillers)
}

#[test]
fn row3_no_notes_heading_moves_when_the_next_paragraph_does_not_fit() {
    let cap = lines_per_page();
    let (d, h) = heading_doc(cap - 2);
    let l = lay(&d);
    assert_eq!((page_of(&l, h), page_of(&l, h + 1)), (1, 1), "{cap} lines a page");
}

#[test]
fn row4_no_notes_all_on_the_page_when_both_fit() {
    let cap = lines_per_page();
    let (d, h) = heading_doc(cap - 3);
    let l = lay(&d);
    assert_eq!((page_of(&l, h), page_of(&l, h + 1)), (0, 0), "{cap} lines a page");
}
