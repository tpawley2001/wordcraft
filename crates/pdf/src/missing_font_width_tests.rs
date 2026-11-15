//! Text in a font that isn't installed, laid out with Word's widths for that font, must keep its
//! words whole in the PDF: each glyph's advance in the PDF font has to match the step to the next
//! glyph, or readers that rebuild words from positions (copy, search, pdftotext, PDFium, pdf.js)
//! split words ("C R-067", "S TORED").

use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::*;

const TEXT: &str = "IF-field probe, round 8 (CR-067), candidate none. Y02_STORED = B2 Sentence case STORED";

/// One glyph as a PDF reader sees it: its character, pen x and y (points) and the advance its
/// font declares for it (1/1000 of the font size).
struct G {
    c: String,
    x: f64,
    y: f64,
    adv: f64,
}

fn glyphs(bytes: &[u8]) -> Vec<G> {
    use hayro_interpret::font::Glyph;
    use hayro_interpret::{
        BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask,
        interpret_page,
    };
    use hayro_syntax::Pdf;
    struct Ex(Vec<G>);
    impl Device<'_> for Ex {
        fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
        fn set_blend_mode(&mut self, _: BlendMode) {}
        fn draw_path(&mut self, _: &kurbo::BezPath, _: kurbo::Affine, _: &Paint<'_>, _: &PathDrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
        fn draw_glyph(&mut self, g: &Glyph<'_>, t: kurbo::Affine, gt: kurbo::Affine, _: &Paint<'_>, mode: &GlyphDrawMode) {
            if matches!(mode, GlyphDrawMode::Stroke(_)) {
                return;
            }
            let Glyph::Outline(o) = g else { return };
            let c = match g.as_unicode() {
                Some(hayro_cmap::BfString::Char(c)) => c.to_string(),
                Some(hayro_cmap::BfString::String(s)) => s,
                None => "\u{FFFD}".into(),
            };
            // The glyph's origin, and the advance its font declares (1/1000 of the font size).
            let o0 = (t * gt) * kurbo::Point::ORIGIN;
            let adv = f64::from(o.advance_width().unwrap_or(0.0));
            self.0.push(G { c, x: o0.x, y: o0.y, adv });
        }
        fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
        fn pop_clip_path(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }
    let pdf = Pdf::new(bytes.to_vec()).expect("parse");
    let cache = InterpreterCache::new();
    let mut out = Vec::new();
    for page in pdf.pages().iter() {
        let mut ctx = Context::new(kurbo::Affine::IDENTITY, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), &cache, pdf.xref(), InterpreterSettings::default());
        let mut ex = Ex(Vec::new());
        interpret_page(page, &mut ctx, &mut ex);
        out.extend(ex.0);
    }
    out
}

fn props(font: &str) -> CharProps {
    CharProps { font: Some(font.into()), ..Default::default() }
}

fn export_text(paras: &[(&str, &str)]) -> Vec<u8> {
    let mut d = Document::new();
    d.body = paras.iter().map(|(text, font)| para_block(Paragraph::with_text(text, props(font)))).collect();
    export(&d, &PdfOptions::default()).unwrap()
}

/// For each line of `gs`: the step from each glyph to the next that its declared advance does
/// not account for, as a share of the font size (kerning in an installed font stays under 0.05;
/// readers insert a space from about 0.1). The font size (points per 1000 units) is fitted
/// per line (least squares of step against advance), so no PDF unit bookkeeping is needed.
fn worst_gap(gs: &[G]) -> (f64, String) {
    let mut worst = (0.0f64, String::new());
    let mut i = 0;
    while i < gs.len() {
        let mut j = i + 1;
        while j < gs.len() && (gs[j].y - gs[i].y).abs() < 0.5 && gs[j].x > gs[j - 1].x {
            j += 1;
        }
        let line = &gs[i..j];
        let pairs: Vec<(&G, &G)> = line.windows(2).map(|w| (&w[0], &w[1])).filter(|(a, b)| a.c.trim() != "" && b.c.trim() != "").collect();
        let (sxy, sxx) = pairs.iter().fold((0.0, 0.0), |(sxy, sxx), (a, b)| (sxy + (b.x - a.x) * a.adv, sxx + a.adv * a.adv));
        if sxx > 0.0 {
            let k = sxy / sxx;
            for (a, b) in &pairs {
                let share = ((b.x - a.x) - k * a.adv) / (k * 1000.0);
                if share.abs() > worst.0.abs() {
                    worst = (share, format!("{}{}", a.c, b.c));
                }
            }
        }
        i = j;
    }
    worst
}

