//! Smart cut and paste (File › Options › Advanced › Cut, copy, and paste › Use smart cut and
//! paste › Adjust sentence and word spacing automatically): deleting or cutting a whole word
//! leaves one space where it was, and a pasted word gets a space on each side, as in Word.
//!
//! Both work after the edit, on the characters either side of where it happened, so they are part
//! of the same undo step. While changes are tracked the spaces are left alone.

use wordcraft_doc::{Paragraph, Pos};

use super::CmdError;
use crate::{Selection, Session};

/// Punctuation that sits against the word before it: no space goes before it.
const CLOSING: &[char] = &['.', ',', ';', ':', '!', '?', ')'];

/// Both settings on (the defaults), and no tracked changes.
pub(crate) fn enabled(s: &Session) -> bool {
    s.prefs.smart_cut_paste && s.prefs.paste_adjust_word_spacing && !s.doc.settings.track_changes
}

fn word(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

fn before(p: &Paragraph, off: usize) -> Option<char> {
    p.text.get(..off).and_then(|t| t.chars().next_back())
}

fn at(p: &Paragraph, off: usize) -> Option<char> {
    p.text.get(off..).and_then(|t| t.chars().next())
}

/// The range `a..b` (in one paragraph) starts and ends on word boundaries and is whole words:
/// the only removals the spacing fix follows.
pub(crate) fn whole_words(s: &Session, a: &Pos, b: &Pos) -> bool {
    if !enabled(s) || a.story != b.story || a.path != b.path || a.off >= b.off {
        return false;
    }
    let Some(p) = s.doc.para_at(a) else { return false };
    !word(before(p, a.off)) && word(at(p, a.off)) && word(before(p, b.off)) && !word(at(p, b.off))
}

/// After whole words were removed with the caret left where they were: two spaces become one, a
/// space left before closing punctuation goes, and so does one left at the paragraph's start.
pub(crate) fn after_delete(s: &mut Session) -> Result<(), CmdError> {
    if !s.sel.is_collapsed() {
        return Ok(());
    }
    let caret = s.sel.focus.clone();
    let Some(p) = s.doc.para_at(&caret) else { return Ok(()) };
    let (prev, next) = (before(p, caret.off), at(p, caret.off));
    let drop = match (prev, next) {
        (Some(' '), Some(' ')) => Some(caret.off),
        (Some(' '), Some(c)) if CLOSING.contains(&c) && word(before(p, caret.off.saturating_sub(1))) => caret.off.checked_sub(1),
        (None, Some(' ')) => Some(caret.off),
        _ => None,
    };
    let Some(d) = drop else { return Ok(()) };
    s.doc.para_mut(caret.story, &caret.path)?.delete(d, d.saturating_add(1))?;
    s.sel = Selection::caret(Pos { off: d.min(caret.off), ..caret });
    Ok(())
}

/// Ends with something a following word needs a space after.
fn spaced_after(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || CLOSING.contains(&c) || c == '"' || c == '\'' || c == '’' || c == '”')
}

/// After a paste from `start` to the caret: a space goes between the pasted text and a word
/// against it on either side (never before closing punctuation), and a pasted leading or
/// trailing space next to a space already there goes. Only the first and last joins are looked
/// at.
pub(crate) fn after_paste(s: &mut Session, start: &Pos) -> Result<(), CmdError> {
    if !enabled(s) || !s.sel.is_collapsed() {
        return Ok(());
    }
    let mut end = s.sel.focus.clone();
    if end.story != start.story {
        return Ok(());
    }
    let one = end.path == start.path;
    if one && end.off <= start.off {
        return Ok(());
    }
    // The end first, so the start's offsets stay put.
    let lower = if one { start.off } else { 0 };
    if let Some(p) = s.doc.para_at(&end)
        && end.off > lower
    {
        let (last, next) = (before(p, end.off), at(p, end.off));
        if last == Some(' ') && next == Some(' ') {
            s.doc.para_mut(end.story, &end.path)?.delete(end.off.saturating_sub(1), end.off)?;
            end.off = end.off.saturating_sub(1);
        } else if spaced_after(last) && (word(next) || next == Some('(')) {
            let props = p.props_at(end.off).clone();
            s.doc.para_mut(end.story, &end.path)?.insert_text(end.off, " ", &props)?;
        }
    }
    if one && end.off <= start.off {
        s.sel = Selection::caret(end);
        return Ok(());
    }
    if let Some(p) = s.doc.para_at(start)
        && (!one || start.off < end.off)
    {
        let (prev, first) = (before(p, start.off), at(p, start.off));
        if first == Some(' ') && matches!(prev, None | Some(' ')) {
            s.doc.para_mut(start.story, &start.path)?.delete(start.off, start.off.saturating_add(1))?;
            if one {
                end.off = end.off.saturating_sub(1);
            }
        } else if word(first) && prev.is_some_and(|c| c.is_alphanumeric() || CLOSING.contains(&c)) {
            let props = p.props_of_char(start.off).clone();
            s.doc.para_mut(start.story, &start.path)?.insert_text(start.off, " ", &props)?;
            if one {
                end.off = end.off.saturating_add(1);
            }
        }
    }
    s.sel = Selection::caret(end);
    Ok(())
}
