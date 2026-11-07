//! Word's accent-composition shortcuts: a dead key (Ctrl+' for acute, Ctrl+` for grave, …)
//! inserts nothing, and the next typed character comes out composed with it. A character the
//! accent has no form for is typed unchanged; any other command cancels the pending accent.

use serde_json::json;

use super::{CmdError, CmdResult, Session, Value, p};
use crate::keymap::normalize;

/// The accent a dead-key shortcut waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accent {
    Acute,
    Grave,
    Circumflex,
    Tilde,
    Diaeresis,
    Ring,
    Ligature,
    Cedilla,
    Stroke,
}

/// The names `text.composeAccent` takes, in the order its error lists them.
const NAMES: &[&str] = &["acute", "grave", "circumflex", "tilde", "diaeresis", "ring", "ligature", "cedilla", "stroke"];

impl Accent {
    /// The accent a `text.composeAccent` param names.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "acute" => Self::Acute,
            "grave" => Self::Grave,
            "circumflex" => Self::Circumflex,
            "tilde" => Self::Tilde,
            "diaeresis" => Self::Diaeresis,
            "ring" => Self::Ring,
            "ligature" => Self::Ligature,
            "cedilla" => Self::Cedilla,
            "stroke" => Self::Stroke,
            _ => return None,
        })
    }

    /// The letters Word composes with this accent, and their composed forms.
    fn table(self) -> &'static [(char, char)] {
        match self {
            Self::Acute => &[
                ('a', 'á'),
                ('e', 'é'),
                ('i', 'í'),
                ('o', 'ó'),
                ('u', 'ú'),
                ('y', 'ý'),
                ('A', 'Á'),
                ('E', 'É'),
                ('I', 'Í'),
                ('O', 'Ó'),
                ('U', 'Ú'),
                ('Y', 'Ý'),
                ('d', 'ð'),
                ('D', 'Ð'),
            ],
            Self::Grave => &[('a', 'à'), ('e', 'è'), ('i', 'ì'), ('o', 'ò'), ('u', 'ù'), ('A', 'À'), ('E', 'È'), ('I', 'Ì'), ('O', 'Ò'), ('U', 'Ù')],
            Self::Circumflex => {
                &[('a', 'â'), ('e', 'ê'), ('i', 'î'), ('o', 'ô'), ('u', 'û'), ('A', 'Â'), ('E', 'Ê'), ('I', 'Î'), ('O', 'Ô'), ('U', 'Û')]
            }
            Self::Tilde => &[('a', 'ã'), ('n', 'ñ'), ('o', 'õ'), ('A', 'Ã'), ('N', 'Ñ'), ('O', 'Õ')],
            Self::Diaeresis => &[
                ('a', 'ä'),
                ('e', 'ë'),
                ('i', 'ï'),
                ('o', 'ö'),
                ('u', 'ü'),
                ('y', 'ÿ'),
                ('A', 'Ä'),
                ('E', 'Ë'),
                ('I', 'Ï'),
                ('O', 'Ö'),
                ('U', 'Ü'),
                ('Y', 'Ÿ'),
            ],
            Self::Ring => &[('a', 'å'), ('A', 'Å')],
            Self::Ligature => &[('a', 'æ'), ('A', 'Æ'), ('o', 'œ'), ('O', 'Œ'), ('s', 'ß')],
            Self::Cedilla => &[('c', 'ç'), ('C', 'Ç')],
            Self::Stroke => &[('o', 'ø'), ('O', 'Ø')],
        }
    }

    /// The composed form of `c`, or `None` when this accent has none for it.
    pub fn compose(self, c: char) -> Option<char> {
        self.table().iter().find(|(from, _)| *from == c).map(|(_, to)| *to)
    }
}

/// The accent a dead-key shortcut starts, from its key (`Mod+'` → acute, `Mod+Shift+6` →
/// circumflex). The front end sends it as `text.composeAccent`'s `accent`.
pub fn accent_for_shortcut(key: &str) -> Option<&'static str> {
    Some(match normalize(key)?.as_str() {
        "Mod+'" => "acute",
        "Mod+`" => "grave",
        "Mod+Shift+6" => "circumflex",
        "Mod+Shift+`" => "tilde",
        "Mod+Shift+;" => "diaeresis",
        "Mod+Shift+2" => "ring",
        "Mod+Shift+7" => "ligature",
        "Mod+," => "cedilla",
        "Mod+/" => "stroke",
        _ => return None,
    })
}

