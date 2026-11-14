//! Print background colors and images (File › Options › Display): `PdfOptions::background`
//! false leaves out the page colour (and page picture); the default (export) draws it, as Word's
//! own Save as PDF does. `PdfOptions::drawings` false hides drawings in the display list.

use std::sync::Arc;

use hayro_interpret::font::Glyph;
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask, interpret_page,
};
use kurbo::Shape;
use wordcraft_doc::{CharProps, Document, Paragraph, Rgb, para_block};
use wordcraft_layout::{DocLayout, LayoutCache, LayoutOptions, layout};

use crate::{PdfOptions, export_layout};

/// Text drawn, and the number of filled paths at least 500 x 700 pt (only a page background is
/// that big here).
struct Seen {
    text: String,
    page_fills: usize,
}

impl Device<'_> for Seen {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, path: &kurbo::BezPath, t: kurbo::Affine, _: &Paint<'_>, mode: &PathDrawMode) {
        if matches!(mode, PathDrawMode::Fill(_)) {
            let b = (t * path.clone()).bounding_box();
            if b.width() >= 500.0 && b.height() >= 700.0 {
                self.page_fills += 1;
            }
        }
    }
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph(&mut self, g: &Glyph<'_>, _: kurbo::Affine, _: kurbo::Affine, _: &Paint<'_>, mode: &GlyphDrawMode) {
        if matches!(mode, GlyphDrawMode::Stroke(_)) {
            return;
        }
        match g.as_unicode() {
            Some(hayro_cmap::BfString::Char(c)) => self.text.push(c),
            Some(hayro_cmap::BfString::String(s)) => self.text.push_str(&s),
            None => self.text.push('\u{FFFD}'),
        }
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// (text, page-size fills) of the first page.
fn first_page(bytes: &[u8]) -> (String, usize) {
    let pdf = hayro_syntax::Pdf::new(bytes.to_vec()).expect("parse");
    let cache = InterpreterCache::new();
    let page = &pdf.pages()[0];
    let mut ctx = Context::new(kurbo::Affine::IDENTITY, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), &cache, pdf.xref(), InterpreterSettings::default());
    let mut seen = Seen { text: String::new(), page_fills: 0 };
    interpret_page(page, &mut ctx, &mut seen);
    (seen.text.split_whitespace().collect::<Vec<_>>().join(" "), seen.page_fills)
}

fn coloured_page() -> (Document, Arc<DocLayout>) {
    let mut d = Document::new();
    d.body = vec![para_block(Paragraph::with_text("Plain body words", CharProps::default()))];
    d.settings.page_color = Some(Rgb(0x92, 0xD0, 0x50));
    let lay = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    (d, Arc::new(lay))
}

#[test]
fn export_draws_the_page_colour_by_default() {
    let (d, lay) = coloured_page();
    let opts = PdfOptions { tagged: false, ..Default::default() };
    assert!(opts.background && opts.drawings, "export draws everything");
    let (text, fills) = first_page(&export_layout(&d, &lay, &opts).expect("exports"));
    assert_eq!(text, "Plain body words");
    assert_eq!(fills, 1, "one page-size fill: the page colour");
}

#[test]
fn background_off_leaves_out_the_page_colour_only() {
    let (d, lay) = coloured_page();
    let opts = PdfOptions { tagged: false, background: false, ..Default::default() };
    let (text, fills) = first_page(&export_layout(&d, &lay, &opts).expect("exports"));
    assert_eq!(text, "Plain body words", "the text still prints");
    assert_eq!(fills, 0, "no page colour");
}

#[test]
fn drawings_off_hides_drawings_in_the_display_list() {
    assert!(!PdfOptions::default().display_options().hide_drawings);
    let opts = PdfOptions { drawings: false, ..Default::default() };
    assert!(opts.display_options().hide_drawings);
}
