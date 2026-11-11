//! Document formatting `\revisions` (Track Changes on) and `\printdata` (Print only the data from a
//! form), RTF 1.9.1: the RTF form of DOCX `w:trackRevisions` and `w:printFormsData`.

use super::*;

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

/// (track changes, print forms data) of `rtf` once opened.
fn flags(rtf: &str) -> (bool, bool) {
    let s = doc(rtf).settings;
    (s.track_changes, s.print_forms_data)
}

/// A one-paragraph document with the two settings as given.
fn with(track: bool, print_data: bool) -> Document {
    let mut d = doc(r"{\rtf1\ansi\sectd a\par}");
    d.settings.track_changes = track;
    d.settings.print_forms_data = print_data;
    d
}

#[test]
fn no_words_keep_both_off() {
    assert_eq!(flags(r"{\rtf1\ansi\sectd a\par}"), (false, false));
}

#[test]
fn revisions_turns_track_changes_on() {
    assert_eq!(flags(r"{\rtf1\ansi\revisions\sectd a\par}"), (true, false));
    assert_eq!(flags(r"{\rtf1\ansi\revisions1\sectd a\par}"), (true, false));
    assert_eq!(flags(r"{\rtf1\ansi\revisions\revisions0\sectd a\par}"), (false, false));
}

#[test]
fn printdata_turns_print_forms_data_on() {
    assert_eq!(flags(r"{\rtf1\ansi\printdata\sectd a\par}"), (false, true));
    assert_eq!(flags(r"{\rtf1\ansi\printdata0\sectd a\par}"), (false, false));
    assert_eq!(flags(r"{\rtf1\ansi\revisions\printdata\sectd a\par}"), (true, true));
}

#[test]
fn words_in_an_unknown_destination_are_ignored() {
    assert_eq!(flags(r"{\rtf1\ansi{\*\wcunknowndest \revisions\printdata}\sectd a\par}"), (false, false));
}

#[test]
fn export_writes_the_words_that_are_on() {
    let both = export(&with(true, true));
    assert!(both.contains("\\revisions"), "{both}");
    assert!(both.contains("\\printdata"), "{both}");
    let first_sectd = both.find("\\sectd").expect("a section");
    assert!(both.find("\\revisions").unwrap() < first_sectd, "\\revisions is document formatting: {both}");
    assert!(both.find("\\printdata").unwrap() < first_sectd, "\\printdata is document formatting: {both}");

    let track_only = export(&with(true, false));
    assert!(track_only.contains("\\revisions") && !track_only.contains("\\printdata"), "{track_only}");
    let print_only = export(&with(false, true));
    assert!(print_only.contains("\\printdata") && !print_only.contains("\\revisions"), "{print_only}");
}

#[test]
fn export_of_a_default_document_writes_neither() {
    let out = export(&with(false, false));
    assert!(!out.contains("\\revisions") && !out.contains("\\printdata"), "{out}");
}

#[test]
fn both_survive_a_round_trip() {
    for (track, print_data) in [(true, false), (false, true), (true, true), (false, false)] {
        let back = doc(&export(&with(track, print_data)));
        assert_eq!((back.settings.track_changes, back.settings.print_forms_data), (track, print_data));
    }
}
