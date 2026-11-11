//! Print document properties together with Print only the data from a form
//! (`w:printFormsData`): the document's pages draw only the form data, but the properties page
//! appended after them is a report about the document and prints in full, as in Word.

use std::sync::Arc;

use hayro_interpret::font::Glyph;
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask, interpret_page,
};
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};
use wordcraft_layout::{DocLayout, LayoutCache, LayoutOptions, layout};

use crate::{PdfOptions, export_layout_with_appendix};

struct Text(String);

impl Device<'_> for Text {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &kurbo::BezPath, _: kurbo::Affine, _: &Paint<'_>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph(&mut self, g: &Glyph<'_>, _: kurbo::Affine, _: kurbo::Affine, _: &Paint<'_>, mode: &GlyphDrawMode) {
        if matches!(mode, GlyphDrawMode::Stroke(_)) {
            return;
        }
        match g.as_unicode() {
            Some(hayro_cmap::BfString::Char(c)) => self.0.push(c),
            Some(hayro_cmap::BfString::String(s)) => self.0.push_str(&s),
            None => self.0.push('\u{FFFD}'),
        }
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// Text of every page of `bytes`, whitespace squashed.
fn page_texts(bytes: &[u8]) -> Vec<String> {
    let pdf = hayro_syntax::Pdf::new(bytes.to_vec()).expect("parse");
    let cache = InterpreterCache::new();
    pdf.pages()
        .iter()
        .map(|page| {
            let mut ctx =
                Context::new(kurbo::Affine::IDENTITY, kurbo::Rect::new(0.0, 0.0, 1.0, 1.0), &cache, pdf.xref(), InterpreterSettings::default());
            let mut t = Text(String::new());
            interpret_page(page, &mut ctx, &mut t);
            t.0.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect()
}

fn one_para(text: &str) -> (Document, Arc<DocLayout>) {
    let mut d = Document::new();
    d.body = vec![para_block(Paragraph::with_text(text, CharProps::default()))];
    let lay = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    (d, Arc::new(lay))
}

fn print(forms_data_only: bool, copies: u32, collate: bool) -> Vec<String> {
    let (doc, lay) = one_para("Plain body words");
    let (adoc, alay) = one_para("Title: Synthetic title");
    let opts = PdfOptions { forms_data_only, sheet_copies: copies, sheet_collate: collate, tagged: false, ..Default::default() };
    page_texts(&export_layout_with_appendix(&doc, &lay, (&adoc, &alay), &opts).expect("exports"))
}

#[test]
fn appendix_prints_in_full_with_forms_data_only() {
    let pages = print(true, 1, true);
    assert_eq!(pages.len(), 2, "{pages:?}");
    assert!(!pages[0].contains("Plain"), "the document page draws only form data: {pages:?}");
    assert!(pages[1].contains("Synthetic title"), "the properties page prints in full: {pages:?}");
}

#[test]
fn appendix_prints_in_full_after_every_copy() {
    for collate in [true, false] {
        let pages = print(true, 2, collate);
        assert_eq!(pages.len(), 4, "collate {collate}: {pages:?}");
        let appendix = pages.iter().filter(|p| p.contains("Synthetic title")).count();
        assert_eq!(appendix, 2, "collate {collate}: one full properties page per copy: {pages:?}");
        assert!(pages.iter().all(|p| !p.contains("Plain")), "collate {collate}: {pages:?}");
    }
}

#[test]
fn without_forms_data_only_both_print_in_full() {
    let pages = print(false, 1, true);
    assert_eq!(pages.len(), 2, "{pages:?}");
    assert!(pages[0].contains("Plain body words"), "{pages:?}");
    assert!(pages[1].contains("Synthetic title"), "{pages:?}");
}
