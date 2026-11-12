//! A document whose first section starts on an even page: Word puts no blank page in front of
//! it and numbers the first page 2, so later odd-page and even-page breaks are decided against
//! those numbers (Microsoft 365 Word's PDF output, PAGE field in the footer).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{Path, StoryRef};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// One short paragraph per section; `sections` gives each section's start type and the number
/// its page numbering restarts at, the last one being the document's last section.
fn doc(sections: &[(SectionStart, Option<u32>)]) -> Document {
    let text: Vec<String> = (0..sections.len()).map(|i| format!("S{i}")).collect();
    let mut d = Document::from_text(&text.join("\n"));
    let (last, earlier) = sections.split_last().expect("at least one section");
    for (i, (start, restart)) in earlier.iter().enumerate() {
        let props = SectionProps { start: *start, page_num_start: *restart, ..Default::default() };
        d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().section = Some(Box::new(props));
    }
    d.last_section.start = last.0;
    d.last_section.page_num_start = last.1;
    d
}

/// Per page: its number, and whether it is the blank page that puts a section on its side.
fn pages(sections: &[(SectionStart, Option<u32>)]) -> Vec<(u32, bool)> {
    lay(&doc(sections)).pages.iter().map(|p| (p.number, p.blank)).collect()
}

const EVEN: (SectionStart, Option<u32>) = (SectionStart::EvenPage, None);
const ODD: (SectionStart, Option<u32>) = (SectionStart::OddPage, None);
const NEXT: (SectionStart, Option<u32>) = (SectionStart::NextPage, None);

#[test]
fn an_even_page_first_section_starts_at_page_2_without_a_blank_page() {
    assert_eq!(pages(&[EVEN]), [(2, false)]);
}

#[test]
fn an_odd_page_section_after_it_needs_no_blank_page() {
    // Word: pages 2 and 3.
    assert_eq!(pages(&[EVEN, ODD]), [(2, false), (3, false)]);
}

#[test]
fn an_even_page_section_after_it_gets_the_blank_page() {
    // Word: 2, a blank page, 4.
    assert_eq!(pages(&[EVEN, EVEN]), [(2, false), (3, true), (4, false)]);
}

#[test]
fn a_restart_at_an_odd_number_takes_the_next_even_one() {
    // Word: restarting the even-page first section at 1 still starts it at 2.
    assert_eq!(pages(&[(SectionStart::EvenPage, Some(1)), ODD]), [(2, false), (3, false)]);
    assert_eq!(pages(&[(SectionStart::EvenPage, Some(4)), ODD]), [(4, false), (5, false)]);
}

#[test]
fn five_sections_fall_on_the_sides_word_gives_them() {
    // Even, Odd, Odd, Even, Even, one page each. Word: 2, 3, blank, 5, 6, blank, 8.
    let got = pages(&[EVEN, ODD, ODD, EVEN, EVEN]);
    assert_eq!(got, [(2, false), (3, false), (4, true), (5, false), (6, false), (7, true), (8, false)]);
}

#[test]
fn other_first_sections_still_start_at_page_1() {
    assert_eq!(pages(&[ODD, EVEN]), [(1, false), (2, false)]);
    assert_eq!(pages(&[ODD, ODD]), [(1, false), (2, true), (3, false)]);
    assert_eq!(pages(&[NEXT, ODD]), [(1, false), (2, true), (3, false)]);
    assert_eq!(pages(&[NEXT]), [(1, false)]);
}
