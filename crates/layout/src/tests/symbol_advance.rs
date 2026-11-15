//! A character in the Symbol font, when Symbol itself isn't installed, is drawn as its Unicode
//! equivalent from another face but is as wide as Symbol makes it, as Word lays it out: a psi
//! (Symbol 0x79) is 0.686 em, not the 0.660 em of a substitute's psi, so it wraps where Word wraps
//! it. A missing Wingdings, Wingdings 2, Wingdings 3 or Webdings likewise keeps Word's widths for
//! its characters. An installed font keeps its own advances.

use std::sync::Arc;

use wordcraft_doc::numbering::{ListKind, levels_for};
use wordcraft_doc::props::NumRef;
use wordcraft_doc::{CharProps, Document, Paragraph, para_block};

use crate::para::ParaLayout;
use crate::{DocLayout, LayoutCache, LayoutOptions, Placed, layout};

fn chr(font: &str, size: f32, scale: Option<f32>) -> CharProps {
    CharProps { font: Some(font.into()), size: Some(size), scale, ..Default::default() }
}

/// A paragraph of `parts`: (text, font) at `size` points and character scale `scale` (%).
fn para(parts: &[(&str, &str)], size: f32, scale: Option<f32>) -> Paragraph {
    let mut it = parts.iter();
    let mut p = it.next().map(|(t, f)| Paragraph::with_text(t, chr(f, size, scale))).unwrap_or_default();
    for (t, f) in it {
        p.append(Paragraph::with_text(t, chr(f, size, scale)));
    }
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p
}

/// `d` with `p` alone in a column `column` points wide, laid out.
fn lay_out_in(mut d: Document, p: Paragraph, column: f32) -> DocLayout {
    crate::test_fonts();
    d.body = vec![para_block(p)];
    d.last_section.page_w = d.last_section.margin_left + d.last_section.margin_right + column;
    let mut c = LayoutCache::new();
    layout(&d, &mut c, &LayoutOptions::default())
}

fn lay_out(p: Paragraph, column: f32) -> DocLayout {
    lay_out_in(Document::new(), p, column)
}

fn first_para(l: &DocLayout) -> Arc<ParaLayout> {
    l.pages.iter().flat_map(|p| &p.items).find_map(|it| if let Placed::Lines { para, .. } = it { Some(para.clone()) } else { None }).unwrap()
}

/// Whether Symbol is missing here and drawn from a substitute; `None` when no face resolves for
/// it at all (the tests then have nothing to measure).
fn symbol_missing() -> Option<bool> {
    crate::test_fonts();
    let r = wordcraft_fonts::word::resolve("Symbol", false, false);
    (r.face.upem > 0.0).then_some(r.substituted)
}

/// The cluster starting at byte `start`.
fn cluster_at(pl: &ParaLayout, start: usize) -> usize {
    pl.clusters.iter().position(|c| c.start == start).unwrap()
}

/// The x of cluster `k` on its line.
fn x_of(pl: &ParaLayout, k: usize) -> f32 {
    let line = pl.lines.iter().find(|l| (l.c0..l.c1).contains(&k)).unwrap();
    line.xs[k - line.c0]
}

/// The line index cluster `k` is on.
fn line_of(pl: &ParaLayout, k: usize) -> usize {
    pl.lines.iter().position(|l| (l.c0..l.c1).contains(&k)).unwrap()
}

/// "a" + `sym` in Symbol + "b" in Times New Roman: the Symbol cluster's advance and how far "b"
/// starts after it.
fn a_sym_b(sym: &str, size: f32, scale: Option<f32>) -> (f32, f32) {
    let pl = first_para(&lay_out(para(&[("a", "Times New Roman"), (sym, "Symbol"), ("b", "Times New Roman")], size, scale), 1000.0));
    let k = cluster_at(&pl, 1);
    let kb = cluster_at(&pl, 1 + sym.len());
    (pl.clusters[k].adv, x_of(&pl, kb) - x_of(&pl, k))
}

/// The advance the installed Symbol font itself gives `sym` (shaped alone).
fn own_advance(sym: &str, size: f32) -> f32 {
    let pl = first_para(&lay_out(para(&[(sym, "Symbol")], size, None), 1000.0));
    pl.clusters[0].adv
}

#[test]
fn a_symbol_psi_is_as_wide_as_symbols() {
    let Some(missing) = symbol_missing() else { return };
    let (adv, b) = a_sym_b("\u{F079}", 12.0, None);
    let want = if missing { 8.232 } else { own_advance("\u{F079}", 12.0) };
    assert!((adv - want).abs() < 0.01, "{adv} vs {want}");
    assert!((b - want).abs() < 0.01, "{b} vs {want}");
}

#[test]
fn a_plain_8_bit_symbol_code_is_as_wide_too() {
    let Some(missing) = symbol_missing() else { return };
    let (adv, _) = a_sym_b("y", 12.0, None);
    let want = if missing { 8.232 } else { own_advance("y", 12.0) };
    assert!((adv - want).abs() < 0.01, "{adv} vs {want}");
}

#[test]
fn the_advance_follows_the_size_and_the_character_scale() {
    let Some(missing) = symbol_missing() else { return };
    let (adv20, _) = a_sym_b("\u{F079}", 20.0, None);
    let (wide, b) = a_sym_b("\u{F079}", 12.0, Some(150.0));
    let (want20, want_wide) = if missing { (13.72, 12.348) } else { (own_advance("\u{F079}", 20.0), own_advance("\u{F079}", 12.0) * 1.5) };
    assert!((adv20 - want20).abs() < 0.01, "{adv20} vs {want20}");
    assert!((wide - want_wide).abs() < 0.01, "{wide} vs {want_wide}");
    assert!((b - want_wide).abs() < 0.01, "{b} vs {want_wide}");
}

