//! A missing Univers lays out as Word does with the real font (Word's PDFs of Univers documents):
//! single-spaced 12 pt lines 14.88 pt apart, and text as wide as Univers' own advances, so lines
//! wrap where Word wraps them. Skipped where Univers is installed.

use crate::{Document, LayoutCache, LayoutOptions, Placed, layout};
use wordcraft_doc::props::LineSpacing;

fn univers_missing() -> bool {
    crate::test_fonts();
    wordcraft_fonts::word::resolve("Univers", false, false).substituted
}

fn chr(family: &str, bold: bool, italic: bool) -> wordcraft_doc::CharProps {
    wordcraft_doc::CharProps { font: Some(family.into()), size: Some(12.0), bold: Some(bold), italic: Some(italic), ..Default::default() }
}

/// The laid-out paragraphs of `texts`, single-spaced with no paragraph spacing, in a column
/// `column` points wide.
fn laid_out(texts: &[&str], chr: wordcraft_doc::CharProps, column: f32) -> Vec<std::sync::Arc<crate::para::ParaLayout>> {
    let mut d = Document::new();
    d.body = texts
        .iter()
        .map(|t| {
            let mut p = wordcraft_doc::Paragraph::with_text(t, chr.clone());
            p.props.space_before = Some(0.0);
            p.props.space_after = Some(0.0);
            p.props.line_spacing = Some(LineSpacing::Multiple(1.0));
            wordcraft_doc::para_block(p)
        })
        .collect();
    d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + column;
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    l.pages.iter().flat_map(|p| &p.items).filter_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).collect()
}

/// The summed advance of the first line of `text`.
fn line_width(text: &str, chr: wordcraft_doc::CharProps) -> f32 {
    let paras = laid_out(&[text], chr, 1000.0);
    let para = &paras[0];
    let line = &para.lines[0];
    para.clusters[line.c0..line.c1].iter().map(|c| c.adv).sum()
}

#[test]
fn missing_univers_spaces_lines_as_word() {
    if !univers_missing() {
        return;
    }
    let mut d = Document::new();
    d.body = ["First line of text.", "Second line of text.", "Third line of text."]
        .iter()
        .map(|t| {
            let mut p = wordcraft_doc::Paragraph::with_text(t, chr("Univers", false, false));
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
    assert_eq!(tops.len(), 3, "{tops:?}");
    for w in tops.windows(2) {
        assert!((w[1] - w[0] - 14.88).abs() <= 0.05, "{tops:?} (Word 14.88 apart)");
    }
}

#[test]
fn missing_univers_text_is_as_wide_as_words() {
    if !univers_missing() {
        return;
    }
    // Sums of Word's advances at 12 pt; italic takes the regular advances.
    for (bold, italic, expected) in [(false, false, 159.36), (true, false, 160.704), (false, true, 159.36)] {
        let got = line_width("current revision came about", chr("Univers", bold, italic));
        assert!((got - expected).abs() <= 0.1, "b{bold} i{italic}: {got} (Word {expected})");
    }
}

#[test]
fn missing_univers_wraps_where_word_does() {
    if !univers_missing() {
        return;
    }
    // In Univers the words up to "late" are 454.776 pt and " 1970s." makes 494.136 pt: Word wraps
    // "1970s." in a 468 pt column (6.5 in, Letter with 1 in margins).
    let text = "The First Responder: National Standard Curriculum was last reviewed in the late 1970s.";
    let paras = laid_out(&[text], chr("Univers", false, false), 468.0);
    let para = &paras[0];
    assert_eq!(para.lines.len(), 2, "two lines, as in Word");
    let second = &para.lines[1];
    let start = para.clusters[second.c0].start;
    let rest: String = text.chars().skip(start).collect();
    assert!(rest.starts_with("1970s."), "second line starts {rest:?}");
}

#[test]
fn guard_installed_metric_fonts_keep_their_advances() {
    crate::test_fonts();
    // Arial is in no table: "nnnn" keeps the drawn face's own advances (the pinned test fonts'
    // Arial is about 0.547 em), never Univers' 4 x 0.611 em = 29.328 pt.
    let one = line_width("n", chr("Arial", false, false));
    let got = line_width("nnnn", chr("Arial", false, false));
    assert!((got - 4.0 * one).abs() <= 0.1 && got < 28.0, "{got} ({one} each)");
}
