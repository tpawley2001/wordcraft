//! Missing Verdana, Georgia, Trebuchet MS, Century Gothic, Consolas, Garamond and Comic Sans MS
//! space single-spaced lines as Word does with the real fonts (Word's PDFs), not like their
//! substitutes: at 12 pt, 14.58, 13.632, 13.932, 14.712, 14.064, 13.5 and 16.728 pt apart.

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

const WORD_PITCH: [(&str, f32); 7] = [
    ("Verdana", 14.58),
    ("Georgia", 13.632),
    ("Trebuchet MS", 13.932),
    ("Century Gothic", 14.712),
    ("Consolas", 14.064),
    ("Garamond", 13.5),
    ("Comic Sans MS", 16.728),
];

#[test]
fn missing_windows_fonts_space_lines_as_word() {
    crate::test_fonts();
    for (family, expected) in WORD_PITCH {
        if !wordcraft_fonts::word::resolve(family, false, false).substituted {
            continue;
        }
        for pitch in pitches(family) {
            assert!((pitch - expected).abs() <= 0.05, "{family}: {pitch} (Word {expected})");
        }
    }
}

#[test]
fn guard_missing_tahoma_keeps_its_line() {
    crate::test_fonts();
    if !wordcraft_fonts::word::resolve("Tahoma", false, false).substituted {
        return;
    }
    for pitch in pitches("Tahoma") {
        assert!((pitch - 14.484).abs() <= 0.05, "{pitch}");
    }
}
