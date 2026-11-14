//! Move Paragraph Up / Down (Alt+Shift+Up/Down) with the selection in a table: as in Word, the
//! whole rows the selection touches move, never the paragraphs inside a cell.

use super::layout::touch_cells;
use super::{Block, CmdError, CmdResult, Path, Pos, Selection, Session, sel_result};
use std::sync::Arc;
use wordcraft_doc::props::VMerge;

/// The rows of one table that the selection touches move a row up or down. At the table's first
/// row (moving up) or last row (moving down) they leave the table and pass the paragraph next to
/// it, becoming a table of their own. `None` when the selection isn't inside one table.
pub(crate) fn move_rows(s: &mut Session, up: bool) -> Option<CmdResult> {
    let (a, f) = (&s.sel.anchor, &s.sel.focus);
    if a.story != f.story {
        return None;
    }
    let (ta, ra, _) = a.path.cell()?;
    let (tf, rf, _) = f.path.cell()?;
    if ta != tf {
        return None;
    }
    Some(move_in(s, &tf, ra.min(rf), ra.max(rf), up))
}

fn refuse(why: &str) -> CmdResult {
    Err(CmdError::Failed(why.into()))
}

fn move_in(s: &mut Session, tp: &Path, r0: usize, r1: usize, up: bool) -> CmdResult {
    let story = s.sel.focus.story;
    let Some(t) = s.doc.table(story, tp) else { return refuse("no table") };
    let n = t.rows.len();
    if r1 >= n {
        return refuse("no such row");
    }
    let inside = if up { r0 > 0 } else { r1 + 1 < n };
    let (lo, hi) = match (inside, up) {
        (true, true) => (r0 - 1, r1),
        (true, false) => (r0, r1 + 1),
        (false, _) => (r0, r1),
    };
    // A vertical merge would be cut (or continue from a different row): refused.
    let has = |r: usize, f: &dyn Fn(VMerge) -> bool| t.rows.get(r).is_some_and(|row| row.cells.iter().any(|c| f(c.props.vmerge)));
    if (lo..=hi).any(|r| has(r, &|v| v != VMerge::None)) || has(hi + 1, &|v| v == VMerge::Continue) {
        return refuse("rows in a vertical merge don't move");
    }
    // A row content control spanning several rows would come apart.
    if t.rows.get(lo..=hi).unwrap_or_default().iter().any(|row| usize::try_from(row.controls.close).ok() != Some(row.controls.open.len())) {
        return refuse("rows in a content control don't move");
    }
    let k = tp.0.len();
    let ti = tp.last();
    if inside {
        let t = s.doc.table_mut(story, tp)?;
        let Some(rows) = t.rows.get_mut(lo..=hi) else { return refuse("no such row") };
        if up {
            rows.rotate_left(1);
        } else {
            rows.rotate_right(1);
        }
        let row = |r: usize| if up { r.saturating_sub(1) } else { r.saturating_add(1) };
        return finish(s, k, std::slice::from_ref(tp), &|_| ti, &row);
    }
    if !t.controls.is_empty() {
        return refuse("a table in a content control doesn't split");
    }
    let bl = s.doc.container_mut(story, tp)?;
    let j = if up { ti.checked_sub(1) } else { ti.checked_add(1) };
    let Some(j) = j.filter(|j| *j < bl.len()) else {
        return refuse(if up { "already at the top" } else { "already at the bottom" });
    };
    match bl.get(j).map(|b| &**b) {
        Some(Block::Para(p)) if p.section.is_none() => {}
        Some(Block::Para(_)) => return refuse("rows don't move across a section break"),
        _ => return refuse("a table is in the way"),
    }
    // The story or cell ends with a paragraph, so a table can't go after its last one.
    if !up && j + 1 >= bl.len() {
        return refuse("already at the bottom");
    }
    let split = if up { r1 + 1 } else { r0 };
    let rest = if split == 0 || split >= n {
        None
    } else {
        match bl.get_mut(ti).map(Arc::make_mut) {
            Some(Block::Table(t)) => Some(t.split_off(split)),
            _ => return refuse("no table"),
        }
    };
    // Up: the moved rows (the table's first ones) swap with the paragraph and the rest follow
    // it. Down: the table keeps its first rows and the moved ones follow the paragraph.
    let (moved_at, tables) = match rest {
        None => {
            bl.swap(ti, j);
            (j, vec![tp.with_last(j)])
        }
        Some(rest) if up => {
            bl.swap(ti, j);
            bl.insert(ti + 1, Arc::new(Block::Table(rest)));
            (j, vec![tp.with_last(j), tp.with_last(ti + 1)])
        }
        Some(rest) => {
            bl.insert(j + 1, Arc::new(Block::Table(rest)));
            (j + 1, vec![tp.clone(), tp.with_last(j + 1)])
        }
    };
    let row = |r: usize| if up { r } else { r.saturating_sub(split) };
    finish(s, k, &tables, &|_| moved_at, &row)
}

