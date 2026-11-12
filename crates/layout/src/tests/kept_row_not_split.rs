//! A table row kept with the next (the first paragraph of its first cell has keep with next) is
//! not split across pages: Word moves it whole to the next page while anything is above it on
//! the page, and splits it from the top of a page only when it is taller than the page. Each
//! case below was measured in Microsoft 365 Word with a small DOCX (Letter page, 12 pt, single
//! spacing, 46 lines to a page): one-line fillers, a one-line paragraph CAP, a table whose tall
//! row does not fit in what is left of the page, and a paragraph AFTER.

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

/// Which paragraphs of the tall row have keep with next.
#[derive(Clone, Copy, PartialEq)]
enum Keep {
    None,
    /// ROWTWO (first cell) and every paragraph of the second cell.
    All,
    /// Only ROWTWO, the first paragraph of the first cell.
    FirstCellFirst,
    /// Only the second cell's paragraphs.
    SecondCellOnly,
    /// Only a second paragraph of the first cell.
    FirstCellSecond,
}

#[derive(Clone, Copy)]
struct Case {
    /// One-line body paragraphs before the table; the last of them is CAP.
    above: usize,
    cap_keeps: bool,
    /// The one-line row before the tall row: None, or Some((kept, repeating header row)).
    row_one: Option<(bool, bool)>,
    keep: Keep,
    /// One-line paragraphs in the tall row's second cell.
    tall: usize,
    /// A one-line row after the tall row.
    row_three: bool,
}

/// Lines the tall row has in the standard cases: more than the 14 left under row 1.
const TALL: usize = 24;

/// 30 lines above the table of a 46-line page, as in the probes: 16 lines are left, row 1 takes
/// one, so the tall row would have about 14.
fn standard(keep: Keep) -> Case {
    Case { above: lines_per_page() - 16, cap_keeps: false, row_one: Some((false, false)), keep, tall: TALL, row_three: true }
}

/// Where things landed (zero-based pages).
#[derive(Debug, PartialEq)]
struct Got {
    cap: Option<usize>,
    /// The first page row 1 has lines on.
    row_one: Option<usize>,
    /// The page of ROWTWO, the tall row's first cell.
    row_two: usize,
    /// For each page with paragraphs of the tall cell: (page, how many of them).
    tall: Vec<(usize, usize)>,
    row_three: Option<usize>,
    after: usize,
}

fn run(c: &Case) -> Got {
    let mut d = Document::new();
    d.body = (0..c.above.saturating_sub(1)).map(|i| para(&format!("F{i}"), &plain())).collect();
    let cap = (c.above > 0).then(|| {
        d.body.push(para("CAP", &kept(c.cap_keeps)));
        d.body.len() - 1
    });
    let mut t = Table::new(1, 2, 440.0);
    let template: Row = t.rows[0].clone();
    t.rows.clear();
    let mut row = |a: Vec<Arc<Block>>, b: Vec<Arc<Block>>, header: bool| {
        let mut r = template.clone();
        r.cells[0].blocks = a;
        r.cells[1].blocks = b;
        r.props.header = header;
        t.rows.push(r);
    };
    if let Some((keeps, header)) = c.row_one {
        row(vec![para("ROWONE", &kept(keeps))], vec![para("h2", &kept(keeps))], header);
    }
    let first_keeps = matches!(c.keep, Keep::All | Keep::FirstCellFirst);
    let mut first_cell = vec![para("ROWTWO", &kept(first_keeps))];
    if c.keep == Keep::FirstCellSecond {
        first_cell.push(para("second", &kept(true)));
    }
    let tall_keeps = matches!(c.keep, Keep::All | Keep::SecondCellOnly);
    row(first_cell, (0..c.tall).map(|i| para(&format!("tall{i}"), &kept(tall_keeps))).collect(), false);
    if c.row_three {
        row(vec![para("ROWTHREE", &plain())], vec![para("end", &plain())], false);
    }
    let table = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(t)));
    let after = d.body.len();
    d.body.push(para("AFTER", &plain()));
    let l = lay(&d);
    let page_of = |block: usize| l.caret(&Pos::body(block, 0)).unwrap().page;
    // Pages with lines of the cell `cell` of row `row`, with the number of its paragraphs there.
    let cell_pages = |row: u32, cell: u32| -> Vec<(usize, usize)> {
        l.pages
            .iter()
            .enumerate()
            .map(|(i, pg)| {
                let n = pg
                    .items
                    .iter()
                    .filter(|it| {
                        matches!(it, Placed::Lines { path, .. }
                            if path.0.len() == 4 && path.0[0] == table && path.0[1] == row && path.0[2] == cell)
                    })
                    .count();
                (i, n)
            })
            .filter(|(_, n)| *n > 0)
            .collect()
    };
    let first = |row: u32| cell_pages(row, 0).first().expect("the row has lines").0;
    let tall_row = u32::from(c.row_one.is_some());
    Got {
        cap: cap.map(page_of),
        row_one: c.row_one.map(|_| first(0)),
        row_two: first(tall_row),
        tall: cell_pages(tall_row, 1),
        row_three: c.row_three.then(|| first(tall_row + 1)),
        after: page_of(after),
    }
}

