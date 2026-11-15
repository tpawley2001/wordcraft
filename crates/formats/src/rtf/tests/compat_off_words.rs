//! `\dntblnsbdb` (don't balance single-byte and double-byte characters) and `\lnbrkrule` (don't
//! use Word 97 line breaking rules) turn off the two compatibility options Word turns on for every
//! RTF document: `Settings::balance_sbcs_dbcs_width` and `Settings::use_word97_line_break_rules`
//! (Microsoft 365 Word, 2026-11-14: each probe saved as .docx loses exactly that `w:compat` child).

use super::*;
use wordcraft_doc::Settings;

fn doc_with(words: &str) -> String {
    format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{words}\pard\plain\f0\fs24 Some text.\par}}")
}

fn has_word(out: &str, word: &str) -> bool {
    let pat = format!("\\{word}");
    out.match_indices(&pat).any(|(i, _)| !out[i + pat.len()..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '0'))
}

/// (balance, Word 97 rules)
fn pair(s: &Settings) -> (bool, bool) {
    (s.balance_sbcs_dbcs_width, s.use_word97_line_break_rules)
}

#[test]
fn each_word_turns_its_option_off() {
    for (words, want) in [
        ("", (true, true)),
        (r"\dntblnsbdb", (false, true)),
        (r"\lnbrkrule", (true, false)),
        (r"\dntblnsbdb\lnbrkrule", (false, false)),
        (r"\dntblnsbdb0", (true, true)),
        (r"\lnbrkrule0", (true, true)),
    ] {
        let d = import(doc_with(words).as_bytes()).unwrap();
        assert_eq!(pair(&d.settings), want, "{words:?}");
    }
}

#[test]
fn the_words_count_only_as_document_formatting() {
    for rtf in [
        r"{\rtf1\ansi{\stylesheet{\s0\dntblnsbdb\lnbrkrule Normal;}}\pard Text.\par}",
        r"{\rtf1\ansi{\header\dntblnsbdb\lnbrkrule\pard Head\par}\pard Text.\par}",
    ] {
        let d = import(rtf.as_bytes()).unwrap();
        assert_eq!(pair(&d.settings), (true, true), "{rtf}");
    }
}

#[test]
fn an_option_turned_off_is_written_and_reopens_off() {
    for (bal, w97) in [(false, true), (true, false), (false, false)] {
        let mut d = import(doc_with("").as_bytes()).unwrap();
        d.settings.balance_sbcs_dbcs_width = bal;
        d.settings.use_word97_line_break_rules = w97;
        let out = export(&d);
        assert_eq!(has_word(&out, "dntblnsbdb"), !bal, "({bal}, {w97}): {out}");
        assert_eq!(has_word(&out, "lnbrkrule"), !w97, "({bal}, {w97}): {out}");
        let back = import(out.as_bytes()).unwrap();
        assert_eq!(pair(&back.settings), (bal, w97), "round trip: {out}");
    }
}

#[test]
fn options_left_on_write_nothing() {
    let d = import(doc_with("").as_bytes()).unwrap();
    let out = export(&d);
    assert!(!has_word(&out, "dntblnsbdb") && !has_word(&out, "lnbrkrule"), "{out}");
}
