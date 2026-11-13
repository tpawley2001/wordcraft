//! Where two table rows meet, the cell above has a bottom border and the cell below a top border;
//! the two make one shared edge as thick as the wider of them, and the lower row's text starts
//! below all of it (measured in Word's PDF: a 2.25 pt rule under a row moves the next row's text
//! down by 2.25 pt, not by the 0.5 pt of the lower row's own top line).

use super::lay;
use crate::{Arc, Block, Document, Placed};
use wordcraft_doc::props::{Border, BorderStyle, Borders, HeightRule, LineSpacing, TableProps, VMerge};
use wordcraft_doc::{CharProps, Paragraph, Table};

const THIN: Border = Border { style: BorderStyle::Single, width: 0.5, color: None, space: 0.0 };

fn single(width: f32) -> Border {
    Border { style: BorderStyle::Single, width, color: None, space: 0.0 }
}

/// `w:val="nil"` on a cell edge: set, and draws nothing.
const NIL: Border = Border { style: BorderStyle::None, width: 0.0, color: None, space: 0.0 };

/// A `rows` x `cols` table, cell margins top/bottom 0, one Arial 10 pt single-spaced line per cell,
/// every table edge `edges` (or no borders at all).
fn table(rows: usize, cols: usize, edges: Option<Border>) -> Table {
    let mut t = Table::new(rows, cols, 300.0);
    t.props.style = None;
    t.props.borders = edges.map(Borders::all);
    for (i, row) in t.rows.iter_mut().enumerate() {
        for c in &mut row.cells {
            c.props.margins = Some([0.0, 5.4, 0.0, 5.4]);
            let name = format!("ROW{}", char::from(b'A' + i as u8));
            let mut p = Paragraph::with_text(&name, CharProps { font: Some("Arial".into()), size: Some(10.0), ..Default::default() });
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            c.blocks = vec![wordcraft_doc::para_block(p)];
        }
    }
    t
}

/// The first text line of every cell as (x, y), in page order.
fn lines(t: Table) -> Vec<(f32, f32)> {
    let mut d = Document::new();
    d.body = vec![Arc::new(Block::Table(t))];
    let l = lay(&d);
    l.pages[0].items.iter().filter_map(|it| if let Placed::Lines { x, y, .. } = it { Some((*x, *y)) } else { None }).collect()
}

/// The y of each row's first line in a one-column table.
fn ys(t: Table) -> Vec<f32> {
    lines(t).into_iter().map(|(_, y)| y).collect()
}

