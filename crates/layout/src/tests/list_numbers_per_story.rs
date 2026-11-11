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
