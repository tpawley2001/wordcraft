//! Missing Tahoma and Lucida Bright space single-spaced lines as Word does with the real fonts
//! (1.207 em and 1.172 em: 14.484 pt and 14.064 pt at 12 pt), not like their substitutes.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;

/// The distances between the tops of three single-spaced 12 pt paragraphs in `family`.
fn pitches(family: &str) -> Vec<f32> {
    let chr = wordcraft_doc::CharProps { font: Some(family.into()), size: Some(12.0), ..Default::default() };
    let mut d = Document::new();
    d.body = ["First line of text.", "Second line of text.", "Third line of text."]
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
    assert_eq!(tops.len(), 3, "{family}: {tops:?}");
    tops.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn a_missing_tahoma_line_is_1_207_em() {
    if !wordcraft_fonts::word::resolve("Tahoma", false, false).substituted {
        return;
    }
    for pitch in pitches("Tahoma") {
        assert!((pitch - 14.484).abs() <= 0.05, "{pitch}");
    }
}

#[test]
fn a_missing_lucida_bright_line_is_1_172_em() {
    if !wordcraft_fonts::word::resolve("Lucida Bright", false, false).substituted {
        return;
    }
    for pitch in pitches("Lucida Bright") {
        assert!((pitch - 14.064).abs() <= 0.05, "{pitch}");
    }
}
