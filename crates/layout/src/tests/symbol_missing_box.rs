//! A code a symbol font has no glyph for (Symbol 0x7F-0x9F, 0xF0, 0xFF; Wingdings, Wingdings 2,
//! Wingdings 3 and Webdings 0x7F) is the font's own empty missing-character box, as wide as
//! Word measures it (Symbol 1224/2048 em, the others 1024/2048 em), never another face's glyph.
//! Old forms use Symbol U+F08E as a check box: "[] Yes [] No".

use std::sync::Arc;

use wordcraft_doc::para::ShapeKind;
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::display::{DisplayOptions, Draw, page_display};
use crate::para::{ClKind, ParaLayout};
use crate::{DocLayout, LayoutCache, LayoutOptions, Placed, layout};

const SYMBOL_EM: f32 = 1224.0 / 2048.0;
const DINGS_EM: f32 = 1024.0 / 2048.0;

fn chr(font: &str, size: f32) -> CharProps {
    CharProps { font: Some(font.into()), size: Some(size), ..Default::default() }
}

/// A paragraph of `parts`: (text, font) at `size` points.
fn para(parts: &[(&str, &str)], size: f32) -> Paragraph {
    let mut it = parts.iter();
    let mut p = it.next().map(|(t, f)| Paragraph::with_text(t, chr(f, size))).unwrap_or_default();
    for (t, f) in it {
        p.append(Paragraph::with_text(t, chr(f, size)));
    }
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p
}

/// `p` laid out alone in a column `column` points wide.
fn lay_out(p: Paragraph, column: f32) -> (Document, DocLayout) {
    crate::test_fonts();
    let mut d = Document::new();
    d.body = vec![para_block(p)];
    d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + column;
    let mut c = LayoutCache::new();
    let l = layout(&d, &mut c, &LayoutOptions::default());
    (d, l)
}

fn first_para(l: &DocLayout) -> Arc<ParaLayout> {
    l.pages.iter().flat_map(|p| &p.items).find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap()
}

/// The layout of "A" + `code` in `font` + "B" at `size`, and the middle cluster's index.
fn a_x_b(font: &str, code: char, size: f32) -> (Arc<ParaLayout>, usize) {
    let (_, l) = lay_out(para(&[("A", "Times New Roman"), (&code.to_string(), font), ("B", "Times New Roman")], size), 1000.0);
    let pl = first_para(&l);
    let k = pl.clusters.iter().position(|c| c.start == 1).unwrap();
    (pl, k)
}

fn line_width(pl: &ParaLayout) -> f32 {
    let line = &pl.lines[0];
    pl.clusters[line.c0..line.c1].iter().map(|c| c.adv).sum()
}

fn has_box(pl: &ParaLayout, k: usize) -> bool {
    pl.enclosures.iter().any(|(i, e)| *i == k && e.kind == ShapeKind::Rectangle)
}

#[test]
fn a_symbol_check_box_is_as_wide_as_symbols_empty_box() {
    let (pl, k) = a_x_b("Symbol", '\u{F08E}', 10.0);
    let adv = pl.clusters[k].adv;
    assert!((adv - 10.0 * SYMBOL_EM).abs() < 0.01, "{adv}");
    assert!(has_box(&pl, k));
    // "A" and "B" each alone: side by side they kern, and the box is between them.
    let alone = |t: &str| line_width(&first_para(&lay_out(para(&[(t, "Times New Roman")], 10.0), 1000.0).1));
    let ab = alone("A") + alone("B");
    assert!((line_width(&pl) - (ab + 10.0 * SYMBOL_EM)).abs() < 0.01, "{} vs {ab}", line_width(&pl));
}

#[test]
fn every_symbol_code_without_a_glyph_is_the_box() {
    for code in ['\u{F07F}', '\u{F080}', '\u{F09F}', '\u{F0F0}', '\u{F0FF}'] {
        let (pl, k) = a_x_b("Symbol", code, 20.0);
        let adv = pl.clusters[k].adv;
        assert!((adv - 20.0 * SYMBOL_EM).abs() < 0.01, "{code:?}: {adv}");
        assert!(has_box(&pl, k), "{code:?}");
    }
}

