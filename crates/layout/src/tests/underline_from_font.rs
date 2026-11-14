//! Underline position and thickness come from the font (its `post` table), as Microsoft 365 Word
//! draws them. Measured 2026-11-14 in Word's PDF of hand-written DOCX files (one underlined run
//! per size, 7-36 pt): the line's centre sits `(-underlinePosition + underlineThickness / 2) / unitsPerEm`
//! times the font size below the baseline, and it is `underlineThickness / unitsPerEm` times the
//! size thick (`underlinePosition` is the top of the line, OpenType `post`).
//!   Times New Roman (pos -223, thick 100, upem 2048): centre 0.1333, thick 0.0488 x size; Word 0.13-0.135 / 0.045-0.053
//!   Arial (pos -217, thick 150, upem 2048): centre 0.1426, thick 0.0732 x size; Word 0.138-0.147 / 0.067-0.077
//! Strikethrough the same way from OS/2: centre above the baseline =
//! `(yStrikeoutPosition - yStrikeoutSize / 2) / unitsPerEm x size`, thickness `yStrikeoutSize / unitsPerEm x size`.
//!   Times New Roman and Arial (530, 102, upem 2048): centre 0.2339, thick 0.0498; Word 0.227-0.237 / 0.045-0.055 (both fonts)
//! (Word's PDF rounds positions and widths to 0.12 pt.)
//! The bundled test fonts carry different values, so these tests pin the rule, not one font:
//!   Source Serif 4 (upem 1000): pos -50, thick 50  -> centre 0.075 x size, thick 0.05 x size
//!   Inter (upem 2048): pos -348, thick 140 -> centre 0.20410 x size, thick 0.06836 x size
//!   strike: Source Serif 4 (285, 50) -> centre 0.26, thick 0.05; Inter (671, 140) -> centre 0.29346, thick 0.06836
use crate::display::{Draw, page_display};
use crate::*;
use wordcraft_doc::para_block;
use wordcraft_doc::props::Underline;

/// One paragraph "Underlined" in `font` at `size` pt with `underline`: the baseline of its
/// glyphs and the horizontal lines drawn (y, width).
fn lay(font: &str, size: f32, underline: Underline) -> (f32, Vec<(f32, f32)>) {
    lay_run(font, size, CharProps { underline: Some(underline), ..CharProps::default() })
}

/// As [`lay`], with the run's other properties from `props`.
fn lay_run(font: &str, size: f32, props: CharProps) -> (f32, Vec<(f32, f32)>) {
    crate::test_fonts();
    let mut d = Document::new();
    let run = CharProps { font: Some(font.into()), size: Some(size), ..props };
    d.body = vec![para_block(Paragraph::with_text("Underlined", run))];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let draws = page_display(&d, page, &Default::default());
    let base = draws
        .iter()
        .find_map(|d| match d {
            Draw::Glyphs { glyphs, text, .. } if text.contains("Under") => glyphs.first().map(|g| g.2),
            _ => None,
        })
        .expect("the text is drawn");
    let lines = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Line { x0, y0, x1, y1, width, .. } if (y0 - y1).abs() < 0.01 && (x1 - x0).abs() > 1.0 => Some((*y0, *width)),
            _ => None,
        })
        .collect();
    (base, lines)
}

/// (font, centre below the baseline, thickness), both as a fraction of the size.
const FONTS: [(&str, f32, f32); 2] = [("Source Serif 4", 0.075, 0.05), ("Inter", 0.204_10, 0.068_36)];

#[test]
fn a_single_underline_sits_where_the_font_puts_it() {
    for (font, centre, thick) in FONTS {
        for size in [8.0f32, 11.0, 12.0, 24.0, 36.0] {
            let (base, lines) = lay(font, size, Underline::Single);
            assert_eq!(lines.len(), 1, "{font} {size}: one underline, got {lines:?}");
            let (y, w) = lines[0];
            assert!((y - base - centre * size).abs() < 0.02, "{font} {size}: centre {} below the baseline, want {}", y - base, centre * size);
            assert!((w - thick * size).abs() < 0.02, "{font} {size}: thickness {w}, want {}", thick * size);
        }
    }
}

/// Words-only underlines use the same line as a single underline.
#[test]
fn a_words_only_underline_uses_the_same_line() {
    for (font, centre, thick) in FONTS {
        let size = 20.0;
        let (base, lines) = lay(font, size, Underline::Words);
        assert!(!lines.is_empty(), "{font}: drawn");
        for (y, w) in lines {
            assert!((y - base - centre * size).abs() < 0.02, "{font}: centre {} below the baseline", y - base);
            assert!((w - thick * size).abs() < 0.02, "{font}: thickness {w}");
        }
    }
}

/// A thick underline is twice the font's thickness, centred where the single line is centred.
#[test]
fn a_thick_underline_doubles_the_font_thickness() {
    for (font, centre, thick) in FONTS {
        let size = 20.0;
        let (base, lines) = lay(font, size, Underline::Thick);
        assert_eq!(lines.len(), 1, "{font}: {lines:?}");
        let (y, w) = lines[0];
        assert!((y - base - centre * size).abs() < 0.02, "{font}: centre {} below the baseline", y - base);
        assert!((w - 2.0 * thick * size).abs() < 0.02, "{font}: thickness {w}");
    }
}

/// (font, strikethrough centre above the baseline, thickness), as a fraction of the size.
const STRIKES: [(&str, f32, f32); 2] = [("Source Serif 4", 0.26, 0.05), ("Inter", 0.293_46, 0.068_36)];

/// A strikethrough sits where the font's OS/2 table puts it, as thick as it says.
#[test]
fn a_strikethrough_sits_where_the_font_puts_it() {
    for (font, centre, thick) in STRIKES {
        for size in [8.0f32, 12.0, 24.0] {
            let (base, lines) = lay_run(font, size, CharProps { strike: Some(true), ..CharProps::default() });
            assert_eq!(lines.len(), 1, "{font} {size}: one line, got {lines:?}");
            let (y, w) = lines[0];
            assert!((base - y - centre * size).abs() < 0.02, "{font} {size}: centre {} above the baseline, want {}", base - y, centre * size);
            assert!((w - thick * size).abs() < 0.02, "{font} {size}: thickness {w}, want {}", thick * size);
        }
    }
}
