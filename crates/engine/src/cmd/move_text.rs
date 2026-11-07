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

/// Undo commands cancel even when there is no history to restore.
pub(crate) fn before_command(s: &mut Session, id: &str) {
    invalidate(s);
    if matches!(id, "edit.undo" | "edit.undoTo" | "edit.redo") {
        s.pending_text_move = None;
    }
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
    let mut fragment = s.doc.copy_range(&source.start, &source.end);
    // A moved inline run ends at the destination's caret boundary. Give that boundary the
    // destination's formatting, so the text after it does not inherit the moved run's emphasis.
    if fragment.blocks.len() == 1
        && let Some(wordcraft_doc::Block::Para(p)) = fragment.blocks.first_mut()
        && let Some(tail) = s.doc.para_at(&at).map(|p| p.props_of_char(at.off).clone())
        && let Some(run) = p.runs.last_mut()
    {
        run.props = tail;
    }
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
    // A selected paragraph mark brings its paragraph formatting with it, including when
    // the destination already has explicit paragraph properties.
    let paragraph_props =
        if fragment.blocks.len() > 1 { fragment.blocks.first().and_then(|block| block.as_para()).map(|p| p.props.clone()) } else { None };
    s.sel = Selection::caret(at.clone());
    super::paste::insert(s, fragment)?;
    if let Some(props) = paragraph_props {
        s.doc.para_mut(at.story, &at.path)?.props = props;
    }
    s.pending = None;
    s.goal_x = None;
    super::sel_result(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wordcraft_doc::StoryRef;

    #[test]
    fn paragraph_move_forward_preserves_style_and_clipboard() {
        let mut s = Session::new(Document::from_text("first\nsecond\nthird"));
        s.doc.para_mut(StoryRef::Body, &wordcraft_doc::Path::top(0)).unwrap().props.keep_next = Some(true);
        s.run("select.text", &json!({"text": "third"})).unwrap();
        s.run("edit.copy", &json!({})).unwrap();
        let clipboard = s.clipboard.clone();
        let clipboard_text = s.clipboard_text.clone();
        s.run("select.range", &json!({"anchor": {"block":0,"off":0}, "focus":{"block":1,"off":0}})).unwrap();
        s.run("edit.moveText", &json!({})).unwrap();
        s.run("caret.docEnd", &json!({})).unwrap();
        let before = s.doc.clone();
        let depth = s.undo_depth();
        s.run("text.newParagraph", &json!({})).unwrap();
        assert_eq!(s.doc.plain_text(StoryRef::Body), "second\nthirdfirst\n");
        assert_eq!(s.undo_depth(), depth + 1);
        assert_eq!(s.doc.para(StoryRef::Body, &wordcraft_doc::Path::top(1)).unwrap().props.keep_next, Some(true));
        assert_eq!(s.clipboard, clipboard);
        assert_eq!(s.clipboard_text, clipboard_text);
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc, before);
        assert!(s.pending_text_move.is_none());
    }

    #[test]
    fn edits_and_document_replacement_clear_source() {
        let mut s = Session::new(Document::from_text("source target"));
        s.run("select.text", &json!({"text":"source"})).unwrap();
        s.run("edit.copyText", &json!({})).unwrap();
        s.run("format.bold", &json!({})).unwrap();
        assert_eq!(s.run("edit.moveTextPending", &json!({})).unwrap()["pending"], Value::Null);
        s.run("edit.moveText", &json!({})).unwrap();
        s.set_document(Document::new());
        assert!(s.pending_text_move.is_none());
    }
}
