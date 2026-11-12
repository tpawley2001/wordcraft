//! A run of table rows kept with the next (the first paragraph of each row's first cell has keep
//! with next, as in a manual laid out as a table with a heading in every row) moves to the next
//! page once when it does not fit under what is above it, then breaks across pages like any
//! table: at the top of a page its rows are placed in order and the first one that does not fit
//! splits at the page foot. The row after a split row starts a new run. Measured in Microsoft 365
//! Word with small synthetic documents (Letter page, 12 pt, single spacing, 46 lines to a page):
//! one-line fillers, a two-column table whose first cells hold one one-line paragraph and whose
//! second cells hold `k` one-line paragraphs, and a paragraph AFTER.

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{Pos, Row, Table, para_block};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

fn plain() -> ParaProps {
    ParaProps {
        space_before: Some(0.0),
        space_after: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(1.0)),
        widow_control: Some(false),
        ..Default::default()
    }
}

fn kept(keep: bool) -> ParaProps {
    ParaProps { keep_next: Some(keep), ..plain() }
}

fn para(text: &str, p: &ParaProps) -> Arc<Block> {
    let mut x = Paragraph::with_text(text, Default::default());
    x.props = p.clone();
    para_block(x)
}

/// One-line plain paragraphs that fit on the first page.
fn lines_per_page() -> usize {
    let mut d = Document::new();
    d.body = (0..400).map(|i| para(&format!("F{i}"), &plain())).collect();
    let l = lay(&d);
    (0..400).take_while(|i| l.caret(&Pos::body(*i, 0)).unwrap().page == 0).count()
}

/// Where a row landed (zero-based pages): the page of its first cell's paragraph, and for each
/// page holding its second cell's paragraphs, how many of them are there (in order).
#[derive(Debug, PartialEq)]
struct RowAt {
    first: usize,
    second: Vec<(usize, usize)>,
}

fn at(first: usize, second: &[(usize, usize)]) -> RowAt {
    RowAt { first, second: second.to_vec() }
}

/// Lays out `above` filler lines, a table of `rows` (kept?, lines in the second cell) and AFTER;
/// returns each row's placement and AFTER's page.
fn run(above: usize, rows: &[(bool, usize)]) -> (Vec<RowAt>, usize) {
    let mut d = Document::new();
    d.body = (0..above).map(|i| para(&format!("F{}", i + 1), &plain())).collect();
    let mut t = Table::new(1, 2, 440.0);
    let template: Row = t.rows[0].clone();
    t.rows.clear();
    for (i, &(keep, k)) in rows.iter().enumerate() {
        let n = i + 1;
        let mut r = template.clone();
        r.cells[0].blocks = vec![para(&format!("R{n}"), &kept(keep))];
        r.cells[1].blocks = (1..=k).map(|j| para(&format!("R{n}L{j}"), &plain())).collect();
        t.rows.push(r);
    }
    let table = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(t)));
    let after = d.body.len();
    d.body.push(para("AFTER", &plain()));
    let l = lay(&d);
    let page = |row: usize, cell: u32, p: usize| {
        let path = Path(vec![table, row as u32, cell, p as u32]);
        l.caret(&Pos::new(StoryRef::Body, path, 0)).unwrap().page
    };
    let placed = rows
        .iter()
        .enumerate()
        .map(|(ri, &(_, k))| {
            let mut second: Vec<(usize, usize)> = Vec::new();
            for p in 0..k {
                let pg = page(ri, 1, p);
                match second.last_mut() {
                    Some((last, n)) if *last == pg => *n += 1,
                    _ => second.push((pg, 1)),
                }
            }
            RowAt { first: page(ri, 0, 0), second }
        })
        .collect();
    (placed, l.caret(&Pos::body(after, 0)).unwrap().page)
}

/// Fixture a (control): a plain row, then a kept row and its plain follower that fit on one
/// page move there together.
#[test]
fn a_kept_row_and_its_follower_move_whole() {
    let (rows, after) = run(30, &[(false, 1), (true, 24), (false, 1)]);
    assert_eq!(rows, [at(0, &[(0, 1)]), at(1, &[(1, 24)]), at(1, &[(1, 1)])]);
    assert_eq!(after, 1);
}

