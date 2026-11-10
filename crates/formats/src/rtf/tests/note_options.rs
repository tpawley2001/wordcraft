//! Footnote and endnote options, RTF 1.9.1: document level (`\ftnbj`/`\ftntj`, `\aendnotes`/`\aenddoc`,
//! `\ftnstartN`, `\aftnstartN`, `\ftnrstpg|restart|rstcont`, `\aftnrestart|rstcont`, `\ftnn*`, `\aftnn*`)
//! and section level (`\sftn*`, `\saftn*`): the RTF form of DOCX `w:footnotePr` / `w:endnotePr`.

use super::*;
use wordcraft_doc::section::{NotePos, NoteProps, NoteRestart, NumFormat};
use wordcraft_doc::{Block, SectionProps};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

fn last(rtf: &str) -> SectionProps {
    doc(rtf).last_section.clone()
}

fn first_section(d: &Document) -> SectionProps {
    d.body
        .iter()
        .find_map(|b| match b.as_ref() {
            Block::Para(p) => p.section.as_deref().cloned(),
            _ => None,
        })
        .expect("first section")
}

const FORMATS: [(&str, NumFormat); 6] = [
    ("nar", NumFormat::Decimal),
    ("nalc", NumFormat::LowerLetter),
    ("nauc", NumFormat::UpperLetter),
    ("nrlc", NumFormat::LowerRoman),
    ("nruc", NumFormat::UpperRoman),
    ("nchi", NumFormat::Chicago),
];

#[test]
fn no_words_leave_everything_unset() {
    let d = doc(r"{\rtf1\ansi\sectd a\par}");
    assert!(d.settings.footnote_pr.is_empty() && d.settings.endnote_pr.is_empty());
    assert_eq!((d.settings.footnote_format, d.settings.endnote_format), (NumFormat::Decimal, NumFormat::LowerRoman));
    assert!(d.last_section.footnote_pr.is_empty() && d.last_section.endnote_pr.is_empty());
}

#[test]
fn document_options_are_read() {
    let d = doc(r"{\rtf1\ansi\ftntj\aendnotes\ftnstart5\aftnstart3\ftnrstpg\aftnrestart\ftnnalc\aftnnruc\sectd a\par}");
    assert_eq!(
        d.settings.footnote_pr,
        NoteProps { pos: Some(NotePos::BeneathText), num_fmt: None, num_start: Some(5), num_restart: Some(NoteRestart::EachPage) }
    );
    assert_eq!(
        d.settings.endnote_pr,
        NoteProps { pos: Some(NotePos::SectEnd), num_fmt: None, num_start: Some(3), num_restart: Some(NoteRestart::EachSect) }
    );
    assert_eq!((d.settings.footnote_format, d.settings.endnote_format), (NumFormat::LowerLetter, NumFormat::UpperRoman));
}

#[test]
fn document_positions_and_restarts() {
    let d = doc(r"{\rtf1\ansi\ftnbj\aenddoc\ftnrestart\aftnrstcont\sectd a\par}");
    assert_eq!((d.settings.footnote_pr.pos, d.settings.endnote_pr.pos), (Some(NotePos::PageBottom), Some(NotePos::DocEnd)));
    assert_eq!((d.settings.footnote_pr.num_restart, d.settings.endnote_pr.num_restart), (Some(NoteRestart::EachSect), Some(NoteRestart::Continuous)));
    assert_eq!(doc(r"{\rtf1\ansi\ftnrstcont\sectd a\par}").settings.footnote_pr.num_restart, Some(NoteRestart::Continuous));
}

#[test]
fn each_document_format_word_is_read() {
    for (suffix, fmt) in FORMATS {
        let d = doc(&format!(r"{{\rtf1\ansi\ftn{suffix}\aftn{suffix}\sectd a\par}}"));
        assert_eq!((d.settings.footnote_format, d.settings.endnote_format), (fmt, fmt), "{suffix}");
    }
}

#[test]
fn section_options_are_read() {
    let s = last(r"{\rtf1\ansi\sectd\sftntj\sftnstart2\sftnrstpg\sftnnauc\saftnstart4\saftnrestart\saftnnchi a\par}");
    assert_eq!(
        s.footnote_pr,
        NoteProps {
            pos: Some(NotePos::BeneathText),
            num_fmt: Some(NumFormat::UpperLetter),
            num_start: Some(2),
            num_restart: Some(NoteRestart::EachPage)
        }
    );
    assert_eq!(
        s.endnote_pr,
        NoteProps { pos: None, num_fmt: Some(NumFormat::Chicago), num_start: Some(4), num_restart: Some(NoteRestart::EachSect) }
    );
}

#[test]
fn each_section_word_is_read() {
    for (suffix, fmt) in FORMATS {
        let s = last(&format!(r"{{\rtf1\ansi\sectd\sftn{suffix}\saftn{suffix} a\par}}"));
        assert_eq!((s.footnote_pr.num_fmt, s.endnote_pr.num_fmt), (Some(fmt), Some(fmt)), "{suffix}");
    }
    let s = last(r"{\rtf1\ansi\sectd\sftnbj\sftnrestart\saftnrstcont a\par}");
    assert_eq!(
        (s.footnote_pr.pos, s.footnote_pr.num_restart, s.endnote_pr.num_restart),
        (Some(NotePos::PageBottom), Some(NoteRestart::EachSect), Some(NoteRestart::Continuous))
    );
    assert_eq!(last(r"{\rtf1\ansi\sectd\sftnrstcont a\par}").footnote_pr.num_restart, Some(NoteRestart::Continuous));
}

