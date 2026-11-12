//! List numbers around vertically merged table cells. A cell that continues a vertical merge is
//! hidden, but Word still lets its numbered paragraphs that have text use up their numbers; an
//! empty numbered paragraph in a hidden cell takes none. Expected labels from Microsoft 365
//! Word's PDFs of six small documents (one decimal list, a 3 x 2 table between two numbered
//! paragraphs).
use crate::*;
use std::sync::Arc;
use wordcraft_doc::props::{NumRef, VMerge};
use wordcraft_doc::{Block, PartKind, Table, para_block};

/// A paragraph of the list `num`; no text gives an empty paragraph.
fn numbered(text: &str, num: u32) -> Paragraph {
    let mut p = if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, CharProps::default()) };
    p.props.numbering = Some(NumRef { num, level: 0 });
    p
}

fn plain(text: &str) -> Paragraph {
    if text.is_empty() { Paragraph::new() } else { Paragraph::with_text(text, CharProps::default()) }
}

/// One cell of a fixture: its paragraphs (text, numbered?) and whether it continues the merge
/// of the cell above.
struct C(&'static [(&'static str, bool)], bool);

/// A two-column table of `rows`; a cell above a hidden one starts the merge.
fn table(rows: &[[C; 2]], num: u32) -> Table {
    let mut t = Table::new(rows.len(), 2, 300.0);
    for (ri, row) in rows.iter().enumerate() {
        for (ci, c) in row.iter().enumerate() {
            let cell = &mut t.rows[ri].cells[ci];
            cell.blocks = c.0.iter().map(|(text, n)| para_block(if *n { numbered(text, num) } else { plain(text) })).collect();
            let below_hidden = rows.get(ri + 1).is_some_and(|r| r[ci].1);
            cell.props.vmerge = if c.1 {
                VMerge::Continue
            } else if below_hidden {
                VMerge::Restart
            } else {
                VMerge::None
            };
        }
    }
    t
}

/// The list labels of `items` in reading order (top to bottom, then left to right).
fn labels(items: &[Placed]) -> Vec<String> {
    let mut v: Vec<(f32, f32, String)> = items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { para, x, y, l0: 0, .. } => para.label.as_ref().map(|l| (*y, *x, l.text.trim().to_owned())),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.into_iter().map(|(_, _, t)| t).collect()
}

fn lay(d: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(d, &mut c, &LayoutOptions::default())
}

/// The labels Word shows for "before", the table of `rows` and "after", all in one list.
fn body_labels(rows: &[[C; 2]]) -> Vec<String> {
    crate::test_fonts();
    let mut d = Document::new();
    let num = Arc::make_mut(&mut d.numbering).add_list(wordcraft_doc::ListKind::Numbered);
    d.body = vec![para_block(numbered("before", num)), Arc::new(Block::Table(table(rows, num))), para_block(numbered("after", num))];
    let l = lay(&d);
    assert_eq!(l.pages.len(), 1);
    labels(&l.pages[0].items)
}

const SHOWN: bool = false;
const HIDDEN: bool = true;

#[test]
fn a_hidden_numbered_paragraph_with_text_uses_up_its_number() {
    let rows = [
        [C(&[("mergeTop", true)], SHOWN), C(&[("r1c2", true)], SHOWN)],
        [C(&[("hiddenA", true)], HIDDEN), C(&[("r2c2", true)], SHOWN)],
        [C(&[("hiddenB", true)], HIDDEN), C(&[("r3c2", true)], SHOWN)],
    ];
    assert_eq!(body_labels(&rows), ["1.", "2.", "3.", "5.", "7.", "8."]);
}

#[test]
fn a_hidden_empty_numbered_paragraph_takes_no_number() {
    let rows = [
        [C(&[("mergeTop", true)], SHOWN), C(&[("r1c2", true)], SHOWN)],
        [C(&[("", true)], HIDDEN), C(&[("r2c2", true)], SHOWN)],
        [C(&[("", true)], HIDDEN), C(&[("r3c2", true)], SHOWN)],
    ];
    assert_eq!(body_labels(&rows), ["1.", "2.", "3.", "4.", "5.", "6."]);
}

