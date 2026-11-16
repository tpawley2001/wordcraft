//! Word draws no ligatures unless the run asks for them (`w14:ligatures`; Font › Advanced ›
//! Ligatures defaults to None), so text with fi, fl and ffi is as wide as its letters and lines
//! break where Word breaks them. Required ligatures (Arabic lam-alef, `rlig`) and Khmer
//! syllables are shaped as before.

use crate::*;
use wordcraft_doc::StoryRef;
use wordcraft_doc::props::{CharProps, Ligatures};

const LIGATURE_FONT: &str = "Source Serif 4";
const TEXT: &str = "office fifty";

fn first_para(l: &DocLayout) -> Option<&ParaLayout> {
    l.pages.iter().flat_map(|p| &p.items).find_map(|it| match it {
        Placed::Lines { story: StoryRef::Body, para, .. } => Some(&**para),
        _ => None,
    })
}

/// `text` in `font` at 14 pt with kerning off (`w:kern w:val="0"`), so each letter's advance is
/// the font's own and the widths compare letter by letter.
fn lay_text(text: &str, font: &str, ligatures: Option<Ligatures>) -> DocLayout {
    let chr = CharProps { font: Some(font.into()), size: Some(14.0), kern: Some(0.0), ligatures, ..Default::default() };
    let mut d = Document::new();
    d.body = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(text, chr))];
    layout(&d, &mut LayoutCache::new(), &LayoutOptions::default())
}

/// (glyph count, total advance in points) of `TEXT` in the ligature font.
fn glyphs_and_width(ligatures: Option<Ligatures>) -> (usize, f32) {
    let l = lay_text(TEXT, LIGATURE_FONT, ligatures);
    let pl = first_para(&l).expect("the paragraph is laid out");
    (pl.glyphs.len(), pl.clusters.iter().map(|c| c.adv).sum())
}

#[test]
fn no_w14_ligatures_draws_one_glyph_per_letter() {
    crate::test_fonts();
    let face = wordcraft_fonts::word::resolve(LIGATURE_FONT, false, false).face;
    assert_eq!(wordcraft_fonts::shape(&face, "fi", &[], |c| c).len(), 1, "the test font draws fi as one glyph");
    let letters = TEXT.chars().count();
    let (n, width) = glyphs_and_width(None);
    assert_eq!(n, letters, "no ligature glyph without w14:ligatures");
    // Each letter keeps its own advance: the width is the sum of the single-letter advances.
    let k = 14.0 / face.upem.max(1.0) as f32;
    let off = [wordcraft_fonts::feature_value(b"kern", 0)];
    let sum: f32 = TEXT
        .chars()
        .map(|c| {
            let s = c.to_string();
            wordcraft_fonts::shape(&face, &s, &off, |c| c).iter().map(|g| g.x_advance as f32 * k).sum::<f32>()
        })
        .sum();
    assert!((width - sum).abs() < 0.01, "width {width} = sum of letters {sum}");
}

#[test]
fn standard_ligatures_still_draw_fi_and_ffi() {
    crate::test_fonts();
    let letters = TEXT.chars().count();
    let (n, _) = glyphs_and_width(Ligatures::from_ooxml("standard"));
    assert!(n < letters, "standard: fi and ffi drawn as ligatures ({n} glyphs for {letters} letters)");
    let (none, _) = glyphs_and_width(Ligatures::from_ooxml("none"));
    assert_eq!(none, letters, "none: one glyph per letter");
    assert_eq!(glyphs_and_width(None), glyphs_and_width(Ligatures::from_ooxml("none")), "no setting is the same as none");
}

/// Arabic lam-alef is a required ligature (`rlig`): it still forms with no `w14:ligatures`.
#[test]
fn arabic_lam_alef_still_forms_without_w14_ligatures() {
    crate::test_fonts();
    let text = "\u{644}\u{627}";
    let l = lay_text(text, "Arial", None);
    let Some(pl) = first_para(&l) else { return };
    let Some(face) = pl.clusters.first().and_then(|c| pl.styles.get(c.style as usize)).map(|s| s.face) else { return };
    // Only meaningful with a font that draws lam-alef as one glyph by default.
    if wordcraft_fonts::shape(&face, text, &[], |c| c).len() != 1 {
        return;
    }
    assert_eq!(pl.glyphs.len(), 1, "lam-alef stays one glyph");
}
