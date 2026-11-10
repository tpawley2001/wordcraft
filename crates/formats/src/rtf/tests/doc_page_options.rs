//! Document formatting `\deftabN`, `\facingp`, `\margmirror`, `\hyphauto`, `\hyphhotzN`, `\hyphcaps`,
//! `\hyphconsecN` and section formatting `\vertalt|c|j|b`, RTF 1.9.1: the RTF form of DOCX
//! `w:defaultTabStop`, `w:evenAndOddHeaders`, `w:mirrorMargins`, the hyphenation settings and `w:vAlign`.

use super::*;
use wordcraft_doc::section::PageVAlign;
use wordcraft_doc::{Block, SectionProps};

fn doc(rtf: &str) -> Document {
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

/// (default tab, facing pages, mirror margins, auto hyphenation, zone, caps, consecutive limit).
fn opts(d: &Document) -> (f32, bool, bool, bool, f32, bool, u32) {
    let s = &d.settings;
    (s.default_tab, s.even_odd_headers, s.mirror_margins, s.auto_hyphenation, s.hyphenation_zone, s.hyphenate_caps, s.consecutive_hyphen_limit)
}

const DEFAULTS: (f32, bool, bool, bool, f32, bool, u32) = (36.0, false, false, false, 18.0, true, 0);

#[test]
fn no_words_keep_the_defaults() {
    let d = doc(r"{\rtf1\ansi\sectd a\par}");
    assert_eq!(opts(&d), DEFAULTS);
    assert_eq!(d.last_section.valign, PageVAlign::Top);
}

#[test]
fn document_words_are_read() {
    let d = doc(r"{\rtf1\ansi\deftab360\facingp\margmirror\hyphauto\hyphhotz425\hyphcaps0\hyphconsec3\sectd a\par}");
    assert_eq!(opts(&d), (18.0, true, true, true, 21.25, false, 3));
}

#[test]
fn toggles_with_zero_turn_off() {
    let d = doc(r"{\rtf1\ansi\facingp\facingp0\margmirror0\hyphauto1\hyphauto0\hyphcaps0\hyphcaps\sectd a\par}");
    assert_eq!(opts(&d), DEFAULTS);
    let d = doc(r"{\rtf1\ansi\hyphhotz0\hyphconsec0\sectd a\par}");
    assert_eq!((d.settings.hyphenation_zone, d.settings.consecutive_hyphen_limit), (0.0, 0));
}

#[test]
fn out_of_range_values_are_ignored() {
    for rtf in
        [r"{\rtf1\ansi\deftab0\hyphhotz-5\hyphconsec-1\sectd a\par}", r"{\rtf1\ansi\deftab-720\sectd a\par}", r"{\rtf1\ansi\deftab31681\sectd a\par}"]
    {
        assert_eq!(opts(&doc(rtf)), DEFAULTS, "{rtf}");
    }
    assert_eq!(doc(r"{\rtf1\ansi\deftab31680\sectd a\par}").settings.default_tab, 1584.0);
    assert_eq!(doc(r"{\rtf1\ansi\deftab1\sectd a\par}").settings.default_tab, 0.05);
}

#[test]
fn section_vertical_alignment_is_read_per_section() {
    for (w, v) in [("vertalt", PageVAlign::Top), ("vertalc", PageVAlign::Center), ("vertalj", PageVAlign::Justify), ("vertalb", PageVAlign::Bottom)] {
        let d = doc(&format!(r"{{\rtf1\ansi\sectd\{w} a\par}}"));
        assert_eq!(d.last_section.valign, v, "{w}");
    }
    let d = doc(r"{\rtf1\ansi\sectd\vertalc a\par\sect\sectd b\par}");
    assert_eq!(first_section(&d).valign, PageVAlign::Center);
    assert_eq!(d.last_section.valign, PageVAlign::Top, "\\sectd resets it");
    let d = doc(r"{\rtf1\ansi\sectd a\par\sect\sectd\vertalb b\par}");
    assert_eq!((first_section(&d).valign, d.last_section.valign), (PageVAlign::Top, PageVAlign::Bottom));
}

#[test]
fn cell_vertical_alignment_does_not_set_the_section() {
    let d = doc(r"{\rtf1\ansi\sectd\trowd\clvertalc\cellx1000\intbl x\cell\row a\par}");
    assert_eq!(d.last_section.valign, PageVAlign::Top);
}

#[test]
fn default_settings_write_none_of_these_words() {
    let out = export(&doc(r"{\rtf1\ansi\sectd a\par}"));
    for w in ["\\deftab", "\\facingp", "\\margmirror", "\\hyphauto", "\\hyphhotz", "\\hyphcaps", "\\hyphconsec", "\\vertal"] {
        assert!(!out.contains(w), "{w} in {out}");
    }
}

#[test]
fn options_survive_a_round_trip() {
    let src =
        doc(r"{\rtf1\ansi\deftab360\facingp\margmirror\hyphauto\hyphhotz425\hyphcaps0\hyphconsec3\sectd\vertalj a\par\sect\sectd\vertalc b\par}");
    let back = doc(&export(&src));
    assert_eq!(opts(&back), (18.0, true, true, true, 21.25, false, 3));
    assert_eq!((first_section(&back).valign, back.last_section.valign), (PageVAlign::Justify, PageVAlign::Center));
    // A bottom-aligned single section, and Top written as nothing.
    let back = doc(&export(&doc(r"{\rtf1\ansi\sectd\vertalb a\par}")));
    assert_eq!(back.last_section.valign, PageVAlign::Bottom);
}
