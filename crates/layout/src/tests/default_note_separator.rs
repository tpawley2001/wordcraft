//! The default footnote separator (a document with no separator story of its own, as Word 97-2003
//! and RTF files that never customised it) is as tall as one single-spaced line of the Normal
//! style's font at its size, as Word lays it out (Microsoft 365, RTF probes): the Normal style's
//! space before / after and line spacing don't count. The heights expected here are measured from
//! the test fonts' metrics by laying out an empty single-spaced Normal paragraph.

use super::*;
use crate::notes::NOTE_SEP;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Pos, para_block};

/// Set the Normal style's run formatting (`font`, `size`) and paragraph formatting.
fn set_normal(d: &mut Document, font: &str, size: f32, para: ParaProps) {
    let styles = std::sync::Arc::make_mut(&mut d.styles);
    let normal = styles.get_mut("Normal").expect("the document has a Normal style");
    normal.chr = CharProps { font: Some(font.into()), size: Some(size), ..Default::default() };
    normal.para = para;
}

fn single(text: &str) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p
}

/// A document of `body` paragraphs whose first refers to one one-line footnote; no separator
/// story. Returns it and the note's id.
fn doc_with(normal: (&str, f32, ParaProps), body: Vec<Paragraph>) -> (Document, u32) {
    let mut d = Document::from_text("");
    set_normal(&mut d, normal.0, normal.1, normal.2);
    d.settings.compat_mode = 11;
    d.body = body.into_iter().map(para_block).collect();
    let id = d.add_part(wordcraft_doc::PartKind::Footnote, vec![para_block(single("Note"))]);
    d.insert_object(
        &Pos::body(0, 1),
        InlineObject::NoteRef { kind: wordcraft_doc::para::NoteKind::Footnote, id, custom: String::new() },
        &Default::default(),
    )
    .unwrap();
    assert!(d.footnote_separators.is_empty());
    (d, id)
}

/// Height of one single-spaced empty line in the document's Normal style, laid out as body text.
pub(super) fn normal_line(d: &Document) -> f32 {
    let mut e = d.clone();
    e.parts.clear();
    let mut p = Paragraph::new().styled("Normal");
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    e.body = vec![para_block(p)];
    let l = lay(&e);
    let (y, para) = l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, para, y, .. } => Some((*y, para.clone())),
            _ => None,
        })
        .unwrap();
    item_bottom(y, &para, 0, para.lines.len()).unwrap() - y
}

/// The top of note `id` on page 0.
fn note_top(l: &DocLayout, id: u32) -> f32 {
    l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Part(s), y, .. } if *s == id => Some(*y),
            _ => None,
        })
        .unwrap()
}

/// The height of the separator above note `id` (referred to from the first body paragraph):
/// with only that paragraph and the notes placed beneath the text, the note starts the
/// separator's height below the text's last line.
pub(super) fn sep_height(d: &Document, id: u32) -> f32 {
    let mut e = d.clone();
    e.body.truncate(1);
    e.settings.footnote_pr.pos = Some(wordcraft_doc::section::NotePos::BeneathText);
    let l = lay(&e);
    let body = l.pages[0]
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, y, para, l0, l1, .. } => item_bottom(*y, para, *l0, *l1),
            _ => None,
        })
        .reduce(f32::max)
        .unwrap();
    note_top(&l, id) - body
}

fn check(font: &str, size: f32, para: ParaProps) {
    crate::test_fonts();
    let (d, id) = doc_with((font, size, para.clone()), vec![single("Body text")]);
    let want = normal_line(&d);
    let got = sep_height(&d, id);
    assert!((got - want).abs() < 0.01, "{font} {size} {para:?}: separator {got}, one Normal line {want}");
}

#[test]
fn default_separator_is_one_line_of_normal_12pt() {
    check("Times New Roman", 12.0, ParaProps::default());
}

#[test]
fn default_separator_is_one_line_of_normal_20pt() {
    check("Times New Roman", 20.0, ParaProps::default());
}

#[test]
fn default_separator_is_one_line_of_normal_8pt() {
    check("Times New Roman", 8.0, ParaProps::default());
}

#[test]
fn default_separator_is_one_line_of_normal_arial() {
    check("Arial", 12.0, ParaProps::default());
}

#[test]
fn default_separator_leaves_out_normal_space_after() {
    check("Times New Roman", 12.0, ParaProps { space_after: Some(12.0), ..Default::default() });
}

#[test]
fn default_separator_leaves_out_normal_line_spacing() {
    check("Times New Roman", 12.0, ParaProps { line_spacing: Some(LineSpacing::Multiple(2.0)), ..Default::default() });
}

#[test]
fn separator_story_keeps_its_own_height() {
    crate::test_fonts();
    let (mut d, id) = doc_with(("Times New Roman", 20.0, ParaProps::default()), vec![single("Body text")]);
    let mut p = single("");
    p.props.line_spacing = Some(LineSpacing::Exactly(17.0));
    p.insert_object(0, InlineObject::note_separator(false), &Default::default()).unwrap();
    d.footnote_separators.separator = Some(vec![para_block(p)]);
    let got = sep_height(&d, id);
    assert!((got - 17.0).abs() < 0.01, "the story's height: {got}");
}

#[test]
fn unusable_normal_size_keeps_12pt() {
    crate::test_fonts();
    let (d, id) = doc_with(("Times New Roman", 0.0, ParaProps::default()), vec![single("Body text")]);
    let got = sep_height(&d, id);
    assert!((got - NOTE_SEP).abs() < 0.01, "fallback: {got}");
}

#[test]
fn one_line_fewer_fits_above_the_note() {
    crate::test_fonts();
    let normal = || ("Times New Roman", 12.0, ParaProps::default());
    let lines = |pitch: f32| -> Vec<Paragraph> {
        (0..200)
            .map(|i| {
                let mut p = single(&format!("L{i}"));
                p.props.line_spacing = Some(LineSpacing::Exactly(pitch));
                p
            })
            .collect()
    };
    // Measure the note and the room on the page.
    let (d, id) = doc_with(normal(), lines(20.0));
    let l = lay(&d);
    let note_bottom = l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Part(s), para, y, l0, l1, .. } if *s == id => item_bottom(*y, para, *l0, *l1),
            _ => None,
        })
        .unwrap();
    let note_h = note_bottom - note_top(&l, id);
    let sep = normal_line(&d);
    assert!(sep > NOTE_SEP + 1.0, "{sep}");
    let s = &d.last_section;
    let room = s.page_h - s.margin_bottom - s.margin_top - note_h;
    // N lines fit with a 12 pt separator, only N - 1 with the Normal line.
    let n = 20usize;
    let pitch = (room - NOTE_SEP) / n as f32 - 0.02;
    assert!((n as f32) * pitch > room - sep);
    let (d, _) = doc_with(normal(), lines(pitch));
    let l = lay(&d);
    let on_first = l.pages[0].items.iter().filter(|it| matches!(it, Placed::Lines { story: StoryRef::Body, .. })).count();
    assert_eq!(on_first, n - 1);
}
