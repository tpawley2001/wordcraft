use super::cells::{cell, selected_cells, with_cells, with_table};
use super::{
    Align, Border, BorderStyle, Borders, CmdError, CmdResult, HeightRule, Path, Pos, Rgb, Selection, Session, Table, TextDirection, VAlign, Value,
    json, p, sel_result,
};

/// Table Layout › AutoFit: Contents sizes the columns to their text, Window spreads them across
/// the text column, Fixed keeps the widths as they are.
pub(super) fn autofit(s: &mut Session, v: &Value) -> CmdResult {
    let mode = p::str(v, "mode").unwrap_or("window");
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    let tw = crate::cmd::page::sect(s).text_width();
    let mut t = s.doc.table(story, &tp).cloned().ok_or_else(|| CmdError::Disabled("the cursor isn't in a table".into()))?;
    fit_table(&s.doc, &mut t, mode, tw);
    *s.doc.table_mut(story, &tp)? = t;
    touch_cells(s, &tp)?;
    sel_result(s)
}

/// AutoFit `t` for a text column `tw` wide (`mode` as `table.autofit` takes it). `t` need not
/// be in `doc` yet: `doc` only supplies the styles its text is measured in.
pub(crate) fn fit_table(doc: &wordcraft_doc::Document, t: &mut Table, mode: &str, tw: f32) {
    match mode {
        "fixed" => t.props.fixed = true,
        "contents" => {
            let avail = (tw - t.props.indent.filter(|i| i.is_finite()).unwrap_or(0.0).max(0.0)).max(72.0);
            let widths = wordcraft_layout::autofit_widths(&wordcraft_layout::measure_table_columns(doc, t), avail);
            t.props.fixed = false;
            t.props.width = None;
            t.props.width_pct = None;
            t.grid = widths.iter().map(|w| w.max(6.0)).collect();
            sync_cell_widths(t);
        }
        _ => {
            t.props.fixed = false;
            t.props.width_pct = Some(100.0);
            let n = t.grid.len().max(1) as f32;
            t.grid.iter_mut().for_each(|g| *g = tw / n);
            sync_cell_widths(t);
        }
    }
}

/// Give every cell the width of the grid columns it spans, so the cells' preferred widths
/// (saved to .docx) agree with the grid.
pub(crate) fn sync_cell_widths(t: &mut Table) {
    let grid = t.grid.clone();
    for row in &mut t.rows {
        // A row's cells start after the grid columns it skips (`w:gridBefore`).
        let mut g = row.first_grid();
        for c in &mut row.cells {
            let span = c.span();
            let w: f32 = grid.get(g..g.saturating_add(span).min(grid.len())).unwrap_or(&[]).iter().sum();
            if w > 0.0 {
                c.props.width = Some(w);
            }
            g = g.saturating_add(span);
        }
    }
}

/// Set the width of the grid column cell `c` of row `r` starts in. The table's preferred width
/// goes, so the layout doesn't scale the column back.
/// Give every grid column the `cells` (row, cell) pairs touch the width `w`.
pub(super) fn set_column_widths(t: &mut Table, cells: &[(usize, usize)], w: f32) {
    let mut cols: Vec<usize> = cells.iter().map(|&(r, c)| t.grid_col(r, c)).collect();
    cols.sort_unstable();
    cols.dedup();
    let mut any = false;
    for g in cols {
        if let Some(x) = t.grid.get_mut(g) {
            *x = w;
            any = true;
        }
    }
    if any {
        t.props.width = None;
        t.props.width_pct = None;
        sync_cell_widths(t);
    }
}

