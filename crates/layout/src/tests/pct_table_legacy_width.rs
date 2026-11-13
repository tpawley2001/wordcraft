//! A table with a percentage width before compatibility mode 15 takes its share of the text width
//! plus its left and right default cell margins. Synthetic tables only.
use super::*;

/// A Letter page with 72pt margins (text width 468pt) and one two-row table.
fn doc_with(mode: u32, t: Table) -> Document {
    let mut d = Document::from_text("Body");
    d.last_section.page_w = 612.0;
    d.last_section.page_h = 792.0;
    d.last_section.margin_left = 72.0;
    d.last_section.margin_right = 72.0;
    d.settings.compat_mode = mode;
    d.insert_block(StoryRef::Body, &Path::top(0), Block::Table(t)).unwrap();
    d
}

fn pct_table(cols: usize, pct: f32) -> Table {
    let mut t = Table::new(2, cols, 468.0);
    t.props.style = None;
    t.props.width_pct = Some(pct);
    t
}

/// The first row's cell rectangles of the table at body block 0, left to right.
fn first_row(d: &Document) -> Vec<Rect> {
    let layout = lay(d);
    let mut cells: Vec<(usize, Rect)> = layout.pages[0]
        .items
        .iter()
        .filter_map(|p| match p {
            Placed::Cell { rect, row: 0, cell, table, .. } if table.0 == vec![0] => Some((*cell, *rect)),
            _ => None,
        })
        .collect();
    cells.sort_by_key(|c| c.0);
    cells.into_iter().map(|c| c.1).collect()
}

fn width(cells: &[Rect]) -> f32 {
    cells.last().unwrap().right() - cells.first().unwrap().x
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.1
}

#[test]
fn a_full_width_percent_table_below_mode_15_adds_both_cell_margins() {
    let cells = first_row(&doc_with(14, pct_table(2, 100.0)));
    assert!(close(width(&cells), 478.8), "{cells:?}");
    assert!(close(cells[0].x, 72.0 - 5.4), "the left edge stays outdented: {cells:?}");
}

#[test]
fn a_half_width_percent_table_below_mode_15_takes_half_of_the_wider_base() {
    let cells = first_row(&doc_with(14, pct_table(2, 50.0)));
    assert!(close(width(&cells), 239.4), "{cells:?}");
    assert!(close(cells[0].w, 119.7) && close(cells[1].w, 119.7), "{cells:?}");
}

#[test]
fn custom_cell_margins_set_the_wider_base() {
    let mut t = pct_table(2, 100.0);
    t.props.cell_margins = Some([0.0, 10.0, 0.0, 3.0]);
    let cells = first_row(&doc_with(14, t));
    assert!(close(width(&cells), 481.0), "{cells:?}");
    assert!(close(cells[0].x, 62.0), "{cells:?}");
}

#[test]
fn a_table_indent_does_not_shrink_a_percent_table() {
    let plain = first_row(&doc_with(14, pct_table(2, 100.0)));
    let mut t = pct_table(2, 100.0);
    t.props.indent = Some(18.0);
    let cells = first_row(&doc_with(14, t));
    assert!(close(width(&cells), 478.8), "{cells:?}");
    assert!(close(cells[0].x - plain[0].x, 18.0), "{cells:?} vs {plain:?}");
}

#[test]
fn modes_11_and_12_follow_the_same_rule() {
    for mode in [11, 12] {
        let cells = first_row(&doc_with(mode, pct_table(2, 100.0)));
        assert!(close(width(&cells), 478.8), "mode {mode}: {cells:?}");
    }
}

#[test]
fn a_fixed_layout_percent_table_follows_the_same_rule() {
    let mut t = pct_table(2, 100.0);
    t.props.fixed = true;
    let cells = first_row(&doc_with(14, t));
    assert!(close(width(&cells), 478.8), "{cells:?}");
}

#[test]
fn mode_15_keeps_the_text_width() {
    let cells = first_row(&doc_with(15, pct_table(2, 100.0)));
    assert!(close(width(&cells), 468.0), "{cells:?}");
    let mut t = pct_table(2, 100.0);
    t.props.cell_margins = Some([0.0, 10.0, 0.0, 3.0]);
    let cells = first_row(&doc_with(15, t));
    assert!(close(width(&cells), 468.0), "{cells:?}");
}

#[test]
fn cell_percentages_share_the_wider_table() {
    let mut t = pct_table(2, 100.0);
    for row in &mut t.rows {
        row.cells[0].props.width = None;
        row.cells[0].props.width_pct = Some(90.0);
        row.cells[1].props.width = None;
        row.cells[1].props.width_pct = Some(10.0);
    }
    t.grid = vec![421.2, 46.8];
    let cells = first_row(&doc_with(14, t));
    assert!(close(cells[0].w, 430.9) && close(cells[1].w, 47.9), "{cells:?}");
}

#[test]
fn a_nested_percent_table_is_unchanged() {
    let mut outer = Table::new(1, 1, 300.0);
    outer.props.style = None;
    outer.props.width = Some(300.0);
    let mut inner = pct_table(2, 100.0);
    inner.grid = vec![144.6, 144.6];
    outer.rows[0].cells[0].blocks = vec![Arc::new(Block::Table(inner))];
    let layout = lay(&doc_with(14, outer));
    let nested: Vec<Rect> = layout.pages[0]
        .items
        .iter()
        .filter_map(|p| match p {
            Placed::Cell { rect, row: 0, table, .. } if table.0.len() > 1 => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(nested.len(), 2, "{nested:?}");
    let w = nested.iter().map(|r| r.right()).fold(f32::MIN, f32::max) - nested.iter().map(|r| r.x).fold(f32::MAX, f32::min);
    // The outer cell's text width: 300 less its two 5.4pt margins.
    assert!(close(w, 289.2), "{nested:?}");
}
