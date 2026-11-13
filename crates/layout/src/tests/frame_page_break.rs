//! A manual page break (`w:br w:type="page"`, ECMA-376 Part 1 §17.3.3.1) inside a framed
//! paragraph (a text frame, `w:framePr`, §17.3.1.11) starts no new page, as Microsoft 365 Word
//! lays it out: the frame and its contents stay where they are and the text after it goes on on
//! the same page. It is the rule Word applies to a page break in a table cell.
//!
//! Measured against Word's PDFs of two Word 97-2003 documents (a letterhead logo in a frame after
//! a break: one page, not two; two empty frames holding only a break: 377 pages, not 378). The
//! bodies here are built with the model types. A drop-cap paragraph never reaches the resolved
//! frame (`ResolvedPara::frame` is dropped when `drop_cap` is set), so it keeps today's behaviour.

use super::*;
use wordcraft_doc::para::PAGE_BREAK;
use wordcraft_doc::props::{ParaProps, TextFrame};
use wordcraft_doc::{PartKind, Path, Pos, para_block};

/// The letterhead frame: anchored to the page horizontally and to the text vertically.
fn fr() -> TextFrame {
    TextFrame { h_anchor: "page".into(), v_anchor: "text".into(), wrap: "around".into(), x: 29.3, y: 0.05, h_space: 9.0, ..Default::default() }
}

/// The empty frame of the 377-page manual, anchored to the page.
fn fr_empty() -> TextFrame {
    TextFrame {
        w: Some(57.85),
        h: Some(39.05),
        h_anchor: "page".into(),
        v_anchor: "page".into(),
        x: 460.65,
        y: 144.05,
        wrap: "auto".into(),
        h_space: 7.2,
        v_space: 7.2,
        ..Default::default()
    }
}

fn para(text: &str, frame: Option<TextFrame>) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, Default::default());
    p.props = ParaProps { frame, ..Default::default() };
    para_block(p)
}

fn brk(s: &str) -> String {
    s.replace('|', &PAGE_BREAK.to_string())
}

fn doc(body: Vec<Arc<Block>>) -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    d.body = body;
    d
}

fn caret(l: &DocLayout, block: usize, off: usize) -> crate::hit::Caret {
    l.caret(&Pos { story: StoryRef::Body, path: Path::top(block), off })
        .unwrap_or_else(|| panic!("no caret at block {block} offset {off}"))
}

const B: usize = PAGE_BREAK.len_utf8();

fn case1() -> Document {
    doc(vec![para("one", None), para(&brk("|FRAME"), Some(fr())), para("two", None)])
}

/// Row 1: a frame holding a break and then its text stays on the first page with the text around it.
#[test]
fn break_before_frame_text_starts_no_page() {
    let l = lay(&case1());
    assert_eq!(l.pages.len(), 1);
    assert_eq!(caret(&l, 0, 0).page, 0);
    assert_eq!(caret(&l, 1, B).page, 0, "FRAME");
    assert_eq!(caret(&l, 2, 0).page, 0, "two");
}

/// Row 2: an empty page-anchored frame holding only a break starts no page.
#[test]
fn frame_holding_only_a_break_starts_no_page() {
    let l = lay(&doc(vec![para("one", None), para(&brk("|"), Some(fr_empty())), para("two", None)]));
    assert_eq!(l.pages.len(), 1);
    assert_eq!(caret(&l, 2, 0).page, 0);
}

/// Row 3: a break between two words of a frame is ignored too: both stay on one line.
#[test]
fn break_between_words_in_frame_is_ignored() {
    let l = lay(&doc(vec![para("one", None), para(&brk("A|B"), Some(fr())), para("two", None)]));
    assert_eq!(l.pages.len(), 1);
    let (a, b) = (caret(&l, 1, 0), caret(&l, 1, 1 + B));
    assert_eq!((a.page, b.page, caret(&l, 2, 0).page), (0, 0, 0));
    assert!((a.top - b.top).abs() < 0.01, "A and B on one line: {} vs {}", a.top, b.top);
}

/// Row 4: without a frame the break starts a page, as before.
#[test]
fn break_outside_a_frame_still_starts_a_page() {
    let l = lay(&doc(vec![para("one", None), para(&brk("|PLAIN"), None), para("two", None)]));
    assert_eq!(l.pages.len(), 2);
    assert_eq!(caret(&l, 1, B).page, 1, "PLAIN");
    assert_eq!(caret(&l, 2, 0).page, 1, "two");
}

/// Row 5: deep in a document, the framed break takes the room of an empty paragraph and no more
/// (the flow's own page-break checks never see it).
#[test]
fn framed_break_takes_the_room_of_an_empty_paragraph() {
    let build = |t: &str| {
        let mut body: Vec<Arc<Block>> = (1..=40).map(|i| para(&format!("L{i}"), None)).collect();
        body.push(para(t, Some(fr())));
        body.push(para("two", None));
        doc(body)
    };
    let (with, without) = (lay(&build(&brk("|"))), lay(&build("")));
    assert_eq!(with.pages.len(), without.pages.len());
    let (a, b) = (caret(&with, 41, 0), caret(&without, 41, 0));
    assert_eq!((a.page, a.top), (b.page, b.top), "two stands where it stands without the break");
}

/// Row 6: in a header the framed break changes nothing: same header, same body top.
#[test]
fn framed_break_in_a_header_changes_nothing() {
    let build = |t: &str| {
        let mut d = doc(vec![para("Body", None)]);
        let part = d.add_part(PartKind::Header, vec![para("Title", None), para(t, Some(fr())), para("After", None)]);
        d.last_section.headers.default = Some(part);
        (lay(&d), part)
    };
    let ((with, part), (without, _)) = (build(&brk("|FRAME")), build("FRAME"));
    assert_eq!(with.pages.len(), 1);
    assert_eq!(with.pages.len(), without.pages.len());
    let hf = |l: &DocLayout, block: usize, off: usize| {
        l.caret(&Pos { story: StoryRef::Part(part), path: Path::top(block), off }).unwrap_or_else(|| panic!("header caret {block}"))
    };
    assert_eq!(hf(&with, 2, 0).top, hf(&without, 2, 0).top, "same header height");
    assert_eq!(hf(&with, 1, B).top, hf(&without, 1, 0).top, "FRAME where it is without the break");
    assert_eq!(caret(&with, 0, 0).top, caret(&without, 0, 0).top, "same body top");
}

/// Row 7: the break keeps its source offset: the caret after it starts `FRAME`'s line, and a hit
/// on `FRAME`'s last letter maps back to its paragraph offset.
#[test]
fn offsets_after_the_framed_break_line_up() {
    let l = lay(&case1());
    let (after, r) = (caret(&l, 1, B), caret(&l, 1, B + 1));
    assert!((after.top - r.top).abs() < 0.01 && after.x < r.x, "caret after the break starts FRAME's line");
    assert_eq!(after.top, caret(&l, 1, 0).top, "the break makes no line of its own");
    let last = B + "FRAM".len();
    let c = caret(&l, 1, last);
    let next = caret(&l, 1, last + 1);
    let hit = l.hit(c.page, c.x + (next.x - c.x) * 0.25, c.top + c.height / 2.0, StoryRef::Body).expect("hit");
    assert_eq!(hit.path, Path::top(1));
    assert_eq!(hit.off, last);
}
