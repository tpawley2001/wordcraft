//! SECTIONPAGES and the blank page that puts an odd-page or even-page section on its side: the
//! blank page counts for NUMPAGES and takes a page number, but it belongs to no section's page
//! count (Microsoft 365 Word's PDF output, PAGE / NUMPAGES / SECTIONPAGES fields in the footer).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{PartKind, Path, StoryRef, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn field(instr: &str) -> InlineObject {
    InlineObject::Field {
        instr: instr.into(),
        result: String::new(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: None,
    }
}

/// `sections` gives each section's start type and how many pages of text it has (one short
/// paragraph per page, each after the first starting with a page break). Every section shows
/// the same footer, which holds only the field `footer_field`.
fn doc(sections: &[(SectionStart, usize)], footer_field: &str) -> Document {
    let count: usize = sections.iter().map(|(_, pages)| *pages).sum();
    let text: Vec<String> = (0..count).map(|i| format!("P{i}")).collect();
    let mut d = Document::from_text(&text.join("\n"));
    let mut fp = Paragraph::new();
    fp.insert_object(0, field(footer_field), &Default::default()).unwrap();
    let foot = d.add_part(PartKind::Footer, vec![para_block(fp)]);
    let mut at = 0;
    for (si, (start, pages)) in sections.iter().enumerate() {
        for k in 1..*pages {
            d.para_mut(StoryRef::Body, &Path::top(at + k)).unwrap().props.page_break_before = Some(true);
        }
        at += pages;
        if si + 1 < sections.len() {
            let mut props = SectionProps { start: *start, ..Default::default() };
            props.footers.default = Some(foot);
            d.para_mut(StoryRef::Body, &Path::top(at - 1)).unwrap().section = Some(Box::new(props));
        } else {
            d.last_section.start = *start;
            d.last_section.footers.default = Some(foot);
        }
    }
    d
}

/// Per page: `None` for a blank side page, else how many glyphs its footer draws (the field's
/// result in capital roman numerals: I, II, III).
fn footer_glyphs(sections: &[(SectionStart, usize)], footer_field: &str) -> Vec<Option<usize>> {
    crate::test_fonts();
    lay(&doc(sections, footer_field))
        .pages
        .iter()
        .map(|p| (!p.blank).then(|| p.footer.iter().map(|it| if let Placed::Lines { para, .. } = it { para.glyphs.len() } else { 0 }).sum()))
        .collect()
}

const SP: &str = r"SECTIONPAGES \* Roman";
const NP: &str = r"NUMPAGES \* Roman";
const ODD: SectionStart = SectionStart::OddPage;
const EVEN: SectionStart = SectionStart::EvenPage;
const NEXT: SectionStart = SectionStart::NextPage;

#[test]
fn a_one_page_section_before_an_odd_page_section_has_one_page() {
    // Word: pages 1, blank, 3; SECTIONPAGES 1 and 1.
    assert_eq!(footer_glyphs(&[(ODD, 1), (ODD, 1)], SP), [Some(1), None, Some(1)]);
}

#[test]
fn a_two_page_section_before_the_blank_page_has_two_pages() {
    // Pages 1, then 2 and 3 (the even-page section needs no blank page), blank, 5: by Word's
    // rule SECTIONPAGES is 1, then 2 and 2, then 1 (the blank page is in no section's count).
    assert_eq!(footer_glyphs(&[(NEXT, 1), (EVEN, 2), (ODD, 1)], SP), [Some(1), Some(2), Some(2), None, Some(1)]);
}

#[test]
fn every_section_before_a_blank_page_leaves_it_out() {
    // Word: pages 1, 2, blank, 4, 5, blank, 7; SECTIONPAGES 2, 2, 1.
    let got = footer_glyphs(&[(NEXT, 2), (EVEN, 2), (ODD, 1)], SP);
    assert_eq!(got, [Some(2), Some(2), None, Some(2), Some(2), None, Some(1)]);
}

#[test]
fn the_blank_page_still_counts_for_the_document_total() {
    // Word: pages 1, blank, 3; NUMPAGES 3 on both.
    assert_eq!(footer_glyphs(&[(ODD, 1), (ODD, 1)], NP), [Some(3), None, Some(3)]);
}

#[test]
fn sections_with_no_blank_page_between_them_count_as_before() {
    // Word: SECTIONPAGES 2 on the first two pages, 3 on the last three.
    assert_eq!(footer_glyphs(&[(NEXT, 2), (NEXT, 3)], SP), [Some(2), Some(2), Some(3), Some(3), Some(3)]);
    // Odd-page section already on an odd page: no blank page, 2 then 1.
    assert_eq!(footer_glyphs(&[(NEXT, 2), (ODD, 1)], SP), [Some(2), Some(2), Some(1)]);
}
