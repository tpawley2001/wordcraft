//! More cases for the rule that a table row kept with the next is not split across pages
//! (`tests/kept_row_not_split.rs` holds the measured probes).

use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{Pos, Row, Table, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn props(keep: bool) -> ParaProps {
    ParaProps {
        space_before: Some(0.0),
        space_after: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(1.0)),
        widow_control: Some(false),
        keep_next: Some(keep),
        ..Default::default()
    }
}

fn para(text: &str, keep: bool) -> Arc<Block> {
    let mut x = Paragraph::with_text(text, Default::default());
    x.props = props(keep);
    para_block(x)
}

fn lines_per_page() -> usize {
    let mut d = Document::new();
    d.body = (0..400).map(|i| para(&format!("F{i}"), false)).collect();
    let l = lay(&d);
    (0..400).take_while(|i| l.caret(&Pos::body(*i, 0)).unwrap().page == 0).count()
}

/// A two-column table of the given rows (first cell, second cell).
fn table(rows: Vec<(Vec<Arc<Block>>, Vec<Arc<Block>>)>) -> Table {
    let mut t = Table::new(1, 2, 440.0);
    let template: Row = t.rows[0].clone();
    t.rows = rows
        .into_iter()
        .map(|(a, b)| {
            let mut r = template.clone();
            r.cells[0].blocks = a;
            r.cells[1].blocks = b;
            r
        })
        .collect();
    t
}

/// Pages holding lines whose path starts with `prefix` and is `len` long.
fn pages_with(l: &DocLayout, prefix: &[u32], len: usize) -> Vec<usize> {
    l.pages
        .iter()
        .enumerate()
        .filter(|(_, pg)| pg.items.iter().any(|it| matches!(it, Placed::Lines { path, .. } if path.0.len() == len && path.0.starts_with(prefix))))
        .map(|(i, _)| i)
        .collect()
}

/// Fillers, then a table: a one-line row and a row whose first cell holds a plain paragraph and
/// a nested table whose one row is kept with the next and taller than the room left.
#[test]
fn a_kept_row_of_a_nested_table_does_not_keep_the_outer_row_whole() {
    let above = lines_per_page() - 16;
    let mut d = Document::new();
    d.body = (0..above).map(|i| para(&format!("F{i}"), false)).collect();
    let nested = table(vec![(vec![para("inner", true)], (0..24).map(|i| para(&format!("n{i}"), true)).collect())]);
    let outer = table(vec![
        (vec![para("ROWONE", false)], vec![para("x", false)]),
        (vec![para("ROWTWO", false), Arc::new(Block::Table(nested))], vec![para("y", false)]),
    ]);
    let tb = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(outer)));
    d.body.push(para("AFTER", false));
    let l = lay(&d);
    // ROWTWO stays on page 1 and the nested row's lines run onto page 2, as before.
    assert_eq!(pages_with(&l, &[tb, 1, 0, 0], 4), [0]);
    assert_eq!(pages_with(&l, &[tb, 1, 0], 7), [0, 1]);
}

/// A kept row that fits where it is stays there, and the next row after it too.
#[test]
fn a_kept_row_that_fits_is_not_moved() {
    let above = lines_per_page() - 16;
    let mut d = Document::new();
    d.body = (0..above).map(|i| para(&format!("F{i}"), false)).collect();
    let t = table(vec![
        (vec![para("ROWONE", true)], vec![para("x", true)]),
        (vec![para("ROWTWO", true)], (0..10).map(|i| para(&format!("t{i}"), true)).collect()),
        (vec![para("ROWTHREE", false)], vec![para("z", false)]),
    ]);
    let tb = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(t)));
    let l = lay(&d);
    for r in 0..3 {
        assert_eq!(pages_with(&l, &[tb, r], 4), [0], "row {r}");
    }
}

/// A plain row that does not fit still splits when the row before it is kept (only the kept
/// row itself keeps whole).
#[test]
fn a_plain_row_after_a_kept_one_still_splits() {
    let above = lines_per_page() - 16;
    let mut d = Document::new();
    d.body = (0..above).map(|i| para(&format!("F{i}"), false)).collect();
    let t = table(vec![
        (vec![para("ROWONE", false)], vec![para("x", false)]),
        (vec![para("ROWTWO", false)], (0..24).map(|i| para(&format!("t{i}"), false)).collect()),
    ]);
    let tb = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(t)));
    let l = lay(&d);
    assert_eq!(pages_with(&l, &[tb, 1, 1], 4), [0, 1]);
}

/// A heading kept with the next above a table whose last row is kept and whose kept first
/// paragraph runs several lines, more than half the row: when that row does not fit whole, the
/// row moves on, the row kept above it follows, and the heading goes with them.
#[test]
fn a_heading_follows_a_kept_last_row_with_a_tall_kept_paragraph() {
    let words = (0..75).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ");
    let t = table(vec![
        (vec![para("ROWONE", true)], vec![para("x", false)]),
        (vec![para(&words, true)], (0..12).map(|i| para(&format!("tall{i}"), false)).collect()),
    ]);
    // How tall the kept paragraph and the row are, in lines, on a page of their own.
    let mut d = Document::new();
    d.body = vec![Arc::new(Block::Table(t.clone()))];
    let l = lay(&d);
    let kept = l
        .pages
        .iter()
        .flat_map(|pg| &pg.items)
        .filter_map(|it| if let Placed::Lines { path, l0, l1, .. } = it { (path.0 == [0, 1, 0, 0]).then_some(l1 - l0) } else { None })
        .sum::<usize>();
    assert!(kept > 7 && kept < 12, "{kept} lines");
    // The heading's line, ROWONE's, then room for the kept paragraph but not the whole row.
    let fillers = lines_per_page() - (kept + 3);
    let mut d = Document::new();
    d.body = (0..fillers).map(|i| para(&format!("F{i}"), false)).collect();
    let heading = d.body.len();
    d.body.push(para("HEADING", true));
    let tb = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(t)));
    d.body.push(para("AFTER", false));
    let l = lay(&d);
    assert_eq!(pages_with(&l, &[tb, 1, 0, 0], 4), [1]);
    assert_eq!(pages_with(&l, &[tb, 0, 0, 0], 4), [1]);
    assert_eq!(l.caret(&Pos::body(heading, 0)).unwrap().page, 1);
}