/// The tall row split at the foot of the first page (rows that are not kept, as today).
fn assert_splits(g: &Got) {
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(0), 0), "{g:?}");
    assert_eq!(g.tall.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [0, 1], "{g:?}");
    assert!(g.tall[0].1 >= 10 && g.tall[0].1 + g.tall[1].1 == TALL, "{g:?}");
}

/// The tall row whole on the second page, row 1 left on the first.
fn assert_moves_whole(g: &Got) {
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(0), 1), "{g:?}");
    assert_eq!(g.tall, [(1, TALL)], "{g:?}");
    assert_eq!((g.row_three, g.after), (Some(1), 1), "{g:?}");
}

/// Probe t1 (control): nothing is kept. The tall row splits at the foot of page 1.
#[test]
fn a_row_that_is_not_kept_splits() {
    assert_splits(&run(&standard(Keep::None)));
}

/// Probes t2/t3/t5 (compatibility modes 11, 12 and 15): every paragraph of the tall row is kept
/// with the next. Word: page 1 ends with ROWONE; the tall row is whole on page 2.
#[test]
fn a_row_whose_paragraphs_are_all_kept_moves_whole() {
    assert_moves_whole(&run(&standard(Keep::All)));
}

/// Probe t6: only the first paragraph of the first cell is kept. Word: the same as t2.
#[test]
fn the_first_paragraph_of_the_first_cell_is_enough() {
    assert_moves_whole(&run(&standard(Keep::FirstCellFirst)));
}

/// Probe t10 (control): only the second cell's paragraphs are kept. Word splits the row.
#[test]
fn kept_paragraphs_in_another_cell_do_not_keep_the_row_whole() {
    assert_splits(&run(&standard(Keep::SecondCellOnly)));
}

/// Probe t11 (control): only a second paragraph of the first cell is kept. Word splits the row.
#[test]
fn a_later_kept_paragraph_of_the_first_cell_does_not_keep_the_row_whole() {
    assert_splits(&run(&standard(Keep::FirstCellSecond)));
}

/// Probe t9: the kept tall row is the table's last row. Word still moves it whole.
#[test]
fn a_kept_last_row_moves_whole() {
    let g = run(&Case { row_three: false, ..standard(Keep::All) });
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(0), 1), "{g:?}");
    assert_eq!((g.tall.as_slice(), g.after), ([(1, TALL)].as_slice(), 1), "{g:?}");
}

/// Probe u1: row 1 is kept too and the kept tall row is the last row. Word: page 1 ends with
/// CAP (not kept); row 1 follows the tall row to page 2.
#[test]
fn a_kept_row_follows_a_kept_last_row_that_moves() {
    let g = run(&Case { row_one: Some((true, false)), row_three: false, ..standard(Keep::All) });
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(1), 1), "{g:?}");
    assert_eq!((g.tall.as_slice(), g.after), ([(1, TALL)].as_slice(), 1), "{g:?}");
}

/// Probe u11 (control): only row 1 is kept; the tall row is not. Row 1 stays and the tall row
/// splits under it.
#[test]
fn a_kept_row_stays_when_the_plain_row_after_it_can_split() {
    assert_splits(&run(&Case { row_one: Some((true, false)), ..standard(Keep::None) }));
}

