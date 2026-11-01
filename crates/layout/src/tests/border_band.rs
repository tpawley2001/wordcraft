//! A cell's text sits below its top border's drawn width. Word draws a double border as line, gap,
//! line, each `width` thick (measured in Word's PDF of a real form: `double` sz 6 = 0.75 pt +
//! 0.75 pt + 0.75 pt = 2.25 pt), so the text clears 3 x width; a single border clears its width.

use super::lay;
use crate::{Arc, Block, Document, Placed};
use wordcraft_doc::props::{Border, BorderStyle, Borders, LineSpacing};
use wordcraft_doc::{CharProps, Paragraph, Table};

/// A table of `rows` one-line rows, cell margins 0, every edge `edge`; returns the offset of each
/// row's first text line below its cell's top.
fn text_offsets(edge: Border, rows: usize) -> Vec<f32> {
    let mut d = Document::new();
    let mut t = Table::new(rows, 1, 300.0);
    t.props.style = None;
    let e = Some(edge);
    t.props.borders = Some(Borders { top: e, bottom: e, left: e, right: e, between: e, inside_v: e });
    for row in &mut t.rows {
        row.cells[0].props.margins = Some([0.0, 5.4, 0.0, 5.4]);
        let mut p = Paragraph::with_text("Cell", CharProps { font: Some("Arial".into()), size: Some(9.0), ..Default::default() });
        p.props.line_spacing = Some(LineSpacing::Exactly(12.0));
        p.props.space_before = Some(0.0);
        p.props.space_after = Some(0.0);
        row.cells[0].blocks = vec![wordcraft_doc::para_block(p)];
    }
    d.body = vec![Arc::new(Block::Table(t))];
    let l = lay(&d);
    let pg = &l.pages[0];
    let cells: Vec<f32> = pg.items.iter().filter_map(|it| if let Placed::Cell { rect, .. } = it { Some(rect.y) } else { None }).collect();
    let lines: Vec<f32> = pg.items.iter().filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect();
    assert_eq!((cells.len(), lines.len()), (rows, rows), "cells {cells:?} lines {lines:?}");
    cells.iter().zip(&lines).map(|(c, y)| y - c).collect()
}

fn border(style: BorderStyle, width: f32) -> Border {
    Border { style, width, color: None, space: 0.0 }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn single_top_border_clears_its_width() {
    for w in [0.5, 0.75, 2.25] {
        let off = text_offsets(border(BorderStyle::Single, w), 1);
        assert!(close(off[0], w), "single {w}: {off:?}");
    }
}

#[test]
fn double_top_border_clears_line_gap_line() {
    for (w, band) in [(0.5, 1.5), (0.75, 2.25), (1.5, 4.5)] {
        let off = text_offsets(border(BorderStyle::Double, w), 1);
        assert!(close(off[0], band), "double {w}: expected {band}, got {off:?}");
    }
}

#[test]
fn double_inside_edge_clears_line_gap_line_in_later_rows() {
    let off = text_offsets(border(BorderStyle::Double, 0.75), 3);
    for (i, o) in off.iter().enumerate() {
        assert!(close(*o, 2.25), "row {i}: {off:?}");
    }
}

#[test]
fn double_border_rows_are_taller_by_the_gap_and_second_line() {
    // Row pitch: each row's text starts 1.5 pt (two extra line widths) further down than with a
    // single 0.75 pt border.
    let pitch = |style| {
        let mut d = text_offsets(border(style, 0.75), 1);
        d.pop().unwrap_or_default()
    };
    assert!(close(pitch(BorderStyle::Double) - pitch(BorderStyle::Single), 1.5));
}