/// The glyphs' text, a space between lines (a new line starts where y changes).
fn text(gs: &[G]) -> String {
    let mut s = String::new();
    for (i, g) in gs.iter().enumerate() {
        if i > 0 && (g.y - gs[i - 1].y).abs() >= 0.5 {
            s.push(' ');
        }
        s.push_str(&g.c);
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn an_installed_font_has_no_gaps_inside_words() {
    let gs = glyphs(&export_text(&[(TEXT, "Source Sans 3")]));
    assert_eq!(text(&gs), TEXT);
    let (gap, at) = worst_gap(&gs);
    assert!(gap.abs() < 0.05, "Source Sans 3: gap {gap:.3} em after {at:?}");
}

#[test]
fn a_missing_font_with_word_widths_has_no_gaps_inside_words() {
    for font in ["Aptos", "Verdana", "Trebuchet MS"] {
        let gs = glyphs(&export_text(&[(TEXT, font)]));
        assert_eq!(text(&gs), TEXT, "{font}");
        let (gap, at) = worst_gap(&gs);
        assert!(gap.abs() < 0.05, "{font}: gap {gap:.3} em between {at:?}");
    }
}

#[test]
fn installed_and_missing_fonts_in_one_document_both_keep_words_whole() {
    let gs = glyphs(&export_text(&[(TEXT, "Source Sans 3"), (TEXT, "Aptos"), (TEXT, "Source Sans 3")]));
    assert_eq!(text(&gs), [TEXT, TEXT, TEXT].join(" "));
    let (gap, at) = worst_gap(&gs);
    assert!(gap.abs() < 0.05, "gap {gap:.3} em between {at:?}");
}

#[test]
fn a_missing_traditional_arabic_keeps_layouts_per_form_steps() {
    // The glyphs sit where layout put them: Word's per-form widths, not the substitute's.
    let r = wordcraft_fonts::word::resolve("Traditional Arabic", false, false);
    if !r.substituted {
        return;
    }
    let text = "\u{628}\u{644}\u{645} \u{645}\u{627} \u{628}\u{644}\u{627}";
    let chr = CharProps { font_cs: Some("Traditional Arabic".into()), size: Some(14.0), size_cs: Some(14.0), rtl: Some(true), ..Default::default() };
    let mut p = Paragraph::with_text(text, chr);
    p.props.bidi = Some(true);
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    let gs: Vec<G> = glyphs(&export(&d, &PdfOptions::default()).unwrap()).into_iter().filter(|g| g.c.trim() != "").collect();
    assert!(!gs.is_empty());
    // From the leftmost glyph (at the line's left edge) to the rightmost (inside the first word's
    // initial beh, 379 units wide; a substitute may draw its dot as a mark glyph within it): the
    // line's width less part of that beh. The substitute's own widths make it over twice as wide.
    let (lo, hi) = gs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), g| (lo.min(g.x), hi.max(g.x)));
    let line = 14.0 * f64::from(1468 + 500 + 1026 + 500 + 1440) / 2048.0;
    let beh = 14.0 * 379.0 / 2048.0;
    let span = hi - lo;
    assert!(span > line - beh - 0.05 && span < line + 0.05, "glyphs span {span:.3} pt, want {:.3} to {line:.3}", line - beh);
}
