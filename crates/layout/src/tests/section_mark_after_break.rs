//! An empty paragraph that only carries a next-page section break, right after a page break (or
//! set to Page break before), starts no page of its own: the next section starts on the page the
//! break opened. Expected pages and text from Microsoft 365 Word's PDF output.

use crate::*;
use std::sync::Arc;
use wordcraft_doc::para::{InlineObject, PAGE_BREAK};
use wordcraft_doc::props::NumRef;
use wordcraft_doc::section::{SectionProps, SectionStart};
use wordcraft_doc::{PartKind, Path, Pos, StoryRef, Table, para_block};

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// The body paragraphs of `text` (`\n` between paragraphs, U+000C a page break), block `mark`
/// carrying the first section's break, compatibility mode `mode`, and a header in both sections.
fn doc(text: &str, mark: usize, mode: u32) -> Document {
    crate::test_fonts();
    let mut d = Document::from_text(text);
    d.settings.compat_mode = mode;
    let head = d.add_part(PartKind::Header, vec![para_block(Paragraph::with_text("Head", CharProps::default()))]);
    let mut first = SectionProps::default();
    first.headers.default = Some(head);
    d.para_mut(StoryRef::Body, &Path::top(mark)).unwrap().section = Some(Box::new(first));
    d.last_section.headers.default = Some(head);
    d
}

fn mark_mut(d: &mut Document, mark: usize) -> &mut Paragraph {
    d.para_mut(StoryRef::Body, &Path::top(mark)).unwrap()
}

/// The text on each page, paragraph by paragraph (breaks left out, empty paragraphs skipped).
fn pages(d: &Document, l: &DocLayout) -> Vec<Vec<String>> {
    l.pages
        .iter()
        .map(|pg| {
            pg.items
                .iter()
                .filter_map(|it| match it {
                    Placed::Lines { story: StoryRef::Body, path, .. } => {
                        let p = d.body.get(*path.0.first()? as usize)?.as_para()?;
                        let t: String = p.plain_text().chars().filter(|c| !c.is_control() && *c != '\u{FFFC}').collect();
                        (!t.is_empty()).then_some(t)
                    }
                    Placed::Cell { .. } => Some("[table]".to_owned()),
                    _ => None,
                })
                .fold(Vec::<String>::new(), |mut v, t| {
                    if v.last() != Some(&t) {
                        v.push(t);
                    }
                    v
                })
        })
        .collect()
}

fn caret_page(l: &DocLayout, block: usize) -> usize {
    l.caret(&Pos::body(block, 0)).unwrap().page
}

/// Rows 1-7: two pages, [COVER] [BODY1], the mark's caret on page 1, no blank page, headers on
/// pages numbered 1 and 2.
fn two_pages(d: &Document, mark: usize) {
    let l = lay(d);
    assert_eq!(pages(d, &l), [vec!["COVER"], vec!["BODY1"]]);
    assert_eq!(caret_page(&l, mark), 0);
    assert_eq!(caret_page(&l, mark + 1), 1);
    assert!(l.pages.iter().all(|p| !p.blank));
    assert!(l.pages.iter().all(|p| !p.header.is_empty()));
    assert_eq!(l.pages.iter().map(|p| p.number).collect::<Vec<_>>(), [1, 2]);
}

fn body(text: &str) -> String {
    text.replace("PB", &PAGE_BREAK.to_string())
}

#[test]
fn row1_an_empty_mark_after_a_page_break_starts_no_page() {
    two_pages(&doc(&body("COVER\nPB\n\nBODY1"), 2, 15), 2);
}

#[test]
fn row2_without_a_compatibility_mode_too() {
    two_pages(&doc(&body("COVER\nPB\n\nBODY1"), 2, 12), 2);
}

#[test]
fn row3_after_a_break_ending_the_cover_paragraph() {
    two_pages(&doc(&body("COVERPB\n\nBODY1"), 1, 15), 1);
}