/// Table Properties: table, row, column and cell settings in one undoable step. Row, column and
/// cell settings apply to the selected rows, columns and cells, as in Word, and to the caret's
/// when nothing is selected (#4857). Without any setting it returns the caret's current values.
pub(super) fn properties(s: &mut Session, v: &Value) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let keys = [
        "align",
        "rtl",
        "width",
        "widthPct",
        "indent",
        "rowHeight",
        "rowHeightRule",
        "allowBreak",
        "headerRow",
        "columnWidth",
        "cellWidth",
        "valign",
        "cellMargins",
        "wrapText",
    ];
    if !keys.iter().any(|k| v.get(*k).is_some()) {
        let t = s.doc.table(s.sel.focus.story, &tp).cloned().unwrap_or_default();
        let mut out = serde_json::to_value(&t.props).unwrap_or(Value::Null);
        let row = t.rows.get(r);
        let cl = row.and_then(|x| x.cells.get(c));
        if let Some(o) = out.as_object_mut() {
            o.insert("row".into(), row.map(|x| serde_json::to_value(&x.props).unwrap_or(Value::Null)).unwrap_or(Value::Null));
            o.insert("columnWidth".into(), t.grid.get(t.grid_col(r, c)).map_or(Value::Null, |w| json!(w)));
            o.insert("cell".into(), cl.map(|x| serde_json::to_value(&x.props).unwrap_or(Value::Null)).unwrap_or(Value::Null));
        }
        return Ok(out);
    }
    let align = match p::str(v, "align") {
        None => None,
        Some("left") => Some(Align::Left),
        Some("center") => Some(Align::Center),
        Some("right") => Some(Align::Right),
        Some(x) => return Err(CmdError::Params(format!("unknown table alignment `{x}`"))),
    };
    let rule = match p::str(v, "rowHeightRule") {
        None => None,
        Some("atLeast") => Some(HeightRule::AtLeast),
        Some("exact") => Some(HeightRule::Exact),
        Some(x) => return Err(CmdError::Params(format!("unknown height rule `{x}`"))),
    };
    let valign = match p::str(v, "valign") {
        None => None,
        Some("top") => Some(VAlign::Top),
        Some("center") => Some(VAlign::Center),
        Some("bottom") => Some(VAlign::Bottom),
        Some(x) => return Err(CmdError::Params(format!("unknown vertical alignment `{x}`"))),
    };
    // `null` clears a preferred size (automatic); a number sets it.
    let size = |k: &str, lo: f32, hi: f32| -> Option<Option<f32>> {
        match v.get(k)? {
            Value::Null => Some(None),
            x => x.as_f64().map(|f| f as f32).filter(|f| f.is_finite()).map(|f| Some(f.clamp(lo, hi))),
        }
    };
    let width = size("width", 6.0, 1584.0);
    let width_pct = p::f32(v, "widthPct").map(|x| x.clamp(1.0, 100.0));
    let indent = p::f32(v, "indent").map(|x| x.clamp(-1584.0, 1584.0));
    let row_height = size("rowHeight", 1.0, 1584.0);
    let allow_break = p::bool(v, "allowBreak");
    let header = p::bool(v, "headerRow");
    let column_width = p::f32(v, "columnWidth").map(|x| x.clamp(6.0, 1584.0));
    let cell_width = size("cellWidth", 6.0, 1584.0);
    let rtl = p::bool(v, "rtl");
    // Cell Options: `null` is Same as the whole table; an object gives the cell its own margins.
    let cell_margins: Option<Option<[Option<f32>; 4]>> = match v.get("cellMargins") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(m @ Value::Object(_)) => {
            let side = |k: &str| p::f32(m, k).map(|x| x.clamp(0.0, 144.0));
            Some(Some([side("top"), side("left"), side("bottom"), side("right")]))
        }
        Some(x) => return Err(CmdError::Params(format!("cellMargins must be an object of margins or null, not {x}"))),
    };
    let wrap_text = p::bool(v, "wrapText");
    let default_margins = s.doc.table(s.sel.focus.story, &tp).map(|t| super::table_cell_margins(&s.doc, t)).unwrap_or([0.0, 5.4, 0.0, 5.4]);
    // The selected cells (a whole table, rows, a column…), else the caret's.
    let cells = Some(selected_cells(s)).filter(|c| !c.is_empty()).unwrap_or_else(|| vec![(r, c)]);
    let mut rows: Vec<usize> = cells.iter().map(|&(r, _)| r).collect();
    rows.sort_unstable();
    rows.dedup();
    with_table(s, |t| {
        if let Some(a) = align {
            t.props.align = Some(a);
        }
        if let Some(on) = rtl {
            t.props.bidi_visual = on;
        }
        if let Some(i) = indent {
            t.props.indent = Some(i);
        }
        if let Some(cw) = column_width {
            set_column_widths(t, &cells, cw);
        }
        if let Some(w) = width {
            t.props.width = w;
            t.props.width_pct = None;
        }
        if let Some(pct) = width_pct {
            t.props.width_pct = Some(pct);
        }
        for &r in &rows {
            let Some(row) = t.rows.get_mut(r) else { continue };
            if let Some(h) = row_height {
                row.props.height = h;
                row.props.height_rule = if h.is_some() { rule.unwrap_or(HeightRule::AtLeast) } else { HeightRule::Auto };
            } else if let Some(rule) = rule
                && row.props.height.is_some()
            {
                row.props.height_rule = rule;
            }
            if let Some(b) = allow_break {
                row.props.cant_split = !b;
            }
            if let Some(h) = header {
                row.props.header = h;
            }
        }
        for &(r, c) in &cells {
            let Some(cl) = t.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) else { continue };
            if let Some(w) = cell_width {
                cl.props.width = w;
            }
            if let Some(va) = valign {
                cl.props.valign = Some(va);
            }
            if let Some(m) = cell_margins {
                cl.props.margins = m.map(|sides| {
                    // A side not passed keeps the margin the cell shows now.
                    let mut out = super::effective_cell_margins(&cl.props, default_margins);
                    for (o, side) in out.iter_mut().zip(sides) {
                        if let Some(x) = side {
                            *o = x;
                        }
                    }
                    out
                });
                cl.props.partial_margins = None;
            }
            if let Some(w) = wrap_text {
                cl.props.no_wrap = !w;
            }
        }
    })
}