/// `text.composeAccent`: wait for the next typed character and compose it with `accent`. The
/// pending accent is session state, not part of the document: no undo step, no unsaved change.
pub(super) fn compose_accent(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "accent")?;
    let accent = Accent::parse(name).ok_or_else(|| CmdError::Params(format!("`accent` must be one of {}", NAMES.join(", "))))?;
    s.pending_accent = Some(accent);
    // The dead key separates the typing before and after it without adding an undo step.
    s.close_typing();
    Ok(json!({"accent": name}))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_doc::StoryRef;

    use super::*;

    fn s() -> Session {
        Session::new(wordcraft_doc::Document::new())
    }

    fn body(s: &Session) -> String {
        s.doc.plain_text(StoryRef::Body).trim_end_matches(['\n', '\r']).to_string()
    }

    #[test]
    fn every_shortcut_names_its_accent() {
        for (key, accent) in [
            ("Mod+'", "acute"),
            ("Mod+`", "grave"),
            ("Mod+Shift+6", "circumflex"),
            ("Mod+Shift+`", "tilde"),
            ("Mod+Shift+;", "diaeresis"),
            ("Mod+Shift+2", "ring"),
            ("Mod+Shift+7", "ligature"),
            ("Mod+,", "cedilla"),
            ("Mod+/", "stroke"),
        ] {
            assert_eq!(accent_for_shortcut(key), Some(accent), "{key}");
        }
        assert_eq!(accent_for_shortcut("Mod+B"), None);
        assert_eq!(accent_for_shortcut("not a key"), None);
    }

    /// The dead key is session state: it adds no undo step and leaves the document unsaved.
    #[test]
    fn a_dead_key_is_not_an_edit() {
        let mut s = s();
        s.run("text.insert", &json!({"text": "a"})).unwrap();
        let steps = s.undo_steps_so_far();
        s.dirty = false;
        let rev = s.rev();
        s.run("text.composeAccent", &json!({"accent": "acute"})).unwrap();
        assert_eq!(s.undo_steps_so_far(), steps, "no undo step");
        assert!(!s.dirty, "not an unsaved change");
        assert_eq!(s.rev(), rev, "no relayout");
        assert_eq!(body(&s), "a");
        // The composed letter is ordinary typing: one more step, and it is undoable.
        s.run("text.insert", &json!({"text": "e"})).unwrap();
        assert_eq!(body(&s), "aé");
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(body(&s), "a");
    }

    /// A composed letter takes the formatting waiting at the caret, like the letter typed directly.
    #[test]
    fn a_composed_letter_takes_the_pending_formatting() {
        let mut s = s();
        s.run("format.bold", &json!({})).unwrap();
        s.run("text.composeAccent", &json!({"accent": "diaeresis"})).unwrap();
        s.run("text.insert", &json!({"text": "u"})).unwrap();
        assert_eq!(body(&s), "ü");
        let at = s.sel.focus.clone();
        let bold = s.doc.para_at(&at).and_then(|p| p.props_of_char(at.off.saturating_sub(1)).bold);
        assert_eq!(bold, Some(true));
    }

    /// A word containing a dead-key composition keeps its first letter as typed.
    #[test]
    fn a_composed_word_is_not_capitalised() {
        let mut s = s();
        s.run("text.composeAccent", &json!({"accent": "acute"})).unwrap();
        for c in ["e", "l", ".", "p", "a", "s", "o", " "] {
            s.run("text.insert", &json!({"text": c})).unwrap();
        }
        assert_eq!(body(&s), "él. Paso");
    }

    /// A multi-character insert ends the wait: the accent applies to one typed character only.
    #[test]
    fn a_multi_character_insert_ends_the_wait() {
        let mut s = s();
        s.run("text.composeAccent", &json!({"accent": "acute"})).unwrap();
        s.run("text.insert", &json!({"text": "ab"})).unwrap();
        s.run("text.insert", &json!({"text": "e"})).unwrap();
        assert_eq!(body(&s), "abe");
    }

    #[test]
    fn a_pending_accent_is_not_saved_in_an_edit_snapshot() {
        let mut s = s();
        s.run("text.composeAccent", &json!({"accent": "acute"})).unwrap();
        let snapshot = s.edit_snapshot();
        s.run("caret.right", &json!({})).unwrap();
        s.restore(snapshot);
        s.run("text.insert", &json!({"text": "e"})).unwrap();
        assert_eq!(body(&s), "e");
    }
}
