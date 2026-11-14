//! Layout › Arrange › Align: line floating objects up with the margin, the page or each other,
//! and distribute them evenly. The whole operation is one command, so one undo step.

use super::arrange::MAX_OFFSET;
use super::select::{edit_float, picked_objects, selected, with_float};
use super::{CmdError, CmdResult, Float, Pos, Session, Value, Wrap, json, p};
use wordcraft_doc::para::Anchor;

/// What the objects line up with: Word's Align to Page, Align to Margin, Align Selected Objects.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Relative {
    Margin,
    Page,
    Selected,
}

/// `relativeTo`, defaulting as Word does: the margin for one object, each other for several.
fn relative(v: &Value, picks: usize) -> Result<Relative, CmdError> {
    match p::str(v, "relativeTo") {
        None if picks > 1 => Ok(Relative::Selected),
        None | Some("margin") => Ok(Relative::Margin),
        Some("page") => Ok(Relative::Page),
        Some("selected") => Ok(Relative::Selected),
        Some(other) => Err(CmdError::Params(format!("unknown relativeTo `{other}` (margin, page or selected)"))),
    }
}

/// One axis of a frame: its start (left or top) and its extent (width or height).
#[derive(Clone, Copy)]
struct Span {
    start: f32,
    len: f32,
}

/// Whether `value` moves objects across (left/center/right) or down (top/middle/bottom), and
/// where in the box their edge goes: 0 = start, 1 = middle, 2 = end.
fn parse_value(value: &str) -> Result<(bool, u8), CmdError> {
    Ok(match value {
        "left" => (true, 0),
        "center" | "centre" => (true, 1),
        "right" => (true, 2),
        "top" => (false, 0),
        "middle" => (false, 1),
        "bottom" => (false, 2),
        other => return Err(CmdError::Params(format!("unknown value `{other}` (left, center, right, top, middle or bottom)"))),
    })
}

/// Where an object `len` long starts when `edge`-aligned in a box `start..start + room`.
fn aligned(edge: u8, start: f32, room: f32, len: f32) -> f32 {
    match edge {
        0 => start,
        2 => start + room - len,
        _ => start + (room - len) / 2.0,
    }
}

/// The margin's or the page's extent along one axis (from the reference's own edge, so 0).
fn room(s: &Session, rel: Relative, across: bool) -> f32 {
    let sect = crate::cmd::page::sect(s);
    match (rel, across) {
        (Relative::Page, true) => sect.page_w,
        (Relative::Page, false) => sect.page_h,
        (_, true) => sect.text_width(),
        (_, false) => sect.text_height(),
    }
}

/// Put the float's edge `at` from the margin's or page's edge, along one axis. An inline object
/// floats first (Square wrapping).
fn place(f: &mut Float, rel: Relative, across: bool, at: f32) {
    if f.wrap == Wrap::Inline {
        f.wrap = Wrap::Square;
        // Word's distance from text for a newly wrapped object: 0.125" at the sides.
        if f.dist == 0.0 {
            f.dist = 9.0;
        }
    }
    let anchor = if rel == Relative::Page { Anchor::Page } else { Anchor::Margin };
    let at = at.clamp(-MAX_OFFSET, MAX_OFFSET);
    if across {
        f.h_rel = anchor;
        f.h_align = None;
        f.x = at;
    } else {
        f.v_rel = anchor;
        f.v_align = None;
        f.y = at;
    }
}

/// An object as laid out: where it is, its frame along the axis on the page, and the offset of
/// that frame from the point its column- and paragraph-relative offsets start at.
struct Laid {
    pos: Pos,
    span: Span,
    from_origin: f32,
}

/// The picked objects as laid out, along one axis. Objects in line with text have no position
/// of their own to move to, so they refuse.
fn laid_out(s: &mut Session, picks: &[Pos], across: bool) -> Result<Vec<Laid>, CmdError> {
    let layout = s.layout();
    let mut out = Vec::with_capacity(picks.len());
    for pos in picks {
        let floating = s.doc.para_at(pos).and_then(|q| q.object_at(pos.off)).is_some_and(|o| o.is_floating());
        if !floating {
            return Err(CmdError::Disabled("objects in line with text can't be aligned to each other: choose a text wrapping for them first".into()));
        }
        let hit = layout.object(pos, s.page_hint).ok_or_else(|| CmdError::Failed("the object isn't laid out".into()))?;
        let r = hit.rect;
        let (span, from_origin) =
            if across { (Span { start: r.x, len: r.w }, r.x - hit.origin.x) } else { (Span { start: r.y, len: r.h }, r.y - hit.origin.y) };
        out.push(Laid { pos: pos.clone(), span, from_origin });
    }
    Ok(out)
}

