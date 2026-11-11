//! Endnotes at the end of each section: a section with Suppress endnotes (`w:noEndnote`, ECMA-376
//! Part 1 §17.6; RTF: a section without `\endnhere`) prints none; its endnotes go to the end of the
//! next section that doesn't suppress them, and the last section prints whatever is left. Measured
//! in Microsoft 365 Word (each section on its own page, one endnote per section).

use super::*;
use wordcraft_doc::section::{NotePos, NoteProps};

/// One paragraph per section, each with one endnote; `suppress[i]` is section i's `no_endnote`.
/// Returns the document and the endnote part ids, in section order.
fn doc(suppress: &[Option<bool>], pos: NotePos) -> (Document, Vec<u32>) {
    let names = ["SecOne", "SecTwo", "SecThree"];
    let mut d = Document::from_text(&names[..suppress.len()].join("\n"));
    let mut ids = Vec::new();
    for (i, name) in names.iter().enumerate().take(suppress.len()) {
        let eid = d.add_part(
            wordcraft_doc::PartKind::Endnote,
            vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(&format!("End{i}."), Default::default()))],
        );
        d.insert_object(
            &Pos::body(i, name.len()),
            InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Endnote, id: eid, custom: String::new() },
            &Default::default(),
        )
        .unwrap();
        ids.push(eid);
    }
    let last = suppress.len() - 1;
    for (i, s) in suppress.iter().enumerate() {
        if i == last {
            d.last_section.no_endnote = *s;
        } else {
            d.para_mut(StoryRef::Body, &Path::top(i)).unwrap().section = Some(Box::new(SectionProps { no_endnote: *s, ..SectionProps::default() }));
        }
    }
    d.settings.endnote_pr = NoteProps { pos: Some(pos), ..Default::default() };
    (d, ids)
}

/// The page each endnote starts on.
fn pages(d: &Document, ids: &[u32]) -> Vec<usize> {
    let l = lay(d);
    ids.iter().map(|&id| l.caret(&d.start_of(StoryRef::Part(id))).unwrap().page).collect()
}

#[test]
fn every_section_suppressed_puts_them_all_after_the_last() {
    crate::test_fonts();
    let (d, ids) = doc(&[Some(true), Some(true)], NotePos::SectEnd);
    assert_eq!(pages(&d, &ids), [1, 1]);
}

#[test]
fn sections_that_include_them_print_their_own() {
    crate::test_fonts();
    for flag in [None, Some(false)] {
        let (d, ids) = doc(&[flag, flag], NotePos::SectEnd);
        assert_eq!(pages(&d, &ids), [0, 1], "{flag:?}");
    }
}

#[test]
fn a_suppressed_section_passes_them_to_the_next_including_one() {
    crate::test_fonts();
    // Word: p1 SecOne; p2 SecTwo, i, ii; p3 SecThree, iii (the last section prints what is left).
    let (d, ids) = doc(&[Some(true), Some(false), Some(true)], NotePos::SectEnd);
    assert_eq!(pages(&d, &ids), [1, 1, 2]);
}

#[test]
fn document_end_ignores_the_flag() {
    crate::test_fonts();
    let (d, ids) = doc(&[Some(false), Some(true), None], NotePos::DocEnd);
    assert_eq!(pages(&d, &ids), [2, 2, 2]);
}
