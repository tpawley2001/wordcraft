//! A face drawn both as itself and as the stand-in for a missing font with Word's widths, in one
//! paragraph: each keeps its own widths in the PDF (two PDF fonts), and both keep words whole.

use wordcraft_doc::para::Run;
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::*;

/// Each drawn glyph's character, pen x and y, and the advance its PDF font declares (1/1000 em).
fn glyphs(bytes: &[u8]) -> Vec<(String, f64, f64, f64)> {
    use hayro_interpret::font::Glyph;
    use hayro_interpret::{
        BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask,
        interpret_page,
    };
    use hayro_syntax::Pdf;
    struct Ex(Vec<(String, f64, f64, f64)>);
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
            let p = (t * gt) * kurbo::Point::ORIGIN;
            self.0.push((c, p.x, p.y, f64::from(o.advance_width().unwrap_or(0.0))));
        }
        fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
        fn pop_clip_path(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }
    let pdf = Pdf::new(bytes.to_vec()).unwrap();
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

#[test]
fn a_face_as_itself_and_as_a_stand_in_in_one_paragraph_keeps_both_widths() {
    let part = "STORED field CR-067 ";
    let fonts = ["Source Sans 3", "Aptos", "Source Sans 3"];
    let mut p = Paragraph::new();
    for font in fonts {
        p.text.push_str(part);
        p.runs.push(Run { len: part.len(), props: CharProps { font: Some(font.into()), ..Default::default() } });
    }
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    let gs = glyphs(&export(&d, &PdfOptions::default()).unwrap());
    let text: String = gs.iter().map(|g| g.0.as_str()).collect();
    assert_eq!(text.split_whitespace().collect::<Vec<_>>(), part.repeat(3).split_whitespace().collect::<Vec<_>>());
    assert!(gs.iter().all(|g| (g.2 - gs[0].2).abs() < 0.5), "one line");
    // Points per 1000 units: every font here is drawn at the same size.
    let pairs: Vec<_> = gs.windows(2).filter(|w| w[0].0.trim() != "" && w[1].0.trim() != "").collect();
    let k = pairs.iter().map(|w| (w[1].1 - w[0].1) * w[0].3).sum::<f64>() / pairs.iter().map(|w| w[0].3 * w[0].3).sum::<f64>();
    for w in &pairs {
        let gap = ((w[1].1 - w[0].1) - k * w[0].3) / (k * 1000.0);
        assert!(gap.abs() < 0.05, "gap {gap:.3} em between {:?}", [&w[0].0, &w[1].0]);
    }
    // "T" is narrower in Word's Aptos than in Source Sans 3: the stand-in is its own PDF font.
    let t: Vec<f64> = gs.iter().filter(|g| g.0 == "T").map(|g| g.3).collect();
    assert_eq!(t.len(), 3);
    assert_eq!(t[0], t[2], "Source Sans 3 keeps its own width");
    assert!((t[0] - t[1]).abs() > 10.0, "Aptos's T takes Word's width: {t:?}");
}
