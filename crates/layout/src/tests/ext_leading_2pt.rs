//! A Word 97-2003 file with `Copts80.f2ptExtLeadingOnly` (`Settings::ext_leading_2pt_only`) lays a
//! Courier New line out one em tall. Measured in Microsoft 365 Word's PDFs of two such files:
//! 12 pt Courier New lines are 12.0 pt apart when single or "at least 12 pt" spaced and 24.0 pt
//! apart when double spaced; files without the flag keep the font's own 13.6 pt line.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;

/// The distances between the tops of three 12 pt Courier New paragraphs spaced `spacing`.
fn pitches(flag: bool, spacing: LineSpacing) -> Vec<f32> {
    let chr = wordcraft_doc::CharProps { font: Some("Courier New".into()), size: Some(12.0), ..Default::default() };
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
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    let tops: Vec<f32> =
        l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect();
    assert_eq!(tops.len(), 3, "{tops:?}");
    tops.windows(2).map(|w| w[1] - w[0]).collect()
}

fn assert_pitch(flag: bool, spacing: LineSpacing, expected: f32) {
    for pitch in pitches(flag, spacing) {
        assert!((pitch - expected).abs() <= 0.05, "{spacing:?} flag {flag}: {pitch} vs {expected}");
    }
}

#[test]
fn courier_new_single_line_is_one_em() {
    assert_pitch(true, LineSpacing::Multiple(1.0), 12.0);
}

#[test]
fn courier_new_double_line_is_two_em() {
    assert_pitch(true, LineSpacing::Multiple(2.0), 24.0);
}

#[test]
fn courier_new_at_least_its_size_is_one_em() {
    assert_pitch(true, LineSpacing::AtLeast(12.0), 12.0);
    assert_pitch(true, LineSpacing::AtLeast(15.0), 15.0);
}

#[test]
fn without_the_flag_courier_new_keeps_its_own_line() {
    let face = wordcraft_fonts::word::resolve("Courier New", false, false).face;
    let (a, d) = wordcraft_fonts::word::line_metrics(&face);
    let own = ((a + d) / face.upem * 12.0) as f32;
    assert!(own > 13.0, "{own}");
    assert_pitch(false, LineSpacing::Multiple(1.0), own);
    assert_pitch(false, LineSpacing::AtLeast(12.0), own);
}
