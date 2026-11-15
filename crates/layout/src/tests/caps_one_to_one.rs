//! All Caps and Small Caps (`w:caps`, `w:smallCaps`, ECMA-376 Part 1 §17.3.2.5 / §17.3.2.33)
//! change a letter only when its capital is a single character. A letter whose capital is longer
//! (ß → SS, ﬁ → FI, ŉ → ʼN, ǰ → J̌, ᾳ → ΑΙ) is drawn unchanged; ΐ is the one exception seen, drawn
//! as Ϊ. Measured in Microsoft 365 Word for Windows (16.0.20527), PDF export and page image,
//! Aptos and Times New Roman, compatibility modes 12 and 15, en-US and de-DE: "Straße" in caps
//! shows "STRAßE", "ﬁne" shows "ﬁNE". This replaces #1581's ß → SS, which was never measured.

use super::{strasse_with, text_clusters};
use crate::{DocLayout, Placed};
use wordcraft_doc::{Document, Pos};

/// Glyph ids of the first line of `text` laid out with `f` applied to all of it.
fn glyphs(text: &str, f: &dyn Fn(&mut wordcraft_doc::props::CharProps)) -> Vec<u32> {
    crate::test_fonts();
    let mut d = Document::from_text(text);
    d.format_range(&Pos::body(0, 0), &Pos::body(0, text.len()), f).unwrap();
    let l: DocLayout = super::lay(&d);
    let pl = l.pages[0].items.iter().find_map(|it| if let Placed::Lines { para, .. } = it { Some((**para).clone()) } else { None }).expect("a line");
    let line = &pl.lines[0];
    (line.c0..line.c1).flat_map(|k| pl.glyphs[pl.clusters[k].g0 as usize..pl.clusters[k].g1 as usize].iter().map(|g| g.gid)).collect()
}

/// (typed, what Word draws with caps on). Each typed letter sits between x and y.
const TABLE: &[(&str, &str)] = &[
    ("xßy", "XßY"),
    ("xﬁy", "XﬁY"),
    ("xﬂy", "XﬂY"),
    ("xﬀy", "XﬀY"),
    ("xﬃy", "XﬃY"),
    ("xŉy", "XŉY"),
    ("xǰy", "XǰY"),
    ("xẖy", "XẖY"),
    ("xᾳy", "XᾳY"),
    ("xΐy", "XΪY"),
    ("xάy", "XΆY"),
    ("xǆy", "XǄY"),
    ("xǈy", "XǇY"),
    ("xıy", "XIY"),
    ("xiy", "XIY"),
    ("Straße", "STRAßE"),
    ("ﬁne", "ﬁNE"),
];

#[test]
fn all_caps_draws_what_word_draws() {
    let caps: &dyn Fn(&mut wordcraft_doc::props::CharProps) = &|c| c.caps = Some(true);
    let plain: &dyn Fn(&mut wordcraft_doc::props::CharProps) = &|_| {};
    for (typed, word) in TABLE {
        let got = glyphs(typed, caps);
        assert_eq!(got.len(), word.chars().count(), "{typed}: one glyph per character Word draws: {got:?}");
        assert_eq!(got, glyphs(word, plain), "{typed} in All Caps draws {word}");
    }
}

#[test]
fn small_caps_draws_the_same_letters() {
    // Small Caps draws the same characters as All Caps (lowercase ones at the smaller size):
    // compare glyph ids only.
    let small: &dyn Fn(&mut wordcraft_doc::props::CharProps) = &|c| c.small_caps = Some(true);
    let plain: &dyn Fn(&mut wordcraft_doc::props::CharProps) = &|_| {};
    for (typed, word) in TABLE {
        assert_eq!(glyphs(typed, small), glyphs(word, plain), "{typed} in Small Caps draws {word}");
    }
}

#[test]
fn caps_sharp_s_is_one_glyph_and_one_caret_stop() {
    let caps: &dyn Fn(&mut wordcraft_doc::props::CharProps) = &|c| c.caps = Some(true);
    let pl = strasse_with(caps);
    let cl = text_clusters(&pl);
    assert_eq!(cl.len(), 6, "{cl:?}");
    assert_eq!(cl.iter().map(|c| c.2.len()).sum::<usize>(), 6, "S T R A ß E: six glyphs: {cl:?}");
    assert_eq!(pl.line_offsets(0), vec![0, 1, 2, 3, 4, 6, 7]);
}

#[test]
fn control_plain_text_is_unchanged() {
    let plain: &dyn Fn(&mut wordcraft_doc::props::CharProps) = &|_| {};
    assert_eq!(glyphs("Straße", plain).len(), 6);
}
