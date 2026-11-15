//! Text in Verdana, Tahoma, Century Gothic, SimSun, Aptos, Aptos Display, Trebuchet MS or
//! Garamond, when the font isn't installed, is drawn
//! with a substitute but keeps the advances Word gives the real font
//! (`wordcraft_fonts::word_widths::missing_font_advance`), so lines hold the words Word's do.

use std::sync::Arc;

use wordcraft_doc::{CharProps, Document, Paragraph, para_block};
use wordcraft_fonts::word_widths::missing_font_advance;

use crate::para::ParaLayout;
use crate::{LayoutCache, LayoutOptions, Placed, layout};

/// `text` in `font` at 10 pt (bold / italic) alone in a column `column` points wide, laid out.
fn lay_out(text: &str, font: &str, bold: bool, italic: bool, column: f32) -> Arc<ParaLayout> {
    crate::test_fonts();
    let chr = CharProps { font: Some(font.into()), size: Some(10.0), bold: Some(bold), italic: Some(italic), ..Default::default() };
    let mut p = Paragraph::with_text(text, chr);
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + column;
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    l.pages.iter().flat_map(|p| &p.items).find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap()
}

/// Whether `family` is missing here (drawn from a substitute); prints why a test is skipped.
fn missing(family: &str) -> bool {
    crate::test_fonts();
    let r = wordcraft_fonts::word::resolve(family, false, false);
    let missing = r.substituted && r.face.upem > 0.0;
    if !missing {
        eprintln!("skipped: {family} is installed here (or nothing resolves for it)");
    }
    missing
}

fn run_width(text: &str, font: &str, bold: bool, italic: bool) -> f32 {
    lay_out(text, font, bold, italic, 1000.0).clusters.iter().map(|c| c.adv).sum()
}

#[test]
fn runs_take_words_widths() {
    let cases: &[(&str, bool, bool, &str, f32)] = &[
        ("Verdana", false, false, "Hello World", 57.68),
        ("Verdana", true, false, "Hello World", 65.67),
        ("Tahoma", false, false, "Parity test", 44.50),
        ("Century Gothic", false, false, "Page 12", 39.83),
        ("SimSun", false, false, "abc 123", 35.00),
        ("Verdana", false, true, "Quick", 28.02),
    ];
    for &(font, bold, italic, text, want) in cases {
        if !missing(font) {
            continue;
        }
        let got = run_width(text, font, bold, italic);
        assert!((got - want).abs() < 0.01, "{font} b={bold} i={italic} {text:?}: {got} vs {want}");
    }
}

#[test]
fn aptos_trebuchet_and_garamond_runs_take_words_widths() {
    // 10 pt: the table sum (1/1000 em) / 100.
    let cases: &[(&str, bool, bool, &str, f32)] = &[
        ("Aptos", false, false, "Meeting notes", 60.83),
        ("Aptos", true, false, "Total 2024", 46.20),
        ("Aptos", false, true, "Drawn", 28.22),
        ("Aptos Display", true, false, "Chapter 3", 41.81),
        ("Trebuchet MS", false, false, "Parity test", 46.89),
        ("Trebuchet MS", true, false, "Annual report", 65.07),
        ("Garamond", true, false, "Chapter 1", 40.32),
    ];
    for &(font, bold, italic, text, want) in cases {
        if !missing(font) {
            continue;
        }
        let got = run_width(text, font, bold, italic);
        assert!((got - want).abs() < 0.01, "{font} b={bold} i={italic} {text:?}: {got} vs {want}");
    }
}

#[test]
fn a_verdana_paragraph_breaks_where_words_widths_put_it() {
    if !missing("Verdana") {
        return;
    }
    let words = 40usize;
    let column = 6.5 * 72.0;
    let text = vec!["Verdana"; words].join(" ");
    let pl = lay_out(&text, "Verdana", false, false, column);
    // Greedy fit by the table: a word is 10 pt x its advances, a space 3.52 pt.
    let em = |c: char| missing_font_advance("Verdana", false, false, c).unwrap() * 10.0;
    let word: f64 = "Verdana".chars().map(em).sum();
    let space = em(' ');
    let (mut lines, mut x) = (1usize, 0.0f64);
    for _ in 0..words {
        let start = if x == 0.0 { 0.0 } else { x + space };
        if x > 0.0 && start + word > f64::from(column) + 1e-6 {
            lines += 1;
            x = word;
        } else {
            x = start + word;
        }
    }
    assert!(lines > 1);
    assert_eq!(pl.lines.len(), lines, "{} lines, the table gives {lines}", pl.lines.len());
}
