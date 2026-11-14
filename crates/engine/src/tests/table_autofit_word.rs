//! Table AutoFit (Window / Contents / Fixed Column Width) sets the widths Word sets. Measured 2026-11-14 in Microsoft 365 Word
//! (16.0.20527) via `Table.AutoFitBehavior`, saved as .docx: Window keeps each column's share and writes `tcW` percentages,
//! Contents clears the cells' preferred widths (`tcW` auto), Fixed drops the table's width and pins `tcW` in twips.

use serde_json::{Value, json};
use wordcraft_doc::{Document, Path, StoryRef, Table};

use crate::Session;

/// A session with a 2x3 table (caret in it) and the table's path.
fn with_table() -> (Session, Path) {
    let mut s = Session::new(Document::new());
    s.run("document.setText", &json!({"text": "Before"})).unwrap();
    let r: Value = s.run("insert.table", &json!({"rows": 2, "cols": 3})).unwrap();
    let path: Vec<u32> = serde_json::from_value(r["table"].clone()).unwrap();
    (s, Path(path))
}

fn table(s: &Session, p: &Path) -> Table {
    s.doc.table(StoryRef::Body, p).cloned().expect("the table")
}

fn text_width(s: &Session) -> f32 {
    s.doc.sections().first().map(|(_, sp)| sp.text_width()).unwrap()
}

/// Give the columns these point widths (grid and every cell), as Column Width would.
fn set_widths(s: &mut Session, p: &Path, widths: &[f32]) {
    let t = s.doc.table_mut(StoryRef::Body, p).unwrap();
    t.grid = widths.to_vec();
    for row in &mut t.rows {
        for (c, cell) in row.cells.iter_mut().enumerate() {
            cell.props.width = Some(widths[c]);
            cell.props.width_pct = None;
        }
    }
}

fn autofit(s: &mut Session, mode: &str) {
    s.run("table.autofit", &json!({"mode": mode})).unwrap();
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// Laid-out widths of row 0's cells.
fn laid_out_widths(s: &mut Session) -> Vec<f32> {
    let mut v: Vec<(usize, f32)> = s
        .layout()
        .pages
        .iter()
        .flat_map(|pg| pg.items.iter())
        .filter_map(|it| match it {
            wordcraft_layout::Placed::Cell { rect, row: 0, cell, .. } => Some((*cell, rect.w)),
            _ => None,
        })
        .collect();
    v.sort_by_key(|(c, _)| *c);
    v.into_iter().map(|(_, w)| w).collect()
}

/// Word: columns 72/144/216 pt -> AutoFit Window -> gridCol 1559/3121/4680 twips of 9360 and tcW 833/1667/2500 fiftieths.
#[test]
fn autofit_window_keeps_each_columns_share() {
    let (mut s, p) = with_table();
    set_widths(&mut s, &p, &[72.0, 144.0, 216.0]);
    autofit(&mut s, "window");
    let t = table(&s, &p);
    let tw = text_width(&s);
    assert!(!t.props.fixed);
    assert_eq!(t.props.width, None);
    assert_eq!(t.props.width_pct, Some(100.0));
    let want = [tw / 6.0, tw / 3.0, tw / 2.0];
    for (g, w) in t.grid.iter().zip(want) {
        assert!(close(*g, w, 0.05), "grid {:?} vs {want:?}", t.grid);
    }
    for row in &t.rows {
        let pct: Vec<f32> = row.cells.iter().map(|c| c.props.width_pct.unwrap_or(f32::NAN)).collect();
        for (got, want) in pct.iter().zip([100.0 / 6.0, 100.0 / 3.0, 50.0]) {
            assert!(close(*got, want, 0.02), "cell percentages {pct:?}");
        }
        assert!(row.cells.iter().all(|c| c.props.width.is_none()), "no point widths on the cells");
    }
    let laid = laid_out_widths(&mut s);
    assert_eq!(laid.len(), 3, "{laid:?}");
    for (l, g) in laid.iter().zip(&t.grid) {
        assert!(close(*l, *g, 0.5), "laid out {laid:?} vs grid {:?}", t.grid);
    }
}

/// Word: a new table's equal columns stay equal (gridCol 3120/3121/3119, tcW 1667 pct each).
#[test]
fn autofit_window_keeps_equal_columns_equal() {
    let (mut s, p) = with_table();
    autofit(&mut s, "window");
    let t = table(&s, &p);
    let tw = text_width(&s);
    assert_eq!(t.props.width_pct, Some(100.0));
    assert!(t.grid.iter().all(|g| close(*g, tw / 3.0, 0.05)), "{:?}", t.grid);
    for c in t.rows.iter().flat_map(|r| &r.cells) {
        assert!(c.props.width_pct.is_some_and(|w| close(w, 100.0 / 3.0, 0.02)), "{:?}", c.props.width_pct);
        assert_eq!(c.props.width, None);
    }
}

/// Word: AutoFit Contents writes tblW 0 auto and tcW 0 auto in every cell, whatever the cells had before.
#[test]
fn autofit_contents_clears_cell_preferred_widths() {
    for first in ["none", "window", "fixed"] {
        let (mut s, p) = with_table();
        set_widths(&mut s, &p, &[72.0, 144.0, 216.0]);
        if first != "none" {
            autofit(&mut s, first);
        }
        autofit(&mut s, "contents");
        let t = table(&s, &p);
        assert!(!t.props.fixed, "after {first}");
        assert_eq!((t.props.width, t.props.width_pct), (None, None), "after {first}");
        for c in t.rows.iter().flat_map(|r| &r.cells) {
            assert_eq!((c.props.width, c.props.width_pct), (None, None), "after {first}: cells have no preferred width");
        }
    }
}

/// Word: Fixed Column Width writes tblW 0 auto, tblLayout fixed and tcW = each column's current width in twips; a table
/// preferred width (here the 100 % AutoFit Window leaves) is dropped and the grid is unchanged.
#[test]
fn fixed_column_width_drops_the_table_width_and_pins_cells() {
    let (mut s, p) = with_table();
    set_widths(&mut s, &p, &[72.0, 144.0, 216.0]);
    autofit(&mut s, "window");
    let grid = table(&s, &p).grid;
    autofit(&mut s, "fixed");
    let t = table(&s, &p);
    assert!(t.props.fixed);
    assert_eq!((t.props.width, t.props.width_pct), (None, None));
    assert_eq!(t.grid, grid, "the columns keep their widths");
    for row in &t.rows {
        for (c, cell) in row.cells.iter().enumerate() {
            assert_eq!(cell.props.width, Some(grid[c]), "cell {c} pinned to its column");
            assert_eq!(cell.props.width_pct, None, "cell {c} has no percentage");
        }
    }
}

/// A Column Width after AutoFit Window gives that column a point width, not the old percentage.
#[test]
fn column_width_after_autofit_window_clears_the_cell_percentage() {
    let (mut s, p) = with_table();
    autofit(&mut s, "window");
    s.run("table.columnWidth", &json!({"width": 100.0})).unwrap();
    let t = table(&s, &p);
    assert_eq!(t.props.width_pct, None);
    let c = &t.rows[0].cells[0].props;
    assert_eq!((c.width, c.width_pct), (Some(100.0), None));
    let laid = laid_out_widths(&mut s);
    assert!(laid.first().is_some_and(|w| close(*w, 100.0, 0.5)), "{laid:?}");
}
