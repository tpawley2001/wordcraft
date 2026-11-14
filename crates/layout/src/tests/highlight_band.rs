//! How tall a highlight (or character shading) is, as Microsoft 365 Word draws it. Measured
//! 2026-11-14 in Word's PDF of hand-written DOCX files (Times New Roman, highlight yellow):
//! | paragraph | band above / below the baseline (pt) | line pitch |
//! |---|---|---|
//! | single, 12 pt | 11.16 / 2.64 (the font's line: ascent + leading / descent) | 13.80 |
//! | double (`w:line="480" auto`), 12 pt | 11.16 / 2.64: the single line, NOT the doubled one | 27.6 |
//! | 12 pt run with 12 pt space before and after | 11.16 / 2.64: no paragraph spacing | |
//! | exactly 30 pt, 12 pt | 24.00 / 6.00: the whole exact line | 30 |
//! | exactly 8 pt, 12 pt | 6.48 / 1.56: the whole exact line | 8 |
//! | 10 pt run beside a 24 pt run, single | 22.44 / 5.16: the whole line (the 24 pt text's) | |
//! So the band is the line's height at single spacing: extra from "multiple" line spacing is
//! not filled; an exact line is filled whole.
use crate::display::{Draw, page_display};
use crate::*;
use wordcraft_doc::para_block;
use wordcraft_doc::props::{Highlight, LineSpacing};

const FONT: &str = "Source Serif 4";

/// Paragraph "Plain Marked" (the second word highlighted) at 12 pt with `spacing`, space
/// before/after `space`: (baseline, [(band top above the baseline, band bottom below it)]).
fn lay(spacing: Option<LineSpacing>, space: f32, big_first: bool) -> (f32, Vec<(f32, f32)>) {
    crate::test_fonts();
    let mut d = Document::new();
    let plain = CharProps { font: Some(FONT.into()), size: Some(if big_first { 24.0 } else { 12.0 }), ..CharProps::default() };
    let marked = CharProps { font: Some(FONT.into()), size: Some(12.0), highlight: Some(Highlight::Yellow), ..CharProps::default() };
    let mut p = Paragraph::with_text("Plain ", plain);
    p.append(Paragraph::with_text("Marked", marked));
    p.props.line_spacing = spacing;
    p.props.space_before = Some(space);
    p.props.space_after = Some(space);
    d.body = vec![
        para_block(Paragraph::with_text("Above", CharProps { font: Some(FONT.into()), size: Some(12.0), ..CharProps::default() })),
        para_block(p),
    ];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let draws = page_display(&d, page, &Default::default());
    let base = draws
        .iter()
        .find_map(|d| match d {
            Draw::Glyphs { glyphs, text, .. } if text.contains("Marked") => glyphs.first().map(|g| g.2),
            _ => None,
        })
        .expect("the highlighted text is drawn");
    let bands = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Fill { rect, text_bg: true, .. } => Some((base - rect.y, rect.y + rect.h - base)),
            _ => None,
        })
        .collect();
    (base, bands)
}

fn one(bands: &[(f32, f32)]) -> (f32, f32) {
    assert!(!bands.is_empty(), "the highlight is drawn");
    let first = bands[0];
    for b in bands {
        assert!((b.0 - first.0).abs() < 0.01 && (b.1 - first.1).abs() < 0.01, "one band height across the run: {bands:?}");
    }
    first
}

#[test]
fn double_spacing_highlights_only_the_single_line() {
    let (_, single) = lay(None, 0.0, false);
    let single = one(&single);
    for k in [1.5f32, 2.0, 3.0] {
        let (_, b) = lay(Some(LineSpacing::Multiple(k)), 0.0, false);
        let b = one(&b);
        assert!((b.0 - single.0).abs() < 0.05 && (b.1 - single.1).abs() < 0.05, "x{k}: band {b:?}, want the single-spaced {single:?}");
    }
}

#[test]
fn paragraph_spacing_is_not_highlighted() {
    let (_, single) = lay(None, 0.0, false);
    let (_, spaced) = lay(None, 12.0, false);
    let (a, b) = (one(&single), one(&spaced));
    assert!((a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05, "{b:?} vs {a:?}");
}

#[test]
fn an_exact_line_is_highlighted_whole() {
    for h in [30.0f32, 8.0] {
        let (_, b) = lay(Some(LineSpacing::Exactly(h)), 0.0, false);
        let b = one(&b);
        assert!((b.0 + b.1 - h).abs() < 0.05, "exactly {h}: band {b:?} should fill the line");
    }
}

/// A small highlighted run beside large text fills the whole (single-spaced) line, as now; under
/// double spacing it still fills only that single line.
#[test]
fn a_mixed_size_line_is_highlighted_to_its_tallest_text() {
    let (_, big) = lay(None, 0.0, true);
    let big = one(&big);
    let (_, small) = lay(None, 0.0, false);
    let small = one(&small);
    assert!(big.0 > small.0 * 1.8, "the band reaches the 24 pt text's top: {big:?} vs {small:?}");
    let (_, dbl) = lay(Some(LineSpacing::Multiple(2.0)), 0.0, true);
    let dbl = one(&dbl);
    assert!((dbl.0 - big.0).abs() < 0.05 && (dbl.1 - big.1).abs() < 0.05, "{dbl:?} vs {big:?}");
}
