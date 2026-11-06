//! Keep with next on a chain of paragraphs too long for one page: Word lets the chain flow and
//! break at each page end (a Word 97-2003 file with a 24-paragraph kept chain prints on 3 full pages
//! in Microsoft 365 Word), instead of moving the chain's tail to a new page.

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
        widow_control: Some(false),
        ..Default::default()
    }
}

fn para(text: &str, p: &ParaProps) -> Arc<Block> {
    let mut x = Paragraph::with_text(text, Default::default());
    x.props = p.clone();
    para_block(x)
}

/// A paragraph of `n` lines (line breaks), each line a short word.
fn lines(tag: &str, n: usize) -> String {
    (0..n).map(|i| format!("{tag}{i}")).collect::<Vec<_>>().join("\n")
}

fn page_of(l: &DocLayout, block: usize) -> usize {
    l.caret(&Pos::body(block, 0)).unwrap().page
}

/// Lines of one-line plain paragraphs that fit on the first page.
fn lines_per_page() -> usize {
    let mut d = Document::new();
    d.body = (0..400).map(|i| para(&format!("F{i}"), &plain())).collect();
    let l = lay(&d);
    (0..400).take_while(|i| page_of(&l, *i) == 0).count()
}

const LINKS: usize = 24;
const LINK_LINES: usize = 7;
const LAST_LINES: usize = 3;

fn chain_doc(fillers: usize, link_lines: usize, links: usize) -> Document {
    let mut d = Document::new();
    d.body = (0..fillers).map(|i| para(&format!("F{i}"), &plain())).collect();
    let kept = ParaProps { keep_next: Some(true), ..plain() };
    for k in 0..links {
        d.body.push(para(&lines(&format!("K{k}_"), link_lines), &kept));
    }
    d.body.push(para(&lines("L", LAST_LINES), &plain()));
    d
}

#[test]
fn a_chain_longer_than_a_page_flows_and_breaks_at_the_page_end() {
    let cap = lines_per_page();
    assert!(cap > LINK_LINES * 2 && cap < LINKS * LINK_LINES, "page holds {cap} lines");
    for fillers in [0, 10] {
        let d = chain_doc(fillers, LINK_LINES, LINKS);
        let l = lay(&d);
        // Each paragraph starts where the lines before it end: pages are filled.
        let mut before = fillers;
        let mut got = Vec::new();
        let mut want = Vec::new();
        for k in 0..=LINKS {
            let block = fillers + k;
            got.push(page_of(&l, block));
            want.push(before / cap);
            before += if k < LINKS { LINK_LINES } else { LAST_LINES };
        }
        assert_eq!(got, want, "fillers {fillers}, {cap} lines a page");
        assert_eq!(l.pages.len(), before.div_ceil(cap), "fillers {fillers}: page count");
    }
}

#[test]
fn a_chain_that_fits_on_a_page_still_moves_together() {
    let cap = lines_per_page();
    // Four kept 7-line paragraphs and the 3-line one after them fit on an empty page; start them
    // with only 10 lines of room left on page 1, so the chain moves over whole.
    let links = 4;
    assert!(links * LINK_LINES + LAST_LINES < cap, "page holds {cap} lines");
    let fillers = cap - 10;
    let d = chain_doc(fillers, LINK_LINES, links);
    let l = lay(&d);
    let got: Vec<usize> = (0..=links).map(|k| page_of(&l, fillers + k)).collect();
    assert_eq!(got, vec![1; links + 1]);
}
