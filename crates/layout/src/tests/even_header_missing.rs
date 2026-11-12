//! Different odd and even headers when a section has no even (or no odd) header or footer: Word
//! shows nothing on those pages instead of falling back to the other one (Microsoft 365 Word's
//! PDF output of DOCX and RTF files with three pages).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::section::SectionProps;
use wordcraft_doc::{PartKind, Path, StoryRef, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn part(d: &mut Document, kind: PartKind, text: &str) -> u32 {
    d.add_part(kind, vec![para_block(Paragraph::with_text(text, Default::default()))])
}

/// The header and footer parts a document's only (or first) section names.
#[derive(Clone, Copy, Default)]
struct Has {
    default: bool,
    even: bool,
    first: bool,
}

/// `pages` one-paragraph pages. The section (the first one, when `second_section_from` splits
/// the document in two) names the header and footer parts `has` asks for; a second section
/// names none, so it links to the first one's.
fn doc(pages: usize, even_odd: bool, has: Has, second_section_from: Option<usize>) -> (Document, [Option<u32>; 3]) {
    let text: Vec<String> = (0..pages).map(|i| format!("P{i}")).collect();
    let mut d = Document::from_text(&text.join("\n"));
    for i in 1..pages {
        d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().props.page_break_before = Some(true);
    }
    let mut sect = SectionProps::default();
    let mut ids = [None; 3];
    if has.default {
        ids[0] = Some(part(&mut d, PartKind::Header, "Default head"));
        sect.headers.default = ids[0];
        sect.footers.default = Some(part(&mut d, PartKind::Footer, "Default foot"));
    }
    if has.even {
        ids[1] = Some(part(&mut d, PartKind::Header, "Even head"));
        sect.headers.even = ids[1];
        sect.footers.even = Some(part(&mut d, PartKind::Footer, "Even foot"));
    }
    if has.first {
        ids[2] = Some(part(&mut d, PartKind::Header, "First head"));
        sect.headers.first = ids[2];
        sect.footers.first = Some(part(&mut d, PartKind::Footer, "First foot"));
        sect.title_page = true;
    }
    match second_section_from {
        Some(at) => {
            // The paragraph before `at` ends the first section; the second starts on a new page
            // (the page break before paragraph `at` is the section's own).
            d.para_mut(StoryRef::Body, &Path::top(at)).unwrap().props.page_break_before = None;
            d.para_mut(StoryRef::Body, &Path::top(at - 1)).unwrap().section = Some(Box::new(sect));
        }
        None => d.last_section = sect,
    }
    d.settings.even_odd_headers = even_odd;
    (d, ids)
}

/// Per page: its number, whether a header is drawn and whether a footer is drawn.
fn shape(l: &DocLayout) -> Vec<(u32, bool, bool)> {
    l.pages.iter().map(|p| (p.number, !p.header.is_empty(), !p.footer.is_empty())).collect()
}

#[test]
fn an_even_page_with_no_even_header_or_footer_shows_none() {
    // Word: D 1 / nothing / D 3.
    let (d, ids) = doc(3, true, Has { default: true, ..Default::default() }, None);
    let l = lay(&d);
    assert_eq!(shape(&l), [(1, true, true), (2, false, false), (3, true, true)]);
    assert_eq!(l.pages[0].header_story, ids[0]);
    assert_eq!((l.pages[1].header_story, l.pages[1].footer_story), (None, None));
}

#[test]
fn an_odd_page_with_no_default_header_or_footer_shows_none() {
    // Word: nothing / E 2 / nothing.
    let (d, ids) = doc(3, true, Has { even: true, ..Default::default() }, None);
    let l = lay(&d);
    assert_eq!(shape(&l), [(1, false, false), (2, true, true), (3, false, false)]);
    assert_eq!(l.pages[1].header_story, ids[1]);
}

#[test]
fn with_both_each_side_shows_its_own() {
    // Word: F 1 / L 2 / F 3.
    let (d, ids) = doc(3, true, Has { default: true, even: true, ..Default::default() }, None);
    let l = lay(&d);
    assert_eq!(shape(&l), [(1, true, true), (2, true, true), (3, true, true)]);
    let stories: Vec<_> = l.pages.iter().map(|p| p.header_story).collect();
    assert_eq!(stories, [ids[0], ids[1], ids[0]]);
}

#[test]
fn a_one_sided_document_shows_the_default_on_every_page() {
    // Word: R 1 / R 2 / R 3.
    let (d, _) = doc(3, false, Has { default: true, ..Default::default() }, None);
    assert_eq!(shape(&lay(&d)), [(1, true, true), (2, true, true), (3, true, true)]);
}

#[test]
fn a_linked_section_has_no_even_header_either() {
    // Two sections of two pages; the second names no header, so it shows the first one's:
    // the default one on odd pages, none on even pages.
    let (d, _) = doc(4, true, Has { default: true, ..Default::default() }, Some(2));
    assert_eq!(shape(&lay(&d)), [(1, true, true), (2, false, false), (3, true, true), (4, false, false)]);
}

#[test]
fn a_different_first_page_is_still_chosen_first() {
    // Different first page + default only: the first header on page 1, none on page 2, the
    // default one on page 3.
    let (d, ids) = doc(3, true, Has { default: true, first: true, ..Default::default() }, None);
    let l = lay(&d);
    assert_eq!(shape(&l), [(1, true, true), (2, false, false), (3, true, true)]);
    assert_eq!(l.pages[0].header_story, ids[2]);
    assert_eq!(l.pages[2].header_story, ids[0]);
}
