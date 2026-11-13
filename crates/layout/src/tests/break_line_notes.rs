//! A page break on a line of its own that stays at the foot of a full page takes no room from
//! the page's footnotes: they are placed as if the line were not there (measured in Word). The
//! expected positions are read from a control document where "Page break before" replaces the
//! break character, so nothing depends on font metrics.
use crate::*;
use wordcraft_doc::para::{InlineObject, NoteKind};
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::section::{NotePos, NoteProps};
use wordcraft_doc::{PartKind, Pos, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn exactly(h: f32) -> ParaProps {
    ParaProps { line_spacing: Some(LineSpacing::Exactly(h)), space_before: Some(0.0), space_after: Some(0.0), ..Default::default() }
}

fn para(text: &str, props: ParaProps) -> std::sync::Arc<Block> {
    let mut p = Paragraph::with_text(text, Default::default());
    p.props = props;
    para_block(p)
}

#[derive(Clone, Copy)]
struct Probe {
    /// `compatibilityMode` 11 with `splitPgBreakAndParaMark`, else 15 without it.
    old: bool,
    /// The break is alone in its paragraph (else it starts the paragraph reading "NEXT").
    own: bool,
    /// Body lines of 24 pt.
    lines: usize,
    /// The body paragraphs (0-based) holding a footnote reference, in order.
    refs: &'static [usize],
    beneath: bool,
    /// The break character is replaced by "Page break before" on its paragraph.
    control: bool,
}

const ROW1: Probe = Probe { old: true, own: false, lines: 24, refs: &[0, 1, 2], beneath: false, control: false };

/// The document and its footnotes' part ids, and the body index of the "NEXT" paragraph.
fn build(p: Probe) -> (Document, Vec<u32>, usize) {
    crate::test_fonts();
    let mut d = Document::from_text("");
    if p.old {
        d.settings.compat_mode = 11;
        d.settings.split_pg_break_and_para_mark = true;
    }
    if p.beneath {
        d.settings.footnote_pr = NoteProps { pos: Some(NotePos::BeneathText), ..Default::default() };
    }
    d.body = (1..=p.lines).map(|i| para(&format!("L{i:02}"), exactly(24.0))).collect();
    let before = ParaProps { page_break_before: Some(true), ..exactly(40.0) };
    let brk = if p.control { "" } else { "\u{000C}" };
    let next = if p.own {
        d.body.push(para(brk, if p.control { before } else { exactly(40.0) }));
        d.body.push(para("NEXT", exactly(24.0)));
        p.lines + 1
    } else {
        d.body.push(para(&format!("{brk}NEXT"), if p.control { before } else { exactly(40.0) }));
        p.lines
    };
    let mut ids = Vec::new();
    for (k, &i) in p.refs.iter().enumerate() {
        let id = d.add_part(PartKind::Footnote, vec![para(&format!("Note {}", k + 1), exactly(12.0))]);
        let off = format!("L{:02}", i + 1).len();
        d.insert_object(&Pos::body(i, off), InlineObject::NoteRef { kind: NoteKind::Footnote, id, custom: String::new() }, &Default::default())
            .unwrap();
        ids.push(id);
    }
    (d, ids, next)
}

/// Each note's (page, top).
fn note_tops(d: &Document, l: &DocLayout, ids: &[u32]) -> Vec<(usize, f32)> {
    ids.iter()
        .map(|&id| {
            let c = l.caret(&d.start_of(StoryRef::Part(id))).expect("the footnote is placed");
            (c.page, c.top)
        })
        .collect()
}

fn has_notes(page: &Page) -> bool {
    page.items.iter().any(|it| matches!(it, Placed::Lines { story: StoryRef::Part(_), .. }))
}

/// The probe lays out on 2 pages with every note on page 1 where the control has it, nothing
/// carried to page 2, and "NEXT" on page 2.
fn assert_like_control(p: Probe) {
    let (d, ids, next) = build(p);
    let (c, cids, _) = build(Probe { control: true, ..p });
    let (l, cl) = (lay(&d), lay(&c));
    assert_eq!(l.pages.len(), 2, "pages: {}", l.pages.len());
    let (got, want) = (note_tops(&d, &l, &ids), note_tops(&c, &cl, &cids));
    for (g, w) in got.iter().zip(&want) {
        assert_eq!((g.0, w.0), (0, 0), "notes on page 1: {got:?}, control {want:?}");
        assert!((g.1 - w.1).abs() < 0.01, "note tops {got:?}, control {want:?}");
    }
    assert!(l.pages.get(1).is_some_and(|pg| !has_notes(pg)), "no note carried to page 2");
    let n = l.caret(&Pos::body(next, d.body.get(next).and_then(|b| b.as_para()).map_or(0, |q| q.text.len()))).unwrap();
    assert_eq!(n.page, 1, "NEXT on page 2");
}

#[test]
fn notes_keep_their_place_above_a_page_break_kept_without_room() {
    assert_like_control(ROW1);
}

#[test]
fn notes_keep_their_place_above_a_page_break_kept_without_room_in_a_current_mode_document() {
    assert_like_control(Probe { old: false, ..ROW1 });
}

#[test]
fn notes_keep_their_place_above_a_page_break_kept_without_room_when_the_break_has_its_own_paragraph() {
    assert_like_control(Probe { own: true, ..ROW1 });
}

#[test]
fn notes_keep_their_place_above_a_page_break_kept_without_room_when_the_last_note_is_referenced_on_the_last_text_line() {
    assert_like_control(Probe { refs: &[0, 1, 23], ..ROW1 });
}

#[test]
fn a_break_line_that_fits_changes_nothing() {
    assert_like_control(Probe { lines: 22, ..ROW1 });
}

#[test]
fn a_single_note_stays_above_a_kept_break_line() {
    assert_like_control(Probe { refs: &[0], ..ROW1 });
}

/// The separator's y: the page's horizontal rule (the body has no borders).
fn separator_y(page: &Page) -> Option<f32> {
    page.items.iter().find_map(|it| match it {
        Placed::Rule { y0, y1, .. } if (y0 - y1).abs() < 0.01 => Some(*y0),
        _ => None,
    })
}

#[test]
fn notes_beneath_text_start_under_the_last_text_line() {
    let p = Probe { beneath: true, ..ROW1 };
    assert_like_control(p);
    let (d, _, _) = build(p);
    let (c, _, _) = build(Probe { control: true, ..p });
    let (got, want) = (lay(&d).pages.first().and_then(separator_y), lay(&c).pages.first().and_then(separator_y));
    let (got, want) = (got.expect("a separator on page 1"), want.expect("a separator in the control"));
    assert!((got - want).abs() < 0.01, "separator at {got}, control {want}");
}
