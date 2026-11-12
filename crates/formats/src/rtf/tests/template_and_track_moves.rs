//! The attached template (`{\*\template …}`) and Track moves (`\trackmovesN`), RTF 1.9.1: the RTF
//! form of DOCX `w:attachedTemplate` and `w:doNotTrackMoves`.

use super::*;

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

/// The attached template of `rtf` once opened.
fn template(rtf: &str) -> Option<String> {
    doc(rtf).settings.attached_template
}

/// Track moves of `rtf` once opened.
fn moves(rtf: &str) -> bool {
    doc(rtf).settings.track_moves
}

/// A one-paragraph document with the two settings as given.
fn with(template: Option<&str>, track_moves: bool) -> Document {
    let mut d = doc(r"{\rtf1\ansi\sectd a\par}");
    d.settings.attached_template = template.map(str::to_string);
    d.settings.track_moves = track_moves;
    d
}

/// The document's text, all paragraphs joined.
fn text(d: &Document) -> String {
    d.plain_text(wordcraft_doc::StoryRef::Body)
}

#[test]
fn no_template_group_leaves_none() {
    assert_eq!(template(r"{\rtf1\ansi\sectd a\par}"), None);
}

#[test]
fn template_group_is_read_with_single_backslashes() {
    assert_eq!(template(r"{\rtf1\ansi{\*\template C:\\Templates\\Report.dotx}\sectd a\par}").as_deref(), Some(r"C:\Templates\Report.dotx"));
}

#[test]
fn template_group_without_the_star_is_read() {
    assert_eq!(template(r"{\rtf1\ansi{\template Letter.dotm}\sectd a\par}").as_deref(), Some("Letter.dotm"));
}

#[test]
fn empty_or_blank_template_is_none() {
    assert_eq!(template(r"{\rtf1\ansi{\*\template   }\sectd a\par}"), None);
    assert_eq!(template(r"{\rtf1\ansi{\*\template}\sectd a\par}"), None);
}

#[test]
fn template_escapes_are_decoded() {
    assert_eq!(template(r"{\rtf1\ansi\ansicpg1252\uc1{\*\template Caf\'e9 \u9731?.dotx}\sectd a\par}").as_deref(), Some("Caf\u{e9} \u{2603}.dotx"));
}

#[test]
fn last_non_empty_template_wins() {
    assert_eq!(template(r"{\rtf1\ansi{\*\template A.dotx}{\*\template B.dotx}{\*\template  }\sectd a\par}").as_deref(), Some("B.dotx"));
}

#[test]
fn template_in_a_skipped_destination_is_ignored() {
    assert_eq!(template(r"{\rtf1\ansi{\*\wcunknowndest {\*\template A.dotx}}\sectd a\par}"), None);
}

#[test]
fn template_text_stays_out_of_the_body() {
    let d = doc(r"{\rtf1\ansi{\*\template {\b Zq}Template.dotx}\sectd body\par}");
    assert_eq!(d.settings.attached_template.as_deref(), Some("ZqTemplate.dotx"));
    let body = text(&d);
    assert!(body.contains("body") && !body.contains("Zq") && !body.contains("dotx"), "{body:?}");
}

#[test]
fn overlong_template_is_cut_to_4096_characters() {
    let rtf = format!(r"{{\rtf1\ansi{{\*\template {}}}\sectd a\par}}", "x".repeat(5000));
    assert_eq!(template(&rtf).map(|t| t.chars().count()), Some(4096));
}

#[test]
fn track_moves_defaults_on_and_reads_the_word() {
    assert!(moves(r"{\rtf1\ansi\sectd a\par}"));
    assert!(!moves(r"{\rtf1\ansi\trackmoves0\sectd a\par}"));
    assert!(moves(r"{\rtf1\ansi\trackmoves1\sectd a\par}"));
    assert!(moves(r"{\rtf1\ansi\trackmoves\sectd a\par}"));
    assert!(moves(r"{\rtf1\ansi\trackmoves0\trackmoves1\sectd a\par}"));
}

#[test]
fn track_moves_in_a_skipped_destination_is_ignored() {
    assert!(moves(r"{\rtf1\ansi{\*\wcunknowndest \trackmoves0}\sectd a\par}"));
}

#[test]
fn export_writes_the_template_before_the_first_section() {
    let out = export(&with(Some(r"C:\Templates\Report.dotx"), true));
    let at = out.find(r"{\*\template C:\\Templates\\Report.dotx}").unwrap_or_else(|| panic!("no template group: {out}"));
    assert!(at < out.find("\\sectd").expect("a section"), "{out}");
    assert_eq!(out.matches("\\template").count(), 1, "{out}");
    assert!(!out.contains("\\trackmoves"), "{out}");
}

#[test]
fn export_without_a_template_writes_no_group() {
    for none in [None, Some(""), Some("   ")] {
        let out = export(&with(none, true));
        assert!(!out.contains("\\template"), "{out}");
    }
}

#[test]
fn export_writes_trackmoves0_only_when_off() {
    let off = export(&with(None, false));
    let at = off.find("\\trackmoves0").unwrap_or_else(|| panic!("no \\trackmoves0: {off}"));
    assert!(at < off.find("\\sectd").expect("a section"), "{off}");
    assert_eq!(off.matches("\\trackmoves").count(), 1, "{off}");
}

#[test]
fn both_survive_a_round_trip() {
    for (t, m) in [(Some("C:\\T\\Caf\u{e9} {x}.dotm"), false), (Some("Letter.dotx"), true), (None, false), (None, true)] {
        let back = doc(&export(&with(t, m)));
        assert_eq!((back.settings.attached_template.as_deref(), back.settings.track_moves), (t, m));
    }
}