/// Put the selection back on the moved rows' content (table index `table`, row `row`) and mark
/// the touched tables' paragraphs for layout.
fn finish(s: &mut Session, k: usize, tables: &[Path], table: &dyn Fn(usize) -> usize, row: &dyn Fn(usize) -> usize) -> CmdResult {
    let remap = |p: &Pos| -> Pos {
        let mut p = p.clone();
        if let Some(t) = k.checked_sub(1).and_then(|i| p.path.0.get_mut(i)) {
            *t = u32::try_from(table(*t as usize)).unwrap_or(*t);
        }
        if let Some(r) = p.path.0.get_mut(k) {
            *r = u32::try_from(row(*r as usize)).unwrap_or(*r);
        }
        p
    };
    let (anchor, focus) = (remap(&s.sel.anchor), remap(&s.sel.focus));
    s.sel = Selection { anchor: s.doc.clamp(&anchor), focus: s.doc.clamp(&focus) };
    for tp in tables {
        touch_cells(s, tp)?;
    }
    s.goal_x = None;
    sel_result(s)
}

#[cfg(test)]
mod tests {
    use crate::Session;
    use serde_json::json;
    use wordcraft_doc::props::VMerge;
    use wordcraft_doc::{Block, Document};

    /// `before`, a 2-column table of `rows` rows (r1.., x), then an empty paragraph.
    fn doc(rows: usize) -> Session {
        let mut text = String::from("before\n");
        for r in 1..=rows {
            text.push_str(&format!("r{r}\tx\n"));
        }
        let mut s = Session::new(Document::from_text(&text));
        let sel = json!({"anchor": {"story": "body", "path": [1], "off": 0}, "focus": {"story": "body", "path": [rows], "off": 4}});
        s.run("select.range", &sel).unwrap();
        s.run("table.fromText", &json!({})).unwrap();
        s
    }

    fn first_cells(s: &Session) -> Vec<String> {
        let Some(Block::Table(t)) = s.doc.body.get(1).map(|b| &**b) else { panic!("no table") };
        t.rows.iter().map(|r| r.cells[0].blocks[0].as_para().unwrap().text.clone()).collect()
    }

    #[test]
    fn a_row_in_a_vertical_merge_does_not_move() {
        let mut s = doc(3);
        if let Some(Block::Table(t)) = s.doc.body.get_mut(1).map(std::sync::Arc::make_mut) {
            t.rows[0].cells[1].props.vmerge = VMerge::Restart;
            t.rows[1].cells[1].props.vmerge = VMerge::Continue;
        }
        s.run("caret.set", &json!({"pos": {"story": "body", "path": [1, 2, 0, 0], "off": 0}})).unwrap();
        assert!(s.run("para.moveUp", &json!({})).is_err(), "row 2 continues a merge");
        assert_eq!(first_cells(&s), ["r1", "r2", "r3"]);
        // The last row can't leave the table past the story's last paragraph.
        assert!(s.run("para.moveDown", &json!({})).is_err());
    }

    #[test]
    fn a_selection_over_two_rows_moves_both() {
        let mut s = doc(4);
        let sel = json!({"anchor": {"story": "body", "path": [1, 1, 0, 0], "off": 0}, "focus": {"story": "body", "path": [1, 2, 1, 0], "off": 1}});
        s.run("select.range", &sel).unwrap();
        s.run("para.moveDown", &json!({})).unwrap();
        assert_eq!(first_cells(&s), ["r1", "r4", "r2", "r3"]);
        assert_eq!(s.sel.anchor.path.0, vec![1, 2, 0, 0]);
        assert_eq!(s.sel.focus.path.0, vec![1, 3, 1, 0]);
    }
}