#[test]
fn the_dingbat_fonts_box_is_half_an_em() {
    for font in ["Wingdings", "Wingdings 2", "Wingdings 3", "Webdings"] {
        if !wordcraft_fonts::word::resolve(font, false, false).substituted {
            continue; // the installed font draws its own missing glyph
        }
        let (pl, k) = a_x_b(font, '\u{F07F}', 20.0);
        let adv = pl.clusters[k].adv;
        assert!((adv - 20.0 * DINGS_EM).abs() < 0.01, "{font}: {adv}");
        assert!(has_box(&pl, k), "{font}");
    }
}

#[test]
fn a_symbol_code_with_a_glyph_is_unchanged() {
    // Symbol 0x61 is alpha, drawn as U+03B1 as before.
    let (pl, k) = a_x_b("Symbol", '\u{F061}', 20.0);
    let (alpha, ka) = a_x_b("Symbol", '\u{3B1}', 20.0);
    assert!(!has_box(&pl, k));
    assert!((pl.clusters[k].adv - 20.0 * SYMBOL_EM).abs() > 0.01);
    assert!((pl.clusters[k].adv - alpha.clusters[ka].adv).abs() < 0.001);
}

#[test]
fn wingdings_ff_is_not_a_box() {
    let (pl, k) = a_x_b("Wingdings", '\u{F0FF}', 20.0);
    assert!(!has_box(&pl, k));
}

#[test]
fn the_box_takes_no_glyph_from_another_face() {
    crate::test_fonts();
    let symbol = wordcraft_fonts::word::resolve("Symbol", false, false).face;
    let (pl, k) = a_x_b("Symbol", '\u{F08E}', 10.0);
    let c = &pl.clusters[k];
    assert_eq!(pl.styles[c.style as usize].face.id(), symbol.id(), "the run's own face, not a fallback");
    assert_eq!(c.kind, ClKind::Text);
}

#[test]
fn a_line_with_two_check_boxes_fits_its_column() {
    let (q, yes, no) = ("Have you been insured otherwise?    ", " Yes      ", " No");
    let (_, l) = lay_out(para(&[(q, "Times New Roman"), (yes, "Times New Roman"), (no, "Times New Roman")], 10.0), 1000.0);
    let text = line_width(&first_para(&l));
    let column = text + 2.0 * 10.0 * SYMBOL_EM + 1.0;
    let p = para(&[(q, "Times New Roman"), ("\u{F08E}", "Symbol"), (yes, "Times New Roman"), ("\u{F08E}", "Symbol"), (no, "Times New Roman")], 10.0);
    let (_, l) = lay_out(p, column);
    assert_eq!(first_para(&l).lines.len(), 1, "column {column}");
}

#[test]
fn the_box_is_drawn_and_keeps_its_character() {
    let (d, l) = lay_out(para(&[("A", "Times New Roman"), ("\u{F08E}", "Symbol"), ("B", "Times New Roman")], 10.0), 1000.0);
    let draws = page_display(&d, &l.pages[0], &DisplayOptions::default());
    let boxes: Vec<_> = draws
        .iter()
        .filter_map(|x| match x {
            Draw::Shape { rect, kind: ShapeKind::Rectangle, fill: None, stroke: Some(_), .. } => Some(*rect),
            _ => None,
        })
        .collect();
    assert_eq!(boxes.len(), 1, "{draws:?}");
    let r = boxes[0];
    assert!((r.w - 5.0).abs() < 0.01 && (r.h - 6.8).abs() < 0.01, "{r:?}");
    let text: String = draws.iter().filter_map(|x| if let Draw::Glyphs { text, .. } = x { Some(text.as_str()) } else { None }).collect();
    assert!(text.contains('\u{F08E}'), "{text:?}");
}

#[test]
fn a_plain_8_bit_code_without_a_glyph_is_the_box() {
    let (pl, k) = a_x_b("Symbol", '\u{F0}', 10.0);
    assert!((pl.clusters[k].adv - 10.0 * SYMBOL_EM).abs() < 0.01, "{}", pl.clusters[k].adv);
    assert!(has_box(&pl, k));
}

#[test]
fn a_soft_hyphen_and_list_free_codes_stay_as_they_were() {
    // Codes below 0xA0 in their plain form are not measured as boxes (space, controls).
    let (pl, k) = a_x_b("Symbol", '\u{9F}', 10.0);
    assert!(!has_box(&pl, k));
}
