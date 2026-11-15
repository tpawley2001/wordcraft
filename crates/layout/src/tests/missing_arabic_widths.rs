//! Arabic text in Traditional Arabic, when the font isn't installed, is drawn with a substitute
//! but each letter keeps the advance Word gives its joining form
//! (`wordcraft_fonts::word_widths_arabic`), so Arabic lines hold the words Word's do.

use std::sync::Arc;

use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::para::ParaLayout;
use crate::{LayoutCache, LayoutOptions, Placed, layout};

/// beh-lam-meem, meem-alef, beh-lam-alef.
const TEXT: &str = "\u{628}\u{644}\u{645} \u{645}\u{627} \u{628}\u{644}\u{627}";

/// Paragraphs of `runs` (text, complex-script font) at 14 pt, right to left, laid out.
fn lay_out(runs: &[(&str, &str)]) -> Arc<ParaLayout> {
    crate::test_fonts();
    let mut p = Paragraph::default();
    for &(text, font) in runs {
        let chr = CharProps {
            font: Some("Times New Roman".into()),
            font_cs: Some(font.into()),
            size: Some(14.0),
            size_cs: Some(14.0),
            rtl: Some(true),
            ..Default::default()
        };
        p.append(Paragraph::with_text(text, chr));
    }
    p.props.bidi = Some(true);
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    l.pages.iter().flat_map(|p| &p.items).find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap()
}

fn natural_width(para: &ParaLayout) -> f32 {
    assert_eq!(para.lines.len(), 1);
    para.clusters.iter().map(|c| c.adv).sum()
}

fn missing(family: &str) -> bool {
    crate::test_fonts();
    let r = wordcraft_fonts::word::resolve(family, false, false);
    let missing = r.substituted && r.face.upem > 0.0;
    if !missing {
        eprintln!("skipped: {family} is installed here (or nothing resolves for it)");
    }
    missing
}

const WORD_WIDTH: f32 = 14.0 * (1468 + 500 + 1026 + 500 + 1440) as f32 / 2048.0;

#[test]
fn traditional_arabic_takes_words_widths_by_form() {
    if !missing("Traditional Arabic") {
        return;
    }
    let got = natural_width(&lay_out(&[(TEXT, "Traditional Arabic")]));
    assert!((got - WORD_WIDTH).abs() < 0.01, "{got} vs {WORD_WIDTH}");
}

#[test]
fn letters_join_across_a_format_change() {
    if !missing("Traditional Arabic") {
        return;
    }
    // The same text split inside its words (beh | lam-meem, beh-lam | alef): same forms, same width.
    let (a, b) = TEXT.split_at('\u{628}'.len_utf8());
    let (b, c) = b.split_at(b.len() - '\u{627}'.len_utf8());
    let got = natural_width(&lay_out(&[(a, "Traditional Arabic"), (b, "traditional arabic"), (c, "Traditional Arabic")]));
    assert!((got - WORD_WIDTH).abs() < 0.01, "{got} vs {WORD_WIDTH}");
}

#[test]
fn a_font_without_a_table_keeps_the_substitutes_widths() {
    let font = "Simplified Arabic";
    if !missing(font) {
        return;
    }
    let drawn = wordcraft_fonts::word::resolve(font, false, false).face.family.clone();
    let got = natural_width(&lay_out(&[(TEXT, font)]));
    let own = natural_width(&lay_out(&[(TEXT, &drawn)]));
    assert!((got - own).abs() < 0.01, "{got} vs the substitute's {own}");
}
