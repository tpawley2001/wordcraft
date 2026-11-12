//! "Don't add leading (extra space) between rows of text" (`Settings::no_leading`, below
//! compatibility mode 15) and fonts that are not installed. Measured in Microsoft 365 Word's PDFs
//! of synthetic single-spaced documents: Century Gothic lines are 11.91 / 14.30 / 23.83 / 42.90 pt
//! apart at 10 / 12 / 20 / 36 pt (1.1916 em; 1.226 em without the option), Lucida Bright lines
//! 11.38 / 22.77 / 41.00 pt at 10 / 20 / 36 pt (1.1386 em), and Palatino Linotype (16.20 pt at
//! 12 pt), Consolas, Georgia, Tahoma and Verdana lines do not change. Mode 15 ignores the option.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;

/// The distances between the tops of four single-spaced paragraphs in `font` at `size`.
fn pitches(font: &str, size: f32, no_leading: bool, mode: u32) -> Vec<f32> {
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
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            wordcraft_doc::para_block(p)
        })
        .collect();
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    let tops: Vec<f32> =
        l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect();
    assert_eq!(tops.len(), 4, "{font}: {tops:?}");
    tops.windows(2).map(|w| w[1] - w[0]).collect()
}

/// True when `font` is not installed here (the tests are about missing fonts only).
fn missing(font: &str) -> bool {
    crate::test_fonts();
    wordcraft_fonts::word::resolve(font, false, false).substituted
}

fn assert_pitch(font: &str, size: f32, no_leading: bool, mode: u32, expected: f32) {
    for pitch in pitches(font, size, no_leading, mode) {
        assert!((pitch - expected).abs() <= 0.05, "{font} {size} no_leading {no_leading} mode {mode}: {pitch} vs {expected}");
    }
}

#[test]
fn a_missing_century_gothic_loses_its_leading() {
    if !missing("Century Gothic") {
        return;
    }
    for (size, expected) in [(10.0, 11.92), (12.0, 14.30), (20.0, 23.83), (36.0, 42.90)] {
        for mode in [11, 12, 14] {
            assert_pitch("Century Gothic", size, true, mode, expected);
        }
    }
}

#[test]
fn a_missing_lucida_bright_loses_its_leading() {
    if !missing("Lucida Bright") {
        return;
    }
    for (size, expected) in [(10.0, 11.39), (12.0, 13.66), (20.0, 22.77), (36.0, 40.99)] {
        for mode in [11, 12, 14] {
            assert_pitch("Lucida Bright", size, true, mode, expected);
        }
    }
}

#[test]
fn mode_15_and_documents_without_the_option_keep_the_leading() {
    for (font, em) in [("Century Gothic", 1.226_f32), ("Lucida Bright", 1.172)] {
        if !missing(font) {
            continue;
        }
        for size in [12.0_f32, 36.0] {
            assert_pitch(font, size, true, 15, em * size);
            assert_pitch(font, size, false, 12, em * size);
            assert_pitch(font, size, false, 15, em * size);
        }
    }
}

#[test]
fn the_other_missing_fonts_keep_their_line_under_the_option() {
    for (font, em) in [("Palatino Linotype", 1.35_f32), ("Consolas", 1.172), ("Georgia", 1.136), ("Tahoma", 1.207), ("Verdana", 1.215)] {
        if !missing(font) {
            continue;
        }
        for mode in [12, 15] {
            assert_pitch(font, 12.0, true, mode, em * 12.0);
            assert_pitch(font, 12.0, false, mode, em * 12.0);
        }
    }
}