/// Probe t8 (modes 12 and 15): the kept row is taller than a page. Word: page 1 ends with
/// ROWONE; the row starts at the top of page 2, fills it, and ends on page 3 with row 3.
#[test]
fn a_kept_row_taller_than_a_page_starts_the_next_page_and_splits_there() {
    let per_page = lines_per_page();
    let g = run(&Case { tall: per_page + 14, ..standard(Keep::All) });
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(0), 1), "{g:?}");
    assert_eq!(g.tall.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [1, 2], "{g:?}");
    assert!(g.tall[0].1 + 2 >= per_page && g.tall[0].1 <= per_page, "the row fills page 2: {g:?}");
    assert_eq!((g.row_three, g.after), (Some(2), 2), "{g:?}");
}

/// Probe u10: one line of text above a table whose first row is kept and taller than a page.
/// Word leaves the line alone on page 1 and starts the row on page 2.
#[test]
fn a_kept_first_row_taller_than_a_page_leaves_the_text_above_it() {
    let per_page = lines_per_page();
    let g = run(&Case { above: 1, cap_keeps: false, row_one: None, keep: Keep::All, tall: per_page + 4, row_three: true });
    assert_eq!((g.cap, g.row_two), (Some(0), 1), "{g:?}");
    assert_eq!(g.tall.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [1, 2], "{g:?}");
}

/// Probe u8: nothing above the table; a plain one-line row, then a kept row taller than a
/// page. Word leaves row 1 alone on page 1.
#[test]
fn a_kept_row_taller_than_a_page_leaves_the_row_above_it() {
    let per_page = lines_per_page();
    let g = run(&Case { above: 0, cap_keeps: false, row_one: Some((false, false)), keep: Keep::All, tall: per_page + 4, row_three: true });
    assert_eq!((g.row_one, g.row_two), (Some(0), 1), "{g:?}");
    assert_eq!(g.tall.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [1, 2], "{g:?}");
}

/// Probe u9 (control): the kept row taller than a page is the first thing on the page. It
/// splits in place (moving on would gain nothing).
#[test]
fn a_kept_row_taller_than_a_page_at_the_top_of_a_page_splits_in_place() {
    let per_page = lines_per_page();
    let g = run(&Case { above: 0, cap_keeps: false, row_one: None, keep: Keep::All, tall: per_page + 4, row_three: true });
    assert_eq!(g.row_two, 0, "{g:?}");
    assert_eq!(g.tall.iter().map(|(p, _)| *p).collect::<Vec<_>>(), [0, 1], "{g:?}");
    assert!(g.tall[0].1 + 2 >= per_page, "{g:?}");
}

/// Probe t12 (compatibility mode 15, the default here): row 1 is a repeating header row, not
/// kept; the tall row is kept. Word: page 1 ends with CAP; the header row goes to page 2 with
/// the tall row.
#[test]
fn a_header_row_goes_with_the_kept_row_under_it() {
    let g = run(&Case { row_one: Some((false, true)), ..standard(Keep::All) });
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(1), 1), "{g:?}");
    assert_eq!(g.tall, [(1, TALL)], "{g:?}");
    assert_eq!((g.row_three, g.after), (Some(1), 1), "{g:?}");
}

/// Probe t13 (modes 12 and 15; the shape of the real document): CAP is kept, row 1 is a kept
/// repeating header row and every paragraph of the tall row is kept. Word puts CAP, the header
/// row and the tall row on page 2. CAP's page is not asserted here (where a paragraph kept with
/// a table goes is another rule's subject).
#[test]
fn a_kept_header_row_and_the_kept_row_under_it_move_together() {
    let g = run(&Case { cap_keeps: true, row_one: Some((true, true)), ..standard(Keep::All) });
    assert_eq!((g.row_one, g.row_two), (Some(1), 1), "{g:?}");
    assert_eq!(g.tall, [(1, TALL)], "{g:?}");
    assert_eq!((g.row_three, g.after), (Some(1), 1), "{g:?}");
}

/// Control: a kept row that fits in the room left stays where it is.
#[test]
fn a_kept_row_that_fits_stays() {
    let g = run(&Case { tall: 6, ..standard(Keep::All) });
    assert_eq!((g.cap, g.row_one, g.row_two), (Some(0), Some(0), 0), "{g:?}");
    assert_eq!(g.tall, [(0, 6)], "{g:?}");
}
