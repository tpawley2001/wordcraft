//! Section paper source: `\binfsxnN` (printer bin for the section's first page) and `\binsxnN` (the
//! pages after it), RTF 1.9.1 section formatting; the RTF form of DOCX `w:paperSrc`.

use super::*;
use wordcraft_doc::{Block, SectionProps};

/// (first, other) trays of the document's last section.
fn trays(rtf: &str) -> (Option<u16>, Option<u16>) {
    let d = import(rtf.as_bytes()).unwrap();
    (d.last_section.paper_src_first, d.last_section.paper_src_other)
}

#[test]
fn both_bins_are_read() {
    assert_eq!(trays(r"{\rtf1\ansi\sectd\binfsxn4\binsxn15 a\par}"), (Some(4), Some(15)));
}

#[test]
fn the_other_pages_bin_alone_is_read() {
    assert_eq!(trays(r"{\rtf1\ansi\sectd\binsxn7 a\par}"), (None, Some(7)));
}

#[test]
fn bin_zero_and_out_of_range_bins_stay_unset() {
    assert_eq!(trays(r"{\rtf1\ansi\sectd\binfsxn0 a\par}"), (None, None));
    assert_eq!(trays(r"{\rtf1\ansi\sectd\binfsxn-3 a\par}"), (None, None));
    assert_eq!(trays(r"{\rtf1\ansi\sectd\binfsxn70000 a\par}"), (None, None));
}

#[test]
fn sectd_resets_the_bins_for_the_next_section() {
    let d = import(br"{\rtf1\ansi\sectd\binfsxn4 a\sect\sectd b\par}").unwrap();
    let first = d
        .body
        .iter()
        .find_map(|b| match b.as_ref() {
            Block::Para(p) => p.section.as_deref(),
            _ => None,
        })
        .expect("first section");
    assert_eq!((first.paper_src_first, first.paper_src_other), (Some(4), None));
    assert_eq!((d.last_section.paper_src_first, d.last_section.paper_src_other), (None, None));
}

#[test]
fn export_writes_the_bins_and_they_round_trip() {
    let mut d = Document::from_text("one");
    d.last_section = SectionProps { paper_src_first: Some(4), paper_src_other: Some(15), ..Default::default() };
    let rtf = export(&d);
    assert!(rtf.contains("\\binfsxn4"), "{rtf}");
    assert!(rtf.contains("\\binsxn15"), "{rtf}");
    let back = import(rtf.as_bytes()).unwrap();
    assert_eq!((back.last_section.paper_src_first, back.last_section.paper_src_other), (Some(4), Some(15)));
}

#[test]
fn export_writes_no_bins_when_none_are_set() {
    let d = Document::from_text("one");
    let rtf = export(&d);
    assert!(!rtf.contains("\\binfsxn"), "{rtf}");
    assert!(!rtf.contains("\\binsxn"), "{rtf}");
}
