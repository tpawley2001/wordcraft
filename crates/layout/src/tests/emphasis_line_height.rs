//! Emphasis marks above the text (`w:em` dot, comma, circle; ECMA-376 17.3.2.12): how much taller
//! the line gets and where the dot is drawn, as Microsoft 365 Word lays them out. Measured
//! 2026-11-14 in Word's PDF of a hand-written DOCX (Times New Roman, single spacing, one paragraph
//! per size after a plain paragraph of the same size):
//!   size 8 / 11 / 14 / 24 / 36 pt: the line with marks starts 2.2-2.32 / 3.19 / 4.06 / 6.86 / 10.35 pt
//!   lower than a plain line would, i.e. 0.2875 x size more above the baseline (all three marks).
//!   A line with dots below (underDot) is as far from the line above as a plain line.
//!   The dot's centre is 1.065 x size above the text baseline and the dot is ~0.10 x size across
//!   (24 pt: 25.4 pt above, 2.4 pt across; 36 pt: 38.6 pt above, 3.5 pt across).
//! Checked the same day in Arial (identical to Times New Roman), Cambria (0.295 x size) and Calibri
//! (0.305 x size): the extra height is close to 0.2875 x size whatever the text font.
//! The bundled test fonts differ from Times New Roman, so these tests pin the rule relative to
//! the plain line of the same font.
use crate::display::{Draw, page_display};
use crate::*;
use wordcraft_doc::para::ShapeKind;
use wordcraft_doc::para_block;
use wordcraft_doc::props::EmphasisMark;

/// Bundled test fonts with different ascents: today's fixed-fraction rule happens to give
/// Source Serif 4 the right height but not Inter.
const FONTS: [&str; 2] = ["Source Serif 4", "Inter"];

/// Three paragraphs "Alpha", "Bravo" (plain) and "Marked" (with `mark`) at `size` pt: the
/// baselines of the three, and the ellipses drawn (centre y, width).
fn lay(font: &str, size: f32, mark: EmphasisMark) -> ([f32; 3], Vec<(f32, f32)>) {
    crate::test_fonts();
    let mut d = Document::new();
    let plain = CharProps { font: Some(font.into()), size: Some(size), ..CharProps::default() };
    let marked = CharProps { emphasis: Some(mark), ..plain.clone() };
    d.body = vec![
        para_block(Paragraph::with_text("Alpha", plain.clone())),
        para_block(Paragraph::with_text("Bravo", plain)),
        para_block(Paragraph::with_text("Marked", marked)),
    ];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let draws = page_display(&d, page, &Default::default());
    let base = |word: &str| {
        draws
            .iter()
            .find_map(|d| match d {
                Draw::Glyphs { glyphs, text, .. } if text.contains(word) => glyphs.first().map(|g| g.2),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{word} is drawn"))
    };
    let dots = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Shape { rect, kind: ShapeKind::Ellipse, .. } => Some((rect.y + rect.h / 2.0, rect.w)),
            _ => None,
        })
        .collect();
    ([base("Alpha"), base("Bravo"), base("Marked")], dots)
}

/// How much further the marked line's baseline is from the line above than a plain line's.
fn extra(font: &str, size: f32, mark: EmphasisMark) -> f32 {
    let ([a, b, m], _) = lay(font, size, mark);
    (m - b) - (b - a)
}

#[test]
fn marks_above_add_0_2875_of_the_size_to_the_line() {
    for font in FONTS {
        for size in [8.0f32, 11.0, 14.0, 24.0, 36.0] {
            let got = extra(font, size, EmphasisMark::Dot);
            let want = 0.2875 * size;
            assert!((got - want).abs() < 0.15, "{font} {size} pt dots: line {got} pt taller than plain, want {want}");
        }
    }
}

#[test]
fn comma_and_circle_marks_make_the_same_line_as_dots() {
    for font in FONTS {
        let dot = extra(font, 11.0, EmphasisMark::Dot);
        for mark in [EmphasisMark::Comma, EmphasisMark::Circle] {
            let got = extra(font, 11.0, mark);
            assert!((got - dot).abs() < 0.01, "{font} {mark:?}: {got} vs dot {dot}");
        }
    }
}

#[test]
fn dots_below_do_not_move_the_line_down() {
    for font in FONTS {
        for size in [11.0f32, 24.0] {
            let got = extra(font, size, EmphasisMark::UnderDot);
            assert!(got.abs() < 0.15, "{font} {size} pt underDot: line {got} pt further than plain, want 0");
        }
    }
}

#[test]
fn the_dot_sits_1_065_of_the_size_above_the_baseline() {
    for size in [24.0f32, 36.0] {
        let ([_, _, m], dots) = lay(FONTS[0], size, EmphasisMark::Dot);
        assert_eq!(dots.len(), "Marked".len(), "{size} pt: one dot per character, got {dots:?}");
        for (cy, w) in dots {
            let above = m - cy;
            assert!((above - 1.065 * size).abs() < 0.3, "{size} pt: dot centre {above} above the baseline, want {}", 1.065 * size);
            assert!((w - 0.10 * size).abs() < 0.3, "{size} pt: dot {w} across, want {}", 0.10 * size);
        }
    }
}
