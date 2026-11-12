//! Keep with next into a table whose first row keeps with the next row: the limits of moving the
//! paragraph with the rows (see `tests/keep_next_table_chain.rs` for the measured cases).

#[allow(unused_imports)]
use crate::*;
use wordcraft_doc::props::{LineSpacing, ParaProps};
use wordcraft_doc::{Pos, Table, para_block};

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

/// A two-column table: row 1 kept with the next, then rows of `tall` lines each, can't split.
fn table(rows: &[usize], all_kept: bool) -> Table {
    let mut t = Table::new(rows.len() + 1, 2, 440.0);
    t.rows[0].cells[0].blocks = vec![para("ROWONE", true)];
    for (r, tall) in rows.iter().enumerate() {
        let row = &mut t.rows[r + 1];
        row.cells[0].blocks = vec![para("ROW", all_kept)];
        row.cells[1].blocks = (0..*tall).map(|i| para(&format!("tall{i}"), all_kept)).collect();
    }
    for row in &mut t.rows {
        row.props.cant_split = true;
    }
    t
}

fn heading_page(t: Table) -> (usize, usize) {
    let fillers = lines_per_page() - 8;
    let mut d = Document::new();
    d.body = (0..fillers).map(|i| para(&format!("F{i}"), false)).collect();
    let heading = d.body.len();
    d.body.push(para("HEADING", true));
    d.body.push(Arc::new(Block::Table(t)));
    d.body.push(para("AFTER", false));
    let l = lay(&d);
    (l.caret(&Pos::body(heading, 0)).unwrap().page, l.pages.len())
}

/// The kept rows are taller than a page: moving the heading gains nothing, so it stays (as
/// before) and the layout ends.
#[test]
fn a_heading_stays_when_the_kept_rows_are_taller_than_a_page() {
    let page = lines_per_page();
    let (heading, pages) = heading_page(table(&[page + 5], false));
    assert_eq!(heading, 0);
    assert!(pages < 6, "{pages} pages");
}

/// A chain of kept rows longer than a page: the table flow does not move them together, and the
/// heading stays.
#[test]
fn a_heading_stays_when_a_chain_of_kept_rows_is_taller_than_a_page() {
    let page = lines_per_page();
    let (heading, pages) = heading_page(table(&[page / 2, page / 2, page / 2, 3], true));
    assert_eq!(heading, 0);
    assert!(pages < 8, "{pages} pages");
}

/// Where each line of the document lands, for comparing layouts.
fn spots(l: &DocLayout) -> Vec<(usize, Vec<u32>, i32)> {
    let mut out = Vec::new();
    for (i, pg) in l.pages.iter().enumerate() {
        for it in &pg.items {
            if let Placed::Lines { path, y, .. } = it {
                out.push((i, path.0.clone(), (*y * 10.0).round() as i32));
            }
        }
    }
    out
}

/// A paragraph kept with a nested table inside a table cell is laid out as before: the rule is
/// for body paragraphs.
#[test]
fn a_kept_paragraph_before_a_table_in_a_cell_is_unchanged() {
    let page = lines_per_page();
    let build = |keep: bool| {
        let mut d = Document::new();
        d.body = (0..page - 8).map(|i| para(&format!("F{i}"), false)).collect();
        let mut outer = Table::new(1, 1, 460.0);
        outer.rows[0].cells[0].blocks = vec![para("INNER HEADING", keep), Arc::new(Block::Table(table(&[15], false)))];
        d.body.push(Arc::new(Block::Table(outer)));
        d.body.push(para("AFTER", false));
        spots(&lay(&d))
    };
    assert_eq!(build(true), build(false));
}
