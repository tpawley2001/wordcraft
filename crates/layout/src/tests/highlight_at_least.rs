//! At-least spacing leaves extra room above the single-spaced character background.
use super::*;
use crate::display::{Draw, page_display};
use wordcraft_doc::props::{Highlight, LineSpacing};

// The same pinned font as highlight_band; expectations come from its actual metrics.
const FONT: &str = "Source Serif 4";

fn single_height(size: f32) -> f32 {
    crate::test_fonts();
    let face = wordcraft_fonts::word::resolve(FONT, false, false).face;
    let (a, d) = wordcraft_fonts::word::line_metrics(&face);
    ((a + d) * f64::from(size) / face.upem.max(1.0)) as f32
}

fn check(spacing: LineSpacing, tallest: f32, shading: bool) {
    crate::test_fonts();
    let plain = CharProps { font: Some(FONT.into()), size: Some(tallest), ..Default::default() };
    let marked = CharProps {
        font: Some(FONT.into()),
        size: Some(12.0),
        highlight: (!shading).then_some(Highlight::Yellow),
        shading: shading.then_some(wordcraft_doc::Rgb(255, 255, 0)),
        ..Default::default()
    };
    let mut p = Paragraph::with_text("Plain ", plain);
    p.append(Paragraph::with_text("Hxgabc", marked));
    p.props.line_spacing = Some(spacing);
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.body = vec![wordcraft_doc::para_block(p)];
    let l = lay(&d);
    let page = l.pages.first().expect("one page");
    let (top, height) = page
        .items
        .iter()
        .find_map(|item| {
            let Placed::Lines { para, l0, y, .. } = item else { return None };
            let line = para.lines.get(*l0)?;
            Some((*y, line.height))
        })
        .expect("one laid-out line");
    let natural = single_height(tallest);
    let (pitch, band, band_top) = match spacing {
        LineSpacing::AtLeast(min) => (natural.max(min), natural, top + height - natural),
        LineSpacing::Exactly(h) => (h, h, top),
        LineSpacing::Multiple(m) => (natural * m, natural, top),
    };
    assert!((height - pitch).abs() < 0.1, "line pitch {height}, expected {pitch}");
    let draws = page_display(&d, page, &Default::default());
    let bands: Vec<_> = draws
        .iter()
        .filter_map(|draw| match draw {
            Draw::Fill { rect, text_bg: true, .. } => Some(rect),
            _ => None,
        })
        .collect();
    assert!(!bands.is_empty(), "character backgrounds are drawn");
    for rect in bands {
        assert!((rect.h - band).abs() < 0.1, "band {rect:?}, expected height {band}");
        assert!((rect.y - band_top).abs() < 0.1, "band {rect:?}, expected top {band_top}");
        if matches!(spacing, LineSpacing::AtLeast(_)) {
            assert!((rect.y + rect.h - top - height).abs() < 0.01, "band bottom equals line bottom: {rect:?}");
        }
    }
}

#[test]
fn at_least_24_bottom_aligns_the_single_spaced_band() {
    check(LineSpacing::AtLeast(24.0), 12.0, false);
}

#[test]
fn at_least_18_bottom_aligns_the_single_spaced_band() {
    check(LineSpacing::AtLeast(18.0), 12.0, false);
}

#[test]
fn at_least_8_still_fills_the_whole_line() {
    check(LineSpacing::AtLeast(8.0), 12.0, false);
}

#[test]
fn exactly_24_still_fills_the_whole_line() {
    check(LineSpacing::Exactly(24.0), 12.0, false);
}

#[test]
fn double_spacing_keeps_its_single_spaced_band_at_the_top() {
    check(LineSpacing::Multiple(2.0), 12.0, false);
}

#[test]
fn mixed_sizes_use_the_tallest_text() {
    check(LineSpacing::AtLeast(48.0), 24.0, false);
}

#[test]
fn run_shading_uses_the_same_bottom_aligned_band() {
    check(LineSpacing::AtLeast(24.0), 12.0, true);
}
