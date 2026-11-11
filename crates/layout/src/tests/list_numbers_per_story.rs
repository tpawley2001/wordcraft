//! Word counts a list separately in each story: the body, the headers and footers together, the
//! text boxes, the footnotes and the endnotes. Expected labels from Microsoft 365 Word's PDF of a
//! document whose numbered paragraphs all use one list (header 1-3, footer 4-6, body 1-3).
use crate::*;
use std::sync::Arc;
use wordcraft_doc::props::NumRef;
use wordcraft_doc::{PartKind, para_block};

fn numbered(text: &str, num: u32) -> Paragraph {
    let mut p = Paragraph::with_text(text, CharProps::default());
    p.props.numbering = Some(NumRef { num, level: 0 });
    p
}

/// A document with a default header and footer of three numbered paragraphs each and three
/// numbered body paragraphs, all in one decimal list.
fn doc() -> Document {
    crate::test_fonts();
    let mut d = Document::new();
    let num = Arc::make_mut(&mut d.numbering).add_list(wordcraft_doc::ListKind::Numbered);
    let h = d.add_part(PartKind::Header, ["H1", "H2", "H3"].iter().map(|t| para_block(numbered(t, num))).collect());
    d.last_section.headers.default = Some(h);
    let f = d.add_part(PartKind::Footer, ["F1", "F2", "F3"].iter().map(|t| para_block(numbered(t, num))).collect());
    d.last_section.footers.default = Some(f);
    d.body = ["B1", "B2", "B3"].iter().map(|t| para_block(numbered(t, num))).collect();
    d
}

/// The list labels of `items`, top to bottom.
fn labels(items: &[Placed]) -> Vec<String> {
    let mut v: Vec<(f32, String)> = items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, y, l0: 0, .. } => para.label.as_ref().map(|l| (*y, l.text.trim().to_owned())),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().map(|(_, t)| t).collect()
}

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

#[test]
fn the_body_counts_from_one() {
    let l = lay(&doc());
    assert_eq!(labels(&l.pages[0].items), ["1.", "2.", "3."]);
}

#[test]
fn the_header_counts_from_one() {
    let l = lay(&doc());
    assert_eq!(labels(&l.pages[0].header), ["1.", "2.", "3."]);
}

#[test]
fn the_footer_continues_the_header_story() {
    let l = lay(&doc());
    assert_eq!(labels(&l.pages[0].footer), ["4.", "5.", "6."]);
}

#[test]
fn a_header_on_a_later_page_shows_the_same_numbers() {
    let mut d = doc();
    let num = d.body.first().and_then(|b| b.as_para()).and_then(|p| p.props.numbering).map(|n| n.num).expect("numbered");
    let mut body: Vec<_> = d.body.clone();
    for i in 0..80 {
        body.push(para_block(Paragraph::with_text(&format!("filler {i}"), CharProps::default())));
    }
    body.push(para_block(numbered("B4", num)));
    d.body = body;
    let l = lay(&d);
    assert!(l.pages.len() >= 2, "the filler runs onto a second page");
    let last = l.pages.last().expect("pages");
    assert_eq!(labels(&last.header), ["1.", "2.", "3."]);
    assert_eq!(labels(&last.footer), ["4.", "5.", "6."]);
    assert_eq!(labels(&last.items).last().map(String::as_str), Some("4."), "the body's count runs on");
}

/// Body B1-B3, then a paragraph with a text box (T1-T3), two footnotes (F1-F3, G1-G3) and an
/// endnote (E1-E3), then body B4-B6: every numbered paragraph in one decimal list, as in the
/// real-file table. Returns the document and the text box, footnote and endnote part ids.
fn doc_with_notes_and_box() -> (Document, [u32; 4]) {
    use wordcraft_doc::para::{InlineObject, NoteKind, ShapeKind};
    use wordcraft_doc::{Path, Pos};
    crate::test_fonts();
    let mut d = Document::new();
    let num = Arc::make_mut(&mut d.numbering).add_list(wordcraft_doc::ListKind::Numbered);
    let three = |a: &str| (1..=3).map(|i| para_block(numbered(&format!("{a}{i}"), num))).collect::<Vec<_>>();
    let tb = d.add_part(PartKind::TextBox, three("T"));
    let f1 = d.add_part(PartKind::Footnote, three("F"));
    let f2 = d.add_part(PartKind::Footnote, three("G"));
    let e1 = d.add_part(PartKind::Endnote, three("E"));
    let mut body = three("B");
    body.push(para_block(Paragraph::with_text("anchor", CharProps::default())));
    body.extend((4..=6).map(|i| para_block(numbered(&format!("B{i}"), num))));
    d.body = body;
    let shape = InlineObject::Shape {
        kind: ShapeKind::TextBox,
        w: 200.0,
        h: 80.0,
        fill: None,
        stroke: None,
        stroke_width: 1.0,
        float: Default::default(),
        story: Some(tb),
        effects: Default::default(),
        freeform: None,
        extra: Default::default(),
    };
    let at = |d: &Document| d.para(wordcraft_doc::StoryRef::Body, &Path::top(3)).map_or(0, |p| p.len());
    d.insert_object(&Pos::body(3, at(&d)), shape, &CharProps::default()).expect("box");
    for (kind, id) in [(NoteKind::Footnote, f1), (NoteKind::Footnote, f2), (NoteKind::Endnote, e1)] {
        let off = at(&d);
        d.insert_object(&Pos::body(3, off), InlineObject::NoteRef { kind, id, custom: String::new() }, &CharProps::default()).expect("note");
    }
    (d, [tb, f1, f2, e1])
}

/// The list labels of story `story` on every page, top to bottom within a page.
fn story_labels(l: &DocLayout, story: wordcraft_doc::StoryRef) -> Vec<String> {
    l.pages
        .iter()
        .flat_map(|pg| {
            let items: Vec<Placed> = pg.items.iter().filter(|it| matches!(it, Placed::Lines { story: s, .. } if *s == story)).cloned().collect();
            labels(&items)
        })
        .collect()
}

#[test]
fn the_body_runs_on_past_a_text_box_and_notes() {
    let (d, _) = doc_with_notes_and_box();
    let l = lay(&d);
    assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Body), ["1.", "2.", "3.", "4.", "5.", "6."]);
}

#[test]
fn a_text_box_counts_from_one() {
    let (d, [tb, ..]) = doc_with_notes_and_box();
    let l = lay(&d);
    assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(tb)), ["1.", "2.", "3."]);
}

#[test]
fn footnotes_share_one_count() {
    let (d, [_, f1, f2, _]) = doc_with_notes_and_box();
    let l = lay(&d);
    assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(f1)), ["1.", "2.", "3."]);
    assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(f2)), ["4.", "5.", "6."]);
}

#[test]
fn an_endnote_counts_from_one() {
    let (d, [.., e1]) = doc_with_notes_and_box();
    let l = lay(&d);
    assert_eq!(story_labels(&l, wordcraft_doc::StoryRef::Part(e1)), ["1.", "2.", "3."]);
}
