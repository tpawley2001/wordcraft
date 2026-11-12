//! Wholly deleted paragraphs (text and mark tracked deletions) laid out with deletions hidden,
//! in table cells and next to tables. Expected values are from Microsoft 365 Word's PDFs of
//! small documents (Times New Roman 12 pt, a two-column fixed table with single borders): a
//! cell's last paragraph mark ends the cell and stays, so a wholly deleted last paragraph is an
//! empty line; and a bookmark in a wholly deleted paragraph doesn't keep its line.
use crate::*;
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::LineSpacing;
use wordcraft_doc::{Block, Pos, Table, para_block};

fn chr() -> CharProps {
    CharProps { font: Some("Times New Roman".into()), size: Some(12.0), ..Default::default() }
}

fn para(text: &str) -> Paragraph {
    let mut p = if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, chr()) };
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
    p.mark = chr();
    p
}

/// A paragraph whose text and mark are both tracked deletions.
fn gone(text: &str) -> Paragraph {
    let mut p = para("");
    if !text.is_empty() {
        p = Paragraph::with_text(text, CharProps { del: Some(0), ..chr() });
        p.props = para("").props;
    }
    p.mark = CharProps { del: Some(0), ..chr() };
    p
}

/// [`gone`] holding a bookmark around its text (the bookmark itself not deleted), as a
/// heading's table-of-contents bookmark is.
fn gone_marked(text: &str) -> Paragraph {
    let mut p = gone(text);
    let end = p.text.len();
    p.insert_object(end, InlineObject::BookmarkEnd { name: "_Toc1".into() }, &chr()).unwrap();
    p.insert_object(0, InlineObject::BookmarkStart { name: "_Toc1".into() }, &chr()).unwrap();
    p
}

/// A document: "Above", then `before` in the body, then a table whose first row's two cells
/// hold `cells`, then rows "R" and "E".
fn doc(before: Vec<Paragraph>, cells: [Vec<Block>; 2]) -> Document {
    let mut d = Document::new();
    let mut body: Vec<_> = std::iter::once(para("Above")).chain(before).map(para_block).collect();
    let mut t = Table::new(3, 2, 300.0);
    for (cell, blocks) in t.rows[0].cells.iter_mut().zip(cells) {
        cell.blocks = blocks.into_iter().map(std::sync::Arc::new).collect();
    }
    for (r, text) in [(1, "R"), (2, "E")] {
        for cell in &mut t.rows[r].cells {
            cell.blocks = vec![para_block(para(text))];
        }
    }
    body.push(std::sync::Arc::new(Block::Table(t)));
    body.push(para_block(para("")));
    d.body = body.into_iter().collect();
    d
}

fn blocks(paras: Vec<Paragraph>) -> Vec<Block> {
    paras.into_iter().map(Block::Para).collect()
}

fn lay_out(d: &Document, hide: bool) -> DocLayout {
    crate::test_fonts();
    layout(d, &mut LayoutCache::new(), &LayoutOptions { hide_deleted: hide, ..Default::default() })
}

/// The table's index in the body.
fn table_at(d: &Document) -> u32 {
    d.body.iter().position(|b| matches!(**b, Block::Table(_))).unwrap() as u32
}

/// The top of row `r` (its first cell's first paragraph's caret) and of the table's row "R".
fn row_top(d: &Document, l: &DocLayout, r: u32) -> f32 {
    l.caret(&Pos { story: StoryRef::Body, path: Path(vec![table_at(d), r, 0, 0]), off: 0 }).unwrap().top
}

/// A text line's pitch in the body.
fn pitch() -> f32 {
    let mut d = Document::new();
    d.body = [para("X"), para("Y")].into_iter().map(para_block).collect();
    let l = lay_out(&d, true);
    l.caret(&Pos::body(1, 0)).unwrap().top - l.caret(&Pos::body(0, 0)).unwrap().top
}

/// The height of the first row (to the top of the second) and of a one-line row, the second.
fn rows(d: &Document, hide: bool) -> (f32, f32) {
    let l = lay_out(d, hide);
    let (a, b, c) = (row_top(d, &l, 0), row_top(d, &l, 1), row_top(d, &l, 2));
    (b - a, c - b)
}

// Word: "A1" over a wholly deleted "GoneA", next to "B1": the row is two lines (28.08 pt), an
// empty line under A1. The row had come out one line.
#[test]
fn a_wholly_deleted_last_paragraph_in_a_cell_keeps_an_empty_line() {
    let d = doc(vec![], [blocks(vec![para("A1"), gone("GoneA")]), blocks(vec![para("B1")])]);
    let (row, line) = rows(&d, true);
    assert!((row - line - pitch()).abs() < 0.1, "row {row}, a one-line row {line}");
    // A chain of them joins into the last, which keeps its line all the same.
    let d = doc(vec![], [blocks(vec![para("A1"), gone("G1"), gone("G2")]), blocks(vec![para("B1")])]);
    let (row, line) = rows(&d, true);
    assert!((row - line - pitch()).abs() < 0.1, "row {row}, a one-line row {line}");
}

