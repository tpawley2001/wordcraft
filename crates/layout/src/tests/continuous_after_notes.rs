//! A continuous section break (ECMA-376 Part 1 §17.6.22 `continuous`) that comes after footnote
//! references on the same page starts the next section on a new page in documents laid out for
//! an earlier Word (`compatibilityMode` below 15) without `w:footnoteLayoutLikeWW8` (§17.15.3.9).
//! With that setting, or in a current-mode (15) document, the next section stays on the page.
//! Measured in Microsoft 365 Word: `One¶ Two<footnote ref>¶ <continuous break> Three¶ Four¶`
//! on US Letter with 1" margins (2 pages in modes 11, 12 and 14 without the setting; 1 page with
//! it, in mode 15, and without the footnote).

use crate::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::section::SectionStart;
use wordcraft_doc::{PartKind, Pos};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// US Letter portrait, 1" margins, continuous.
fn letter() -> SectionProps {
    SectionProps { page_w: 612.0, page_h: 792.0, start: SectionStart::Continuous, ..SectionProps::default() }
}

/// One paragraph per text, a section ending after each paragraph index in `ends` (the last
/// index ends the document's last section), and a footnote reference at the end of each
/// paragraph in `refs`.
fn doc(texts: &[&str], ends: &[usize], refs: &[usize], mode: u32, ww8: bool) -> Document {
    let mut d = Document::from_text(&texts.join("\n"));
    d.settings.compat_mode = mode;
    d.settings.footnote_layout_like_ww8 = ww8;
    let last = ends.len() - 1;
    for (i, end) in ends.iter().enumerate() {
        if i == last {
            d.last_section = letter();
        } else {
            d.para_mut(StoryRef::Body, &Path::top(*end)).unwrap().section = Some(Box::new(letter()));
        }
    }
    for &i in refs {
        let mut note = Paragraph::with_text("The note.", Default::default());
        note.props = Default::default();
        let id = d.add_part(PartKind::Footnote, vec![wordcraft_doc::para_block(note)]);
        let off = texts[i].chars().count();
        d.insert_object(&Pos::body(i, off), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &Default::default())
            .unwrap();
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

const TWO: [&str; 4] = ["One", "Two", "Three", "Four"];

/// (pages, page of "Three") for `One¶ Two¶ <continuous> Three¶ Four¶`.
fn two(refs: &[usize], mode: u32, ww8: bool) -> (usize, usize) {
    let l = lay(&doc(&TWO, &[1, 3], refs, mode, ww8));
    (l.pages.len(), page_of(&l, 2))
}

#[test]
fn notes_before_a_continuous_break_start_a_new_page_in_older_modes() {
    for mode in [11, 12, 14] {
        assert_eq!(two(&[1], mode, false), (2, 1), "mode {mode}, reference in Two");
        assert_eq!(two(&[0], mode, false), (2, 1), "mode {mode}, reference in One");
    }
}

#[test]
fn footnote_layout_like_ww8_keeps_the_next_section_on_the_page() {
    for mode in [11, 12, 14] {
        assert_eq!(two(&[1], mode, true), (1, 0), "mode {mode}");
    }
}

#[test]
fn current_mode_keeps_the_next_section_on_the_page() {
    assert_eq!(two(&[1], 15, false), (1, 0));
}

#[test]
fn without_notes_the_break_stays_continuous() {
    for mode in [11, 14, 15] {
        assert_eq!(two(&[], mode, false), (1, 0), "mode {mode}");
    }
}

#[test]
fn the_note_stays_on_the_page_of_its_reference() {
    let d = doc(&TWO, &[1, 3], &[1], 14, false);
    let l = lay(&d);
    assert_eq!(l.pages.len(), 2);
    let has_note = |p: &Page| p.items.iter().any(|it| matches!(it, Placed::Lines { story: StoryRef::Part(_), .. }));
    assert!(has_note(&l.pages[0]), "the footnote is on page 1");
    assert!(!has_note(&l.pages[1]), "page 2 has no footnote");
}

/// Three sections of two paragraphs, all breaks continuous, compatibility mode 14 (Word: 2 pages
/// both ways).
#[test]
fn only_the_break_after_the_notes_moves() {
    let texts = ["S1a", "S1b", "S2a", "S2b", "S3a", "S3b"];
    // Reference in section 1: section 2 starts page 2 and section 3 follows it there.
    let l = lay(&doc(&texts, &[1, 3, 5], &[1], 14, false));
    assert_eq!((l.pages.len(), page_of(&l, 2), page_of(&l, 4)), (2, 1, 1));
    // Reference in section 2: section 2 stays on page 1, section 3 starts page 2.
    let l = lay(&doc(&texts, &[1, 3, 5], &[3], 14, false));
    assert_eq!((l.pages.len(), page_of(&l, 2), page_of(&l, 4)), (2, 0, 1));
}
