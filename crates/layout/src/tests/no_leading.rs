//! The compatibility option "Don't add leading (extra space) between rows of text"
//! (`Settings::no_leading`: `w:noLeading`, .doc `Copts80.fNoExtLeading`). Measured in Microsoft 365
//! Word's PDFs of synthetic documents: below compatibility mode 15 a line is as tall as the font's
//! ascent plus descent, without its line gap. Times New Roman 14 pt lines are 15.50 pt apart
//! instead of 16.09 (12 pt: 13.29 / 13.80, 10 pt: 11.07 / 11.50), Arial 14 pt 15.64 instead of
//! 16.09, Courier New (no line gap) stays 15.86. Multiple spacing multiplies the smaller line
//! (1.5 lines: 23.25, double: 31.00, Arial at 1.15: 17.98), "at least 12 pt" gives 15.50, "at
//! least 20 pt" 20.00 and "exactly 14 pt" 14.00 either way. Mode 15 ignores the option.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;

/// The distances between the tops of four paragraphs in `font` at `size`, spaced `spacing`.
fn pitches(font: &str, size: f32, spacing: LineSpacing, no_leading: bool, mode: u32) -> Vec<f32> {
    let chr = wordcraft_doc::CharProps { font: Some(font.into()), size: Some(size), ..Default::default() };
    let mut d = Document::new();
    d.settings.no_leading = no_leading;
    d.settings.compat_mode = mode;
    d.body = ["First line", "Second line", "Third line", "Fourth line"]
        .iter()
        .map(|t| {
            let mut p = wordcraft_doc::Paragraph::with_text(t, chr.clone());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(spacing);
            wordcraft_doc::para_block(p)
        })
        .collect();
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    let tops: Vec<f32> =
        l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect();
    assert_eq!(tops.len(), 4, "{tops:?}");
    tops.windows(2).map(|w| w[1] - w[0]).collect()
}

/// A single line of `font` in em: (with the font's line gap, ascent plus descent only).
fn line_em(font: &str) -> (f32, f32) {
    crate::test_fonts();
    let face = wordcraft_fonts::word::resolve(font, false, false).face;
    let (a, d) = wordcraft_fonts::word::line_metrics(&face);
    (((a + d) / face.upem) as f32, ((face.ascent + face.descent.abs()) / face.upem) as f32)
}

fn assert_pitch(font: &str, size: f32, spacing: LineSpacing, no_leading: bool, mode: u32, expected: f32) {
    for pitch in pitches(font, size, spacing, no_leading, mode) {
        assert!((pitch - expected).abs() <= 0.05, "{font} {size} {spacing:?} no_leading {no_leading} mode {mode}: {pitch} vs {expected}");
    }
}

/// The fonts these tests lay out with keep a line gap (Times New Roman's is 87/2048 em, Arial's
/// 67/2048), so the option has something to remove.
#[test]
fn the_test_fonts_have_a_line_gap() {
    for font in ["Times New Roman", "Arial"] {
        let (full, bare) = line_em(font);
        assert!(full - bare > 0.02, "{font}: {full} vs {bare}");
        assert!((1.05..1.2).contains(&bare), "{font}: {bare}");
    }
}

#[test]
fn single_lines_are_ascent_plus_descent() {
    for (font, sizes) in [("Times New Roman", [14.0, 12.0, 10.0]), ("Arial", [14.0, 11.0, 10.0])] {
        let (_, bare) = line_em(font);
        for size in sizes {
            for mode in [11, 12, 14] {
                assert_pitch(font, size, LineSpacing::Multiple(1.0), true, mode, bare * size);
            }
        }
    }
}

#[test]
fn a_font_without_a_line_gap_keeps_its_line() {
    let (full, bare) = line_em("Courier New");
    assert!((full - bare).abs() < 0.001, "{full} vs {bare}");
    assert_pitch("Courier New", 14.0, LineSpacing::Multiple(1.0), true, 11, full * 14.0);
    assert_pitch("Courier New", 10.0, LineSpacing::Multiple(1.0), true, 11, full * 10.0);
}

#[test]
fn multiple_spacing_multiplies_the_smaller_line() {
    let (_, times) = line_em("Times New Roman");
    assert_pitch("Times New Roman", 14.0, LineSpacing::Multiple(1.5), true, 11, 1.5 * 14.0 * times);
    assert_pitch("Times New Roman", 14.0, LineSpacing::Multiple(2.0), true, 11, 2.0 * 14.0 * times);
    let (_, arial) = line_em("Arial");
    assert_pitch("Arial", 14.0, LineSpacing::Multiple(1.15), true, 11, 1.15 * 14.0 * arial);
}

#[test]
fn at_least_compares_against_the_smaller_line() {
    let (_, bare) = line_em("Times New Roman");
    assert_pitch("Times New Roman", 14.0, LineSpacing::AtLeast(12.0), true, 11, bare * 14.0);
    assert_pitch("Times New Roman", 14.0, LineSpacing::AtLeast(20.0), true, 11, 20.0);
}

#[test]
fn exact_spacing_is_unchanged() {
    assert_pitch("Times New Roman", 14.0, LineSpacing::Exactly(14.0), true, 11, 14.0);
    assert_pitch("Times New Roman", 14.0, LineSpacing::Exactly(14.0), false, 11, 14.0);
}

#[test]
fn compatibility_mode_15_ignores_the_option() {
    for font in ["Times New Roman", "Arial"] {
        let (full, _) = line_em(font);
        assert_pitch(font, 14.0, LineSpacing::Multiple(1.0), true, 15, full * 14.0);
        assert_pitch(font, 14.0, LineSpacing::Multiple(1.5), true, 15, 1.5 * full * 14.0);
        assert_pitch(font, 14.0, LineSpacing::AtLeast(12.0), true, 15, full * 14.0);
    }
}

#[test]
fn without_the_option_lines_keep_the_gap() {
    for font in ["Times New Roman", "Arial"] {
        let (full, _) = line_em(font);
        for mode in [11, 14, 15] {
            assert_pitch(font, 14.0, LineSpacing::Multiple(1.0), false, mode, full * 14.0);
            assert_pitch(font, 10.0, LineSpacing::Multiple(1.0), false, mode, full * 10.0);
        }
    }
}

/// Word's numbers for Times New Roman (2268/2048 em without the gap): 15.50, 13.29 and 11.07 pt.
#[test]
fn times_new_roman_matches_words_pitches() {
    let (_, bare) = line_em("Times New Roman");
    if (bare - 2268.0 / 2048.0).abs() > 0.0005 {
        return; // a substitute with other metrics: the tests above still hold
    }
    assert_pitch("Times New Roman", 14.0, LineSpacing::Multiple(1.0), true, 11, 15.50);
    assert_pitch("Times New Roman", 12.0, LineSpacing::Multiple(1.0), true, 11, 13.29);
    assert_pitch("Times New Roman", 10.0, LineSpacing::Multiple(1.0), true, 11, 11.07);
    assert_pitch("Times New Roman", 14.0, LineSpacing::Multiple(1.5), true, 11, 23.25);
    assert_pitch("Times New Roman", 14.0, LineSpacing::Multiple(2.0), true, 11, 31.00);
}