// Word: a cell holding only a wholly deleted "GoneC", next to "D1": the row is one line.
#[test]
fn a_cell_of_only_a_wholly_deleted_paragraph_is_one_line() {
    let d = doc(vec![], [blocks(vec![gone("GoneC")]), blocks(vec![para("D1")])]);
    let (row, line) = rows(&d, true);
    assert!((row - line).abs() < 0.1, "row {row}, a one-line row {line}");
}

// Word: a row (not itself deleted) whose cells hold only wholly deleted paragraphs is kept, one
// line high (14.4 pt). It had shrunk to a few points.
#[test]
fn a_row_of_wholly_deleted_cells_keeps_one_line() {
    let d = doc(vec![], [blocks(vec![gone("Gone1"), gone("Gone2")]), blocks(vec![gone("Gone3")])]);
    let (row, line) = rows(&d, true);
    assert!((row - line).abs() < 0.1, "row {row}, a one-line row {line}");
}

// Word: "Mid", then a wholly deleted paragraph holding a bookmark, then a table: no line between
// Mid and the table. It had kept a blank line.
#[test]
fn a_bookmark_in_a_wholly_deleted_paragraph_before_a_table_takes_no_line() {
    let cells = || [blocks(vec![para("A1")]), blocks(vec![para("B1")])];
    let with = doc(vec![para("Mid"), gone_marked("Gone")], cells());
    let without = doc(vec![para("Mid")], cells());
    let (a, b) = (lay_out(&with, true), lay_out(&without, true));
    let (ta, tb) = (row_top(&with, &a, 0), row_top(&without, &b, 0));
    assert!((ta - tb).abs() < 0.05, "table at {ta} with the bookmark, {tb} without");
    // A bookmark alone in a paragraph with a deleted mark is as empty.
    let mut alone = para("");
    alone.insert_object(0, InlineObject::BookmarkStart { name: "b".into() }, &chr()).unwrap();
    alone.mark.del = Some(0);
    let only = doc(vec![para("Mid"), alone], cells());
    let o = lay_out(&only, true);
    assert!((row_top(&only, &o, 0) - tb).abs() < 0.05);
}

// Markup shown: the deleted paragraph is shown with its text, so it keeps its line.
#[test]
fn with_deletions_shown_the_bookmarked_paragraph_keeps_its_line() {
    let cells = || [blocks(vec![para("A1")]), blocks(vec![para("B1")])];
    let with = doc(vec![para("Mid"), gone_marked("Gone")], cells());
    let without = doc(vec![para("Mid")], cells());
    let (a, b) = (lay_out(&with, false), lay_out(&without, false));
    let gap = row_top(&with, &a, 0) - row_top(&without, &b, 0);
    assert!((gap - pitch()).abs() < 0.1, "gap {gap}, one line {}", pitch());
}

// Word: "A1" over a wholly deleted paragraph holding a bookmark, last in its cell: the empty
// line is kept (28.08 pt).
#[test]
fn a_bookmarked_wholly_deleted_last_paragraph_in_a_cell_keeps_its_line() {
    let d = doc(vec![], [blocks(vec![para("A1"), gone_marked("Gone")]), blocks(vec![para("B1")])]);
    let (row, line) = rows(&d, true);
    assert!((row - line - pitch()).abs() < 0.1, "row {row}, a one-line row {line}");
}

// Word: a paragraph whose text is deleted but whose mark is not keeps an empty line.
#[test]
fn a_deleted_text_with_its_mark_kept_keeps_an_empty_line() {
    let mut mid = gone("Gone");
    mid.mark = chr();
    let mut d = Document::new();
    d.body = [para("After"), mid, para("End"), para("Next")].into_iter().map(para_block).collect();
    let l = lay_out(&d, true);
    let top = |i: usize| l.caret(&Pos::body(i, 0)).unwrap().top;
    let line = top(3) - top(2);
    assert!((top(2) - top(0) - 2.0 * line).abs() < 0.1, "{} vs a line of {line}", top(2) - top(0));
}

// A cell whose last block closes a nested table is as before: the paragraph after the table
// takes no room, deleted mark or not.
#[test]
fn a_nested_table_cell_is_unchanged() {
    let cell = |tail: Paragraph| {
        let mut inner = Table::new(1, 1, 100.0);
        inner.rows[0].cells[0].blocks = vec![para_block(para("In"))];
        vec![Block::Para(para("A1")), Block::Table(inner), Block::Para(tail)]
    };
    let kept = doc(vec![], [cell(para("")), blocks(vec![para("B1")])]);
    let deleted = doc(vec![], [cell(gone("")), blocks(vec![para("B1")])]);
    let (a, _) = rows(&kept, true);
    let (b, _) = rows(&deleted, true);
    assert!((a - b).abs() < 0.05, "row {a} with a kept mark, {b} with a deleted one");
}
