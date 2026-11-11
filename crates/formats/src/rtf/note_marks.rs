//! Custom note reference marks (RTF 1.9.1: a note whose reference has no `\chftn` uses the text
//! written there as its mark). `Alpha{\super *}{\footnote\pard {\super *} Star note.}` is a
//! footnote marked "*": the group just before `\footnote` is the mark, and a group with the same
//! text at the start of the note is the note's own mark.

use super::reader::Reader;
use super::state::{Dest, MARK_CLOSE, MARK_OPEN};
use super::{Fmt, Inline};

/// The longest text read as a custom mark, in characters (as Word).
const MAX_MARK_CHARS: usize = 10;

/// Where the current paragraph's text stood when a group opened.
#[derive(Clone, Copy, PartialEq, Eq)]
struct At {
    serial: u64,
    /// The number of inlines.
    n: usize,
    /// The byte length of the last inline when it is text.
    last_len: usize,
}

struct Open {
    depth: usize,
    at: At,
    chftn: u64,
}

/// A group just closed: the text it added and where the paragraph stood before and after.
struct Closed {
    before: At,
    after: At,
    text: String,
}

/// What the reader tracks to find custom marks.
#[derive(Default)]
pub(super) struct MarkTrack {
    /// Bumped whenever the current paragraph is replaced.
    pub(super) serial: u64,
    /// `\chftn` words read so far.
    chftn: u64,
    open: Vec<Open>,
    last: Option<Closed>,
}

impl MarkTrack {
    pub(super) fn saw_chftn(&mut self) {
        self.chftn = self.chftn.wrapping_add(1);
    }
}

impl Reader {
    fn mark_at(&self) -> At {
        let last_len = match self.para.inlines.last() {
            Some(Inline::Text(t, _)) => t.len(),
            _ => 0,
        };
        At { serial: self.note_mark.serial, n: self.para.inlines.len(), last_len }
    }

    /// A group opened (the reader's depth already counts it).
    pub(super) fn mark_group_open(&mut self) {
        if !self.marks_on || self.hf.is_some() || self.st.dest != Dest::Body {
            return;
        }
        let at = self.mark_at();
        let depth = self.depth();
        let t = &mut self.note_mark;
        // A deep nest is bounded by the reader's group limit.
        t.open.push(Open { depth, at, chftn: t.chftn });
    }

    /// A body group is closing (the reader's depth still counts it).
    pub(super) fn mark_group_close(&mut self) {
        let depth = self.depth();
        while self.note_mark.open.last().is_some_and(|o| o.depth > depth) {
            self.note_mark.open.pop();
        }
        if !self.note_mark.open.last().is_some_and(|o| o.depth == depth) {
            return;
        }
        let Some(open) = self.note_mark.open.pop() else { return };
        self.note_mark.last = None;
        if open.chftn != self.note_mark.chftn || self.st.dest != Dest::Body {
            return;
        }
        let Some((text, fmt)) = self.group_text(open.at) else { return };
        let after = self.mark_at();
        let leading = open.at.n == 0 && open.at.last_len == 0 && self.body.is_empty();
        // Inside a custom-marked note, a leading group with the mark's text is the note's own mark.
        if let Some(sub) = self.sub.as_mut()
            && !sub.comment
            && !sub.own_mark_done
            && !sub.custom.is_empty()
        {
            sub.own_mark_done = true;
            let own = leading && text == sub.custom;
            if own {
                self.para.inlines.clear();
                self.push_mark('S', 0, fmt);
            }
            return;
        }
        self.note_mark.last = Some(Closed { before: open.at, after, text });
    }

    /// The text a group added since `at`, when it added only text, and the formatting of its
    /// first run.
    fn group_text(&self, at: At) -> Option<(String, Fmt)> {
        if at.serial != self.note_mark.serial || self.para.inlines.len() < at.n {
            return None;
        }
        let mut text = String::new();
        let mut fmt = None;
        if at.n > 0
            && let Some(Inline::Text(t, f)) = self.para.inlines.get(at.n - 1)
            && t.len() > at.last_len
        {
            text.push_str(t.get(at.last_len..)?);
            fmt = Some(f.clone());
        }
        for i in self.para.inlines.get(at.n..)? {
            let Inline::Text(t, f) = i else { return None };
            text.push_str(t);
            fmt.get_or_insert_with(|| f.clone());
            if text.len() > MAX_MARK_CHARS * 4 {
                return None;
            }
        }
        let chars = text.chars().count();
        if !(1..=MAX_MARK_CHARS).contains(&chars) || text.contains([MARK_OPEN, MARK_CLOSE]) || text.trim().is_empty() {
            return None;
        }
        Some((text, fmt.unwrap_or_default()))
    }

    /// A note starts in the body: when the group just before it is a mark, take it out of the
    /// text and return it.
    pub(super) fn take_custom_mark(&mut self) -> String {
        let Some(c) = self.note_mark.last.take() else { return String::new() };
        if self.sub.is_some() || self.mark_at() != c.after {
            return String::new();
        }
        self.para.inlines.truncate(c.before.n);
        if c.before.n > 0
            && let Some(Inline::Text(t, _)) = self.para.inlines.last_mut()
            && t.is_char_boundary(c.before.last_len)
        {
            t.truncate(c.before.last_len);
        }
        c.text
    }
}

#[cfg(test)]
mod tests {
    use super::super::{export, import};
    use wordcraft_doc::para::{InlineObject, NoteKind};

    fn refs(rtf: &str) -> (String, Vec<(NoteKind, String)>) {
        let d = import(format!(r"{{\rtf1\ansi\pard {rtf}}}").as_bytes()).unwrap();
        let p = d.body.iter().find_map(|b| b.as_para()).unwrap();
        let r = p
            .objects
            .iter()
            .filter_map(|o| match o {
                InlineObject::NoteRef { kind, custom, .. } => Some((*kind, custom.clone())),
                _ => None,
            })
            .collect();
        (p.plain_text().trim().to_string(), r)
    }

    #[test]
    fn a_unicode_escape_mark_is_read_and_written() {
        // The dagger as an RTF Unicode escape (U+2020, 8224 in decimal).
        let dagger = format!("{}u{}?", '\\', 8224);
        let rtf = format!(r"D{{\super{dagger}}}{{\footnote\ftnalt\pard {{\super{dagger}}} Dag.}}\par");
        assert_eq!(refs(&rtf), ("D".to_string(), vec![(NoteKind::Endnote, "\u{2020}".to_string())]));
        let d = import(format!(r"{{\rtf1\ansi\pard {rtf}}}").as_bytes()).unwrap();
        let out = export(&d);
        assert!(out.contains(&format!(r"{{\super {dagger}}}{{\footnote\ftnalt")), "{out}");
        assert!(!out.contains(r"\chftn"), "{out}");
    }

    #[test]
    fn ungrouped_text_before_a_note_stays_text() {
        assert_eq!(refs(r"Alpha*{\footnote\pard x}\par"), ("Alpha*".to_string(), vec![(NoteKind::Footnote, String::new())]));
    }

    #[test]
    fn text_between_the_group_and_the_note_keeps_it_numbered() {
        assert_eq!(refs(r"A{\super *}b{\footnote\pard x}\par"), ("A*b".to_string(), vec![(NoteKind::Footnote, String::new())]));
    }

    #[test]
    fn a_numbered_note_in_a_group_is_unchanged() {
        assert_eq!(refs(r"A{\super\chftn}{\footnote\pard {\super\chftn} x}\par"), ("A".to_string(), vec![(NoteKind::Footnote, String::new())]));
    }
}
