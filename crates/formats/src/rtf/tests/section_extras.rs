//! Section line numbers (`\linemod`, `\linex`, `\linestarts`, `\lineppage`/`\linerestart`/`\linecont`),
//! direction (`\rtlsect`/`\ltrsect`) and chapter page numbers (`\pgnhnN`, `\pgnhnsh|sp|sc|sm|sn`), RTF 1.9.1
//! section formatting: the RTF form of DOCX `w:lnNumType`, `w:bidi` and `w:pgNumType` chapStyle/chapSep.

use super::*;
use wordcraft_doc::section::{ChapterSep, LineNumberRestart, LineNumbering};
use wordcraft_doc::{Block, SectionProps};

fn last(rtf: &str) -> SectionProps {
    import(rtf.as_bytes()).unwrap().last_section.clone()
}

fn lines(count_by: u32, start: u32, distance: f32, restart: LineNumberRestart) -> Option<LineNumbering> {
    Some(LineNumbering { count_by, start, distance, restart })
}

#[test]
fn no_words_leave_everything_unset() {
    let s = last(r"{\rtf1\ansi\sectd a\par}");
    assert_eq!((s.line_numbers, s.rtl, s.chapter_style, s.chapter_sep), (None, false, None, None));
}

#[test]
fn line_numbers_are_read() {
    let s = last(r"{\rtf1\ansi\sectd\linemod5\linex360\linestarts10\linerestart a\par}");
    assert_eq!(s.line_numbers, lines(5, 10, 18.0, LineNumberRestart::Section));
}

#[test]
fn line_number_words_before_linemod_count() {
    let s = last(r"{\rtf1\ansi\sectd\linecont\linestarts3\linex720\linemod1 a\par}");
    assert_eq!(s.line_numbers, lines(1, 3, 36.0, LineNumberRestart::Continuous));
}

#[test]
fn each_restart_word_is_read() {
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod1\lineppage a\par}").line_numbers.map(|l| l.restart), Some(LineNumberRestart::Page));
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod1\linerestart a\par}").line_numbers.map(|l| l.restart), Some(LineNumberRestart::Section));
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod1\linecont a\par}").line_numbers.map(|l| l.restart), Some(LineNumberRestart::Continuous));
}

#[test]
fn linemod_alone_takes_the_defaults() {
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod2 a\par}").line_numbers, lines(2, 1, 0.0, LineNumberRestart::Page));
}

#[test]
fn line_numbers_need_linemod_and_clamp_their_values() {
    assert_eq!(last(r"{\rtf1\ansi\sectd\linex360\linestarts4\linerestart a\par}").line_numbers, None);
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod0 a\par}").line_numbers, None);
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod-2 a\par}").line_numbers, None);
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod500\linex-40\linestarts0 a\par}").line_numbers, lines(100, 1, 0.0, LineNumberRestart::Page));
    assert_eq!(last(r"{\rtf1\ansi\sectd\linemod1\linestarts99999 a\par}").line_numbers.map(|l| l.start), Some(32768));
}

#[test]
fn section_direction_is_read() {
    assert!(last(r"{\rtf1\ansi\sectd\rtlsect a\par}").rtl);
    assert!(!last(r"{\rtf1\ansi\sectd\rtlsect\ltrsect a\par}").rtl);
}

#[test]
fn chapter_numbers_are_read() {
    let s = last(r"{\rtf1\ansi\sectd\pgnhn2\pgnhnsc a\par}");
    assert_eq!((s.chapter_style, s.chapter_sep), (Some(2), Some(ChapterSep::Colon)));
    let sep = |w: &str| last(&format!(r"{{\rtf1\ansi\sectd\pgnhn1\pgnhns{w} a\par}}")).chapter_sep;
    assert_eq!(
        [sep("h"), sep("p"), sep("c"), sep("m"), sep("n")],
        [Some(ChapterSep::Hyphen), Some(ChapterSep::Period), Some(ChapterSep::Colon), Some(ChapterSep::EmDash), Some(ChapterSep::EnDash)]
    );
    assert_eq!(last(r"{\rtf1\ansi\sectd\pgnhn0 a\par}").chapter_style, None);
    assert_eq!(last(r"{\rtf1\ansi\sectd\pgnhn12 a\par}").chapter_style, None);
}

#[test]
fn sectd_resets_them_for_the_next_section() {
    let d = import(br"{\rtf1\ansi\sectd\linemod1\rtlsect\pgnhn1\pgnhnsp a\sect\sectd b\par}").unwrap();
    let first = d
        .body
        .iter()
        .find_map(|b| match b.as_ref() {
            Block::Para(p) => p.section.as_deref(),
            _ => None,
        })
        .expect("first section");
    assert_eq!((first.line_numbers.is_some(), first.rtl, first.chapter_style, first.chapter_sep), (true, true, Some(1), Some(ChapterSep::Period)));
    let s = &d.last_section;
    assert_eq!((s.line_numbers.is_some(), s.rtl, s.chapter_style, s.chapter_sep), (false, false, None, None));
}

#[test]
fn export_writes_them_and_they_round_trip() {
    let mut d = Document::from_text("one");
    d.last_section = SectionProps {
        line_numbers: lines(5, 10, 18.0, LineNumberRestart::Continuous),
        rtl: true,
        chapter_style: Some(3),
        chapter_sep: Some(ChapterSep::EnDash),
        ..Default::default()
    };
    let rtf = export(&d);
    for w in ["\\linemod5", "\\linex360", "\\linestarts10", "\\linecont", "\\rtlsect", "\\pgnhn3", "\\pgnhnsn"] {
        assert!(rtf.contains(w), "{w} in {rtf}");
    }
    let back = import(rtf.as_bytes()).unwrap().last_section;
    assert_eq!(back.line_numbers, lines(5, 10, 18.0, LineNumberRestart::Continuous));
    assert_eq!((back.rtl, back.chapter_style, back.chapter_sep), (true, Some(3), Some(ChapterSep::EnDash)));
}

#[test]
fn each_restart_round_trips() {
    for (restart, word) in
        [(LineNumberRestart::Page, "\\lineppage"), (LineNumberRestart::Section, "\\linerestart"), (LineNumberRestart::Continuous, "\\linecont")]
    {
        let mut d = Document::from_text("one");
        d.last_section = SectionProps { line_numbers: lines(1, 1, 0.0, restart), ..Default::default() };
        let rtf = export(&d);
        assert!(rtf.contains(word), "{word} in {rtf}");
        assert_eq!(import(rtf.as_bytes()).unwrap().last_section.line_numbers.map(|l| l.restart), Some(restart));
    }
}

#[test]
fn export_writes_none_of_them_by_default() {
    let rtf = export(&Document::from_text("one"));
    for w in ["\\linemod", "\\linestarts", "\\rtlsect", "\\pgnhn"] {
        assert!(!rtf.contains(w), "{w} in {rtf}");
    }
}