/// Table Layout › Text Direction: turn the selected cells' text. Without a value it cycles the
/// caret cell's direction (horizontal → down → up) and gives every selected cell the result.
pub(crate) fn text_direction(s: &mut Session, v: &Value) -> CmdResult {
    let (tp, r, c) = cell(s)?;
    let dir = match p::str(v, "value") {
        Some("horizontal" | "lrTb") => TextDirection::Horizontal,
        Some("down" | "tbRl") => TextDirection::Down,
        Some("up" | "btLr") => TextDirection::Up,
        Some(x) => return Err(CmdError::Params(format!("unknown text direction `{x}`"))),
        None => {
            let t = s.doc.table(s.sel.focus.story, &tp).ok_or_else(|| CmdError::Disabled("the cursor isn't in a table".into()))?;
            t.rows.get(r).and_then(|row| row.cells.get(c)).map(|cl| cl.props.text_direction).unwrap_or_default().next()
        }
    };
    with_cells(s, |cl| cl.props.text_direction = dir)
}

/// Bump the revision of every paragraph in a table (table style changes alter their layout).
pub(crate) fn touch_cells(s: &mut Session, tp: &Path) -> Result<(), CmdError> {
    let story = s.sel.focus.story;
    let paths: Vec<Path> = s.doc.para_paths(story).into_iter().filter(|p| p.0.len() > tp.0.len() && p.0.starts_with(&tp.0)).collect();
    for p in paths {
        s.doc.para_mut(story, &p)?.touch();
    }
    Ok(())
}

pub(super) fn fix_caret(s: &mut Session, tp: &Path, r: usize, c: usize) -> CmdResult {
    let story = s.sel.focus.story;
    let Some(t) = s.doc.table(story, tp) else {
        s.sel = Selection::caret(s.doc.clamp(&Pos { story, path: tp.clone(), off: 0 }));
        return sel_result(s);
    };
    let r = r.min(t.rows.len().saturating_sub(1));
    let c = c.min(t.rows.get(r).map(|x| x.cells.len().saturating_sub(1)).unwrap_or(0));
    let mut path = tp.0.clone();
    path.extend([r as u32, c as u32, 0]);
    s.sel = Selection::caret(Pos { story, path: Path(path), off: 0 });
    sel_result(s)
}

