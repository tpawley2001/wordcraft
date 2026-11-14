//! AutoCorrect's built-in symbol entries as Microsoft 365 Word 16.0.20527 (English, US) has them: read from
//! `Application.AutoCorrect.Entries` and applied with `Entry.Apply`, 2026-11-14. Word's Wingdings arrows are stored as
//! U+F000 + the font's code in a "Wingdings" run, as Insert › Symbol stores them.

use serde_json::json;
use wordcraft_doc::{Document, Pos, StoryRef};

use crate::Session;

/// Type `keys` one character at a time into an empty document.
fn typed(keys: &str) -> Session {
    let mut s = Session::new(Document::new());
    for c in keys.chars() {
        s.run("text.insert", &json!({"text": c.to_string()})).unwrap();
    }
    s
}

fn body(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body)
}

/// The font set on the character at byte `off` of the first paragraph.
fn font_at(s: &Session, off: usize) -> Option<String> {
    s.doc.para_at(&Pos::body(0, 0)).unwrap().props_of_char(off).font.clone()
}

#[test]
fn plain_symbol_entries_match_word() {
    for (typed_text, want) in [
        ("so (c) ", "So \u{A9} "),
        ("so (r) ", "So \u{AE} "),
        ("so (tm) ", "So \u{2122} "),
        ("so (e) ", "So \u{20AC} "),
    ] {
        assert_eq!(body(&typed(typed_text)), want, "typed {typed_text:?}");
    }
}

#[test]
fn smileys_match_word() {
    for (typed_text, want) in [
        ("so :) ", "So \u{1F60A} "),
        ("so :-) ", "So \u{1F60A} "),
        ("so ;) ", "So \u{1F609} "),
        ("so ;-) ", "So \u{1F609} "),
        ("so :( ", "So \u{2639} "),
        ("so :-( ", "So \u{2639} "),
        ("so :| ", "So \u{1F610} "),
        ("so :-| ", "So \u{1F610} "),
        ("so ok;) ", "So ok\u{1F609} "),
    ] {
        let s = typed(typed_text);
        assert_eq!(body(&s), want, "typed {typed_text:?}");
        assert_eq!(font_at(&s, 3), None, "no font is set on the smiley of {typed_text:?}");
    }
}

#[test]
fn arrows_are_word_wingdings_symbols() {
    for (typed_text, sym) in [
        ("so --> x", '\u{F0E0}'),
        ("so <-- x", '\u{F0DF}'),
        ("so ==> x", '\u{F0E8}'),
        ("so <== x", '\u{F0E7}'),
        ("so <=> x", '\u{F0F3}'),
    ] {
        let s = typed(typed_text);
        let text = body(&s);
        assert_eq!(text, format!("So {sym} x"), "typed {typed_text:?}");
        let at = text.find(sym).unwrap();
        assert_eq!(font_at(&s, at).as_deref(), Some("Wingdings"), "the arrow of {typed_text:?} is in Wingdings");
        let x = text.find('x').unwrap();
        assert_eq!(font_at(&s, x), None, "text typed after the arrow of {typed_text:?} keeps its font");
    }
}

#[test]
fn arrows_word_has_no_entry_for_stay_as_typed() {
    for t in ["so -> x", "so <- x", "so => x"] {
        assert_eq!(body(&typed(t)), format!("S{}", &t[1..]), "Word has no AutoCorrect entry in {t:?}");
    }
}
