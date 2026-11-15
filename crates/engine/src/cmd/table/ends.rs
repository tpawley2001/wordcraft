use super::cells::{cell, grid_cols};
use super::select::select_range;
use super::{CmdError, CmdResult, Path, Pos, Selection, Session, sel_result};

fn missing() -> CmdError {
    CmdError::Disabled("the cursor isn't in a table cell".into())
}

/// The first or last cell of the row (`row`) or column of cell `(r, c)` of table `tp`, in the
/// innermost table; rows without the cell index are skipped for a column.
fn edge_cell(s: &Session, tp: &Path, r: usize, c: usize, row: bool, first: bool) -> Result<(usize, usize), CmdError> {
    let t = s.doc.table(s.sel.focus.story, tp).ok_or_else(missing)?;
    let current = t.rows.get(r).ok_or_else(missing)?;
    current.cells.get(c).ok_or_else(missing)?;
    if row {
        return Ok((r, if first { 0 } else { current.cells.len().saturating_sub(1) }));
    }
    let mut rows = t.rows.iter().enumerate().filter(|(_, row)| row.cells.get(c).is_some());
    let target = if first { rows.next() } else { rows.next_back() };
    Ok((target.ok_or_else(missing)?.0, c))
}

/// The start of cell `(r, c)` of table `tp`.
fn cell_start(s: &Session, tp: &Path, r: usize, c: usize) -> Result<Pos, CmdError> {
    let mut path = tp.0.clone();
    path.push(u32::try_from(r).map_err(|_| CmdError::Failed("row index is too large".into()))?);
    path.push(u32::try_from(c).map_err(|_| CmdError::Failed("cell index is too large".into()))?);
    path.push(0);
    Ok(s.doc.clamp(&Pos { story: s.sel.focus.story, path: Path(path), off: 0 }))
}

/// Move within the innermost table, keeping the row or the cell index in the column.
pub(super) fn move_to_cell_end(s: &mut Session, row: bool, first: bool) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let (r, c) = edge_cell(s, &tp, r, c, row, first)?;
    s.sel = Selection::caret(cell_start(s, &tp, r, c)?);
    s.column = None;
    s.column_mode = false;
    s.widened = None;
    s.goal_x = None;
    s.pending = None;
    sel_result(s)
}

/// Alt+Shift+Home/End/PageUp/PageDown: whole cells from the selection's anchor cell to the first
/// or last cell of the focus cell's row or column. Where the selection really ends is kept
/// (`Session::sel_ends`), so a second key extends from the same anchor.
pub(super) fn select_to_cell_end(s: &mut Session, row: bool, first: bool) -> CmdResult {
    let ends = s.sel_ends().clone();
    let (tp, r, c) = ends.focus.path.cell().ok_or_else(missing)?;
    let story = ends.focus.story;
    let (tr, tc) = edge_cell(s, &tp, r, c, row, first)?;
    // The anchor cell, when it is in the same table; otherwise the focus cell.
    let (anchor, ar, ac) = match ends.anchor.path.cell() {
        Some((atp, ar, ac)) if atp == tp && ends.anchor.story == story => (ends.anchor.clone(), ar, ac),
        _ => (cell_start(s, &tp, r, c)?, r, c),
    };
    let (g0, g1) = {
        let t = s.doc.table(story, &tp).ok_or_else(missing)?;
        let (a, b) = (grid_cols(t, ar, ac), grid_cols(t, tr, tc));
        (a.0.min(b.0), a.1.max(b.1))
    };
    let focus = cell_start(s, &tp, tr, tc)?;
    let result = select_range(s, story, &tp, (ar, tr), (g0, g1))?;
    s.widened = Some(crate::session::Widened { sel: s.sel.clone(), ends: Selection { anchor, focus } });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wordcraft_doc::{Block, Document, StoryRef, Table};

    fn session(table: Table, path: Vec<u32>) -> Session {
        let mut doc = Document::new();
        doc.body.insert(0, Block::Table(table).into());
        let mut s = Session::new(doc);
        s.sel = Selection::caret(Pos { story: StoryRef::Body, path: Path(path), off: 0 });
        s
    }

    #[test]
    fn column_ends_skip_rows_without_the_cell_and_add_no_undo_step() {
        let mut table = Table::new(5, 4, 400.0);
        for r in [0, 2, 4] {
            table.rows[r].cells.truncate(1);
        }
        let mut s = session(table, vec![0, 3, 2, 0]);
        let before = s.doc.clone();
        let undo = s.undo_labels();
        s.run("table.firstCellInColumn", &json!({})).unwrap();
        assert_eq!(s.sel.focus.path, Path(vec![0, 1, 2, 0]));
        s.run("table.lastCellInColumn", &json!({})).unwrap();
        assert_eq!(s.sel.focus.path, Path(vec![0, 3, 2, 0]));
        assert_eq!(s.sel.focus.off, 0);
        assert_eq!(s.doc, before);
        assert_eq!(s.undo_labels(), undo);
    }

    #[test]
    fn all_four_moves_stay_in_the_innermost_table() {
        let mut outer = Table::new(3, 4, 400.0);
        outer.rows[1].cells[1].blocks.insert(0, Block::Table(Table::new(3, 4, 300.0)).into());
        let mut s = session(outer, vec![0, 1, 1, 0, 1, 1, 0]);
        for (id, r, c) in
            [("table.firstCellInRow", 1, 0), ("table.lastCellInRow", 1, 3), ("table.firstCellInColumn", 0, 1), ("table.lastCellInColumn", 2, 1)]
        {
            s.sel = Selection::caret(Pos { story: StoryRef::Body, path: Path(vec![0, 1, 1, 0, 1, 1, 0]), off: 0 });
            s.run(id, &json!({})).unwrap();
            assert_eq!(s.sel.focus.path, Path(vec![0, 1, 1, 0, r, c, 0]), "{id}");
            assert_eq!(s.sel.focus.off, 0);
            assert!(s.sel.is_collapsed());
        }
    }
}
