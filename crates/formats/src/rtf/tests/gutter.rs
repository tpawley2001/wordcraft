//! Gutter: document `\gutterN`, section `\guttersxnN` (twips) and `\rtlgutter`, RTF 1.9.1: the RTF
//! form of DOCX `w:pgMar/@w:gutter` and `w:rtlGutter`.

use super::*;
use wordcraft_doc::{Block, SectionProps};

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

/// The section properties in document order (each is stored on a paragraph).
fn sections(d: &Document) -> Vec<SectionProps> {
    d.body
        .iter()
        .filter_map(|b| match b.as_ref() {
            Block::Para(p) => p.section.as_deref().cloned(),
            _ => None,
        })
        .collect()
}

/// (gutter in points, gutter on the right) of the first and the last section.
fn gutters(d: &Document) -> ((f32, bool), (f32, bool)) {
    let all = sections(d);
    let first = all.first().expect("first section");
    let last = all.last().expect("last section");
    ((first.gutter, first.rtl_gutter), (last.gutter, last.rtl_gutter))
}

#[test]
fn no_gutter_words_keep_zero() {
    let d = doc(r"{\rtf1\ansi\sectd a\par}");
    assert_eq!(gutters(&d), ((0.0, false), (0.0, false)));
    let out = export(&d);
    assert!(!out.contains("\\gutter") && !out.contains("\\rtlgutter"), "{out}");
}

#[test]
fn the_document_gutter_applies_to_every_section() {
    // 720 twips = 36 pt; `\sectd` resets a section to the document's gutter.
    let d = doc(r"{\rtf1\ansi\gutter720\sectd a\par\sect\sectd b\par}");
    assert_eq!(gutters(&d), ((36.0, false), (36.0, false)));
}

#[test]
fn a_section_gutter_overrides_the_document_gutter() {
    let d = doc(r"{\rtf1\ansi\gutter720\sectd\guttersxn360 a\par\sect\sectd b\par}");
    assert_eq!(gutters(&d), ((18.0, false), (36.0, false)));
}

#[test]
fn a_right_side_gutter_is_read() {
    let d = doc(r"{\rtf1\ansi\gutter720\rtlgutter\sectd a\par}");
    assert_eq!(gutters(&d), ((36.0, true), (36.0, true)));
}

#[test]
fn gutter_and_side_round_trip() {
    for rtf in [
        r"{\rtf1\ansi\gutter720\sectd a\par}",
        r"{\rtf1\ansi\gutter720\rtlgutter\sectd a\par}",
        r"{\rtf1\ansi\gutter720\sectd\guttersxn360 a\par\sect\sectd b\par}",
    ] {
        let src = doc(rtf);
        let out = export(&src);
        assert!(out.contains("\\gutter"), "{rtf}: {out}");
        assert_eq!(gutters(&doc(&out)), gutters(&src), "{rtf}");
    }
}

#[test]
fn a_negative_gutter_counts_as_zero() {
    let d = doc(r"{\rtf1\ansi\gutter-720\sectd\guttersxn-360 a\par}");
    assert_eq!(gutters(&d), ((0.0, false), (0.0, false)));
}