#[test]
fn a_hidden_cell_in_the_second_column_counts_after_the_cell_before_it() {
    let rows = [
        [C(&[("r1c1", true)], SHOWN), C(&[("mergeTop", true)], SHOWN)],
        [C(&[("r2c1", true)], SHOWN), C(&[("hiddenA", true)], HIDDEN)],
        [C(&[("r3c1", true)], SHOWN), C(&[("hiddenB", true)], HIDDEN)],
    ];
    assert_eq!(body_labels(&rows), ["1.", "2.", "3.", "4.", "6.", "8."]);
}

#[test]
fn a_table_without_merges_counts_every_cell() {
    let rows = [
        [C(&[("c1", true)], SHOWN), C(&[("r1c2", true)], SHOWN)],
        [C(&[("c2", true)], SHOWN), C(&[("r2c2", true)], SHOWN)],
        [C(&[("c3", true)], SHOWN), C(&[("r3c2", true)], SHOWN)],
    ];
    assert_eq!(body_labels(&rows), ["1.", "2.", "3.", "4.", "5.", "6.", "7.", "8."]);
}

#[test]
fn an_empty_numbered_paragraph_in_a_visible_cell_shows_its_number() {
    let rows = [
        [C(&[("c1", true)], SHOWN), C(&[("r1c2", true)], SHOWN)],
        [C(&[("", true)], SHOWN), C(&[("r2c2", true)], SHOWN)],
        [C(&[("", true)], SHOWN), C(&[("r3c2", true)], SHOWN)],
    ];
    assert_eq!(body_labels(&rows), ["1.", "2.", "3.", "4.", "5.", "6.", "7.", "8."]);
}

#[test]
fn only_the_hidden_numbered_paragraphs_with_text_count() {
    let rows = [
        [C(&[("mergeTop", true)], SHOWN), C(&[("r1c2", true)], SHOWN)],
        [C(&[("", true), ("hiddenA", true), ("", false)], HIDDEN), C(&[("r2c2", true)], SHOWN)],
        [C(&[("plainHidden", false)], HIDDEN), C(&[("r3c2", true)], SHOWN)],
    ];
    assert_eq!(body_labels(&rows), ["1.", "2.", "3.", "5.", "6.", "7."]);
}

/// A header holding a 2 x 2 table (first column merged, `hidden` in its hidden cell) and a
/// footer of one numbered paragraph: the header's and the footer's labels.
fn header_and_footer(hidden: &'static str) -> (Vec<String>, Vec<String>) {
    crate::test_fonts();
    let mut d = Document::new();
    let num = Arc::make_mut(&mut d.numbering).add_list(wordcraft_doc::ListKind::Numbered);
    let cell: &'static [(&'static str, bool)] = if hidden.is_empty() { &[("", true)] } else { &[("hidden", true)] };
    let rows = [[C(&[("HT", true)], SHOWN), C(&[("H1", true)], SHOWN)], [C(cell, HIDDEN), C(&[("H2", true)], SHOWN)]];
    let h = d.add_part(PartKind::Header, vec![Arc::new(Block::Table(table(&rows, num))), para_block(plain("under the table"))]);
    d.last_section.headers.default = Some(h);
    let f = d.add_part(PartKind::Footer, vec![para_block(numbered("F1", num))]);
    d.last_section.footers.default = Some(f);
    d.body = vec![para_block(plain("body"))];
    let l = lay(&d);
    (labels(&l.pages[0].header), labels(&l.pages[0].footer))
}

#[test]
fn the_footer_continues_after_a_header_table_with_an_empty_hidden_cell() {
    let (header, footer) = header_and_footer("");
    assert_eq!(header, ["1.", "2.", "3."]);
    assert_eq!(footer, ["4."]);
}

#[test]
fn the_footer_continues_after_a_header_table_whose_hidden_cell_has_text() {
    let (header, footer) = header_and_footer("hidden");
    assert_eq!(header, ["1.", "2.", "4."]);
    assert_eq!(footer, ["5."]);
}
