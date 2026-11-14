//! A double strikethrough is drawn as Microsoft 365 Word draws it (measured 2026-11-14 in its PDF
//! export, Times New Roman, Arial and Calibri, 7-36 pt): two lines, each as thick as the single
//! strikethrough, the upper one just above the single line's place and the lower one just below
//! it. The pair is centred where the single line is centred and spans three thicknesses.
//!   Times New Roman 11 pt: single 2.28..2.88 above the baseline; double 2.88..3.48 + 1.68..2.28
//! The bundled test fonts carry their own OS/2 strikeout values:
//!   Source Serif 4 (285, 50, upem 1000) -> centre 0.26 x size, thick 0.05 x size
//!   Inter (671, 140, upem 2048) -> centre 0.29346 x size, thick 0.06836 x size
use crate::display::{Draw, Stroke, page_display};
use crate::*;
use wordcraft_doc::para_block;

const FONTS: [(&str, f32, f32); 2] = [("Source Serif 4", 0.26, 0.05), ("Inter", 0.293_46, 0.068_36)];
const TOL: f32 = 0.02;

/// "Struck" in `font` at `size` pt: its baseline and the horizontal lines drawn (y, width, stroke).
fn lay(font: &str, size: f32, double: bool) -> (f32, Vec<(f32, f32, Stroke)>) {
    crate::test_fonts();
    let run = CharProps { font: Some(font.into()), size: Some(size), strike: Some(!double), double_strike: Some(double), ..CharProps::default() };
    let mut d = Document::new();
    d.body = vec![para_block(Paragraph::with_text("Struck", run))];
    let l = layout(&d, &mut LayoutCache::new(), &LayoutOptions::default());
    let page = l.pages.first().expect("one page");
    let draws = page_display(&d, page, &Default::default());
    let base = draws
        .iter()
        .find_map(|d| match d {
            Draw::Glyphs { glyphs, text, .. } if text.contains("Str") => glyphs.first().map(|g| g.2),
            _ => None,
        })
        .expect("the text is drawn");
    let lines = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Line { x0, y0, x1, y1, width, stroke, .. } if (y0 - y1).abs() < 0.01 && (x1 - x0).abs() > 1.0 => Some((*y0, *width, *stroke)),
            _ => None,
        })
        .collect();
    (base, lines)
}

#[test]
fn a_double_strikethrough_is_two_single_thickness_lines_around_the_single_place() {
    for (font, centre, thick) in FONTS {
        for size in [8.0f32, 11.0, 24.0, 36.0] {
            let (base, lines) = lay(font, size, true);
            assert_eq!(lines.len(), 1, "{font} {size}: one line per span: {lines:?}");
            let (y, w, stroke) = lines[0];
            let (c, t) = (centre * size, thick * size);
            assert!((base - y - c).abs() < TOL, "{font} {size}: centre {} above the baseline, want {c}", base - y);
            assert!((w - 3.0 * t).abs() < TOL, "{font} {size}: spans {w}, want 3 x {t}");
            let parts = stroke.underline_double(w).unwrap_or_else(|| panic!("{font} {size}: a two-line stroke, got {stroke:?}"));
            for ((off, pt), want) in parts.into_iter().zip([-t, t]) {
                assert!((off - want).abs() < TOL, "{font} {size}: a line {off} from the centre, want {want}");
                assert!((pt - t).abs() < TOL, "{font} {size}: a line {pt} thick, want {t}");
            }
        }
    }
}

#[test]
fn the_pair_touches_the_single_strikethrough_band() {
    // Word 11 pt Times New Roman: the single band is 2.28..2.88; the double's lines are
    // 2.88..3.48 and 1.68..2.28, so the gap between them is exactly the single band.
    for (font, centre, thick) in FONTS {
        let size = 11.0;
        let (sbase, single) = lay(font, size, false);
        assert_eq!(single.len(), 1, "{font}: {single:?}");
        let (sy, sw, sstroke) = single[0];
        assert_eq!(sstroke, Stroke::Solid, "{font}: a single strikethrough stays one solid line");
        assert!((sbase - sy - centre * size).abs() < TOL && (sw - thick * size).abs() < TOL, "{font}: single {single:?}");
        let (dbase, double) = lay(font, size, true);
        let (dy, dw, dstroke) = double[0];
        let parts = dstroke.underline_double(dw).expect("two lines");
        let (stop, sbot) = (sbase - sy + sw / 2.0, sbase - sy - sw / 2.0);
        let upper = parts
            .iter()
            .map(|&(off, t)| (dbase - dy - off + t / 2.0, dbase - dy - off - t / 2.0))
            .fold((f32::MIN, 0.0), |a, b| if b.0 > a.0 { b } else { a });
        let lower = parts
            .iter()
            .map(|&(off, t)| (dbase - dy - off + t / 2.0, dbase - dy - off - t / 2.0))
            .fold((f32::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
        assert!((upper.1 - stop).abs() < TOL, "{font}: upper line's bottom {} at the single's top {stop}", upper.1);
        assert!((lower.0 - sbot).abs() < TOL, "{font}: lower line's top {} at the single's bottom {sbot}", lower.0);
    }
}
