//! A row whose exceptions (`w:tblPrEx`) set every table border to "no border" draws no grid and
//! takes no border width, even when the table itself has borders (a form whose first row is
//! boxed and whose later rows are borderless, as a `.doc` stores it row by row).

use super::lay;
use crate::{Arc, Block, Document, Placed};
use wordcraft_doc::props::{Border, BorderStyle, Borders, TableProps};
use wordcraft_doc::{CharProps, Paragraph, Table};

const S: Border = Border { style: BorderStyle::Single, width: 0.5, color: None, space: 0.0 };

fn table(rows: usize, borders: Option<Borders>) -> Table {
    let mut t = Table::new(rows, 2, 300.0);
    t.props.style = None;
    t.props.borders = borders;
    for (i, row) in t.rows.iter_mut().enumerate() {
        for c in &mut row.cells {
            let p = Paragraph::with_text(&format!("Row {i}"), CharProps { font: Some("Arial".into()), size: Some(10.0), ..Default::default() });
            c.blocks = vec![wordcraft_doc::para_block(p)];
        }
    }
    t
}

/// Each row's height (from its first cell) and every rule on the page.
fn measure(d: &Document) -> (Vec<f32>, Vec<(f32, f32)>) {
    let l = lay(d);
    let pg = &l.pages[0];
    let mut cells: Vec<(f32, f32)> =
        pg.items.iter().filter_map(|it| if let Placed::Cell { rect, .. } = it { Some((rect.y, rect.h)) } else { None }).collect();
    cells.dedup_by(|a, b| (a.0 - b.0).abs() < 0.01);
    let rules = pg.items.iter().filter_map(|it| if let Placed::Rule { y0, y1, .. } = it { Some((y0.min(*y1), y0.max(*y1))) } else { None }).collect();
    (cells.iter().map(|c| c.1).collect(), rules)
}

#[test]
fn borderless_row_exceptions_add_no_height_and_draw_nothing() {
    let none = Borders::all(Border::default());
    let mut t = table(14, Some(Borders::all(S)));
    for row in t.rows.iter_mut().skip(1) {
        row.props.exceptions = Some(Box::new(TableProps { borders: Some(none), ..Default::default() }));
    }
    let mut d = Document::new();
    d.body = vec![Arc::new(Block::Table(t))];
    let (heights, rules) = measure(&d);
    assert_eq!(heights.len(), 14, "{heights:?}");

    // The same rows as a second table without borders under a one-row bordered table.
    let mut d2 = Document::new();
    d2.body = vec![Arc::new(Block::Table(table(1, Some(Borders::all(S))))), Arc::new(Block::Table(table(13, None)))];
    let (heights2, _) = measure(&d2);
    assert_eq!(heights2.len(), 14, "{heights2:?}");
    for r in 2..14 {
        assert!((heights[r] - heights2[r]).abs() < 0.01, "row {r}: {heights:?} vs {heights2:?}");
    }
    // The line row 1 draws on its own bottom edge is cleared by the row below it (as every inside
    // edge is), not by row 1: the two rows together are as tall as in the split tables.
    let (a, b) = (heights[0] + heights[1], heights2[0] + heights2[1]);
    assert!((a - b).abs() < 0.01, "rows 1-2: {heights:?} vs {heights2:?}");
    assert!((heights[1] - heights2[1] - S.width).abs() < 0.01, "only row 1's bottom line: {heights:?} vs {heights2:?}");

    // Nothing is drawn below row 1's bottom edge.
    let l = lay(&d);
    let row1 = l.pages[0].items.iter().find_map(|it| if let Placed::Cell { rect, .. } = it { Some(rect.y + rect.h) } else { None }).unwrap();
    assert!(!rules.is_empty());
    for (y0, y1) in &rules {
        assert!(*y1 <= row1 + 0.6, "a rule at {y0}..{y1} below row 1 ({row1})");
    }
}