/// Move the laid-out object `by` points along one axis, keeping what it is positioned from. An
/// aligned one (left, centred, …) becomes an offset from its column or paragraph first.
fn shift(s: &mut Session, o: &Laid, across: bool, by: f32) -> Result<(), CmdError> {
    if by == 0.0 || !by.is_finite() {
        return Ok(());
    }
    let start = o.from_origin;
    edit_float(s, &o.pos, |f| {
        if across {
            if f.h_align.take().is_some() {
                f.h_rel = Anchor::Column;
                f.x = start;
            }
            f.x = (f.x + by).clamp(-MAX_OFFSET, MAX_OFFSET);
        } else {
            if f.v_align.take().is_some() {
                f.v_rel = Anchor::Paragraph;
                f.y = start;
            }
            f.y = (f.y + by).clamp(-MAX_OFFSET, MAX_OFFSET);
        }
    })?;
    Ok(())
}

/// The selection after an align or distribute: the objects it moved.
fn done(s: &Session, picks: &[Pos]) -> CmdResult {
    let moved: Vec<Value> = picks.iter().map(crate::cmd::pos_json).collect();
    let first = selected(s).and_then(|(_, o)| serde_json::to_value(o).ok()).unwrap_or(Value::Null);
    Ok(json!({"object": first, "objects": moved}))
}

/// `arrange.align`: the selected objects' left, centre, right, top, middle or bottom go to the
/// margin's or page's edge, or to the selection's bounding box.
pub(super) fn align(s: &mut Session, v: &Value) -> CmdResult {
    let (across, edge) = parse_value(p::str(v, "value").unwrap_or("center"))?;
    let picks = picked_objects(s);
    let rel = relative(v, picks.len())?;
    if rel != Relative::Selected {
        let room = room(s, rel, across);
        if picks.len() <= 1 {
            // One object (a Drawing Canvas member too): the selection's.
            let (w, h) = selected(s).map(|(_, o)| super::select::obj_size(&o)).unwrap_or((72.0, 72.0));
            let at = aligned(edge, 0.0, room, if across { w } else { h });
            return with_float(s, |f| place(f, rel, across, at));
        }
        for pos in &picks {
            let Some((w, h)) = s.doc.para_at(pos).and_then(|q| q.object_at(pos.off)).and_then(|o| o.frame()).map(|(w, h, _)| (w, h)) else {
                continue;
            };
            let at = aligned(edge, 0.0, room, if across { w } else { h });
            edit_float(s, pos, |f| place(f, rel, across, at))?;
        }
        return done(s, &picks);
    }
    let laid = laid_out(s, &picks, across)?;
    let lo = laid.iter().map(|o| o.span.start).fold(f32::INFINITY, f32::min);
    let hi = laid.iter().map(|o| o.span.start + o.span.len).fold(f32::NEG_INFINITY, f32::max);
    if !lo.is_finite() || !hi.is_finite() {
        return Err(CmdError::Disabled("no picture or shape selected".into()));
    }
    for o in &laid {
        let to = aligned(edge, lo, hi - lo, o.span.len);
        shift(s, o, across, to - o.span.start)?;
    }
    done(s, &picks)
}

/// `arrange.distribute`: equal gaps between the selected objects, across or down. Relative to
/// each other, the first and last stay and at least three objects are needed; relative to the
/// margin or page, they spread from its one edge to the other.
pub(super) fn distribute(s: &mut Session, v: &Value) -> CmdResult {
    let across = match p::str(v, "direction").unwrap_or("horizontal") {
        "horizontal" => true,
        "vertical" => false,
        other => return Err(CmdError::Params(format!("unknown direction `{other}` (horizontal or vertical)"))),
    };
    let picks = picked_objects(s);
    let rel = relative(v, picks.len())?;
    if picks.is_empty() {
        return Err(CmdError::Disabled("no picture or shape selected".into()));
    }
    if rel == Relative::Selected && picks.len() < 3 {
        return Err(CmdError::Disabled("select three or more pictures or shapes to distribute them".into()));
    }
    let mut laid = laid_out(s, &picks, across)?;
    laid.sort_by(|a, b| a.span.start.total_cmp(&b.span.start));
    let total: f32 = laid.iter().map(|o| o.span.len).sum();
    if rel == Relative::Selected {
        let (Some(first), Some(last)) = (laid.first(), laid.last()) else { return done(s, &picks) };
        let (lo, hi) = (first.span.start, last.span.start + last.span.len);
        let gap = (hi - lo - total) / (laid.len() - 1) as f32;
        let mut at = lo;
        for o in &laid {
            shift(s, o, across, at - o.span.start)?;
            at += o.span.len + gap;
        }
        return done(s, &picks);
    }
    let room = room(s, rel, across);
    if let [only] = laid.as_slice() {
        let at = aligned(1, 0.0, room, only.span.len);
        edit_float(s, &only.pos, |f| place(f, rel, across, at))?;
        return done(s, &picks);
    }
    let gap = (room - total) / (laid.len() - 1) as f32;
    let mut at = 0.0;
    for o in &laid {
        let here = at;
        edit_float(s, &o.pos, |f| place(f, rel, across, here))?;
        at += o.span.len + gap;
    }
    done(s, &picks)
}
