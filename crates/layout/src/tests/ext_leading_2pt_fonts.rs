//! A Word 97-2003 file with `Copts80.f2ptExtLeadingOnly` (`Settings::ext_leading_2pt_only`): every
//! line of a font other than Courier New is its font size plus 2 pt tall, with the font's own
//! descent below the baseline and the rest above it. Measured in Microsoft 365 Word's PDFs of 19
//! such files: Times New Roman 10 / 11 / 12 pt lines are 12.0 / 13.0 / 14.0 pt apart (11.5 / 12.65 /
//! 13.8 without the flag), a missing font (Univers) 12 pt 14.0 and 24 pt 26.0, double-spaced 12 pt
//! 28.0; a 10 pt Times New Roman baseline is 9.86 pt below its line's top and a 12 pt one 11.41.
//! Courier New keeps its one-em line (`ext_leading_2pt.rs`).

use crate::*;
use wordcraft_doc::props::LineSpacing;

/// Three paragraphs of `font` at `size`, spaced `spacing`, no space before or after.
fn doc(flag: bool, font: &str, size: f32, bold: bool, spacing: LineSpacing) -> Document {
    let chr = wordcraft_doc::CharProps { font: Some(font.into()), size: Some(size), bold: Some(bold), ..Default::default() };
    let mut d = Document::new();
    d.settings.ext_leading_2pt_only = flag;
    d.body = ["First line", "Second line", "Third line"]
        .iter()
        .map(|t| {
            let mut p = wordcraft_doc::Paragraph::with_text(t, chr.clone());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(spacing);
            wordcraft_doc::para_block(p)
        })
        .collect();
    d
}

/// The tops of the three paragraphs' lines.
fn tops(d: &Document) -> Vec<f32> {
    let mut c = LayoutCache::new();
    let l = layout(d, &mut c, &LayoutOptions::default());
    let tops: Vec<f32> =
        l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect();
    assert_eq!(tops.len(), 3, "{tops:?}");
    tops
}

fn assert_pitch(flag: bool, font: &str, size: f32, bold: bool, spacing: LineSpacing, expected: f32) {
    let tops = tops(&doc(flag, font, size, bold, spacing));
    for w in tops.windows(2) {
        let pitch = w[1] - w[0];
        assert!((pitch - expected).abs() <= 0.05, "{font} {size} pt bold {bold} {spacing:?} flag {flag}: {pitch} vs {expected}");
    }
}

/// The baseline of the first drawn glyph on page 1, measured from the top of the first line.
fn first_baseline_below_line_top(d: &Document) -> f32 {
    let mut c = LayoutCache::new();
    let l = layout(d, &mut c, &LayoutOptions::default());
    let top = l.pages[0].items.iter().find_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).expect("a line");
    for i in display::page_display(d, &l.pages[0], &Default::default()) {
        if let display::Draw::Glyphs { glyphs, .. } = &i
            && let Some(g) = glyphs.first()
        {
            return g.2 - top;
        }
    }
    panic!("no glyph drawn");
}

const SINGLE: LineSpacing = LineSpacing::Multiple(1.0);

#[test]
fn times_new_roman_lines_are_their_size_plus_2pt() {
    // Word: 12.0, 13.0, 14.0 (11.5, 12.65, 13.8 without the option).
    assert_pitch(true, "Times New Roman", 10.0, false, SINGLE, 12.0);
    assert_pitch(true, "Times New Roman", 11.0, false, SINGLE, 13.0);
    assert_pitch(true, "Times New Roman", 12.0, false, SINGLE, 14.0);
}

#[test]
fn bold_text_and_other_fonts_follow_the_same_rule() {
    assert_pitch(true, "Times New Roman", 10.0, true, SINGLE, 12.0);
    assert_pitch(true, "Times New Roman", 12.0, true, SINGLE, 14.0);
    assert_pitch(true, "Arial Narrow", 12.0, true, SINGLE, 14.0);
}

#[test]
fn a_font_that_is_not_installed_follows_the_rule_too() {
    // Word: Univers (not installed there either) 10 pt 12.0, 12 pt 14.0, 24 pt bold 26.0.
    assert_pitch(true, "Univers", 10.0, false, SINGLE, 12.0);
    assert_pitch(true, "Univers", 12.0, false, SINGLE, 14.0);
    assert_pitch(true, "Univers", 24.0, true, SINGLE, 26.0);
}

#[test]
fn line_spacing_works_on_the_size_plus_2pt() {
    // Word: double-spaced 12 pt Times New Roman lines are 28.0 pt apart; "at least 12 pt" 14.0.
    assert_pitch(true, "Times New Roman", 12.0, false, LineSpacing::Multiple(2.0), 28.0);
    assert_pitch(true, "Times New Roman", 12.0, false, LineSpacing::AtLeast(12.0), 14.0);
    assert_pitch(true, "Times New Roman", 12.0, false, LineSpacing::AtLeast(20.0), 20.0);
    assert_pitch(true, "Times New Roman", 12.0, false, LineSpacing::Exactly(12.0), 12.0);
}

/// Times New Roman's own (ascent + descent with its line gap, descent) in points at `size`, from
/// the face the layout resolves for it here (a metric-compatible or fallback face).
fn times_own(size: f32) -> (f32, f32) {
    let face = wordcraft_fonts::word::resolve("Times New Roman", false, false).face;
    let (a, d) = wordcraft_fonts::word::line_metrics(&face);
    let (_, d0) = wordcraft_fonts::word::line_metrics_no_leading(&face);
    let k = f64::from(size) / face.upem.max(1.0);
    (((a + d) * k) as f32, (d0 * k) as f32)
}

#[test]
fn the_extra_room_goes_above_the_letters() {
    // Word: a 10 pt Times New Roman baseline is 9.86 pt below the line's top (12 - 0.216 x 10 =
    // 9.84) and a 12 pt one 11.41 (14 - 0.216 x 12): the line's height less the font's descent.
    for size in [10.0_f32, 12.0] {
        let (_, descent) = times_own(size);
        let expected = size + 2.0 - descent;
        let got = first_baseline_below_line_top(&doc(true, "Times New Roman", size, false, SINGLE));
        assert!((got - expected).abs() <= 0.05, "{size} pt baseline {got} below the line top, expected {expected}");
    }
}

#[test]
fn courier_new_keeps_its_one_em_line() {
    assert_pitch(true, "Courier New", 12.0, false, SINGLE, 12.0);
    assert_pitch(true, "Courier New", 10.0, false, SINGLE, 10.0);
}

#[test]
fn without_the_option_lines_keep_the_fonts_own_height() {
    // Word, files without the option: Times New Roman 12 pt 13.8, 10 pt 11.5 (the font's own line).
    for size in [10.0_f32, 12.0] {
        let (own, _) = times_own(size);
        assert!((own - (size + 2.0)).abs() > 0.05, "the face's own {size} pt line must differ from size + 2: {own}");
        assert_pitch(false, "Times New Roman", size, false, SINGLE, own);
        assert_pitch(false, "Times New Roman", size, false, LineSpacing::Multiple(2.0), 2.0 * own);
    }
}