#[test]
fn row4_a_mark_holding_only_a_bookmark() {
    let mut d = doc(&body("COVER\nPB\n\nBODY1"), 2, 15);
    let m = mark_mut(&mut d, 2);
    m.insert_object(0, InlineObject::BookmarkStart { name: "b".into() }, &CharProps::default()).unwrap();
    m.insert_object(m.text.len(), InlineObject::BookmarkEnd { name: "b".into() }, &CharProps::default()).unwrap();
    two_pages(&d, 2);
}

#[test]
fn row5_a_numbered_mark_shows_no_number() {
    let mut d = doc(&body("COVER\nPB\n\nBODY1"), 2, 15);
    let num = Arc::make_mut(&mut d.numbering).add_list(wordcraft_doc::ListKind::Numbered);
    mark_mut(&mut d, 2).props.numbering = Some(NumRef { num, level: 0 });
    two_pages(&d, 2);
    let l = lay(&d);
    let labelled = l.pages.iter().flat_map(|p| &p.items).any(|it| match it {
        Placed::Lines { para, .. } => para.label.as_ref().is_some_and(|lb| lb.text.contains("1.")),
        _ => false,
    });
    assert!(!labelled);
}

#[test]
fn row6_a_mark_with_page_break_before_after_a_page_break() {
    let mut d = doc(&body("COVER\nPB\n\nBODY1"), 2, 15);
    mark_mut(&mut d, 2).props.page_break_before = Some(true);
    two_pages(&d, 2);
}

#[test]
fn row7_a_mark_with_page_break_before_alone() {
    let mut d = doc("COVER\n\nBODY1", 1, 15);
    mark_mut(&mut d, 1).props.page_break_before = Some(true);
    two_pages(&d, 1);
}

#[test]
fn row8_an_empty_paragraph_between_break_and_mark_keeps_its_page() {
    let d = doc(&body("COVER\nPB\n\n\nBODY1"), 3, 15);
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec![], vec!["BODY1"]]);
}

#[test]
fn row9_a_mark_with_text_keeps_its_page() {
    let d = doc(&body("COVER\nPB\nMARK\nBODY1"), 2, 15);
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec!["MARK"], vec!["BODY1"]]);
}

#[test]
fn row10_split_page_break_and_mark_keeps_three_pages() {
    let mut d = doc(&body("COVER\nPB\n\nBODY1"), 2, 12);
    d.settings.split_pg_break_and_para_mark = true;
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec![], vec!["BODY1"]]);
}

#[test]
fn row11_a_continuous_next_section_is_unchanged() {
    let mut d = doc(&body("COVER\nPB\n\nBODY1"), 2, 15);
    d.last_section.start = SectionStart::Continuous;
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec!["BODY1"]]);
}

#[test]
fn row12_an_empty_mark_without_a_break_is_unchanged() {
    let d = doc("COVER\n\nBODY1", 1, 15);
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec!["BODY1"]]);
}

#[test]
fn a_table_after_the_break_keeps_the_mark_on_its_page() {
    let mut d = doc(&body("COVER\nPB\n\nBODY1"), 2, 15);
    d.body.insert(2, Arc::new(Block::Table(Table::new(1, 1, 200.0))));
    let mark = 3;
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec!["[table]"], vec!["BODY1"]]);
    assert_eq!(caret_page(&l, mark), 1);
}

#[test]
fn in_two_columns_the_mark_goes_under_the_break_in_its_column() {
    let text = format!("COVER{}\nPB\n\nBODY1", wordcraft_doc::para::COLUMN_BREAK);
    let mut d = doc(&body(&text), 2, 15);
    let s = mark_mut(&mut d, 2).section.as_mut().unwrap();
    s.columns.count = 2;
    s.columns.space = 36.0;
    let l = lay(&d);
    assert_eq!(pages(&d, &l), [vec!["COVER"], vec!["BODY1"]]);
    let x_of = |block: usize| {
        l.pages[0].items.iter().find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, x, .. } if path.0 == [block as u32] => Some(*x),
            _ => None,
        })
    };
    let (brk, mark, cover) = (x_of(1).unwrap(), x_of(2).unwrap(), x_of(0).unwrap());
    assert!((brk - mark).abs() < 0.01 && brk > cover + 100.0, "{brk} {mark} {cover}");
}
