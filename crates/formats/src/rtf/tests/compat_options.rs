//! Four Word compatibility options the document model stores are read from and written to RTF's
//! document formatting properties (RTF 1.9.1, "Document Formatting Properties"):
//! `\notabind` (no automatic tab stop at a hanging indent), `\truncex` (don't add leading between
//! rows of text), `\sprslnsp` (suppress extra line spacing like WordPerfect 5.x), and `\wpjst`
//! (full justification like WordPerfect 6.x). `\spltpgpar` works the other way round: see
//! `compat_defaults.rs`.

use super::*;
use wordcraft_doc::Settings;

const WORDS: [&str; 4] = ["notabind", "truncex", "sprslnsp", "wpjst"];

fn field(s: &Settings, word: &str) -> bool {
    match word {
        "notabind" => s.no_tab_hang_ind,
        "truncex" => s.no_leading,
        "sprslnsp" => s.ext_leading_2pt_only,
        "wpjst" => s.wp_justification,
        _ => false,
    }
}

fn set_field(s: &mut Settings, word: &str) {
    match word {
        "notabind" => s.no_tab_hang_ind = true,
        "truncex" => s.no_leading = true,
        "sprslnsp" => s.ext_leading_2pt_only = true,
        "wpjst" => s.wp_justification = true,
        _ => {}
    }
}

fn doc_with(words: &str) -> String {
    format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{words}\pard\plain\f0\fs24 Some text.\par}}")
}

fn has_word(out: &str, word: &str) -> bool {
    let pat = format!("\\{word}");
    out.match_indices(&pat).any(|(i, _)| !out[i + pat.len()..].starts_with(|c: char| c.is_ascii_alphabetic()))
}

#[test]
fn each_word_turns_its_option_on() {
    for w in WORDS {
        let d = import(doc_with(&format!("\\{w}")).as_bytes()).unwrap();
        for other in WORDS {
            assert_eq!(field(&d.settings, other), other == w, "\\{w}: {other}");
        }
    }
}

#[test]
fn without_the_word_or_with_zero_the_option_is_off() {
    let d = import(doc_with("").as_bytes()).unwrap();
    for w in WORDS {
        assert!(!field(&d.settings, w), "{w}");
        let d0 = import(doc_with(&format!("\\{w}0")).as_bytes()).unwrap();
        assert!(!field(&d0.settings, w), "\\{w}0");
    }
}

#[test]
fn a_set_option_is_written_and_reopens_set() {
    for w in WORDS {
        let mut d = import(doc_with("").as_bytes()).unwrap();
        set_field(&mut d.settings, w);
        let out = export(&d);
        assert!(has_word(&out, w), "\\{w} missing: {out}");
        let back = import(out.as_bytes()).unwrap();
        for other in WORDS {
            assert_eq!(field(&back.settings, other), other == w, "\\{w} round trip: {other}");
        }
    }
}

#[test]
fn options_left_off_write_nothing() {
    let d = import(doc_with("").as_bytes()).unwrap();
    let out = export(&d);
    for w in WORDS {
        assert!(!has_word(&out, w), "\\{w} written: {out}");
    }
}
