//! Form Field Shading is a screen aid: a PDF never draws legacy form fields on grey, whether or
//! not the document asks for the shading (`w:doNotShadeFormData`), as Word prints and saves them.

use hayro_interpret::font::Glyph;
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask, interpret_page,
};
use wordcraft_doc::para::InlineObject;
use wordcraft_doc::props::Rgb;
use wordcraft_doc::{CharProps, Document, Pos};

use crate::{PdfOptions, export};

/// The colours of every filled path.
struct Fills(Vec<[u8; 3]>);

impl Device<'_> for Fills {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &kurbo::BezPath, _: kurbo::Affine, paint: &Paint<'_>, mode: &PathDrawMode) {
        if let (Paint::Color(c), PathDrawMode::Fill(_)) = (paint, mode) {
            let [r, g, b, _] = c.to_rgba().to_rgba8();
            self.0.push([r, g, b]);
        }
    }
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn pop_clip_path(&mut self) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn pop_transparency_group(&mut self) {}
    fn draw_glyph(&mut self, _: &Glyph<'_>, _: kurbo::Affine, _: kurbo::Affine, _: &Paint<'_>, _: &GlyphDrawMode) {}
    fn draw_image(&mut self, _: Image<'_, '_>, _: kurbo::Affine) {}
}

fn form(instr: &str, result: &str, ff: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: result.into(), locked: false, dirty: false, code: false, form: ff.into(), code_paras: None }
}

/// A text field (its result optionally on red run shading), a check box and a drop-down.
fn doc(no_shade: bool, red: bool) -> Document {
    let mut d = Document::from_text("x y z ");
    d.settings.do_not_shade_form_data = no_shade;
    let dd = r#"<w:ffData><w:name w:val="List"/><w:ddList><w:result w:val="1"/><w:listEntry w:val="Red"/><w:listEntry w:val="Beta"/></w:ddList></w:ffData>"#;
    let cb = r#"<w:ffData><w:name w:val="Box"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/></w:checkBox></w:ffData>"#;
    let tx = r#"<w:ffData><w:name w:val="Text1"/><w:textInput/></w:ffData>"#;
    let shaded = CharProps { shading: red.then_some(Rgb(0xFF, 0, 0)), ..Default::default() };
    d.insert_object(&Pos::body(0, 6), form("FORMDROPDOWN", "", dd), &CharProps::default()).unwrap();
    d.insert_object(&Pos::body(0, 4), form("FORMCHECKBOX", "", cb), &CharProps::default()).unwrap();
    d.insert_object(&Pos::body(0, 2), form("FORMTEXT", "Alice", tx), &shaded).unwrap();
    d
}

fn fills(d: &Document) -> Vec<[u8; 3]> {
    let bytes = export(d, &PdfOptions { tagged: false, ..Default::default() }).unwrap();
    let pdf = hayro_syntax::Pdf::new(bytes).unwrap();
    let cache = InterpreterCache::new();
    let mut out = Fills(Vec::new());
    for page in pdf.pages().iter() {
        let b = page.media_box();
        let mut ctx = Context::new(
            kurbo::Affine::IDENTITY,
            kurbo::Rect::new(0.0, 0.0, b.width(), b.height()),
            &cache,
            pdf.xref(),
            InterpreterSettings::default(),
        );
        interpret_page(page, &mut ctx, &mut out);
    }
    out.0
}

const GREY: [u8; 3] = [0xD9, 0xD9, 0xD9];

#[test]
fn pdf_never_draws_form_field_shading() {
    let on = fills(&doc(false, false));
    assert!(!on.contains(&GREY), "no grey fill in the PDF: {on:?}");
    assert_eq!(on, fills(&doc(true, false)), "the setting changes nothing that is drawn");
    // Control: run shading on the result is a fill the PDF does draw.
    assert!(fills(&doc(false, true)).contains(&[0xFF, 0, 0]), "the run's red shading is drawn");
}
