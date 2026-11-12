//! A Next Page section that restarts its page numbering: with different odd and even headers or
//! mirror margins, Word puts a blank page in front of it when the restart number has the same
//! parity as the number of the page before it, so odd numbers land on odd pages (Microsoft 365
//! Word's PDF output, PAGE field in the footer). The side comes from the page number before, not
//! from how many pages there are.

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{Path, StoryRef};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

#[derive(Clone, Copy)]
enum Sides {
    /// Neither different odd and even headers nor mirror margins.
    Neither,
    /// Different odd and even headers.
    Facing,
    /// Mirror margins.
    Mirror,
}

/// One short paragraph per section; `sections` gives each section's start type and the number
/// its page numbering restarts at, the last one being the document's last section.
fn doc(sides: Sides, sections: &[(SectionStart, Option<u32>)]) -> Document {
    let text: Vec<String> = (0..sections.len()).map(|i| format!("S{i}")).collect();
    let mut d = Document::from_text(&text.join("\n"));
    let (last, earlier) = sections.split_last().expect("at least one section");
    for (i, (start, restart)) in earlier.iter().enumerate() {
        let props = SectionProps { start: *start, page_num_start: *restart, ..Default::default() };
        d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().section = Some(Box::new(props));
    }
    d.last_section.start = last.0;
    d.last_section.page_num_start = last.1;
    d.settings.even_odd_headers = matches!(sides, Sides::Facing);
    d.settings.mirror_margins = matches!(sides, Sides::Mirror);
    d
}

/// Per page: `Some(number)` for a page with content, `None` for the blank page that puts a
/// section on its side.
fn pages(sides: Sides, sections: &[(SectionStart, Option<u32>)]) -> Vec<Option<u32>> {
    lay(&doc(sides, sections)).pages.iter().map(|p| (!p.blank).then_some(p.number)).collect()
}

const NEXT: (SectionStart, Option<u32>) = (SectionStart::NextPage, None);
const EVEN: (SectionStart, Option<u32>) = (SectionStart::EvenPage, None);

fn next_at(n: u32) -> (SectionStart, Option<u32>) {
    (SectionStart::NextPage, Some(n))
}

#[test]
fn a_restart_at_1_after_page_1_gets_a_blank_page_with_odd_and_even_headers() {
    // Word: 1, a blank page, 1.
    assert_eq!(pages(Sides::Facing, &[NEXT, next_at(1)]), [Some(1), None, Some(1)]);
}

#[test]
fn the_side_comes_from_the_number_of_the_page_before_not_the_page_count() {
    // The first section restarts at 2, so the document's first page is an even page.
    // Word: 2, 1 (page 1 follows page 2 on the odd side: no blank page).
    assert_eq!(pages(Sides::Facing, &[next_at(2), next_at(1)]), [Some(2), Some(1)]);
    // Word: 2, a blank page, 2.
    assert_eq!(pages(Sides::Facing, &[next_at(2), next_at(2)]), [Some(2), None, Some(2)]);
}

#[test]
fn after_an_even_page_first_section_the_side_follows_its_number_2() {
    // An even-page first section is page 2. Word: 2, 1 and 2, a blank page, 2.
    assert_eq!(pages(Sides::Facing, &[EVEN, next_at(1)]), [Some(2), Some(1)]);
    assert_eq!(pages(Sides::Facing, &[EVEN, next_at(2)]), [Some(2), None, Some(2)]);
}

#[test]
fn mirror_margins_put_the_restart_on_its_side_too() {
    // Word: 1, a blank page, 1.
    assert_eq!(pages(Sides::Mirror, &[NEXT, next_at(1)]), [Some(1), None, Some(1)]);
    // Word: 2, 1.
    assert_eq!(pages(Sides::Mirror, &[next_at(2), next_at(1)]), [Some(2), Some(1)]);
    // Word: 2, a blank page, 2.
    assert_eq!(pages(Sides::Mirror, &[next_at(2), next_at(2)]), [Some(2), None, Some(2)]);
}

#[test]
fn a_one_sided_document_never_gets_the_blank_page() {
    // Word: 1, 1; 2, 1 (even-page first section); 2, 2.
    assert_eq!(pages(Sides::Neither, &[NEXT, next_at(1)]), [Some(1), Some(1)]);
    assert_eq!(pages(Sides::Neither, &[EVEN, next_at(1)]), [Some(2), Some(1)]);
    assert_eq!(pages(Sides::Neither, &[next_at(2), next_at(2)]), [Some(2), Some(2)]);
}

#[test]
fn a_next_page_section_that_does_not_restart_gets_no_blank_page() {
    assert_eq!(pages(Sides::Facing, &[NEXT, NEXT]), [Some(1), Some(2)]);
    assert_eq!(pages(Sides::Mirror, &[next_at(2), NEXT]), [Some(2), Some(3)]);
}