#[test]
fn two_symbol_characters_in_one_run_each_take_their_width() {
    let Some(missing) = symbol_missing() else { return };
    let pl = first_para(&lay_out(para(&[("\u{F044}\u{F079}", "Symbol")], 12.0, None), 1000.0));
    let (d, psi) = (pl.clusters[cluster_at(&pl, 0)].adv, pl.clusters[cluster_at(&pl, 3)].adv);
    if missing {
        assert!((d - 7.344).abs() < 0.01, "{d}");
        assert!((psi - 8.232).abs() < 0.01, "{psi}");
    } else {
        assert!(d > 0.0 && psi > 0.0);
    }
}

#[test]
fn a_psi_that_symbol_makes_too_wide_starts_the_next_line() {
    let Some(missing) = symbol_missing() else { return };
    let prefix = "the ellipsometric parameters ";
    let rest = " and more words";
    let parts = [(prefix, "Times New Roman"), ("\u{F079}", "Symbol"), (rest, "Times New Roman")];
    // The prefix as laid out: everything before the psi on one wide line.
    let wide = first_para(&lay_out(para(&parts, 12.0, None), 1000.0));
    let s = x_of(&wide, cluster_at(&wide, prefix.len())) - x_of(&wide, 0);
    let psi_w = wide.clusters[cluster_at(&wide, prefix.len())].adv;
    let psi_line = |column: f32| {
        let pl = first_para(&lay_out(para(&parts, 12.0, None), column));
        line_of(&pl, cluster_at(&pl, prefix.len()))
    };
    if missing {
        assert_eq!(psi_line(s + 8.10), 1, "8.232 pt does not fit in 8.10");
        assert_eq!(psi_line(s + 8.30), 0, "8.232 pt fits in 8.30");
        // A substitute wider than Symbol (8.268 pt in some faces) no longer wraps it either.
        assert_eq!(psi_line(s + 8.25), 0, "8.232 pt fits in 8.25");
    } else {
        assert_eq!(psi_line(s + psi_w - 0.1), 1);
        assert_eq!(psi_line(s + psi_w + 0.1), 0);
    }
}

#[test]
fn a_symbol_code_without_a_glyph_is_still_the_box() {
    let Some(missing) = symbol_missing() else { return };
    if !missing {
        return; // the installed font draws its own missing glyph
    }
    let (adv, _) = a_sym_b("\u{F080}", 12.0, None);
    let want = 12.0 * 1224.0 / 2048.0;
    assert!((adv - want).abs() < 0.01, "{adv} vs {want}");
}

/// Whether Wingdings is missing here and drawn from a substitute; `None` when no face resolves
/// for it at all.
fn wingdings_missing() -> Option<bool> {
    crate::test_fonts();
    let r = wordcraft_fonts::word::resolve("Wingdings", false, false);
    (r.face.upem > 0.0).then_some(r.substituted)
}

#[test]
fn missing_wingdings_takes_words_advance() {
    let Some(true) = wingdings_missing() else { return };
    let pl = first_para(&lay_out(para(&[("a", "Times New Roman"), ("\u{F06C}", "Wingdings"), ("b", "Times New Roman")], 12.0, None), 1000.0));
    let k = cluster_at(&pl, 1);
    let adv = pl.clusters[k].adv;
    assert!((adv - 0.747 * 12.0).abs() < 0.01, "{adv}");
    let b = x_of(&pl, cluster_at(&pl, 1 + "\u{F06C}".len())) - x_of(&pl, k);
    assert!((b - 0.747 * 12.0).abs() < 0.01, "{b}");
}

#[test]
fn wingdings_check_boxes_wrap_where_word_wraps_them() {
    let Some(true) = wingdings_missing() else { return };
    let boxes = "\u{F0A8}".repeat(30);
    let pl = first_para(&lay_out(para(&[(boxes.as_str(), "Wingdings")], 10.0, None), 260.0));
    // 29 x 8.91 pt = 258.4 fits in 260 pt; 30 x 8.91 = 267.3 does not.
    assert_eq!(pl.lines.len(), 2);
    let w = "\u{F0A8}".len();
    assert_eq!(line_of(&pl, cluster_at(&pl, 28 * w)), 0);
    assert_eq!(line_of(&pl, cluster_at(&pl, 29 * w)), 1);
}

#[test]
fn a_symbol_bullet_is_as_wide_as_symbols_and_the_tab_absorbs_it() {
    let Some(missing) = symbol_missing() else { return };
    let mut d = Document::new();
    let mut levels = levels_for(ListKind::BulletChar('\u{F0B7}'));
    if let Some(l) = levels.first_mut() {
        l.chr.font = Some("Symbol".into());
    }
    let num = Arc::make_mut(&mut d.numbering).try_add_levels(None, levels.clone()).unwrap();
    let mut p = para(&[("Item text", "Times New Roman")], 12.0, None);
    p.props.numbering = Some(NumRef { num, level: 0 });
    p.mark = chr("Times New Roman", 12.0, None);
    let pl = first_para(&lay_out_in(d, p, 400.0));
    let label = pl.label.as_ref().unwrap();
    if missing {
        assert!((label.width - 12.0 * 0.460).abs() < 0.01, "{}", label.width);
    }
    // The text starts at the level's indent whatever the bullet's width.
    let indent = levels[0].indent;
    let x = x_of(&pl, cluster_at(&pl, 0));
    assert!((x - indent).abs() < 0.01, "{x} vs {indent}");
}