#[test]
fn section_words_stay_out_of_the_document_options() {
    let d = doc(r"{\rtf1\ansi\sectd\sftnnalc\sftnstart7 a\par}");
    assert!(d.settings.footnote_pr.is_empty());
    assert_eq!(d.settings.footnote_format, NumFormat::Decimal);
}

#[test]
fn out_of_range_starts_and_unknown_formats_are_ignored() {
    let d = doc(r"{\rtf1\ansi\ftnstart0\aftnstart-4\ftnnchosung\sectd\sftnstart40000\saftnstart0\sftnndbnum a\par}");
    assert_eq!((d.settings.footnote_pr.num_start, d.settings.endnote_pr.num_start), (None, None));
    assert_eq!(d.settings.footnote_format, NumFormat::Decimal);
    assert!(d.last_section.footnote_pr.is_empty() && d.last_section.endnote_pr.is_empty());
}

#[test]
fn sectd_resets_them_for_the_next_section() {
    let d = doc(r"{\rtf1\ansi\sectd\sftnnruc\sftnstart3\saftnrestart a\sect\sectd b\par}");
    let first = first_section(&d);
    assert_eq!((first.footnote_pr.num_fmt, first.footnote_pr.num_start), (Some(NumFormat::UpperRoman), Some(3)));
    assert_eq!(first.endnote_pr.num_restart, Some(NoteRestart::EachSect));
    assert!(d.last_section.footnote_pr.is_empty() && d.last_section.endnote_pr.is_empty());
}

#[test]
fn export_writes_document_options_and_they_round_trip() {
    let mut d = Document::from_text("one");
    d.settings.footnote_pr =
        NoteProps { pos: Some(NotePos::BeneathText), num_fmt: None, num_start: Some(5), num_restart: Some(NoteRestart::EachPage) };
    d.settings.endnote_pr = NoteProps { pos: Some(NotePos::SectEnd), num_fmt: None, num_start: Some(2), num_restart: Some(NoteRestart::EachSect) };
    d.settings.footnote_format = NumFormat::LowerLetter;
    d.settings.endnote_format = NumFormat::Decimal;
    let rtf = export(&d);
    for w in ["\\ftntj", "\\aendnotes", "\\ftnstart5", "\\aftnstart2", "\\ftnrstpg", "\\aftnrestart", "\\ftnnalc", "\\aftnnar"] {
        assert!(rtf.contains(w), "{w} in {rtf}");
    }
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!((back.settings.footnote_pr, back.settings.endnote_pr), (d.settings.footnote_pr, d.settings.endnote_pr));
    assert_eq!((back.settings.footnote_format, back.settings.endnote_format), (NumFormat::LowerLetter, NumFormat::Decimal));
}

#[test]
fn export_writes_section_options_and_they_round_trip() {
    let mut d = Document::from_text("one");
    let foot = NoteProps {
        pos: Some(NotePos::BeneathText),
        num_fmt: Some(NumFormat::UpperRoman),
        num_start: Some(9),
        num_restart: Some(NoteRestart::EachSect),
    };
    let end = NoteProps { pos: None, num_fmt: Some(NumFormat::Chicago), num_start: Some(3), num_restart: Some(NoteRestart::Continuous) };
    d.last_section = SectionProps { footnote_pr: foot, endnote_pr: end, ..Default::default() };
    let rtf = export(&d);
    for w in ["\\sftntj", "\\sftnnruc", "\\sftnstart9", "\\sftnrestart", "\\saftnnchi", "\\saftnstart3", "\\saftnrstcont"] {
        assert!(rtf.contains(w), "{w} in {rtf}");
    }
    let back = import(rtf.as_bytes()).unwrap().last_section;
    assert_eq!((back.footnote_pr, back.endnote_pr), (foot, end));
}

#[test]
fn every_format_round_trips_at_both_levels() {
    for (suffix, fmt) in FORMATS {
        let mut d = Document::from_text("one");
        d.settings.footnote_format = fmt;
        d.settings.endnote_format = fmt;
        d.last_section = SectionProps {
            footnote_pr: NoteProps { num_fmt: Some(fmt), ..Default::default() },
            endnote_pr: NoteProps { num_fmt: Some(fmt), ..Default::default() },
            ..Default::default()
        };
        let back = import(export(&d).as_bytes()).unwrap();
        assert_eq!((back.settings.footnote_format, back.settings.endnote_format), (fmt, fmt), "{suffix}");
        assert_eq!((back.last_section.footnote_pr.num_fmt, back.last_section.endnote_pr.num_fmt), (Some(fmt), Some(fmt)), "{suffix}");
    }
}

#[test]
fn export_writes_none_of_them_by_default() {
    let rtf = export(&Document::from_text("one"));
    for w in ["\\ftntj", "\\ftnbj", "\\aendnotes", "\\aenddoc", "\\ftnstart", "\\ftnrst", "\\ftnrestart", "\\ftnn", "\\aftnn", "\\sftn", "\\saftn"] {
        assert!(!rtf.contains(w), "{w} in {rtf}");
    }
}
