//! The gutter (`\gutterN`, `\guttersxnN`, `\rtlgutter`) read through `Document::sections`, which
//! includes the document's last section (`Document::last_section`, not held on a paragraph).

use super::*;
use wordcraft_doc::Document;

fn doc(rtf: &str) -> Document {
    import(rtf.as_bytes()).unwrap()
}

/// (gutter in points, gutter on the right) of every section in order.
fn gutters(d: &Document) -> Vec<(f32, bool)> {
    d.sections().into_iter().map(|(_, s)| (s.gutter, s.rtl_gutter)).collect()
}

#[test]
fn no_gutter_words_keep_zero_and_write_nothing() {
    let d = doc(r"{\rtf1\ansi\sectd a\par}");
    assert_eq!(gutters(&d), [(0.0, false)]);
    let out = export(&d);
    assert!(!out.contains("\\gutter") && !out.contains("\\rtlgutter"), "{out}");
}

#[test]
fn the_document_gutter_applies_to_every_section() {
    let d = doc(r"{\rtf1\ansi\gutter720\sectd a\par\sect\sectd b\par}");
    assert_eq!(gutters(&d), [(36.0, false), (36.0, false)]);
}

#[test]
fn a_section_gutter_overrides_the_document_gutter() {
    let d = doc(r"{\rtf1\ansi\gutter720\sectd\guttersxn360 a\par\sect\sectd b\par}");
    assert_eq!(gutters(&d), [(18.0, false), (36.0, false)]);
}

#[test]
fn a_right_side_gutter_is_read() {
    let d = doc(r"{\rtf1\ansi\gutter720\rtlgutter\sectd a\par}");
    assert_eq!(gutters(&d), [(36.0, true)]);
}

#[test]
fn gutter_and_side_round_trip() {
    for rtf in [
        r"{\rtf1\ansi\gutter720\sectd a\par}",
        r"{\rtf1\ansi\gutter720\rtlgutter\sectd a\par}",
        r"{\rtf1\ansi\gutter720\sectd\guttersxn360 a\par\sect\sectd b\par}",
        r"{\rtf1\ansi\sectd a\par\sect\sectd\guttersxn360 b\par}",
    ] {
        let src = doc(rtf);
        let out = export(&src);
        assert!(out.contains("\\gutter"), "{rtf}: {out}");
        assert_eq!(gutters(&doc(&out)), gutters(&src), "{rtf}: {out}");
    }
}

#[test]
fn a_negative_or_huge_gutter_is_clamped() {
    let d = doc(r"{\rtf1\ansi\gutter-720\sectd\guttersxn-360 a\par}");
    assert_eq!(gutters(&d), [(0.0, false)]);
    let d = doc(r"{\rtf1\ansi\gutter2147483647\sectd a\par}");
    assert_eq!(gutters(&d), [(720.0, false)]);
}