pub(super) fn borders(s: &mut Session, v: &Value) -> CmdResult {
    let kind = p::str(v, "kind").unwrap_or("all");
    let w = p::f32(v, "width").unwrap_or(0.5).clamp(0.25, 6.0);
    let color = p::str(v, "color").and_then(Rgb::parse);
    let b = Border { style: BorderStyle::Single, width: w, color, space: 0.0 };
    let none = Border { style: BorderStyle::None, width: 0.0, color: None, space: 0.0 };
    with_table(s, |t| {
        let mut cur = t.props.borders.unwrap_or_default();
        match kind {
            "all" => cur = Borders::all(b),
            "none" => cur = Borders::all(none),
            "outside" => {
                cur.top = Some(b);
                cur.bottom = Some(b);
                cur.left = Some(b);
                cur.right = Some(b);
                // The cleared inside sides must not fall back to the table style (Word writes nil).
                cur.between = Some(none);
                cur.inside_v = Some(none);
            }
            "inside" => {
                cur.between = Some(b);
                cur.inside_v = Some(b);
                // The cleared outer sides must not fall back to the table style (Word writes nil).
                cur.top = Some(none);
                cur.bottom = Some(none);
                cur.left = Some(none);
                cur.right = Some(none);
            }
            "top" => cur.top = Some(b),
            "bottom" => cur.bottom = Some(b),
            "left" => cur.left = Some(b),
            "right" => cur.right = Some(b),
            _ => {}
        }
        t.props.borders = Some(cur);
        for r in &mut t.rows {
            for c in &mut r.cells {
                c.props.borders = None;
            }
        }
    })
}

/// Table Layout › Distribute Rows, as in Word: the selected rows (every row of the table with just
/// a caret in it) share the total of their laid-out heights equally, each set to that height At
/// least (`w:trHeight w:hRule="atLeast"`); the other rows keep theirs.
pub(super) fn distribute_rows(s: &mut Session) -> CmdResult {
    let (tp, _, _) = cell(s)?;
    let story = s.sel.focus.story;
    let count = s.doc.table(story, &tp).map_or(0, |t| t.rows.len());
    let mut rows: Vec<usize> = if s.sel.is_collapsed() && s.cell_range().is_none() {
        (0..count).collect()
    } else {
        selected_cells(s).into_iter().map(|(r, _)| r).filter(|r| *r < count).collect()
    };
    rows.sort_unstable();
    rows.dedup();
    if rows.is_empty() {
        return sel_result(s);
    }
    let total: f32 = laid_out_heights(s, story, &tp, &rows).iter().sum();
    let h = (total / rows.len() as f32).clamp(1.0, 1584.0);
    with_table(s, |t| {
        for r in &rows {
            if let Some(row) = t.rows.get_mut(*r) {
                row.props.height = Some(h);
                row.props.height_rule = HeightRule::AtLeast;
            }
        }
    })
}

/// The heights `rows` of the table at `tp` have as laid out, in row height units (what an At
/// least height would need to show them so): an exact row's set height; an auto or at-least row's
/// content height or set height, whichever is larger. The layout draws an at-least or auto row
/// with its cells' margins and border bands added, so a second layout with each such row set At
/// least a little taller than drawn shows how much that adds. A row the layout doesn't draw
/// counts its set height.
fn laid_out_heights(s: &Session, story: wordcraft_doc::StoryRef, tp: &Path, rows: &[usize]) -> Vec<f32> {
    let Some(t) = s.doc.table(story, tp) else { return Vec::new() };
    let count = t.rows.len();
    let set = |r: usize| t.rows.get(r).and_then(|row| row.props.height).filter(|h| h.is_finite() && *h > 0.0);
    let exact = |r: usize| t.rows.get(r).is_some_and(|row| row.props.height_rule == HeightRule::Exact) && set(r).is_some();
    let drawn = drawn_row_heights(&s.output_layout_of(&s.doc, false, false, false), &s.doc, story, tp, count);
    // The probe: every measured auto or at-least row set At least 1pt taller than drawn.
    let mut probe = s.doc.clone();
    let mut probed: Vec<Option<f32>> = vec![None; count];
    if let Ok(pt) = probe.table_mut(story, tp) {
        for &r in rows {
            if let (false, Some(Some(d)), Some(row), Some(slot)) = (exact(r), drawn.get(r), pt.rows.get_mut(r), probed.get_mut(r)) {
                let h = (d + 1.0).min(1584.0);
                row.props.height = Some(h);
                row.props.height_rule = HeightRule::AtLeast;
                *slot = Some(h);
            }
        }
    }
    let after = if probed.iter().any(Option::is_some) {
        drawn_row_heights(&s.output_layout_of(&probe, false, false, false), &probe, story, tp, count)
    } else {
        Vec::new()
    };
    rows.iter()
        .map(|&r| {
            if exact(r) {
                return set(r).unwrap_or(0.0);
            }
            let inner = match (drawn.get(r).copied().flatten(), probed.get(r).copied().flatten(), after.get(r).copied().flatten()) {
                (Some(d), Some(h), Some(a)) => Some(d - (a - h).max(0.0)),
                _ => None,
            };
            match (inner, set(r)) {
                (Some(i), Some(h)) => i.max(h),
                (Some(i), None) => i.max(0.0),
                (None, h) => h.unwrap_or(0.0),
            }
        })
        .collect()
}

