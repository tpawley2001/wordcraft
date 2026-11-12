//! Update Table of Contents › Update page numbers only (`references.updateToc` with
//! `pageNumbersOnly`): the entries stay as they are, text typed into them included, and only the
//! page numbers follow the headings; headings with the same text each keep their own page.
//! Update entire table (the default for scripts) still rebuilds the entries. All documents are
//! built in memory.

use serde_json::json;
use wordcraft_doc::{Block, CharProps, Document, Paragraph, Pos, StoryRef};
use wordcraft_engine::{Selection, Session};

/// A document of Heading 1 paragraphs, each followed by a line of text and each after the first
/// starting a new page.
fn doc(headings: &[&str]) -> Document {
    let mut d = Document::new();
    let mut body = Vec::new();
    for (i, h) in headings.iter().enumerate() {
        let mut head = Paragraph::with_text(h, CharProps::default()).styled("Heading1");
        head.props.page_break_before = (i > 0).then_some(true);
        body.push(Block::Para(head));
        body.push(Block::Para(Paragraph::with_text("Some text.", CharProps::default())));
    }
    d.body = body.into_iter().map(std::sync::Arc::new).collect();
    d
}

/// A session on `doc(headings)` with a table of contents inserted at the top.
fn with_toc(headings: &[&str], hyperlinks: bool) -> Session {
    let mut s = Session::new(doc(headings));
    s.run("caret.docStart", &json!({})).unwrap();
    s.run("references.toc", &json!({"hyperlinks": hyperlinks})).unwrap();
    s
}

/// The body block of the first paragraph in `style` whose text (fields as their results) starts
/// with `text`.
fn block(s: &Session, style: &str, text: &str) -> usize {
    s.doc
        .body
        .iter()
        .position(|b| b.as_para().is_some_and(|p| p.props.style.as_deref() == Some(style) && p.plain_text().starts_with(text)))
        .unwrap_or_else(|| panic!("no {style} paragraph starting with {text:?}"))
}

/// The entries of the table of contents: label, tab, page number (a `PAGEREF` field as its
/// result).
fn entries(s: &Session) -> Vec<String> {
    s.doc
        .body
        .iter()
        .filter_map(|b| b.as_para())
        .filter(|p| p.props.style.as_deref().is_some_and(|st| st.starts_with("TOC") && st != "TOCHeading"))
        .map(Paragraph::plain_text)
        .collect()
}

/// Page Break Before on the first paragraph in Heading 1 starting with `text`: it and everything
/// after it move one page down.
fn push_down(s: &mut Session, text: &str) {
    let i = block(s, "Heading1", text);
    s.sel = Selection::caret(Pos::body(i, 0));
    s.run("para.pageBreakBefore", &json!({"value": true})).unwrap();
}

/// Type "X" after the second character of the first entry starting with `text`.
fn edit_entry(s: &mut Session, text: &str) {
    let i = block(s, "TOC1", text);
    s.sel = Selection::caret(Pos::body(i, 2));
    s.run("text.insert", &json!({"text": "X"})).unwrap();
}

fn pages_only(s: &mut Session) {
    s.run("references.updateToc", &json!({"pageNumbersOnly": true})).unwrap();
}

#[test]
fn page_numbers_only_keeps_an_edited_entry_and_gives_repeated_headings_their_own_pages() {
    let mut s = with_toc(&["Intro", "Middle", "Intro"], true);
    assert_eq!(entries(&s), ["Intro\t1", "Middle\t2", "Intro\t3"]);
    push_down(&mut s, "Intro");
    edit_entry(&mut s, "Intro");
    assert_eq!(entries(&s), ["InXtro\t1", "Middle\t2", "Intro\t3"], "stale until updated");
    pages_only(&mut s);
    assert_eq!(entries(&s), ["InXtro\t2", "Middle\t3", "Intro\t4"]);
}

#[test]
fn page_numbers_only_matches_repeated_headings_in_order_without_links() {
    let mut s = with_toc(&["Intro", "Middle", "Intro"], false);
    assert_eq!(entries(&s), ["Intro\t1", "Middle\t2", "Intro\t3"]);
    push_down(&mut s, "Intro");
    pages_only(&mut s);
    assert_eq!(entries(&s), ["Intro\t2", "Middle\t3", "Intro\t4"]);
    // Nothing moved since: a second update changes nothing and adds no undo step.
    let steps = s.undo_depth();
    pages_only(&mut s);
    assert_eq!(entries(&s), ["Intro\t2", "Middle\t3", "Intro\t4"]);
    assert_eq!(s.undo_depth(), steps);
}

#[test]
fn an_unlinked_entry_that_matches_no_heading_keeps_its_number() {
    let mut s = with_toc(&["Intro", "Middle", "Intro"], false);
    push_down(&mut s, "Intro");
    edit_entry(&mut s, "Middle");
    pages_only(&mut s);
    // "MiXddle" is no heading's text: its number stays, and the "Intro" after it is still the
    // second "Intro" heading.
    assert_eq!(entries(&s), ["Intro\t2", "MiXddle\t2", "Intro\t4"]);
}

#[test]
fn update_entire_table_still_rebuilds_the_entries() {
    for params in [json!({}), json!({"pageNumbersOnly": false})] {
        let mut s = with_toc(&["Intro", "Middle", "Intro"], true);
        push_down(&mut s, "Intro");
        edit_entry(&mut s, "Intro");
        s.run("references.updateToc", &params).unwrap();
        assert_eq!(entries(&s), ["Intro\t2", "Middle\t3", "Intro\t4"], "{params}");
    }
}

#[test]
fn page_numbers_only_follows_a_heading_with_a_tracked_deletion() {
    let mut s = with_toc(&["Alpha", "Beta budget"], true);
    assert_eq!(entries(&s), ["Alpha\t1", "Beta budget\t2"]);
    // " budget" deleted in the heading with Track Changes on: still in the paragraph, struck out.
    let i = block(&s, "Heading1", "Beta");
    let text = s.doc.para_at(&Pos::body(i, 0)).unwrap().text.clone();
    let at = text.find(" budget").unwrap();
    s.run("review.trackChanges", &json!({"value": true})).unwrap();
    s.sel = Selection { anchor: Pos::body(i, at), focus: Pos::body(i, at + " budget".len()) };
    s.run("text.delete", &json!({})).unwrap();
    s.run("review.trackChanges", &json!({"value": false})).unwrap();
    assert!(!s.doc.para_at(&Pos::body(i, 0)).unwrap().deleted_ranges().is_empty(), "the deletion is tracked");
    push_down(&mut s, "Alpha");
    edit_entry(&mut s, "Alpha");
    pages_only(&mut s);
    // The entries' text is not rebuilt; their numbers follow the headings.
    assert_eq!(entries(&s), ["AlXpha\t2", "Beta budget\t3"]);
}

#[test]
fn page_numbers_only_is_one_undo_step() {
    let mut s = with_toc(&["Intro", "Middle", "Intro"], true);
    push_down(&mut s, "Intro");
    edit_entry(&mut s, "Intro");
    let steps = s.undo_depth();
    pages_only(&mut s);
    assert_eq!(s.undo_depth(), steps + 1);
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(entries(&s), ["InXtro\t1", "Middle\t2", "Intro\t3"]);
}

#[test]
fn page_numbers_only_without_a_table_changes_nothing() {
    let mut s = Session::new(doc(&["Intro", "Middle"]));
    let (steps, text) = (s.undo_depth(), s.doc.plain_text(StoryRef::Body));
    pages_only(&mut s);
    assert_eq!((s.undo_depth(), s.doc.plain_text(StoryRef::Body)), (steps, text));
}