/// The distance between the first lines of two adjacent rows when no border is visible.
fn pitch() -> f32 {
    let y = ys(table(2, 1, None));
    y[1] - y[0]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

/// Rows B and D get `bottom` on their cells, rows C and E get `top`; returns the five row tops.
fn five_rows(bottom: Option<Border>, top: Option<Border>) -> Vec<f32> {
    let mut t = table(5, 1, Some(THIN));
    for r in [1, 3] {
        t.rows[r].cells[0].props.borders = bottom.map(|b| Borders { bottom: Some(b), ..Default::default() });
    }
    for r in [2, 4] {
        t.rows[r].cells[0].props.borders = top.map(|b| Borders { top: Some(b), ..Default::default() });
    }
    let y = ys(t);
    assert_eq!(y.len(), 5, "{y:?}");
    y
}

fn check(bottom: Option<Border>, top: Option<Border>, wider: f32) {
    let line = pitch();
    let y = five_rows(bottom, top);
    for (a, b, want) in [(0, 1, 0.5), (1, 2, wider), (2, 3, 0.5), (3, 4, wider)] {
        let got = y[b] - y[a];
        assert!(close(got, line + want), "rows {a}-{b}: {got} != {line} + {want} ({y:?})");
    }
}

#[test]
fn thin_edges_both_sides() {
    check(None, None, 0.5);
}

#[test]
fn thick_top_on_lower_row() {
    check(None, Some(single(2.25)), 2.25);
}

#[test]
fn very_thick_top_on_lower_row() {
    check(None, Some(single(6.0)), 6.0);
}

#[test]
fn thick_bottom_on_upper_row() {
    check(Some(single(2.25)), None, 2.25);
}

#[test]
fn very_thick_bottom_on_upper_row() {
    check(Some(single(6.0)), None, 6.0);
}

#[test]
fn thick_bottom_over_thin_cell_top() {
    check(Some(single(2.25)), Some(single(0.5)), 2.25);
}

#[test]
fn thin_cell_bottom_over_very_thick_top() {
    check(Some(single(0.5)), Some(single(6.0)), 6.0);
}

#[test]
fn very_thick_bottom_over_thick_top() {
    check(Some(single(6.0)), Some(single(2.25)), 6.0);
}

#[test]
fn thick_bottom_over_thick_top() {
    check(Some(single(2.25)), Some(single(2.25)), 2.25);
}

#[test]
fn thick_bottom_over_nil_top() {
    check(Some(single(2.25)), Some(NIL), 2.25);
}

#[test]
fn wide_cell_above_pushes_both_cells_below() {
    let line = pitch();
    let mut t = table(2, 2, Some(THIN));
    let first = &mut t.rows[0];
    first.cells.truncate(1);
    first.cells[0].props.span = 2;
    first.cells[0].props.width = Some(300.0);
    first.cells[0].props.borders = Some(Borders { bottom: Some(single(6.0)), ..Default::default() });
    let l = lines(t);
    assert_eq!(l.len(), 3, "{l:?}");
    for (_, y) in &l[1..] {
        assert!(close(y - l[0].1, line + 6.0), "{l:?}");
    }
}

#[test]
fn thick_bottom_on_one_cell_moves_only_the_cell_below_it() {
    let line = pitch();
    let mut t = table(2, 2, Some(THIN));
    t.rows[0].cells[0].props.borders = Some(Borders { bottom: Some(single(6.0)), ..Default::default() });
    let mut l = lines(t);
    assert_eq!(l.len(), 4, "{l:?}");
    l.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    let top = l[0].1;
    let (left, right) = if l[2].0 < l[3].0 { (l[2], l[3]) } else { (l[3], l[2]) };
    assert!(close(left.1 - top, line + 6.0), "{l:?}");
    // The right cell keeps the start its own thin top border gives it.
    assert!(close(right.1 - top, line + 0.5), "{l:?}");
}

#[test]
fn vertical_merge_continuation_is_unchanged() {
    let line = pitch();
    let mut t = table(3, 2, Some(THIN));
    t.rows[0].cells[0].props.borders = Some(Borders { bottom: Some(single(6.0)), ..Default::default() });
    t.rows[0].cells[0].props.vmerge = VMerge::Restart;
    t.rows[1].cells[0].props.vmerge = VMerge::Continue;
    t.rows[1].cells[0].blocks = vec![wordcraft_doc::para_block(Paragraph::default())];
    let mut l = lines(t);
    l.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    let top = l[0].1;
    let rel: Vec<f32> = l.iter().map(|(_, y)| y - top).collect();
    // Recorded from the layout before shared row edges took the wider border (each row one line
    // plus the thin edge above it: the continuation cell keeps its own band).
    let want = [0.0, 0.0, line + 0.5, 2.0 * (line + 0.5), 2.0 * (line + 0.5)];
    assert_eq!(rel.len(), want.len(), "{l:?}");
    for (g, w) in rel.iter().zip(want) {
        assert!(close(*g, w), "{rel:?} vs {want:?}");
    }
}

#[test]
fn row_exception_outer_edges_do_not_apply_between_rows() {
    let plain = ys(table(5, 1, Some(THIN)));
    for w in [2.25, 6.0] {
        let mut t = table(5, 1, Some(THIN));
        t.rows[2].props.exceptions =
            Some(Box::new(TableProps { borders: Some(Borders { bottom: Some(single(w)), ..Borders::all(THIN) }), ..Default::default() }));
        let y = ys(t);
        for (a, b) in y.iter().zip(&plain) {
            assert!(close(a - y[0], b - plain[0]), "{w}: {y:?} vs {plain:?}");
        }
    }
}

#[test]
fn exact_row_height_wins_over_the_shared_edge() {
    let mut t = table(3, 1, Some(THIN));
    t.rows[0].cells[0].props.borders = Some(Borders { bottom: Some(single(6.0)), ..Default::default() });
    for row in &mut t.rows {
        row.props.height = Some(20.0);
        row.props.height_rule = HeightRule::Exact;
    }
    let mut d = Document::new();
    d.body = vec![Arc::new(Block::Table(t))];
    let l = lay(&d);
    let mut cells: Vec<(f32, f32)> =
        l.pages[0].items.iter().filter_map(|it| if let Placed::Cell { rect, .. } = it { Some((rect.y, rect.h)) } else { None }).collect();
    cells.dedup_by(|a, b| (a.0 - b.0).abs() < 0.01);
    assert_eq!(cells.len(), 3, "{cells:?}");
    for (i, (y, h)) in cells.iter().enumerate() {
        assert!(close(*h, 20.0), "{cells:?}");
        assert!(close(*y - cells[0].0, 20.0 * i as f32), "{cells:?}");
    }
}