/// Each row's height as drawn in `l` (summed over the pages a row splits across; a repeated
/// header row counts once), or `None` for a row the layout doesn't draw.
fn drawn_row_heights(
    l: &wordcraft_layout::DocLayout,
    doc: &wordcraft_doc::Document,
    story: wordcraft_doc::StoryRef,
    tp: &Path,
    count: usize,
) -> Vec<Option<f32>> {
    let mut out: Vec<Option<f32>> = vec![None; count];
    let mut seen: Vec<bool> = vec![false; count];
    let header = |r: usize| doc.table(story, tp).and_then(|t| t.rows.get(r)).is_some_and(|row| row.props.header);
    for page in 0..l.pages.len() {
        for t in crate::cmd::table_resize::tables_on(l, doc, page) {
            if t.story != story || t.table != *tp {
                continue;
            }
            for d in &t.rows {
                let hgt = (d.bottom - d.top).max(0.0);
                let (Some(slot), Some(was)) = (out.get_mut(d.row), seen.get_mut(d.row)) else { continue };
                if !hgt.is_finite() || (*was && header(d.row)) {
                    continue;
                }
                *was = true;
                *slot = Some(slot.unwrap_or(0.0) + hgt);
            }
        }
    }
    out
}

#[cfg(test)]
mod distribute_tests {
    use serde_json::json;
    use wordcraft_doc::props::HeightRule;
    use wordcraft_doc::{Document, Path, StoryRef};

    use crate::Session;

    /// Auto rows count as tall as the layout draws their content: (2 empty auto rows + 50) / 3, At least.
    #[test]
    fn auto_rows_count_their_laid_out_height() {
        let mut s = Session::new(Document::new());
        s.run("insert.table", &json!({"rows": 3, "cols": 1})).unwrap();
        let ti = s.doc.body.iter().position(|b| b.as_table().is_some()).unwrap();
        s.run("caret.set", &json!({"pos": {"story": "body", "path": [ti, 2, 0, 0], "off": 0}})).unwrap();
        s.run("table.rowHeight", &json!({"height": 50.0, "rule": "exact"})).unwrap();
        let heights = super::laid_out_heights(&s, StoryRef::Body, &Path::top(ti), &[0, 1, 2]);
        let (a, b) = (heights[0], heights[1]);
        assert!(a > 5.0 && a < 40.0, "an empty auto row is a line tall, got {a}");
        assert!((a - b).abs() < 0.01 && (heights[2] - 50.0).abs() < 0.01, "{heights:?}");
        s.run("table.distributeRows", &json!({})).unwrap();
        let want = (a + b + 50.0) / 3.0;
        let t = s.doc.body[ti].as_table().unwrap();
        for r in &t.rows {
            assert!((r.props.height.unwrap() - want).abs() < 0.01, "{:?} vs {want}", r.props.height);
            assert_eq!(r.props.height_rule, HeightRule::AtLeast);
        }
        // Each row now shows that height: the empty rows' content is shorter than it.
        let after = super::laid_out_heights(&s, StoryRef::Body, &Path::top(ti), &[0, 1, 2]);
        assert!(after.iter().all(|h| (h - want).abs() < 0.01), "{after:?} vs {want}");
    }
}
