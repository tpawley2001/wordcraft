//! A keep-with-next chain ends at a page or column break inside one of its links: the chain is
//! measured up to and including the break line, nothing after it (Word, in every compatibility
//! mode). A heading kept with a paragraph holding a page break stays at the foot of the page
//! with that paragraph's lines before the break, instead of moving and leaving the page blank.

use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{COMPAT_MODE_CURRENT, Pos, para_block};

/// A Word 2010 document (compatibility mode 14).
const MODE_2010: u32 = 14;

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn plain() -> ParaProps {
    ParaProps {
        space_before: Some(0.0),
        space_after: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(1.0)),
        widow_control: Some(false),
        ..Default::default()
    }
}

fn kept() -> ParaProps {
    ParaProps { keep_next: Some(true), ..plain() }
}

fn para(text: &str, p: &ParaProps) -> Arc<Block> {
    let mut x = Paragraph::with_text(text, Default::default());
    x.props = p.clone();
    para_block(x)
}

fn page_at(l: &DocLayout, block: usize, offset: usize) -> usize {
    l.caret(&Pos::body(block, offset)).unwrap().page
}

/// One-line plain paragraphs that fit on the first page (every line in these fixtures is one
/// line of the default font, so the page holds this many of them).
fn lines_per_page(mode: u32) -> usize {
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    d.body = (0..400).map(|i| para(&format!("F{i}"), &plain())).collect();
    let l = lay(&d);
    (0..400).take_while(|i| page_at(&l, *i, 0) == 0).count()
}

/// `room` lines left on page 1 after the filler, then the paragraphs `links` (in order, all
/// kept with the next) and a last paragraph `last`. Returns the document and the body
/// index of the first link.
fn chain_doc(mode: u32, room: usize, links: &[&str], last: &str) -> (Document, usize) {
    let cap = lines_per_page(mode);
    assert!(cap > room + 10, "page holds {cap} lines");
    let fillers = cap - room;
    let mut d = Document::new();
    d.settings.compat_mode = mode;
    d.body = (0..fillers).map(|i| para(&format!("F{i}"), &plain())).collect();
    for t in links {
        d.body.push(para(t, &kept()));
    }
    d.body.push(para(last, &plain()));
    (d, fillers)
}

/// Case 1 (and 5): H0 -> K (`text`, page break, `more`) -> A with two lines left: H0 and K's
/// line before the break fit, the whole chain (four lines) would not.
fn break_in_second_link(mode: u32) {
    let (d, h0) = chain_doc(mode, 2, &["H0", "text\u{000C}more"], "after");
    let l = lay(&d);
    let k = h0 + 1;
    let more = "text\u{000C}".len();
    assert_eq!((page_at(&l, h0, 0), page_at(&l, k, 0)), (0, 0), "mode {mode}: H0 and K's first line stay");
    assert_eq!((page_at(&l, k, more), page_at(&l, k + 1, 0)), (1, 1), "mode {mode}: after the break");
    assert_eq!(l.pages.len(), 2, "mode {mode}: no blank page");
}

#[test]
fn a_chain_ends_at_a_page_break_in_a_later_link() {
    break_in_second_link(MODE_2010);
}

#[test]
fn a_chain_ends_at_a_page_break_in_a_current_mode_document() {
    break_in_second_link(COMPAT_MODE_CURRENT);
}

#[test]
fn without_the_break_the_chain_still_moves() {
    // Case 2: K on one line, three lines of chain in two lines of room.
    let (d, h0) = chain_doc(MODE_2010, 2, &["H0", "text more"], "after");
    let l = lay(&d);
    let got: Vec<usize> = (h0..h0 + 3).map(|b| page_at(&l, b, 0)).collect();
    assert_eq!(got, vec![1; 3]);
    assert_eq!(l.pages.len(), 2);
}

#[test]
fn a_heading_chain_up_to_a_break_stays_at_the_page_foot() {
    // Case 3: H1 -> H2 (a page break after its first line, then two more lines) -> a five-line P,
    // three lines above the page bottom. H1 and H2 up to its break take two lines.
    let p = (0..5).map(|i| format!("P{i}")).collect::<Vec<_>>().join("\n");
    let (d, h1) = chain_doc(MODE_2010, 3, &["H1", "first\u{000C}second\nthird"], &p);
    let l = lay(&d);
    let h2 = h1 + 1;
    assert_eq!((page_at(&l, h1, 0), page_at(&l, h2, 0)), (0, 0));
    assert_eq!(page_at(&l, h2, "first\u{000C}".len()), 1);
    assert_eq!(page_at(&l, h2 + 1, 0), 1);
    // The filler and the chain's start fill page 1; the rest (2 + 5 lines) fits on page 2.
    assert_eq!(l.pages.len(), 2);
}

#[test]
fn a_chain_without_a_break_too_tall_for_the_room_moves_together() {
    // Case 4: H1 -> H2 (three lines) -> P (three lines): seven lines in four lines of room, well
    // within an empty page.
    let (d, h1) = chain_doc(MODE_2010, 4, &["H1", "a\nb\nc"], "x\ny\nz");
    let l = lay(&d);
    let got: Vec<usize> = (h1..h1 + 3).map(|b| page_at(&l, b, 0)).collect();
    assert_eq!(got, vec![1; 3]);
    assert_eq!(l.pages.len(), 2);
}
