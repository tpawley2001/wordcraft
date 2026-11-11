//! RTF `\endnhere` ("endnotes included in the section", RTF 1.9.1): with endnotes at the end of each
//! section (`\aendnotes`), a section without it suppresses its endnotes (DOCX `w:noEndnote`), as
//! Word reads it. With endnotes at the document end the flag means nothing and isn't recorded.

use super::*;
use wordcraft_doc::{Block, Document, SectionProps};

fn doc(words: &str, s1: &str, s2: &str) -> Document {
    let rtf = format!(
        r"{{\rtf1\ansi\deff0{{\fonttbl{{\f0 Times New Roman;}}}}{words}\sectd{s1}\pard SecOne{{\super\chftn}}{{\footnote\ftnalt\pard {{\super\chftn}} EndA.}}\par\sect\sectd\sbkpage{s2}\pard SecTwo{{\super\chftn}}{{\footnote\ftnalt\pard {{\super\chftn}} EndB.}}\par}}"
    );
    import(rtf.as_bytes()).unwrap()
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

fn flags(d: &Document) -> (Option<bool>, Option<bool>) {
    (first_section(d).no_endnote, d.last_section.no_endnote)
}

#[test]
fn section_end_endnotes_read_endnhere() {
    assert_eq!(flags(&doc(r"\ftnbj\aendnotes", "", "")), (Some(true), Some(true)));
    assert_eq!(flags(&doc(r"\ftnbj\aendnotes", r"\endnhere", r"\endnhere")), (Some(false), Some(false)));
    assert_eq!(flags(&doc(r"\ftnbj\aendnotes", "", r"\endnhere")), (Some(true), Some(false)));
}

#[test]
fn sectd_resets_endnhere() {
    // `\endnhere` before the first `\sectd` of section 2 doesn't carry over: `\sectd` resets it.
    assert_eq!(flags(&doc(r"\ftnbj\aendnotes", r"\endnhere", "")), (Some(false), Some(true)));
}

#[test]
fn document_end_endnotes_leave_it_unset() {
    for words in [r"\ftnbj\aenddoc", r"\ftnbj", ""] {
        assert_eq!(flags(&doc(words, "", r"\endnhere")), (None, None), "{words:?}");
    }
}

#[test]
fn export_writes_endnhere_for_including_sections() {
    let d = doc(r"\ftnbj\aendnotes", "", r"\endnhere");
    let rtf = export(&d);
    assert_eq!(rtf.matches(r"\endnhere").count(), 1, "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!(flags(&back), (Some(true), Some(false)));
}

#[test]
fn export_writes_nothing_new_at_document_end() {
    let d = doc(r"\ftnbj\aenddoc", "", "");
    assert!(!export(&d).contains(r"\endnhere"));
    let mut d2 = d.clone();
    d2.last_section.no_endnote = Some(false);
    assert_eq!(export(&d2).matches(r"\endnhere").count(), 1);
}
