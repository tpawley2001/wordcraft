//! Relocate a selection after choosing its destination, without involving the clipboard.

use serde_json::{Value, json};
use wordcraft_doc::{Document, Pos};

use crate::{CmdError, CmdResult, CommandSpec, Selection, Session};

/// A source belongs to one unchanged document revision; navigation may change the selection.
#[derive(Clone, Debug)]
pub struct PendingMove {
    start: Pos,
    end: Pos,
    copy: bool,
    revision: u64,
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.moveText", "Move Text", "Editing", |s, _| begin(s, false)).key(super::non_mac("F2")).when(has_text_sel).pure(),
        CommandSpec::new("edit.copyText", "Copy Text", "Editing", |s, _| begin(s, true)).key(super::non_mac("Shift+F2")).when(has_text_sel).pure(),
        CommandSpec::new("edit.moveTextHere", "Place Text", "Editing", place),
        CommandSpec::new("edit.moveTextCancel", "Cancel Text Move", "Editing", |s, _| {
            s.pending_text_move = None;
            Ok(json!({}))
        })
        .pure(),
        CommandSpec::new("edit.moveTextPending", "Pending Text Move", "Editing", |s, _| {
            Ok(json!({"pending": s.pending_text_move.as_ref().map(|m| if m.copy { "copy" } else { "move" })}))
        })
        .pure(),
    ]
}

fn has_text_sel(s: &Session) -> Option<&'static str> {
    if s.sel.is_collapsed() || s.sel.anchor.story != s.sel.focus.story { Some("select text in one story first") } else { None }
}

fn begin(s: &mut Session, copy: bool) -> CmdResult {
    let (start, end) = s.sel.ordered();
    s.pending_text_move = Some(PendingMove { start, end, copy, revision: s.rev() });
    Ok(json!({"status": pending_status(s)}))
}

/// The persistent prompt, independent of temporary status messages and caret navigation.
pub fn pending_status(s: &Session) -> Option<&'static str> {
    s.pending_text_move.as_ref().map(|m| if m.copy { "Copy to where?" } else { "Move to where?" })
}

/// Discard sources after edits, including commands that join an existing undo step.
pub(crate) fn invalidate(s: &mut Session) {
    if s.pending_text_move.as_ref().is_some_and(|m| m.revision != s.rev()) {
        s.pending_text_move = None;
    }
}

// Stored byte offsets plus one byte per paragraph mark retain inline object positions.
fn offset(doc: &Document, at: &Pos) -> Result<usize, CmdError> {
    let mut n = 0usize;
    for path in doc.para_paths(at.story) {
        let Some(p) = doc.para(at.story, &path) else { continue };
        if path == at.path && p.text.is_char_boundary(at.off) {
            return Ok(n.saturating_add(at.off));
        }
        n = n.saturating_add(p.len()).saturating_add(1);
    }
    Err(CmdError::Failed("invalid text destination".into()))
}

pub(crate) fn place(s: &mut Session, _: &Value) -> CmdResult {
    invalidate(s);
    let source = s.pending_text_move.take().ok_or_else(|| CmdError::Failed("no text move is pending".into()))?;
    let (mut at, _) = s.sel.ordered();
    if at.story == source.start.story && at >= source.start && at <= source.end {
        return super::sel_result(s);
    }
    let fragment = s.doc.copy_range(&source.start, &source.end);
    if !source.copy {
        let dest_offset = offset(&s.doc, &at)?;
        let old_end = s.doc.end_of(at.story);
        let old_len = offset(&s.doc, &old_end)?;
        s.sel = Selection { anchor: source.start.clone(), focus: source.end.clone() };
        super::delete_selection(s)?;
        if at.story == source.start.story {
            let new_len = offset(&s.doc, &s.doc.end_of(at.story))?;
            let mut remaining = if at > source.end { dest_offset.saturating_sub(old_len.saturating_sub(new_len)) } else { dest_offset };
            let mut mapped = None;
            for path in s.doc.para_paths(at.story) {
                let Some(p) = s.doc.para(at.story, &path) else { continue };
                if remaining <= p.len() {
                    mapped = Some(Pos { story: at.story, path, off: remaining });
                    break;
                }
                remaining = remaining.saturating_sub(p.len().saturating_add(1));
            }
            at = mapped.ok_or_else(|| CmdError::Failed("text destination no longer exists".into()))?;
        }
    }
    s.sel = Selection::caret(at);
    super::paste::insert(s, fragment)?;
    s.pending = None;
    s.goal_x = None;
    super::sel_result(s)
}
