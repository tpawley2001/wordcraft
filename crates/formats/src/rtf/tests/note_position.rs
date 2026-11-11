//! A `{\footnote}` in a file that never says where footnotes go (no document-level `\ftnbj` or
//! `\ftntj`, `\fet` absent or 0) is an endnote in Word, numbered i, ii; and saved RTF always says
//! where footnotes go, so Word reads WordCraft's footnotes as footnotes. Measured in Microsoft 365
//! Word (one probe per row of the task's table).

use super::*;
use wordcraft_doc::para::NoteKind;
use wordcraft_doc::section::{NotePos, NumFormat};
use wordcraft_doc::{Document, PartKind};

const NOTES: &str =
    r"\pard First{\super\chftn}{\footnote\pard {\super\chftn} NoteA.}\par Last{\super\chftn}{\footnote\pard {\super\chftn} NoteB.}\par";

fn open(words: &str, body: &str) -> Document {
    import(format!(r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{words}{body}}}").as_bytes()).unwrap()
}

/// The kinds of the note references in the body, in order.
fn kinds(d: &Document) -> Vec<NoteKind> {
    d.body
        .iter()
        .filter_map(|b| b.as_para())
        .flat_map(|p| p.objects.iter())
        .filter_map(|o| if let InlineObject::NoteRef { kind, .. } = o { Some(*kind) } else { None })
        .collect()
}

fn parts(d: &Document, kind: PartKind) -> usize {
    d.parts.values().filter(|p| p.kind == kind).count()
}

#[test]
fn no_position_words_make_endnotes() {
    for words in ["", r"\fet0", r"\endnotes", r"\enddoc", r"\ftnnar", r"\sectd\sftnbj"] {
        let d = open(words, NOTES);
        assert_eq!(kinds(&d), [NoteKind::Endnote, NoteKind::Endnote], "{words:?}");
        assert_eq!((parts(&d, PartKind::Endnote), parts(&d, PartKind::Footnote)), (2, 0), "{words:?}");
        assert_eq!(d.settings.endnote_format, NumFormat::LowerRoman, "{words:?}");
    }
}

#[test]
fn converted_notes_ignore_the_endnote_number_format() {
    // Word numbered these i, ii although the file says `\aftnnar`.
    let d = open(r"\aftnnar", NOTES);
    assert_eq!(kinds(&d), [NoteKind::Endnote, NoteKind::Endnote]);
    assert_eq!(d.settings.endnote_format, NumFormat::LowerRoman);
}

#[test]
fn notes_in_every_section_convert() {
    let body = r"\pard SecOne{\super\chftn}{\footnote\pard {\super\chftn} NoteA.}\par\sect\sectd\sbkpage\pard SecTwo{\super\chftn}{\footnote\pard {\super\chftn} NoteB.}\par";
    assert_eq!(kinds(&open("", body)), [NoteKind::Endnote, NoteKind::Endnote]);
}

#[test]
fn a_position_word_or_fet_keeps_footnotes() {
    for words in [r"\ftnbj", r"\ftntj", r"\fet1\aendnotes", r"\fet2\aenddoc"] {
        let d = open(words, NOTES);
        assert_eq!(kinds(&d), [NoteKind::Footnote, NoteKind::Footnote], "{words:?}");
        assert_eq!((parts(&d, PartKind::Footnote), parts(&d, PartKind::Endnote)), (2, 0), "{words:?}");
    }
}

#[test]
fn fet2_keeps_both_kinds() {
    let body =
        r"\pard First{\super\chftn}{\footnote\pard {\super\chftn} NoteA.}\par Last{\super\chftn}{\footnote\ftnalt\pard {\super\chftn} NoteB.}\par";
    for words in [r"\fet2\aenddoc", r"\fet2\ftnbj\aenddoc"] {
        assert_eq!(kinds(&open(words, body)), [NoteKind::Footnote, NoteKind::Endnote], "{words:?}");
    }
}

#[test]
fn ftnalt_is_an_endnote_either_way() {
    let body = r"\pard A{\super\chftn}{\footnote\ftnalt\pard {\super\chftn} E.}\par";
    assert_eq!(kinds(&open("", body)), [NoteKind::Endnote]);
    assert_eq!(kinds(&open(r"\ftnbj", body)), [NoteKind::Endnote]);
}

#[test]
fn export_states_the_positions_when_there_are_notes() {
    // A footnote with no footnote options (as from a DOCX without w:footnotePr).
    let d = open(r"\ftnbj", NOTES);
    let mut d2 = d.clone();
    d2.settings.footnote_pr.pos = None;
    d2.settings.endnote_pr.pos = None;
    let rtf = export(&d2);
    for w in [r"\ftnbj", r"\aenddoc", r"\fet0"] {
        assert!(rtf.contains(w), "{w} missing");
    }
    assert!(!rtf.contains(r"\fet2") && !rtf.contains(r"\ftntj"));
    // Read back: still footnotes (Word reads them so too).
    assert_eq!(kinds(&import(rtf.as_bytes()).unwrap()), [NoteKind::Footnote, NoteKind::Footnote]);
}

#[test]
fn export_writes_fet2_with_endnotes_and_keeps_set_positions() {
    let body =
        r"\pard First{\super\chftn}{\footnote\pard {\super\chftn} NoteA.}\par Last{\super\chftn}{\footnote\ftnalt\pard {\super\chftn} NoteB.}\par";
    let mut d = open(r"\ftnbj", body);
    d.settings.footnote_pr.pos = Some(NotePos::BeneathText);
    d.settings.endnote_pr.pos = Some(NotePos::SectEnd);
    let rtf = export(&d);
    for w in [r"\ftntj", r"\aendnotes", r"\fet2"] {
        assert!(rtf.contains(w), "{w} missing");
    }
    assert!(!rtf.contains(r"\ftnbj") && !rtf.contains(r"\aenddoc") && !rtf.contains(r"\fet0"));
    assert_eq!(kinds(&import(rtf.as_bytes()).unwrap()), [NoteKind::Footnote, NoteKind::Endnote]);
}

#[test]
fn only_endnotes_export_still_says_footnotes_at_the_bottom() {
    let body = r"\pard A{\super\chftn}{\footnote\ftnalt\pard {\super\chftn} E.}\par";
    let mut d = open("", body);
    d.settings.footnote_pr.pos = None;
    let rtf = export(&d);
    assert!(rtf.contains(r"\ftnbj") && rtf.contains(r"\fet2"), "{rtf}");
    assert_eq!(kinds(&import(rtf.as_bytes()).unwrap()), [NoteKind::Endnote]);
}
