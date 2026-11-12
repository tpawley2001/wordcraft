//! A cell's last paragraph takes no room when it is empty and comes right after a nested table
//! (the paragraph a DOCX cell needs after a table, ECMA-376 Part 1 §17.4.66), as in Word.
//! Measured in Word's PDFs of six synthetic documents (`TOP`, a one-cell table, `AFTER`; the
//! distance from `TOP` to `AFTER` in points): nested table + empty paragraph 52.0; nested table +
//! `X` 77.0; `T` + empty paragraph 76.0; nested table + two empty paragraphs 101.9; nested table +
//! empty paragraph with a 24 pt mark 52.0; `T` + nested table + empty paragraph 77.0.
use crate::*;
use wordcraft_doc::{Table, para_block};

fn para(text: &str) -> Arc<Block> {
    let mut p = Paragraph::with_text(text, Default::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    para_block(p)
}

fn empty_with_mark_size(size: f32) -> Arc<Block> {
    let mut p = Paragraph::with_text("", Default::default());
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.mark.size = Some(size);
    para_block(p)
}

fn nested() -> Arc<Block> {
    let mut inner = Table::new(1, 1, 200.0);
    inner.rows[0].cells[0].blocks = vec![para("IN")];
    Arc::new(Block::Table(inner))
}

/// Where the paragraph `AFTER` starts, below `TOP` and a one-cell table whose cell holds `blocks`.
fn after_y(blocks: Vec<Arc<Block>>) -> f32 {
    let mut d = Document::from_text("TOP");
    if let Some(Block::Para(p)) = d.body.first_mut().map(Arc::make_mut) {
        p.props.space_before = Some(0.0);
        p.props.space_after = Some(0.0);
    }
    let mut outer = Table::new(1, 1, 440.0);
    outer.rows[0].cells[0].blocks = blocks;
    d.insert_block(StoryRef::Body, &Path::top(1), Block::Table(outer)).unwrap();
    d.body.push(para("AFTER"));
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    assert_eq!(l.pages.len(), 1, "the probe fits one page");
    let last = d.body.len() as u32 - 1;
    l.pages[0]
        .items
        .iter()
        .find_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, path, y, .. } if path.0.len() == 1 && path.0[0] == last => Some(*y),
            _ => None,
        })
        .expect("AFTER is placed")
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

/// One line of the default paragraph: what an empty paragraph after text adds.
fn line() -> f32 {
    after_y(vec![para("T"), para("")]) - after_y(vec![para("T")])
}

#[test]
fn an_empty_paragraph_after_text_keeps_its_line() {
    assert!(line() > 5.0, "an empty paragraph after text is a line, got {}", line());
}

#[test]
fn an_empty_last_paragraph_after_a_nested_table_takes_no_room() {
    let (with, without) = (after_y(vec![nested(), para("")]), after_y(vec![nested()]));
    assert!(close(with, without), "AFTER at {with}, expected {without} (no room for the closing paragraph)");
}

#[test]
fn no_room_with_text_before_the_nested_table_either() {
    let (with, without) = (after_y(vec![para("T"), nested(), para("")]), after_y(vec![para("T"), nested()]));
    assert!(close(with, without), "AFTER at {with}, expected {without}");
}

#[test]
fn the_size_of_the_empty_paragraphs_mark_does_not_matter() {
    let (with, without) = (after_y(vec![nested(), empty_with_mark_size(24.0)]), after_y(vec![nested()]));
    assert!(close(with, without), "AFTER at {with}, expected {without} (a 24 pt mark adds nothing)");
}

#[test]
fn a_last_paragraph_with_text_keeps_its_line() {
    let (with, without) = (after_y(vec![nested(), para("X")]), after_y(vec![nested()]));
    assert!(close(with - without, line()), "X adds {} pt, expected one line of {}", with - without, line());
}

#[test]
fn two_empty_paragraphs_after_a_nested_table_both_keep_their_lines() {
    let (with, without) = (after_y(vec![nested(), para(""), para("")]), after_y(vec![nested()]));
    assert!(close(with - without, 2.0 * line()), "two empty paragraphs add {} pt, expected {}", with - without, 2.0 * line());
}