/// Fixture d (control): a run of two kept rows that fits on a page moves there whole.
#[test]
fn a_run_that_fits_on_a_page_moves_whole() {
    let (rows, after) = run(30, &[(true, 1), (true, 24), (false, 1)]);
    assert_eq!(rows, [at(1, &[(1, 1)]), at(1, &[(1, 24)]), at(1, &[(1, 1)])]);
    assert_eq!(after, 1);
}

/// Fixture b: five kept rows of 12 lines. The run moves to page 2 once; R4 splits at its foot.
#[test]
fn a_long_run_of_kept_rows_moves_once_then_splits() {
    let n = lines_per_page();
    let (rows, after) = run(n - 16, &[(true, 12); 5]);
    let head = n - 36;
    assert_eq!(rows, [at(1, &[(1, 12)]), at(1, &[(1, 12)]), at(1, &[(1, 12)]), at(1, &[(1, head), (2, 12 - head)]), at(2, &[(2, 12)])]);
    assert_eq!(after, 2);
}

/// Fixture c: a run longer than a page moves to page 2 whole from its first row; R3 splits
/// there.
#[test]
fn a_run_longer_than_a_page_starts_the_next_page() {
    let n = lines_per_page();
    let (rows, after) = run(n - 16, &[(true, 1), (true, 24), (true, 30), (false, 1)]);
    let head = n - 25;
    assert_eq!(rows, [at(1, &[(1, 1)]), at(1, &[(1, 24)]), at(1, &[(1, head), (2, 30 - head)]), at(2, &[(2, 1)])]);
    assert_eq!(after, 2);
}

/// Fixture e: four kept rows; R3 splits at the foot of page 2 and R4 stays under its tail.
#[test]
fn a_kept_row_after_a_split_rows_tail_stays_when_its_run_fits() {
    let n = lines_per_page();
    let (rows, after) = run(n - 16, &[(true, 1), (true, 24), (true, 24), (true, 24), (false, 1)]);
    let head = n - 25;
    assert_eq!(rows, [at(1, &[(1, 1)]), at(1, &[(1, 24)]), at(1, &[(1, head), (2, 24 - head)]), at(2, &[(2, 24)]), at(2, &[(2, 1)])]);
    assert_eq!(after, 2);
}

/// Fixture f: nine kept rows of 12 lines. After R4 splits, the run R5..R9 does not fit under
/// R4's tail, so page 3 holds only that tail and the run starts page 4.
#[test]
fn the_row_after_a_split_row_starts_a_new_run() {
    let n = lines_per_page();
    let (rows, after) = run(n - 16, &[(true, 12); 9]);
    let head = n - 36;
    let whole = |p| at(p, &[(p, 12)]);
    let split = |p| at(p, &[(p, head), (p + 1, 12 - head)]);
    assert_eq!(rows, [whole(1), whole(1), whole(1), split(1), whole(3), whole(3), whole(3), split(3), whole(4)]);
    assert_eq!(after, 4);
}

/// Fixture g: the run starts at the top of the first page: no keep check, R4 splits there.
#[test]
fn a_run_at_the_top_of_a_page_splits_like_any_table() {
    let n = lines_per_page();
    let (rows, after) = run(0, &[(true, 12); 5]);
    let head = n - 36;
    assert_eq!(rows, [at(0, &[(0, 12)]), at(0, &[(0, 12)]), at(0, &[(0, 12)]), at(0, &[(0, head), (1, 12 - head)]), at(1, &[(1, 12)])]);
    assert_eq!(after, 1);
}

/// Fixture h: a plain row stays on page 1; the run R2..R4 moves to page 2 and R3 splits there.
#[test]
fn a_run_under_a_plain_row_moves_once_then_splits() {
    let n = lines_per_page();
    let (rows, after) = run(n - 16, &[(false, 1), (true, 24), (true, 30), (false, 1)]);
    let head = n - 24;
    assert_eq!(rows, [at(0, &[(0, 1)]), at(1, &[(1, 24)]), at(1, &[(1, head), (2, 30 - head)]), at(2, &[(2, 1)])]);
    assert_eq!(after, 2);
}
