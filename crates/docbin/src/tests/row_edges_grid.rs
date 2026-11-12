//! `.doc` table rows with their own cell edges (`sprmTDefTable`): each cell is as wide as its own
//! edges make it, and rows of different cell counts share one grid whose columns their edges
//! cover, so a header cell over two or three columns spans them instead of being squeezed into one.

use std::time::{Duration, Instant};

use wordcraft_doc::Blocks;
use wordcraft_doc::para::Paragraph;
use wordcraft_doc::props::{CharProps, VMerge};
use wordcraft_doc::table::Table;

use crate::table::{self, ParaOut};

const TTP: &[u8] = &[0x17, 0x24, 1];

const SIX: &[i16] = &[0, 2430, 4140, 6930, 7740, 8550, 9360];
const NINE: &[i16] = &[0, 2430, 4140, 5535, 6930, 7740, 8010, 8280, 8550, 9360];

/// A depth-1 cell mark with extra PAPX sprms.
fn mark(text: &str, extra: &[u8]) -> ParaOut {
    let mut papx = vec![0x16, 0x24, 1]; // sprmPFInTable
    papx.extend([0x49, 0x66]); // sprmPItap
    papx.extend(1i32.to_le_bytes());
    papx.extend(extra);
    let mut row = table::decode(&papx);
    row.cell_mark = true;
    ParaOut { para: Paragraph::with_text(text, CharProps::default()), end_cp: 0, row: Some(Box::new(row)) }
}

/// `sprmTVertMerge` for cell `itc`: 3 restarts a merge, 1 continues the one above.
fn vert_merge(itc: u8, flag: u8) -> Vec<u8> {
    super::grpprl(&[(0xD62B, &[2, itc, flag])])
}

/// The marks of one row whose cells end at `edges` (twips), the row end carrying `extra` too.
fn row(edges: &[i16], extra: &[u8]) -> Vec<ParaOut> {
    let cells = edges.len().saturating_sub(1);
    let mut end = TTP.to_vec();
    end.extend(super::t_def_table(edges));
    end.extend(extra);
    let mut marks: Vec<ParaOut> = (0..cells).map(|i| mark(&format!("c{i}"), &[])).collect();
    marks.push(mark("", &end));
    marks
}

fn table(blocks: &Blocks) -> &Table {
    let tables: Vec<&Table> = blocks.iter().filter_map(|b| b.as_table()).collect();
    assert_eq!(tables.len(), 1, "{blocks:?}");
    tables[0]
}

fn spans(t: &Table, r: usize) -> Vec<usize> {
    t.rows[r].cells.iter().map(|c| c.span()).collect()
}

fn widths(t: &Table, r: usize) -> Vec<Option<f32>> {
    t.rows[r].cells.iter().map(|c| c.props.width).collect()
}

/// The widths (points) between consecutive twip edges.
fn edge_widths(edges: &[i16]) -> Vec<Option<f32>> {
    edges.windows(2).map(|w| Some(f32::from(w[1] - w[0]) / 20.0)).collect()
}

fn six_over_nine(first_extra: &[u8], second_extra: &[u8]) -> Blocks {
    let mut marks = row(SIX, first_extra);
    marks.extend(row(NINE, second_extra));
    table::assemble(marks)
}

#[test]
fn the_row_with_more_cells_is_the_grid_and_the_header_cells_span_its_columns() {
    let body = six_over_nine(&[], &[]);
    let t = table(&body);
    assert_eq!(t.grid, vec![121.5, 85.5, 69.75, 69.75, 40.5, 13.5, 13.5, 13.5, 40.5]);
    assert_eq!(spans(t, 0), [1, 1, 2, 1, 3, 1]);
    assert_eq!(spans(t, 1), [1; 9]);
    assert_eq!(widths(t, 0), edge_widths(SIX));
    assert_eq!(widths(t, 1), edge_widths(NINE));
    for r in &t.rows {
        assert_eq!((r.props.grid_before, r.props.grid_after), (0, 0));
    }
}

/// This reader takes vertical merges from `sprmTVertMerge` on the row (not from the TC80 cell
/// definitions); a merge given that way stays on its cells when the rows are aligned.
#[test]
fn a_vertical_merge_survives_the_shared_grid() {
    let body = six_over_nine(&vert_merge(2, 3), &vert_merge(2, 1));
    let t = table(&body);
    assert_eq!(t.grid.len(), 9);
    assert_eq!(spans(t, 0), [1, 1, 2, 1, 3, 1]);
    assert_eq!(spans(t, 1), [1; 9]);
    assert_eq!(t.rows[0].cells[2].props.vmerge, VMerge::Restart);
    assert_eq!(t.rows[1].cells[2].props.vmerge, VMerge::Continue);
    let others = |r: usize, skip: usize| t.rows[r].cells.iter().enumerate().filter(|(i, _)| *i != skip).all(|(_, c)| c.props.vmerge == VMerge::None);
    assert!(others(0, 2) && others(1, 2));
}

#[test]
fn a_one_row_table_keeps_its_simple_grid() {
    let body = table::assemble(row(&[0, 1440, 4320], &[]));
    let t = table(&body);
    assert_eq!(t.grid, vec![72.0, 144.0]);
    assert_eq!(widths(t, 0), [Some(72.0), Some(144.0)]);
    assert_eq!(spans(t, 0), [1, 1]);
}

#[test]
fn rows_whose_edges_all_drift_keep_the_first_rows_grid_and_finish_fast() {
    // 64 rows of 63 cells, every edge one twip further right than the row above: no row holds
    // every edge, and the union of them is far more columns than a table has.
    const ROWS: i16 = 64;
    const CELLS: i16 = 63;
    let mut marks = Vec::new();
    for r in 0..ROWS {
        let edges: Vec<i16> = (0..=CELLS).map(|c| c * 150 + r).collect();
        marks.extend(row(&edges, &[]));
    }
    let start = Instant::now();
    let body = table::assemble(marks);
    assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
    let t = table(&body);
    assert_eq!(t.rows.len(), usize::from(ROWS.unsigned_abs()));
    assert_eq!(t.grid, vec![7.5; 63], "the first row's grid");
    for r in &t.rows {
        assert_eq!(r.cells.len(), 63);
        // Each cell keeps its own edge width (7.5 pt, up to the float rounding of the edges).
        assert!(r.cells.iter().all(|c| c.span() == 1 && c.props.width.is_some_and(|w| (w - 7.5).abs() < 0.01)), "{:?}", widths(t, 0));
        assert_eq!((r.props.grid_before, r.props.grid_after), (0, 0));
    }
}
