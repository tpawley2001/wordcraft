//! Document formatting `\formshade` (Shade form fields, RTF 1.9.1): the RTF form of DOCX
//! `w:doNotShadeFormData` (ECMA-376 Part 1 §17.15.1.42), with the opposite sense. A flag word, so a
//! document without it has form field shading off.

use super::*;

fn off(rtf: &str) -> bool {
    import(rtf.as_bytes()).unwrap().settings.do_not_shade_form_data
}

fn with(do_not_shade: bool) -> Document {
    let mut d = import(br"{\rtf1\ansi\formshade\sectd a\par}").unwrap();
    d.settings.do_not_shade_form_data = do_not_shade;
    d
}

#[test]
fn formshade_turns_shading_on() {
    assert!(!off(r"{\rtf1\ansi\formshade\sectd a\par}"));
    assert!(!off(r"{\rtf1\ansi\formshade1\sectd a\par}"));
}

#[test]
fn no_word_or_formshade0_leaves_shading_off() {
    assert!(off(r"{\rtf1\ansi\sectd a\par}"));
    assert!(off(r"{\rtf1\ansi\formshade0\sectd a\par}"));
    assert!(off(r"{\rtf1\ansi\formshade\formshade0\sectd a\par}"));
}

#[test]
fn word_in_an_unknown_destination_is_ignored() {
    assert!(off(r"{\rtf1\ansi{\*\wcunknowndest \formshade}\sectd a\par}"));
}

#[test]
fn export_writes_formshade_only_when_shading_is_on() {
    let on = export(&with(false));
    assert_eq!(on.matches("\\formshade").count(), 1, "{on}");
    let first_sectd = on.find("\\sectd").expect("a section");
    assert!(on.find("\\formshade").unwrap() < first_sectd, "document formatting: {on}");
    let not = export(&with(true));
    assert!(!not.contains("\\formshade"), "{not}");
}

#[test]
fn export_then_import_keeps_the_setting() {
    for v in [false, true] {
        let back = import(export(&with(v)).as_bytes()).unwrap();
        assert_eq!(back.settings.do_not_shade_form_data, v, "{v}");
    }
}
