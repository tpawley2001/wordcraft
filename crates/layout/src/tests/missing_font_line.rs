//! A missing Lucida Console spaces its lines as Word does with the real font (one em), not like
//! its Courier-New-like substitute; an installed font keeps its own line.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;

/// The tops of the paragraphs of three single-spaced 10 pt paragraphs in `family`.
fn para_tops(family: &str) -> Vec<f32> {
    let chr = wordcraft_doc::CharProps { font: Some(family.into()), size: Some(10.0), ..Default::default() };
    let mut d = Document::new();
    d.body = ["for i in 0..3 {", "    step(i);", "}"]
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
    l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { y, .. } = it { Some(*y) } else { None }).collect()
}

fn pitches(family: &str) -> Vec<f32> {
    let tops = para_tops(family);
    assert_eq!(tops.len(), 3, "{family}: {tops:?}");
    tops.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn a_missing_lucida_console_line_is_one_em() {
    if !wordcraft_fonts::word::resolve("Lucida Console", false, false).substituted {
        return;
    }
    for pitch in pitches("Lucida Console") {
        assert!((pitch - 10.0).abs() <= 0.05, "{pitch}");
    }
}

#[test]
fn courier_new_keeps_its_own_line() {
    let face = wordcraft_fonts::word::resolve("Courier New", false, false).face;
    let (a, d) = wordcraft_fonts::word::line_metrics(&face);
    let expected = ((a + d) / face.upem * 10.0) as f32;
    for pitch in pitches("Courier New") {
        assert!((pitch - expected).abs() <= 0.05, "{pitch} vs {expected}");
    }
}
