use super::cells::cell;
use super::{CmdResult, Path, Pos, Selection, Session, StoryRef, sel_result};

/// The first or last cell of the caret's row or column, as Word's Alt+Home / Alt+End /
/// Alt+PageUp / Alt+PageDown go: `row` keeps the caret's row and goes to its first or last
/// cell; a column move keeps the caret's cell index and goes to the first row, or the last row
/// that has a cell at that index (rows can be ragged). The innermost table around the caret.
pub(super) fn move_to_cell_end(s: &mut Session, row: bool, first: bool) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let story = s.sel.focus.story;
    let Some(t) = s.doc.table(story, &tp) else { return sel_result(s) };
    let target = if row {
        let row = t.rows.get(r);
        let c = if first { 0 } else { row.map_or(0, |row| row.cells.len().saturating_sub(1)) };
        row.is_some().then_some((r, c))
    } else if first {
        (t.rows.first().is_some_and(|row| row.cells.get(c).is_some())).then_some((0, c))
    } else {
        (0..t.rows.len().rev()).find(|&i| t.rows.get(i).is_some_and(|row| row.cells.get(c).is_some())).map(|i| (i, c))
    };
    let Some((r, c)) = target else { return sel_result(s) };
    let mut path = tp.0.clone();
    path.push(u32::try_from(r).ok().ok_or_else(|| super::CmdError::Failed("the row is too deep to name".into()))?);
    path.push(u32::try_from(c).ok().ok_or_else(|| super::CmdError::Failed("the column is too deep to name".into()))?);
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

/// The story of the caret, for callers that need it after the move.
pub(super) fn story_of(s: &Session) -> StoryRef {
    s.sel.focus.story
}
