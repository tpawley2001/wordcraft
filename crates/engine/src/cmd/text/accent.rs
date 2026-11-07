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
            Self::Grave => &[
                ('a', 'à'),
                ('e', 'è'),
                ('i', 'ì'),
                ('o', 'ò'),
                ('u', 'ù'),
                ('A', 'À'),
                ('E', 'È'),
                ('I', 'Ì'),
                ('O', 'Ò'),
                ('U', 'Ù'),
            ],
            Self::Circumflex => &[
                ('a', 'â'),
                ('e', 'ê'),
                ('i', 'î'),
                ('o', 'ô'),
                ('u', 'û'),
                ('A', 'Â'),
                ('E', 'Ê'),
                ('I', 'Î'),
                ('O', 'Ô'),
                ('U', 'Û'),
            ],
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
    Ok(json!({"accent": name}))
}
