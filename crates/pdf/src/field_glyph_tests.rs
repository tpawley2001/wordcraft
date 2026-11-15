//! A field result holding a character its font has no glyph for must not make the PDF lose the
//! other runs drawn in that face (they all vanished: no bold face in the file, no bold text).

use wordcraft_doc::para::InlineObject;
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::*;

fn props(bold: bool) -> CharProps {
    CharProps { font: Some("Source Sans 3".into()), bold: Some(bold), ..Default::default() }
}

fn field(instr: &str, result: &str) -> InlineObject {
    InlineObject::Field {
        instr: instr.into(),
        result: result.into(),
        locked: false,
        dirty: false,
        code: false,
        form: String::new(),
        code_paras: None,
    }
}

/// `before` / "X = " + the field / `after`, the field's result in `field_bold`.
fn doc(before: (&str, bool), instr: &str, result: &str, field_bold: bool, after: (&str, bool)) -> Document {
    let mut d = Document::new();
    let mut p = Paragraph::with_text("X = ", props(false));
    p.insert_object(4, field(instr, result), &props(field_bold)).unwrap();
    d.body =
        vec![para_block(Paragraph::with_text(before.0, props(before.1))), para_block(p), para_block(Paragraph::with_text(after.0, props(after.1)))];
    d
}

fn text_of(d: &Document) -> String {
    let bytes = export(d, &PdfOptions::default()).unwrap();
    crate::tests::extract_text(&bytes).concat().split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn bold_formula_result_with_a_missing_glyph_keeps_the_bold_paragraphs() {
    let d = doc(("BOLD two", true), " =nope ", "!Undefined Bookmark, NOPE\u{9F6F}", true, ("BOLD three", true));
    let t = text_of(&d);
    for want in ["BOLD two", "Undefined Bookmark, NOPE", "BOLD three"] {
        assert!(t.contains(want), "{want:?} missing from {t:?}");
    }
}

#[test]
fn plain_formula_result_with_a_missing_glyph_keeps_the_plain_paragraphs() {
    let d = doc(("PLAIN one", false), " =2+3 ", "5\u{9F6F}", false, ("PLAIN four", false));
    let t = text_of(&d);
    for want in ["PLAIN one", "X = 5", "PLAIN four"] {
        assert!(t.contains(want), "{want:?} missing from {t:?}");
    }
}

#[test]
fn ref_result_with_missing_glyphs_keeps_the_bold_paragraphs() {
    let d = doc(("BOLD two", true), " REF x ", "Ref\u{65E5}\u{672C}", true, ("BOLD three", true));
    let t = text_of(&d);
    for want in ["BOLD two", "Ref", "BOLD three"] {
        assert!(t.contains(want), "{want:?} missing from {t:?}");
    }
}

#[test]
fn the_bold_glyphs_are_still_drawn() {
    // Every glyph but the missing one is drawn: at least as many as without the CJK character.
    let with =
        export(&doc(("BOLD two", true), " =nope ", "!Undefined Bookmark, NOPE\u{9F6F}", true, ("BOLD three", true)), &PdfOptions::default()).unwrap();
    let without =
        export(&doc(("BOLD two", true), " =nope ", "!Undefined Bookmark, NOPE", true, ("BOLD three", true)), &PdfOptions::default()).unwrap();
    let (g_with, _) = crate::tests::count_glyphs_and_paths(&with);
    let (g_without, _) = crate::tests::count_glyphs_and_paths(&without);
    assert!(g_without > 0, "control: the glyphs are drawn without the CJK character");
    assert!(g_with >= g_without, "glyphs went missing: {g_with} drawn, {g_without} without the CJK character");
}
