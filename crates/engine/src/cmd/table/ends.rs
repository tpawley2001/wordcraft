use super::cells::cell;
use super::{CmdError, CmdResult, Path, Pos, Selection, Session, sel_result};

/// Move within the innermost table, keeping the row or the cell index in the column.
pub(super) fn move_to_cell_end(s: &mut Session, row: bool, first: bool) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let story = s.sel.focus.story;
    let missing = || CmdError::Disabled("the cursor isn't in a table cell".into());
    let t = s.doc.table(story, &tp).ok_or_else(missing)?;
    let current = t.rows.get(r).ok_or_else(missing)?;
    current.cells.get(c).ok_or_else(missing)?;
    let (r, c) = if row {
        (r, if first { 0 } else { current.cells.len().saturating_sub(1) })
    } else {
        let mut rows = t.rows.iter().enumerate().filter(|(_, row)| row.cells.get(c).is_some());
        let target = if first { rows.next() } else { rows.next_back() };
        (target.ok_or_else(missing)?.0, c)
    };
    let mut path = tp.0;
    path.push(u32::try_from(r).map_err(|_| CmdError::Failed("row index is too large".into()))?);
    path.push(u32::try_from(c).map_err(|_| CmdError::Failed("cell index is too large".into()))?);
    path.push(0);
    let at = Pos { story, path: Path(path), off: 0 };
    s.sel = Selection::caret(s.doc.clamp(&at));
    s.column = None;
    s.column_mode = false;
    s.widened = None;
    s.goal_x = None;
    s.pending = None;
    sel_result(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wordcraft_doc::{Block, Document, StoryRef, Table};

    fn session(table: Table, path: Vec<u32>) -> Session {
        let mut doc = Document::new();
        doc.body.insert(0, Block::Table(table));
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
        outer.rows[1].cells[1].blocks.insert(0, Block::Table(Table::new(3, 4, 300.0)));
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
