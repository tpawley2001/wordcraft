//! A continuous section break (ECMA-376 Part 1 §17.6.22 `continuous`, RTF `\sbknone`) starts a new
//! page when the new section's paper width or height (§17.6.13 `pgSz`) differs from the section
//! before: one page has one paper size. A change of margins alone keeps the same page. Measured in
//! Microsoft 365 Word with `One¶ Two¶ <continuous break> Three¶ Four¶`, section 1 US Letter
//! portrait with 1" margins.

use super::*;
use wordcraft_doc::section::SectionStart;

/// US Letter portrait, 1" margins, continuous (the first section's start is not looked at).
fn letter() -> SectionProps {
    SectionProps { page_w: 612.0, page_h: 792.0, start: SectionStart::Continuous, ..SectionProps::default() }
}

/// One paragraph per text; `sections` are `(index of the paragraph ending it, props)`, the last
/// one being the document's last section.
fn doc(texts: &[&str], sections: &[(usize, SectionProps)]) -> Document {
    let mut d = Document::from_text(&texts.join("\n"));
    let last = sections.len() - 1;
    for (i, (end, props)) in sections.iter().enumerate() {
        if i == last {
            d.last_section = props.clone();
        } else {
            d.para_mut(StoryRef::Body, &Path::top(*end)).unwrap().section = Some(Box::new(props.clone()));
        }
    }
    d
}

/// The page the first line of body paragraph `i` is on.
fn page_of(l: &DocLayout, i: usize) -> usize {
    l.pages
        .iter()
        .position(|p| p.items.iter().any(|it| matches!(it, Placed::Lines { story: StoryRef::Body, path, l0: 0, .. } if path.0 == [i as u32])))
        .unwrap_or_else(|| panic!("paragraph {i} is laid out"))
}

/// Lays out `One¶ Two¶ <continuous> Three¶ Four¶` with `second` as section 2; returns the page
/// count, the page of "Three", and the size of the last page.
fn two(second: SectionProps) -> (usize, usize, (f32, f32)) {
    let l = lay(&doc(&["One", "Two", "Three", "Four"], &[(1, letter()), (3, second)]));
    let last = l.pages.last().map(|p| (p.w, p.h)).unwrap();
    (l.pages.len(), page_of(&l, 2), last)
}

fn twips(t: f32) -> f32 {
    t / 20.0
}

#[test]
fn a_landscape_continuous_section_starts_a_new_page() {
    let (pages, three, size) = two(SectionProps { page_w: twips(15840.0), page_h: twips(12240.0), ..letter() });
    assert_eq!((pages, three), (2, 1));
    assert_eq!(size, (792.0, 612.0));
}

#[test]
fn wider_side_margins_keep_the_same_page() {
    let (pages, three, _) = two(SectionProps { margin_left: 144.0, margin_right: 144.0, ..letter() });
    assert_eq!((pages, three), (1, 0));
}

#[test]
fn an_a4_continuous_section_starts_a_new_page() {
    let (pages, three, size) = two(SectionProps { page_w: twips(11906.0), page_h: twips(16838.0), ..letter() });
    assert_eq!((pages, three), (2, 1));
    assert_eq!(size, (twips(11906.0), twips(16838.0)));
}

#[test]
fn an_identical_continuous_section_keeps_the_same_page() {
    let (pages, three, _) = two(letter());
    assert_eq!((pages, three), (1, 0));
}

#[test]
fn a_deeper_top_margin_keeps_the_same_page() {
    let (pages, three, _) = two(SectionProps { margin_top: 144.0, ..letter() });
    assert_eq!((pages, three), (1, 0));
}

#[test]
fn a_wider_only_continuous_section_starts_a_new_page() {
    let (pages, three, size) = two(SectionProps { page_w: twips(14400.0), ..letter() });
    assert_eq!((pages, three), (2, 1));
    assert_eq!(size, (720.0, 792.0));
}

#[test]
fn letter_landscape_letter_takes_three_pages() {
    let land = SectionProps { page_w: 792.0, page_h: 612.0, ..letter() };
    let l = lay(&doc(&["One", "Two", "Three", "Four", "Five", "Six"], &[(1, letter()), (3, land), (5, letter())]));
    assert_eq!(l.pages.len(), 3);
    assert_eq!([0, 2, 4].map(|i| page_of(&l, i)), [0, 1, 2]);
    let sizes: Vec<(f32, f32)> = l.pages.iter().map(|p| (p.w, p.h)).collect();
    assert_eq!(sizes, [(612.0, 792.0), (792.0, 612.0), (612.0, 792.0)]);
}

#[test]
fn web_view_keeps_one_page() {
    let land = SectionProps { page_w: 792.0, page_h: 612.0, ..letter() };
    let d = doc(&["One", "Two", "Three", "Four"], &[(1, letter()), (3, land)]);
    let opts = LayoutOptions { view: ViewMode::Web, ..LayoutOptions::default() };
    let l = layout(&d, &mut LayoutCache::new(), &opts);
    assert_eq!(l.pages.len(), 1);
}
