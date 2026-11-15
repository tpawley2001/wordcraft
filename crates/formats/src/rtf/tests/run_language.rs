//! Text languages, RTF 1.9.1: `\langN` / `\langnpN` (Latin text), `\langfeN` / `\langfenpN` (East Asian
//! text), `\alangN` (complex-script text) and the document defaults `\deflangN`, `\deflangfeN`,
//! `\adeflangN`, read into the run's `lang` / `lang_ea` / `lang_bidi` (DOCX `w:lang`) and written back.
//! N is a Windows LCID ([MS-LCID] 2.2): 1033 en-US, 1031 de-DE, 1036 fr-FR, 2052 zh-CN, 1041 ja-JP,
//! 1025 ar-SA, 1037 he-IL; 1024 means "no language".

use super::*;
use wordcraft_doc::Document;
use wordcraft_doc::props::CharProps;

fn open_with(header: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{header}{{\fonttbl{{\f0 Times New Roman;}}}}{body}}}").as_bytes()).unwrap()
}

fn open(body: &str) -> Document {
    open_with("", body)
}

/// The properties of the run holding `needle` in paragraph 0.
fn run(d: &Document, needle: &str) -> CharProps {
    let p = d.body[0].as_para().unwrap();
    let chars: Vec<char> = p.text.chars().collect();
    let mut at = 0;
    for r in &p.runs {
        let s: String = chars[at..(at + r.len).min(chars.len())].iter().collect();
        at += r.len;
        if s.contains(needle) {
            return r.props.clone();
        }
    }
    panic!("no run with {needle:?} in {:?}", p.text);
}

fn langs(d: &Document, needle: &str) -> (Option<String>, Option<String>, Option<String>) {
    let c = run(d, needle);
    (c.lang, c.lang_ea, c.lang_bidi)
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

#[test]
fn run_languages_are_read() {
    let d = open(
        r"\pard{\lang1031 Deutsch}{\lang1036\langnp1036 Francais}{\langfe2052 Han}{\alang1025 Arab}{\langfenp1041 Kana}rest\par",
    );
    assert_eq!(langs(&d, "Deutsch"), (s("de-DE"), None, None));
    assert_eq!(langs(&d, "Francais"), (s("fr-FR"), None, None));
    assert_eq!(langs(&d, "Han"), (None, s("zh-CN"), None));
    assert_eq!(langs(&d, "Arab"), (None, None, s("ar-SA")));
    assert_eq!(langs(&d, "Kana"), (None, s("ja-JP"), None));
    assert_eq!(langs(&d, "rest"), (None, None, None));
}

#[test]
fn langnp_alone_sets_the_language() {
    let d = open(r"\pard{\langnp1031 Nur}rest\par");
    assert_eq!(run(&d, "Nur").lang.as_deref(), Some("de-DE"));
}

#[test]
fn no_language_and_unknown_lcids_leave_it_unset() {
    let d = open(r"\pard{\lang1024 None}{\lang7 Junk}{\lang1031{\lang1024 Inner}}rest\par");
    assert_eq!(run(&d, "None").lang, None);
    assert_eq!(run(&d, "Junk").lang, None);
    assert_eq!(run(&d, "Inner").lang, None);
}

#[test]
fn plain_resets_and_groups_scope_the_language() {
    let d = open(r"\pard{\lang1031 Ein{\lang1036 Deux}Zwei}{\lang1031\plain Plain}rest\par");
    assert_eq!(run(&d, "Ein").lang.as_deref(), Some("de-DE"));
    assert_eq!(run(&d, "Deux").lang.as_deref(), Some("fr-FR"));
    assert_eq!(run(&d, "Zwei").lang.as_deref(), Some("de-DE"));
    assert_eq!(run(&d, "Plain").lang, None);
    assert_eq!(run(&d, "rest").lang, None);
}

#[test]
fn document_default_languages_are_read() {
    let d = open_with(r"\deflang1031\deflangfe2052\adeflang1037", r"\pard text\par");
    assert_eq!(d.styles.default_chr.lang.as_deref(), Some("de-DE"));
    assert_eq!(d.styles.default_chr.lang_ea.as_deref(), Some("zh-CN"));
    assert_eq!(d.styles.default_chr.lang_bidi.as_deref(), Some("he-IL"));
}

#[test]
fn an_unknown_default_language_keeps_the_existing_default() {
    let before = open(r"\pard text\par").styles.default_chr.lang.clone();
    let d = open_with(r"\deflang1024", r"\pard text\par");
    assert_eq!(d.styles.default_chr.lang, before);
}

#[test]
fn languages_survive_export() {
    let d = open_with(
        r"\deflang1031\deflangfe1041",
        r"\pard{\lang1036 Francais}{\langfe2052 Han}{\alang1025 Arab}rest\par",
    );
    let out = export(&d);
    for w in ["\\deflang1031", "\\deflangfe1041", "\\lang1036", "\\langfe2052", "\\alang1025"] {
        assert!(out.contains(w), "{w} missing: {out}");
    }
    let again = import(out.as_bytes()).unwrap();
    assert_eq!(again.styles.default_chr.lang.as_deref(), Some("de-DE"));
    assert_eq!(again.styles.default_chr.lang_ea.as_deref(), Some("ja-JP"));
    assert_eq!(run(&again, "Francais").lang.as_deref(), Some("fr-FR"));
    assert_eq!(run(&again, "Han").lang_ea.as_deref(), Some("zh-CN"));
    assert_eq!(run(&again, "Arab").lang_bidi.as_deref(), Some("ar-SA"));
    let rest = run(&again, "rest");
    assert!(rest.lang.is_none() || rest.lang.as_deref() == Some("de-DE"), "{:?}", rest.lang);
}

#[test]
fn a_document_without_languages_writes_the_english_default_only() {
    let d = open(r"\pard plain text\par");
    let out = export(&d);
    assert!(out.contains("\\deflang1033"), "{out}");
    for w in ["\\lang", "\\langfe", "\\alang"] {
        let hits = out.matches(w).count();
        assert_eq!(hits, 0, "{w} written {hits} times: {out}");
    }
}
