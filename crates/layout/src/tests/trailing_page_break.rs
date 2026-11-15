//! A page break that is the last thing in the document starts no page: the document ends on the
//! page that holds the break. Expected pages from Microsoft Word's PDF output.

use crate::*;
use wordcraft_doc::para::{InlineObject, PAGE_BREAK};
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{PartKind, Path, Pos, StoryRef, para_block};

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// Body paragraphs of `text` (`\n` between paragraphs, `PB` a page break), compatibility mode
/// `mode`, a header on every page.
fn doc(text: &str, mode: u32) -> Document {
    crate::test_fonts();
    let mut d = Document::from_text(&text.replace("PB", &PAGE_BREAK.to_string()));
    d.settings.compat_mode = mode;
    let head = d.add_part(PartKind::Header, vec![para_block(Paragraph::with_text("Head", CharProps::default()))]);
    d.last_section.headers.default = Some(head);
    d
}

/// The text on each page (control characters left out, empty paragraphs skipped).
fn pages(d: &Document, l: &DocLayout) -> Vec<Vec<String>> {
    l.pages
        .iter()
        .map(|pg| {
            let mut v: Vec<String> = Vec::new();
            for it in &pg.items {
                if let Placed::Lines { story: StoryRef::Body, path, .. } = it {
                    let Some(p) = path.0.first().and_then(|b| d.body.get(*b as usize)).and_then(|b| b.as_para()) else { continue };
                    let t: String = p.plain_text().chars().filter(|c| !c.is_control() && *c != '\u{FFFC}').collect();
                    if !t.is_empty() && v.last() != Some(&t) {
                        v.push(t);
                    }
                }
            }
            v
        })
        .collect()
}

fn caret_page(l: &DocLayout, block: usize, offset: usize) -> usize {
    l.caret(&Pos::body(block, offset)).unwrap().page
}

/// One page [AAA], numbered 1, not blank, with its header.
fn one_page(d: &Document) -> DocLayout {
    let l = lay(d);
    assert_eq!(pages(d, &l), [vec!["AAA"]]);
    assert_eq!(l.pages.len(), 1);
    assert!(!l.pages[0].blank);
    assert!(!l.pages[0].header.is_empty());
    assert_eq!(l.pages[0].number, 1);
    l
}

#[test]
fn a_last_paragraph_of_only_a_page_break_starts_no_page() {
    let d = doc("AAA\nPB", 15);
    let l = one_page(&d);
    // The caret before and after the break both stay on the only page.
    assert_eq!(caret_page(&l, 1, 0), 0);
    assert_eq!(caret_page(&l, 1, 1), 0);
}

#[test]
fn without_a_compatibility_mode_too() {
    one_page(&doc("AAA\nPB", 0));
    one_page(&doc("AAA\nPB", 12));
}

#[test]
fn a_page_break_ending_the_last_paragraph_after_text_starts_no_page() {
    let d = doc("AAAPB", 15);
    let l = one_page(&d);
    assert_eq!(caret_page(&l, 0, 4), 0);
}

#[test]
fn only_bookmarks_after_the_break_still_start_no_page() {
    let mut d = doc("AAA\nPB", 15);
    let m = d.para_mut(StoryRef::Body, &Path::top(1)).unwrap();
    m.insert_object(m.text.len(), InlineObject::BookmarkStart { name: "b".into() }, &CharProps::default()).unwrap();
    m.insert_object(m.text.len(), InlineObject::BookmarkEnd { name: "b".into() }, &CharProps::default()).unwrap();
    one_page(&d);
}

#[test]
fn an_empty_paragraph_after_the_break_keeps_its_page() {
    // Word: 2 pages, the empty last paragraph on page 2 (the caret there is on page 2).
    let d = doc("AAA\nPB\n", 15);
    let l = lay(&d);
    assert_eq!(d.body.len(), 3);
    assert_eq!(l.pages.len(), 2);
    assert_eq!(pages(&d, &l)[0], ["AAA"]);
    assert_eq!(caret_page(&l, 2, 0), 1);
    assert_eq!(l.pages.iter().map(|p| p.number).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn text_after_the_break_keeps_its_page() {
    let d = doc("AAA\nPBZZZ", 15);
    let l = lay(&d);
    // The break's line sits on page 1, ZZZ on page 2.
    assert_eq!(l.pages.len(), 2);
    assert_eq!(pages(&d, &l)[1], ["ZZZ"]);
}

#[test]
fn a_break_in_the_middle_still_starts_a_page() {
    let d = doc("AAA\nPB\nBBB\nPB", 15);
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["AAA"], vec!["BBB"]]);
    assert_eq!(l.pages.iter().map(|p| p.number).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn a_trailing_break_in_the_last_section_of_two() {
    // Section 1 ends with BBB on a new page; the last section ends with a page break.
    let mut d = doc("AAA\nBBB\nPB", 15);
    let mut first = SectionProps { start: SectionStart::NextPage, ..SectionProps::default() };
    first.headers.default = d.last_section.headers.default;
    d.para_mut(StoryRef::Body, &Path::top(0)).unwrap().section = Some(Box::new(first));
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["AAA"], vec!["BBB"]]);
}
