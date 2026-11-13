//! The First Page header and footer (`w:titlePg`) go on the page where their section starts, and
//! only there: a section that starts part-way down a page (a Continuous break) has no first page
//! of its own, so the next page shows the default header (Microsoft 365 Word, RTF probes).
//!
//! Section 1 has header "S1HDR" and no Different first page; section 2 has Different first page
//! with "FIRSTHDR" and default "DEFHDR". Synthetic documents built in code.

use crate::*;
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{PartKind, Path, StoryRef, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn part(d: &mut Document, kind: PartKind, text: &str) -> u32 {
    d.add_part(kind, vec![para_block(Paragraph::with_text(text, Default::default()))])
}

fn break_before(d: &mut Document, i: usize) {
    d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().props.page_break_before = Some(true);
}

fn landscape(s: &mut SectionProps) {
    (s.page_w, s.page_h) = (s.page_h.max(s.page_w), s.page_h.min(s.page_w));
    s.landscape = true;
}

/// `text`'s paragraphs; section 1 (S1HDR / S1FTR) ends at paragraph `s1_end`. Section 2 (first
/// page FIRSTHDR / FIRSTFTR, default DEFHDR / DEFFTR) starts with `start`, landscape if asked,
/// and is the last section unless `s2_end` ends it.
fn doc(text: &str, s1_end: usize, start: SectionStart, land: bool, s2_end: Option<usize>) -> Document {
    let mut d = Document::from_text(text);
    let mut s1 = SectionProps::default();
    s1.headers.default = Some(part(&mut d, PartKind::Header, "S1HDR"));
    s1.footers.default = Some(part(&mut d, PartKind::Footer, "S1FTR"));
    let mut s2 = SectionProps { start, title_page: true, ..Default::default() };
    if land {
        landscape(&mut s2);
    }
    s2.headers.first = Some(part(&mut d, PartKind::Header, "FIRSTHDR"));
    s2.footers.first = Some(part(&mut d, PartKind::Footer, "FIRSTFTR"));
    s2.headers.default = Some(part(&mut d, PartKind::Header, "DEFHDR"));
    s2.footers.default = Some(part(&mut d, PartKind::Footer, "DEFFTR"));
    d.para_mut(StoryRef::Body, &Path::top(s1_end)).unwrap().section = Some(Box::new(s1));
    match s2_end {
        Some(at) => d.para_mut(StoryRef::Body, &Path::top(at)).unwrap().section = Some(Box::new(s2)),
        None => d.last_section = s2,
    }
    d
}

fn part_text(d: &Document, id: Option<u32>) -> String {
    id.and_then(|id| d.parts.get(&id)).map(|p| p.blocks.iter().filter_map(|b| b.as_para().map(|p| p.text.clone())).collect()).unwrap_or_default()
}

/// Per page: header text, footer text and slot.
fn heads(d: &Document, l: &DocLayout) -> Vec<(String, String, HfSlot)> {
    l.pages.iter().map(|p| (part_text(d, p.header_story), part_text(d, p.footer_story), p.hf_slot)).collect()
}

fn row(h: &str, slot: HfSlot) -> (String, String, HfSlot) {
    (h.to_string(), h.replace("HDR", "FTR"), slot)
}

const S1: (&str, HfSlot) = ("S1HDR", HfSlot::Default);
const FIRST: (&str, HfSlot) = ("FIRSTHDR", HfSlot::First);
const DEF: (&str, HfSlot) = ("DEFHDR", HfSlot::Default);

fn expect(rows: &[(&str, HfSlot)]) -> Vec<(String, String, HfSlot)> {
    rows.iter().map(|&(h, s)| row(h, s)).collect()
}

/// Row 1: a Continuous section starting mid-page 1 never shows its first-page header.
#[test]
fn a_section_starting_mid_page_shows_no_first_page_header() {
    let mut d = doc("one\ntwo\nthree\nfour", 0, SectionStart::Continuous, false, None);
    break_before(&mut d, 2);
    break_before(&mut d, 3);
    let l = lay(&d);
    assert_eq!(heads(&d, &l), expect(&[S1, DEF, DEF]));
}

/// Row 2: a Next page section shows its first-page header on the page it starts.
#[test]
fn a_next_page_section_shows_its_first_page_header_once() {
    let mut d = doc("one\ntwo\nthree\nfour", 0, SectionStart::NextPage, false, None);
    break_before(&mut d, 2);
    break_before(&mut d, 3);
    let l = lay(&d);
    assert_eq!(heads(&d, &l), expect(&[S1, FIRST, DEF, DEF]));
}

/// Row 3: a Continuous section with another page size starts a new page, its first page.
#[test]
fn a_continuous_section_turned_landscape_starts_its_first_page() {
    let mut d = doc("one\ntwo\nthree\nfour", 0, SectionStart::Continuous, true, None);
    break_before(&mut d, 2);
    break_before(&mut d, 3);
    let l = lay(&d);
    assert_eq!(heads(&d, &l), expect(&[S1, FIRST, DEF, DEF]));
}

/// Row 4: section 1 ends with an empty paragraph after a page break; section 2 starts below it
/// on page 2, whose header is still section 1's. Page 3 is an ordinary page of section 2.
#[test]
fn a_section_starting_below_an_earlier_one_on_page_two_has_no_first_page() {
    let mut d = doc("one\n\ntwo\nthree\nfour", 1, SectionStart::Continuous, false, None);
    break_before(&mut d, 1);
    break_before(&mut d, 3);
    break_before(&mut d, 4);
    let l = lay(&d);
    assert_eq!(heads(&d, &l), expect(&[S1, S1, DEF, DEF]));
}

/// Row 5: as row 4, but section 2 is landscape, so it starts page 3, its first page.
#[test]
fn a_landscape_section_after_an_empty_page_starts_its_first_page() {
    let mut d = doc("one\n\ntwo\nthree\nfour", 1, SectionStart::Continuous, true, None);
    break_before(&mut d, 1);
    break_before(&mut d, 3);
    break_before(&mut d, 4);
    let l = lay(&d);
    assert_eq!(heads(&d, &l), expect(&[S1, S1, FIRST, DEF, DEF]));
}

/// Row 6: as row 3, then a third section (Continuous, landscape, Different first page, no
/// headers of its own) starting after `two` on page 2: page 3 is not its first page.
#[test]
fn a_third_section_starting_mid_page_inherits_no_first_page() {
    let mut d = doc("one\ntwo\ntworest\nthree\nfour", 0, SectionStart::Continuous, true, Some(1));
    let mut s3 = SectionProps { start: SectionStart::Continuous, title_page: true, ..Default::default() };
    landscape(&mut s3);
    d.last_section = s3;
    break_before(&mut d, 3);
    break_before(&mut d, 4);
    let l = lay(&d);
    assert_eq!(heads(&d, &l), expect(&[S1, FIRST, DEF, DEF]));
}

/// A first-page header taller than the default one does not push down the body of the page after
/// a mid-page section start: that page's body starts where the next page's does.
#[test]
fn a_tall_first_page_header_does_not_shrink_the_page_after_a_mid_page_start() {
    let mut d = doc("one\ntwo\nthree\nfour", 0, SectionStart::Continuous, false, None);
    break_before(&mut d, 2);
    break_before(&mut d, 3);
    let tall = "FIRSTHDR\n".repeat(12);
    let tall_id = d.add_part(PartKind::Header, tall.lines().map(|t| para_block(Paragraph::with_text(t, Default::default()))).collect());
    d.last_section.headers.first = Some(tall_id);
    let l = lay(&d);
    assert_eq!(l.pages.len(), 3);
    assert_eq!(l.pages[1].body.y, l.pages[2].body.y);
    assert_ne!(l.pages[1].header_story, Some(tall_id));
}
