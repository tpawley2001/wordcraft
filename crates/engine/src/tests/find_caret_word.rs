//! Find what `^` codes in a normal search (Use wildcards off) as Word reads them: `^nnn` without a
//! leading 0 is a code 1–127 only, `^unnnn` is any Unicode character, and `^?` and `^p` reach every
//! paragraph mark, the last one included. Expected results measured in Microsoft 365 Word 16.0
//! (Find.Execute with MatchWildcards and MatchCase off, Replace All with `[^&]`), 2026-11-14.

use serde_json::json;

use crate::Session;

/// The body's paragraphs after Replace All of `pattern` with `[^&]` in `text` (`\n` = new paragraph).
fn bracket(text: &str, pattern: &str) -> Vec<String> {
    let mut s = Session::new(wordcraft_doc::Document::new());
    s.run("document.setText", &json!({ "text": text })).unwrap();
    s.run("edit.replaceAll", &json!({ "text": pattern, "with": "[^&]" })).unwrap();
    s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect()
}

const CODES: &str = "e\u{2013}d \u{20AC} q\u{2019}s \u{96}x \u{E9} z\u{7F}y Ab \u{A9}";

#[test]
fn codes_up_to_127_without_a_leading_zero() {
    assert_eq!(bracket(CODES, "^127"), ["e\u{2013}d \u{20AC} q\u{2019}s \u{96}x \u{E9} z[\u{7F}]y Ab \u{A9}"]);
    for pattern in ["^65", "^065", "^0065", "^u65"] {
        assert_eq!(bracket(CODES, pattern), ["e\u{2013}d \u{20AC} q\u{2019}s \u{96}x \u{E9} z\u{7F}y [A]b \u{A9}"], "{pattern}");
    }
    assert_eq!(bracket("a\tb", "^9"), ["a[\t]b"]);
}

#[test]
fn codes_from_128_without_a_leading_zero_find_nothing() {
    for pattern in ["^128", "^150", "^169", "^233", "^0", "^00169"] {
        assert_eq!(bracket(CODES, pattern), [CODES], "{pattern}");
    }
}

#[test]
fn ansi_and_unicode_codes() {
    assert_eq!(bracket(CODES, "^0150"), ["e[\u{2013}]d \u{20AC} q\u{2019}s \u{96}x \u{E9} z\u{7F}y Ab \u{A9}"]);
    assert_eq!(bracket(CODES, "^0128"), ["e\u{2013}d [\u{20AC}] q\u{2019}s \u{96}x \u{E9} z\u{7F}y Ab \u{A9}"]);
    assert_eq!(bracket(CODES, "^0233"), ["e\u{2013}d \u{20AC} q\u{2019}s \u{96}x [\u{E9}] z\u{7F}y Ab \u{A9}"]);
    assert_eq!(bracket(CODES, "^u150"), ["e\u{2013}d \u{20AC} q\u{2019}s [\u{96}]x \u{E9} z\u{7F}y Ab \u{A9}"]);
    for pattern in ["^u169", "^u0169", "^u00169"] {
        assert_eq!(bracket(CODES, pattern), ["e\u{2013}d \u{20AC} q\u{2019}s \u{96}x \u{E9} z\u{7F}y Ab [\u{A9}]"], "{pattern}");
    }
    assert_eq!(bracket(CODES, "^u8211"), ["e[\u{2013}]d \u{20AC} q\u{2019}s \u{96}x \u{E9} z\u{7F}y Ab \u{A9}"]);
}

#[test]
fn any_character_takes_paragraph_marks_but_not_optional_hyphens() {
    assert_eq!(bracket("s\u{AD}t", "^?"), ["[s]\u{AD}[t]"]);
    assert_eq!(bracket("ab\ncd", "^?"), ["[a][b][", "][c][d][]"]);
}

#[test]
fn the_last_paragraph_mark_is_found_but_kept() {
    assert_eq!(bracket("ab\ncd", "^p"), ["ab[", "]cd[]"]);
    assert_eq!(bracket("ab\ncd", "^?^p"), ["a[b", "]c[d]"]);
}
