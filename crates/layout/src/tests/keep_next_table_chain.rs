//! Keep with next through a table row that is itself kept with the next row: Word moves a
//! paragraph kept with a table together with the table's first row when that row moves on with
//! the row after it, instead of leaving the paragraph alone at the foot of the page. Each case
//! below was measured in Microsoft 365 Word with a small DOCX (Letter page, 12 pt, single
//! spacing): fillers, a one-line HEADING, a three-row table whose second row is too tall for
//! the rest of the page, and a paragraph after it.

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{Pos, Table, para_block};

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

/// Lines in the tall second row.
const TALL: usize = 15;

struct Case {
    /// Lines left on the first page under the fillers (the heading takes the first of them).
    room: usize,
    /// A second kept paragraph before the heading.
    pre: bool,
    heading_keeps: bool,
    row_one_keeps: bool,
    row_two_keeps: bool,
    cant_split: bool,
    header: bool,
}

const CHAIN: Case = Case { room: 8, pre: false, heading_keeps: true, row_one_keeps: true, row_two_keeps: false, cant_split: true, header: false };

/// Where things landed (zero-based pages).
#[derive(Debug, PartialEq)]
struct Pages {
    last_filler: usize,
    pre: Option<usize>,
    heading: usize,
    /// The first page each of the three rows has lines on.
    rows: [usize; 3],
    /// The last page the tall second row has lines on.
    row_two_end: usize,
    after: usize,
}

fn run(c: &Case) -> Pages {
    let fillers = lines_per_page() - c.room - usize::from(c.pre);
    let mut d = Document::new();
    d.body = (0..fillers).map(|i| para(&format!("F{i}"), &plain())).collect();
    if c.pre {
        d.body.push(para("PREHEAD", &kept(true)));
    }
    let heading = d.body.len();
    d.body.push(para("HEADING", &kept(c.heading_keeps)));
    let mut t = Table::new(3, 2, 440.0);
    // Row 1: one cell over both columns, as a caption row.
    t.rows[0].cells.truncate(1);
    t.rows[0].cells[0].props.span = 2;
    t.rows[0].cells[0].blocks = vec![para("ROWONE", &kept(c.row_one_keeps))];
    t.rows[0].props.header = c.header;
    t.rows[1].cells[0].blocks = vec![para("ROWTWO", &kept(c.row_two_keeps))];
    t.rows[1].cells[1].blocks = (0..TALL).map(|i| para(&format!("tall{i}"), &kept(c.row_two_keeps))).collect();
    t.rows[2].cells[0].blocks = vec![para("ROWTHREE", &plain())];
    t.rows[2].cells[1].blocks = vec![para("end", &plain())];
    for row in &mut t.rows {
        row.props.cant_split = c.cant_split;
    }
    let table = d.body.len() as u32;
    d.body.push(Arc::new(Block::Table(t)));
    let after = d.body.len();
    d.body.push(para("AFTER", &plain()));
    let l = lay(&d);
    let page_of = |block: usize| l.caret(&Pos::body(block, 0)).unwrap().page;
    let row_pages = |row: u32| -> Vec<usize> {
        l.pages
            .iter()
            .enumerate()
            .filter(|(_, pg)| {
                pg.items.iter().any(|it| matches!(it, Placed::Lines { path, .. } if path.0.len() == 4 && path.0[0] == table && path.0[1] == row))
            })
            .map(|(i, _)| i)
            .collect()
    };
    let first = |row: u32| *row_pages(row).first().expect("the row has lines");
    Pages {
        last_filler: page_of(heading - 1 - usize::from(c.pre)),
        pre: c.pre.then(|| page_of(heading - 1)),
        heading: page_of(heading),
        rows: [first(0), first(1), first(2)],
        row_two_end: *row_pages(1).last().expect("the row has lines"),
        after: page_of(after),
    }
}

fn moved(pre: Option<usize>) -> Pages {
    Pages { last_filler: 0, pre, heading: 1, rows: [1, 1, 1], row_two_end: 1, after: 1 }
}

/// Probe a: HEADING keeps, ROWONE keeps, rows can't split. Word: page 1 ends with the last
/// filler; the heading and all three rows are on page 2.
#[test]
fn a_heading_moves_with_a_first_row_kept_with_the_tall_row_after_it() {
    assert_eq!(run(&CHAIN), moved(None));
}

/// Probe c: the same with row 1 a repeating header row.
#[test]
fn a_heading_moves_with_a_kept_header_row() {
    assert_eq!(run(&Case { header: true, ..CHAIN }), moved(None));
}

/// Probe e: two kept paragraphs before the table move together with the rows.
#[test]
fn a_chain_of_kept_paragraphs_moves_with_the_kept_rows() {
    assert_eq!(run(&Case { pre: true, ..CHAIN }), moved(Some(1)));
}

/// Probe h: row 2's paragraphs keep with the next too (row 3 follows on the same page).
#[test]
fn a_heading_moves_when_the_second_row_keeps_too() {
    assert_eq!(run(&Case { row_two_keeps: true, ..CHAIN }), moved(None));
}

/// Probe b (control): ROWONE does not keep. The heading and row 1 stay on page 1; row 2 starts
/// page 2.
#[test]
fn a_heading_stays_with_a_first_row_that_is_not_kept() {
    let p = run(&Case { row_one_keeps: false, ..CHAIN });
    assert_eq!((p.heading, p.rows), (0, [0, 1, 1]), "{p:?}");
}

/// Probe d (control): only ROWONE keeps. The heading stays on page 1; rows 1 and 2 are on
/// page 2.
#[test]
fn a_paragraph_that_does_not_keep_stays_when_the_rows_move() {
    let p = run(&Case { heading_keeps: false, ..CHAIN });
    assert_eq!((p.heading, p.rows), (0, [1, 1, 1]), "{p:?}");
}

/// Probe f (control): rows may split. The heading and row 1 stay on page 1 and row 2 splits
/// across the page end.
#[test]
fn a_heading_stays_when_the_tall_row_can_split() {
    let p = run(&Case { cant_split: false, ..CHAIN });
    assert_eq!((p.heading, p.rows[0], p.rows[1], p.row_two_end), (0, 0, 0, 1), "{p:?}");
}

/// Probe g (control): everything fits on page 1.
#[test]
fn nothing_moves_when_the_table_fits() {
    let p = run(&Case { room: TALL + 12, ..CHAIN });
    assert_eq!((p.heading, p.rows, p.after), (0, [0, 0, 0], 0), "{p:?}");
}
